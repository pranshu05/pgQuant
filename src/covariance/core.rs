use nalgebra::DMatrix;
use pgrx::prelude::*;
use std::collections::{BTreeMap, BTreeSet};

type TabularCovResult = Result<
    TableIterator<
        'static,
        (
            name!(symbol1, String),
            name!(symbol2, String),
            name!(covariance, f64),
        ),
    >,
    pgrx::spi::Error,
>;

type CovMatrixResult = Result<
    TableIterator<'static, (name!(symbols, Vec<String>), name!(cov_matrix, Vec<f64>))>,
    pgrx::spi::Error,
>;

type CovInternalResult = Result<Option<(Vec<String>, DMatrix<f64>)>, pgrx::spi::Error>;

fn fetch_and_align_returns(query: &str, min_dates: usize) -> CovInternalResult {
    let data = crate::spi_helpers::fetch_timeseries(query)?;
    if data.is_empty() {
        return Ok(None);
    }

    let mut symbols_set = BTreeSet::new();
    let mut date_map: BTreeMap<pgrx::datum::Date, BTreeMap<String, f64>> = BTreeMap::new();

    for (sym, date, val) in data {
        symbols_set.insert(sym.clone());
        date_map.entry(date).or_default().insert(sym, val);
    }

    let symbols: Vec<String> = symbols_set.into_iter().collect();
    let num_symbols = symbols.len();

    if num_symbols < 2 {
        pgrx::error!("Need at least 2 symbols to compute covariance");
    }

    let mut aligned_dates = Vec::new();
    for (date, sym_vals) in &date_map {
        if sym_vals.len() == num_symbols {
            aligned_dates.push(*date);
        }
    }

    let num_dates = aligned_dates.len();
    if num_dates < min_dates {
        pgrx::error!(
            "Need at least {} overlapping dates across all symbols to compute covariance",
            min_dates
        );
    }

    let mut matrix_data = Vec::with_capacity(num_dates * num_symbols);
    for date in &aligned_dates {
        pgrx::check_for_interrupts!();
        let sym_vals = match date_map.get(date) {
            Some(map) => map,
            None => pgrx::error!("Missing data for aligned date {:?}", date),
        };
        for sym in &symbols {
            match sym_vals.get(sym) {
                Some(&val) => matrix_data.push(val),
                None => pgrx::error!("Missing data for symbol {} on date {:?}", sym, date),
            }
        }
    }

    let returns_matrix = DMatrix::from_row_slice(num_dates, num_symbols, &matrix_data);
    Ok(Some((symbols, returns_matrix)))
}

pub fn compute_sample_cov_internal(query: &str) -> CovInternalResult {
    let (symbols, returns_matrix) = match fetch_and_align_returns(query, 2)? {
        Some(res) => res,
        None => return Ok(None),
    };

    let num_dates = returns_matrix.nrows();
    let num_symbols = symbols.len();

    let mut sums = nalgebra::DVector::zeros(num_symbols);
    for j in 0..num_symbols {
        sums[j] = returns_matrix.column(j).sum();
    }

    let xt_x = returns_matrix.tr_mul(&returns_matrix);
    let s_st = &sums * sums.transpose();
    let cov_matrix = (xt_x - s_st / (num_dates as f64)) / ((num_dates - 1) as f64);

    Ok(Some((symbols, cov_matrix)))
}

