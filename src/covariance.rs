use nalgebra::DMatrix;
use pgrx::prelude::*;
use std::collections::{BTreeMap, BTreeSet};

/// Computes the sample covariance matrix for a panel of returns.
/// The input query must return (symbol TEXT, date DATE, return DOUBLE PRECISION).
/// Returns a flattened table of (symbol1, symbol2, covariance).
#[pg_extern]
pub fn pgquant_sample_cov(
    query: &str,
) -> Result<
    TableIterator<
        'static,
        (
            name!(symbol1, String),
            name!(symbol2, String),
            name!(covariance, f64),
        ),
    >,
    pgrx::spi::Error,
> {
    // Fetch data
    let data = crate::spi_helpers::fetch_timeseries(query)?;
    if data.is_empty() {
        return Ok(TableIterator::new(vec![]));
    }

    // Pivot data to find overlapping dates
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

    // Filter to dates where ALL symbols have data (list-wise deletion)
    let mut aligned_dates = Vec::new();
    for (date, sym_vals) in &date_map {
        if sym_vals.len() == num_symbols {
            aligned_dates.push(date.clone());
        }
    }

    let num_dates = aligned_dates.len();
    if num_dates < 2 {
        pgrx::error!("Need at least 2 overlapping dates across all symbols to compute covariance");
    }

    // Build nalgebra matrix (num_dates x num_symbols)
    let mut matrix_data = Vec::with_capacity(num_dates * num_symbols);
    for date in &aligned_dates {
        let sym_vals = date_map.get(date).unwrap();
        for sym in &symbols {
            matrix_data.push(*sym_vals.get(sym).unwrap());
        }
    }

    // DMatrix::from_row_slice expects data row by row
    let mut returns_matrix = DMatrix::from_row_slice(num_dates, num_symbols, &matrix_data);

    // Mean-center the columns
    for j in 0..num_symbols {
        let col = returns_matrix.column(j);
        let mean = col.sum() / (num_dates as f64);
        for i in 0..num_dates {
            returns_matrix[(i, j)] -= mean;
        }
    }

    // Compute Sample Covariance: C = (R^T * R) / (N - 1)
    let cov_matrix = (returns_matrix.transpose() * returns_matrix) / ((num_dates - 1) as f64);

    // Flatten to tabular output
    let mut results = Vec::with_capacity(num_symbols * num_symbols);
    for i in 0..num_symbols {
        for j in 0..num_symbols {
            results.push((symbols[i].clone(), symbols[j].clone(), cov_matrix[(i, j)]));
        }
    }

    Ok(TableIterator::new(results))
}

/// Computes the Ledoit-Wolf shrinkage covariance matrix towards a scaled identity target.
/// The input query must return (symbol TEXT, date DATE, return DOUBLE PRECISION).
/// Returns a flattened table of (symbol1, symbol2, covariance).
#[pg_extern]
pub fn pgquant_shrinkage_cov_lw(
    query: &str,
) -> Result<
    TableIterator<
        'static,
        (
            name!(symbol1, String),
            name!(symbol2, String),
            name!(covariance, f64),
        ),
    >,
    pgrx::spi::Error,
> {
    // Fetch data
    let data = crate::spi_helpers::fetch_timeseries(query)?;
    if data.is_empty() {
        return Ok(TableIterator::new(vec![]));
    }

    // Pivot data to find overlapping dates
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

    // Filter to dates where ALL symbols have data
    let mut aligned_dates = Vec::new();
    for (date, sym_vals) in &date_map {
        if sym_vals.len() == num_symbols {
            aligned_dates.push(date.clone());
        }
    }

    let num_dates = aligned_dates.len();
    if num_dates < 3 {
        pgrx::error!("Need at least 3 overlapping dates across all symbols for Ledoit-Wolf");
    }

    // Build nalgebra matrix (num_dates x num_symbols)
    let mut matrix_data = Vec::with_capacity(num_dates * num_symbols);
    for date in &aligned_dates {
        let sym_vals = date_map.get(date).unwrap();
        for sym in &symbols {
            matrix_data.push(*sym_vals.get(sym).unwrap());
        }
    }

    let mut returns_matrix = DMatrix::from_row_slice(num_dates, num_symbols, &matrix_data);

    // Mean-center the columns
    for j in 0..num_symbols {
        let col = returns_matrix.column(j);
        let mean = col.sum() / (num_dates as f64);
        for i in 0..num_dates {
            returns_matrix[(i, j)] -= mean;
        }
    }

    // Compute Sample Covariance: S = (R^T * R) / (N - 1)
    let sample_cov = (returns_matrix.transpose() * &returns_matrix) / ((num_dates - 1) as f64);

    // Compute Target F = mu * I, where mu = trace(S) / num_symbols
    let mut trace = 0.0;
    for i in 0..num_symbols {
        trace += sample_cov[(i, i)];
    }
    let mu = trace / (num_symbols as f64);

    // Compute optimal shrinkage intensity lambda
    let mut sum_var_s_ij = 0.0;
    let mut sum_sq_diff = 0.0;

    let t_f64 = num_dates as f64;
    let t_minus_1 = t_f64 - 1.0;
    let var_multiplier = t_f64 / (t_minus_1 * t_minus_1);

    for i in 0..num_symbols {
        for j in 0..num_symbols {
            // Target matrix entry F_ij
            let f_ij = if i == j { mu } else { 0.0 };
            let s_ij = sample_cov[(i, j)];

            sum_sq_diff += (s_ij - f_ij) * (s_ij - f_ij);

            // Compute empirical variance of S_ij
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
            sum_var_s_ij += var_s_ij;
        }
    }

    let mut lambda = if sum_sq_diff > 0.0 {
        sum_var_s_ij / sum_sq_diff
    } else {
        1.0
    };

    // Bound lambda between 0 and 1
    lambda = lambda.max(0.0).min(1.0);

    // Compute shrunk covariance matrix: S* = lambda * F + (1 - lambda) * S
    let mut shrunk_cov = DMatrix::zeros(num_symbols, num_symbols);
    for i in 0..num_symbols {
        for j in 0..num_symbols {
            let f_ij = if i == j { mu } else { 0.0 };
            shrunk_cov[(i, j)] = lambda * f_ij + (1.0 - lambda) * sample_cov[(i, j)];
        }
    }

    // Flatten to tabular output
    let mut results = Vec::with_capacity(num_symbols * num_symbols);
    for i in 0..num_symbols {
        for j in 0..num_symbols {
            results.push((symbols[i].clone(), symbols[j].clone(), shrunk_cov[(i, j)]));
        }
    }

    Ok(TableIterator::new(results))
}

#[cfg(any(test, feature = "pg_test"))]
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
