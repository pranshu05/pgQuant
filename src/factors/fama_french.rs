use pgrx::prelude::*;
use std::collections::BTreeMap;

/// Internal helper to calculate 6-portfolio returns (2x3 sort) for a given date
/// Expects data: (symbol, date, return, size_bucket (1..2), value_bucket (1..3), weight)
fn calculate_daily_factor_returns(
    data: &[(String, pgrx::datum::Date, f64, i32, i32, f64)],
) -> BTreeMap<pgrx::datum::Date, (f64, f64)> {
    // Returns (SMB, HML)
    // Group by date
    let mut by_date: BTreeMap<
        pgrx::datum::Date,
        Vec<&(String, pgrx::datum::Date, f64, i32, i32, f64)>,
    > = BTreeMap::new();
    for row in data {
        by_date.entry(row.1.clone()).or_default().push(row);
    }

    let mut factors = BTreeMap::new();

    for (date, rows) in by_date {
        // We need 6 portfolios: SV, SN, SG, BV, BN, BG
        // Arrays for sum_ret*weight and sum_weight
        // indices: [size-1][btm-1] -> [0..1][0..2]
        let mut sums = [[0.0; 3]; 2];
        let mut weights = [[0.0; 3]; 2];

        for r in rows {
            let size_idx = (r.3 - 1).clamp(0, 1) as usize;
            let btm_idx = (r.4 - 1).clamp(0, 2) as usize;
            let ret = r.2;
            let w = r.5;

            sums[size_idx][btm_idx] += ret * w;
            weights[size_idx][btm_idx] += w;
        }

        // Calculate weighted average returns
        let mut p_ret = [[0.0; 3]; 2];
        for s in 0..2 {
            for b in 0..3 {
                if weights[s][b] > 0.0 {
                    p_ret[s][b] = sums[s][b] / weights[s][b];
                }
            }
        }

        // Fama-French formulations
        // SMB = 1/3 (SV + SN + SG) - 1/3 (BV + BN + BG)
        let smb = (p_ret[0][0] + p_ret[0][1] + p_ret[0][2]) / 3.0
            - (p_ret[1][0] + p_ret[1][1] + p_ret[1][2]) / 3.0;

        // HML = 1/2 (SV + BV) - 1/2 (SG + BG)
        // Note: Growth is bucket 1, Neutral is bucket 2, Value is bucket 3
        let hml = (p_ret[0][2] + p_ret[1][2]) / 2.0 - (p_ret[0][0] + p_ret[1][0]) / 2.0;

        factors.insert(date, (smb, hml));
    }

    factors
}

/// Constructs the Fama-French SMB factor return from a double-sort panel.
/// Query must return (symbol TEXT, date DATE, return FLOAT, size_bucket INT, btm_bucket INT, weight FLOAT)
#[pg_extern]
pub fn pgquant_construct_smb(
    query: &str,
) -> Result<
    TableIterator<'static, (name!(date, pgrx::datum::Date), name!(smb_return, f64))>,
    pgrx::spi::Error,
> {
    let data = crate::spi_helpers::fetch_factor_panel(query)?;
    let factors = calculate_daily_factor_returns(&data);

    let mut results = Vec::new();
    for (date, (smb, _)) in factors {
        results.push((date, smb));
    }

    Ok(TableIterator::new(results))
}

/// Constructs the Fama-French HML factor return from a double-sort panel.
/// Query must return (symbol TEXT, date DATE, return FLOAT, size_bucket INT, btm_bucket INT, weight FLOAT)
#[pg_extern]
pub fn pgquant_construct_hml(
    query: &str,
) -> Result<
    TableIterator<'static, (name!(date, pgrx::datum::Date), name!(hml_return, f64))>,
    pgrx::spi::Error,
> {
    let data = crate::spi_helpers::fetch_factor_panel(query)?;
    let factors = calculate_daily_factor_returns(&data);

    let mut results = Vec::new();
    for (date, (_, hml)) in factors {
        results.push((date, hml));
    }

    Ok(TableIterator::new(results))
}

#[cfg(any(test, feature = "pg_test"))]
#[pg_schema]
mod tests {
    use super::*;

    #[pg_test]
    fn test_pgquant_construct_smb_hml() {
        Spi::run("CREATE TEMP TABLE ff_test (symbol text, date date, ret double precision, size int, btm int, weight double precision);").unwrap();
        // SV: A, SN: B, SG: C
        // BV: D, BN: E, BG: F
        Spi::run(
            "INSERT INTO ff_test VALUES 
            ('A', '2026-01-01', 0.10, 1, 3, 1.0),
            ('B', '2026-01-01', 0.05, 1, 2, 1.0),
            ('C', '2026-01-01', 0.02, 1, 1, 1.0),
            ('D', '2026-01-01', 0.08, 2, 3, 1.0),
            ('E', '2026-01-01', 0.04, 2, 2, 1.0),
            ('F', '2026-01-01', 0.01, 2, 1, 1.0)
        ;",
        )
        .unwrap();

        let query = "SELECT symbol, date, ret, size, btm, weight FROM ff_test";

        let smb_result = pgquant_construct_smb(query).unwrap();
        let smb_rows: Vec<_> = smb_result.collect();
        assert_eq!(smb_rows.len(), 1);

        // Small = (0.10 + 0.05 + 0.02)/3 = 0.17/3 = 0.05666...
        // Big = (0.08 + 0.04 + 0.01)/3 = 0.13/3 = 0.04333...
        // SMB = 0.05666 - 0.04333 = 0.01333...
        assert!((smb_rows[0].1 - 0.0133333333333333).abs() < 1e-6);

        let hml_result = pgquant_construct_hml(query).unwrap();
        let hml_rows: Vec<_> = hml_result.collect();
        assert_eq!(hml_rows.len(), 1);

        // Value = (0.10 + 0.08)/2 = 0.09
        // Growth = (0.02 + 0.01)/2 = 0.015
        // HML = 0.09 - 0.015 = 0.075
        assert!((hml_rows[0].1 - 0.075).abs() < 1e-6);
    }
}