pub fn compute_shrinkage_cov_lw_internal(query: &str) -> CovInternalResult {
    let (symbols, mut returns_matrix) = match fetch_and_align_returns(query, 3)? {
        Some(res) => res,
        None => return Ok(None),
    };

    let num_dates = returns_matrix.nrows();
    let num_symbols = symbols.len();

    let mut sums = nalgebra::DVector::zeros(num_symbols);
    for j in 0..num_symbols {
        let col = returns_matrix.column(j);
        let sum = col.sum();
        sums[j] = sum;
        let mean = sum / (num_dates as f64);
        for i in 0..num_dates {
            returns_matrix[(i, j)] -= mean;
        }
    }

    let sample_cov = returns_matrix.tr_mul(&returns_matrix) / ((num_dates - 1) as f64);

    let mut trace = 0.0;
    for i in 0..num_symbols {
        trace += sample_cov[(i, i)];
    }
    let mu = trace / (num_symbols as f64);

    let mut sum_var_s_ij = 0.0;
    let mut sum_sq_diff = 0.0;

    let t_f64 = num_dates as f64;
    let t_minus_1 = t_f64 - 1.0;
    let var_multiplier = t_f64 / (t_minus_1 * t_minus_1);

    for i in 0..num_symbols {
        pgrx::check_for_interrupts!();
        for j in i..num_symbols {
            let f_ij = if i == j { mu } else { 0.0 };
            let s_ij = sample_cov[(i, j)];
            let sq_diff = (s_ij - f_ij) * (s_ij - f_ij);

            let w_bar = s_ij * (t_minus_1 / t_f64);
            let mut var_s_ij = 0.0;

            for t in 0..num_dates {
                let x_ti = returns_matrix[(t, i)];
                let x_tj = returns_matrix[(t, j)];
                let w_t = x_ti * x_tj;
                let diff = w_t - w_bar;
                var_s_ij += diff * diff;
            }
            var_s_ij *= var_multiplier;

            if i == j {
                sum_sq_diff += sq_diff;
                sum_var_s_ij += var_s_ij;
            } else {
                sum_sq_diff += 2.0 * sq_diff;
                sum_var_s_ij += 2.0 * var_s_ij;
            }
        }
    }

    let mut lambda = if sum_sq_diff > 0.0 {
        sum_var_s_ij / sum_sq_diff
    } else {
        1.0
    };

    lambda = lambda.clamp(0.0, 1.0);

    let mut shrunk_cov = DMatrix::zeros(num_symbols, num_symbols);
    for i in 0..num_symbols {
        pgrx::check_for_interrupts!();
        for j in i..num_symbols {
            let f_ij = if i == j { mu } else { 0.0 };
            let val = lambda * f_ij + (1.0 - lambda) * sample_cov[(i, j)];
            shrunk_cov[(i, j)] = val;
            shrunk_cov[(j, i)] = val;
        }
    }

    Ok(Some((symbols, shrunk_cov)))
}

fn flatten_cov_matrix(
    symbols: Vec<String>,
    cov_matrix: DMatrix<f64>,
) -> Vec<(String, String, f64)> {
    let num_symbols = symbols.len();
    let mut results = Vec::with_capacity(num_symbols * num_symbols);
    for i in 0..num_symbols {
        pgrx::check_for_interrupts!();
        let s1 = symbols[i].clone();
        for j in 0..num_symbols {
            results.push((s1.clone(), symbols[j].clone(), cov_matrix[(i, j)]));
        }
    }
    results
}

fn flatten_cov_matrix_1d(
    symbols: Vec<String>,
    cov_matrix: DMatrix<f64>,
) -> (Vec<String>, Vec<f64>) {
    let num_symbols = symbols.len();
    let mut cov_flat = Vec::with_capacity(num_symbols * num_symbols);
    for i in 0..num_symbols {
        pgrx::check_for_interrupts!();
        for j in 0..num_symbols {
            cov_flat.push(cov_matrix[(i, j)]);
        }
    }
    (symbols, cov_flat)
}

/// Computes the sample covariance matrix for a panel of returns.
/// The input query must return (symbol TEXT, date DATE, return DOUBLE PRECISION).
/// Returns a flattened table of (symbol1, symbol2, covariance).
#[pg_extern]
pub fn pgquant_sample_cov(query: &str) -> TabularCovResult {
    match compute_sample_cov_internal(query)? {
        Some((symbols, cov_matrix)) => {
            Ok(TableIterator::new(flatten_cov_matrix(symbols, cov_matrix)))
        }
        None => Ok(TableIterator::new(vec![])),
    }
}

/// Computes the sample covariance matrix as compact 1D array output.
/// Returns (symbols TEXT[], cov_matrix DOUBLE PRECISION[]).
#[pg_extern]
pub fn pgquant_sample_cov_matrix(query: &str) -> CovMatrixResult {
    match compute_sample_cov_internal(query)? {
        Some((symbols, cov_matrix)) => {
            let (syms, flat_cov) = flatten_cov_matrix_1d(symbols, cov_matrix);
            Ok(TableIterator::new(std::iter::once((syms, flat_cov))))
        }
        None => Ok(TableIterator::new(std::iter::empty())),
    }
}

