use crate::spi_helpers::fetch_timeseries;
use pgrx::prelude::*;

/// Standalone mathematical computation for log return of a single period.
pub fn compute_log_return(p_t: f64, p_t_minus_1: f64) -> Result<f64, &'static str> {
    if p_t <= 0.0 || p_t.is_nan() || p_t.is_infinite() {
        return Err("Price p_t must be strictly positive and finite");
    }
    if p_t_minus_1 <= 0.0 || p_t_minus_1.is_nan() || p_t_minus_1.is_infinite() {
        return Err("Previous price p_t_minus_1 must be strictly positive and finite");
    }
    Ok((p_t / p_t_minus_1).ln())
}

/// Standalone mathematical computation for simple return of a single period.
pub fn compute_simple_return(p_t: f64, p_t_minus_1: f64) -> Result<f64, &'static str> {
    if p_t <= 0.0 || p_t.is_nan() || p_t.is_infinite() {
        return Err("Price p_t must be strictly positive and finite");
    }
    if p_t_minus_1 <= 0.0 || p_t_minus_1.is_nan() || p_t_minus_1.is_infinite() {
        return Err("Previous price p_t_minus_1 must be strictly positive and finite");
    }
    Ok((p_t - p_t_minus_1) / p_t_minus_1)
}

/// Standalone mathematical computation for annualizing daily simple return.
pub fn compute_annualize_simple(daily_return: f64, trading_days: i32) -> f64 {
    (1.0 + daily_return).powi(trading_days) - 1.0
}

/// Standalone mathematical computation for annualizing daily log return.
pub fn compute_annualize_log(daily_return: f64, trading_days: i32) -> f64 {
    daily_return * (trading_days as f64)
}

/// Standalone mathematical computation for cumulative simple return.
pub fn compute_cumulative_simple_return(returns: &[f64]) -> f64 {
    returns.iter().fold(1.0, |acc, &r| acc * (1.0 + r)) - 1.0
}

/// Standalone mathematical computation for cumulative log return.
pub fn compute_cumulative_log_return(returns: &[f64]) -> f64 {
    returns.iter().sum()
}

/// Computes the log return of a single period given p_t and p_t_minus_1.
#[pg_extern]
pub fn pgquant_log_return(p_t: f64, p_t_minus_1: f64) -> f64 {
    compute_log_return(p_t, p_t_minus_1).unwrap_or_else(|e| pgrx::error!("{}", e))
}

/// Takes a price series query and computes log returns per symbol over time.
/// The query must return (symbol text, date date, price double precision) ordered by symbol and date.
#[pg_extern]
pub fn pgquant_log_returns(
    query: &str,
) -> pgrx::iter::TableIterator<
    'static,
    (
        name!(symbol, String),
        name!(date, pgrx::datum::Date),
        name!(log_return, f64),
    ),
> {
    let timeseries = fetch_timeseries(query).unwrap_or_else(|e| {
        pgrx::error!("Failed to fetch timeseries: {}", e);
    });

    if timeseries.is_empty() {
        return pgrx::iter::TableIterator::new(Vec::new());
    }

    let mut results = Vec::with_capacity(timeseries.len());
    let mut iter = timeseries.into_iter();
    let mut prev = iter.next().unwrap();

    for current in iter {
        if current.0 == prev.0 {
            let log_ret = (current.2 / prev.2).ln();
            results.push((current.0, current.1, log_ret));
            prev.2 = current.2;
        } else {
            prev = current;
        }
    }

    pgrx::iter::TableIterator::new(results)
}

/// Computes the simple return of a single period given p_t and p_t_minus_1.
#[pg_extern]
pub fn pgquant_simple_return(p_t: f64, p_t_minus_1: f64) -> f64 {
    compute_simple_return(p_t, p_t_minus_1).unwrap_or_else(|e| pgrx::error!("{}", e))
}

/// Takes a price series query and computes simple returns per symbol over time.
/// The query must return (symbol text, date date, price double precision) ordered by symbol and date.
#[pg_extern]
pub fn pgquant_simple_returns(
    query: &str,
) -> pgrx::iter::TableIterator<
    'static,
    (
        name!(symbol, String),
        name!(date, pgrx::datum::Date),
        name!(simple_return, f64),
    ),
> {
    let timeseries = fetch_timeseries(query).unwrap_or_else(|e| {
        pgrx::error!("Failed to fetch timeseries: {}", e);
    });

    if timeseries.is_empty() {
        return pgrx::iter::TableIterator::new(Vec::new());
    }

    let mut results = Vec::with_capacity(timeseries.len());
    let mut iter = timeseries.into_iter();
    let mut prev = iter.next().unwrap();

    for current in iter {
        if current.0 == prev.0 {
            let simple_ret = (current.2 - prev.2) / prev.2;
            results.push((current.0, current.1, simple_ret));
            prev.2 = current.2;
        } else {
            prev = current;
        }
    }

    pgrx::iter::TableIterator::new(results)
}

/// Annualizes a daily simple return given a number of trading days in a year (e.g., 252).
#[pg_extern]
pub fn pgquant_annualize_simple(daily_return: f64, trading_days: i32) -> f64 {
    compute_annualize_simple(daily_return, trading_days)
}