/// Computes the Ledoit-Wolf shrinkage covariance matrix towards a scaled identity target.
/// The input query must return (symbol TEXT, date DATE, return DOUBLE PRECISION).
/// Returns a flattened table of (symbol1, symbol2, covariance).
#[pg_extern]
pub fn pgquant_shrinkage_cov_lw(query: &str) -> TabularCovResult {
    match compute_shrinkage_cov_lw_internal(query)? {
        Some((symbols, cov_matrix)) => {
            Ok(TableIterator::new(flatten_cov_matrix(symbols, cov_matrix)))
        }
        None => Ok(TableIterator::new(vec![])),
    }
}

/// Computes the Ledoit-Wolf shrinkage covariance matrix as compact 1D array output.
/// Returns (symbols TEXT[], cov_matrix DOUBLE PRECISION[]).
#[pg_extern]
pub fn pgquant_shrinkage_cov_lw_matrix(query: &str) -> CovMatrixResult {
    match compute_shrinkage_cov_lw_internal(query)? {
        Some((symbols, cov_matrix)) => {
            let (syms, flat_cov) = flatten_cov_matrix_1d(symbols, cov_matrix);
            Ok(TableIterator::new(std::iter::once((syms, flat_cov))))
        }
        None => Ok(TableIterator::new(std::iter::empty())),
    }
}

#[cfg(feature = "pg_test")]
#[pg_schema]
mod tests {
    use super::*;

    #[pg_test]
    fn test_pgquant_sample_cov() {
        Spi::run("CREATE TEMP TABLE cov_test (symbol text, date date, ret double precision);")
            .unwrap();
        Spi::run(
            "INSERT INTO cov_test VALUES 
            ('A', '2026-01-01', 0.01), ('A', '2026-01-02', 0.02), ('A', '2026-01-03', -0.01),
            ('B', '2026-01-01', 0.02), ('B', '2026-01-02', 0.01), ('B', '2026-01-03', -0.02)
        ;",
        )
        .unwrap();

        let query = "SELECT symbol, date, ret FROM cov_test ORDER BY date, symbol";
        let result = pgquant_sample_cov(query).unwrap();
        let rows: Vec<_> = result.collect();

        assert_eq!(rows.len(), 4); // 2x2 matrix

        // Calculate expected covariance:
        // A means: (0.01 + 0.02 - 0.01) / 3 = 0.006666...
        // B means: (0.02 + 0.01 - 0.02) / 3 = 0.003333...
        // Var(A) = [(0.01 - 0.0066)^2 + (0.02 - 0.0066)^2 + (-0.01 - 0.0066)^2] / 2
        // Var(A) = [0.0000111 + 0.0001777 + 0.0002777] / 2 = 0.0002333

        let mut var_a = 0.0;
        let mut cov_ab = 0.0;
        for (s1, s2, cov) in rows {
            if s1 == "A" && s2 == "A" {
                var_a = cov;
            } else if s1 == "A" && s2 == "B" {
                cov_ab = cov;
            }
        }

        assert!((var_a - 0.000233333).abs() < 1e-6);
        assert!((cov_ab - 0.000266666).abs() < 1e-6);
    }

    #[pg_test]
    fn test_pgquant_shrinkage_cov_lw() {
        Spi::run("CREATE TEMP TABLE cov_test_lw (symbol text, date date, ret double precision);")
            .unwrap();
        Spi::run(
            "INSERT INTO cov_test_lw VALUES 
            ('A', '2026-01-01', 0.01), ('A', '2026-01-02', 0.02), ('A', '2026-01-03', -0.01),
            ('B', '2026-01-01', 0.02), ('B', '2026-01-02', 0.01), ('B', '2026-01-03', -0.02)
        ;",
        )
        .unwrap();

        let query = "SELECT symbol, date, ret FROM cov_test_lw ORDER BY date, symbol";
        let result = pgquant_shrinkage_cov_lw(query).unwrap();
        let rows: Vec<_> = result.collect();

        assert_eq!(rows.len(), 4);

        // We mainly want to ensure that it converges and calculates something
        // without panicking.
        let mut _cov_ab = 0.0;
        let mut var_a = 0.0;
        let mut var_b = 0.0;
        for (s1, s2, cov) in rows {
            if s1 == "A" && s2 == "A" {
                var_a = cov;
            } else if s1 == "B" && s2 == "B" {
                var_b = cov;
            } else if s1 == "A" && s2 == "B" {
                _cov_ab = cov;
            }
        }

        assert!(var_a > 0.0);
        assert!(var_b > 0.0);
    }
}