/// Annualizes a daily log return given a number of trading days in a year (e.g., 252).
#[pg_extern]
pub fn pgquant_annualize_log(daily_return: f64, trading_days: i32) -> f64 {
    compute_annualize_log(daily_return, trading_days)
}

/// Computes the cumulative simple return from an array of simple returns.
#[pg_extern]
pub fn pgquant_cumulative_simple_return(returns: Vec<f64>) -> f64 {
    compute_cumulative_simple_return(&returns)
}

/// Computes the cumulative log return from an array of log returns.
#[pg_extern]
pub fn pgquant_cumulative_log_return(returns: Vec<f64>) -> f64 {
    compute_cumulative_log_return(&returns)
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn test_compute_log_return() {
        let ret = compute_log_return(105.0, 100.0).unwrap();
        assert!((ret - 0.048790164169).abs() < 1e-6);
    }

    #[test]
    fn test_compute_simple_return() {
        let ret = compute_simple_return(105.0, 100.0).unwrap();
        assert!((ret - 0.05).abs() < 1e-12);
    }

    #[test]
    fn test_returns_validation() {
        assert!(compute_log_return(-1.0, 100.0).is_err());
        assert!(compute_log_return(100.0, 0.0).is_err());
        assert!(compute_log_return(f64::NAN, 100.0).is_err());
        assert!(compute_simple_return(-1.0, 100.0).is_err());
        assert!(compute_simple_return(100.0, 0.0).is_err());
    }

    #[test]
    fn test_annualize_simple() {
        let ret = compute_annualize_simple(0.001, 252);
        let expected = (1.001_f64).powi(252) - 1.0;
        assert!((ret - expected).abs() < 1e-6);
    }

    #[test]
    fn test_annualize_log() {
        let ret = compute_annualize_log(0.001, 252);
        assert!((ret - 0.252).abs() < 1e-6);
    }

    #[test]
    fn test_cumulative_simple() {
        let returns = [0.01, -0.02, 0.03];
        let ret = compute_cumulative_simple_return(&returns);
        assert!((ret - 0.019494).abs() < 1e-6);
    }

    #[test]
    fn test_cumulative_log() {
        let returns = [0.01, -0.02, 0.03];
        let ret = compute_cumulative_log_return(&returns);
        assert!((ret - 0.02).abs() < 1e-6);
    }
}

#[cfg(feature = "pg_test")]
#[pg_schema]
mod tests {
    use super::*;

    #[pg_test]
    fn test_pgquant_log_return() {
        let ret = pgquant_log_return(105.0, 100.0);
        assert!((ret - 0.048790164169).abs() < 1e-6);
    }

    #[pg_test]
    fn test_pgquant_log_returns() {
        // Create a temporary table for testing
        Spi::run("CREATE TEMP TABLE test_prices_returns (symbol text, date date, price double precision);").unwrap();
        Spi::run(
            "INSERT INTO test_prices_returns VALUES 
            ('AAPL', '2026-10-01', 150.0), 
            ('AAPL', '2026-10-02', 155.0),
            ('AAPL', '2026-10-03', 153.0),
            ('MSFT', '2026-10-01', 300.0),
            ('MSFT', '2026-10-02', 295.0);
        ",
        )
        .unwrap();

        let query = "SELECT symbol, date, price FROM test_prices_returns ORDER BY symbol, date;";
        let results_iter = pgquant_log_returns(query);
        let results: Vec<_> = results_iter.collect();

        // AAPL should have 2 returns, MSFT should have 1 return
        assert_eq!(results.len(), 3);

        // AAPL 2026-10-02 return: ln(155/150)
        assert_eq!(results[0].0, "AAPL");
        assert!((results[0].2 - (155.0_f64 / 150.0_f64).ln()).abs() < 1e-6);

        // AAPL 2026-10-03 return: ln(153/155)
        assert_eq!(results[1].0, "AAPL");
        assert!((results[1].2 - (153.0_f64 / 155.0_f64).ln()).abs() < 1e-6);

        // MSFT 2026-10-02 return: ln(295/300)
        assert_eq!(results[2].0, "MSFT");
        assert!((results[2].2 - (295.0_f64 / 300.0_f64).ln()).abs() < 1e-6);
    }

    #[pg_test]
    fn test_annualize_simple() {
        // A daily simple return of 0.001 (0.1%) annualized over 252 days
        let ret = pgquant_annualize_simple(0.001, 252);
        let expected = (1.001_f64).powi(252) - 1.0;
        assert!((ret - expected).abs() < 1e-6);
    }

    #[pg_test]
    fn test_annualize_log() {
        // A daily log return of 0.001 annualized over 252 days
        let ret = pgquant_annualize_log(0.001, 252);
        assert!((ret - 0.252).abs() < 1e-6);
    }

    #[pg_test]
    fn test_cumulative_simple() {
        let returns = vec![0.01, -0.02, 0.03];
        // (1.01 * 0.98 * 1.03) - 1.0 = 1.019494 - 1.0 = 0.019494
        let ret = pgquant_cumulative_simple_return(returns);
        assert!((ret - 0.019494).abs() < 1e-6);
    }

    #[pg_test]
    fn test_cumulative_log() {
        let returns = vec![0.01, -0.02, 0.03];
        // 0.01 - 0.02 + 0.03 = 0.02
        let ret = pgquant_cumulative_log_return(returns);
        assert!((ret - 0.02).abs() < 1e-6);
    }
}
