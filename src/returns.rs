use crate::spi_helpers::fetch_timeseries;
use pgrx::prelude::*;

/// Computes the log return of a single period given p_t and p_t_minus_1.
#[pg_extern]
pub fn pgquant_log_return(p_t: f64, p_t_minus_1: f64) -> f64 {
    (p_t / p_t_minus_1).ln()
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

    let mut results = Vec::new();

    if timeseries.is_empty() {
        return pgrx::iter::TableIterator::new(results);
    }

    let mut prev_symbol = &timeseries[0].0;
    let mut prev_price = timeseries[0].2;

    for row in timeseries.iter().skip(1) {
        let current_symbol = &row.0;
        let current_date = row.1;
        let current_price = row.2;

        if current_symbol == prev_symbol {
            let log_ret = (current_price / prev_price).ln();
            results.push((current_symbol.clone(), current_date, log_ret));
        }

        prev_symbol = current_symbol;
        prev_price = current_price;
    }

    pgrx::iter::TableIterator::new(results)
}

/// Computes the simple return of a single period given p_t and p_t_minus_1.
#[pg_extern]
pub fn pgquant_simple_return(p_t: f64, p_t_minus_1: f64) -> f64 {
    (p_t - p_t_minus_1) / p_t_minus_1
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

    let mut results = Vec::new();

    if timeseries.is_empty() {
        return pgrx::iter::TableIterator::new(results);
    }

    let mut prev_symbol = &timeseries[0].0;
    let mut prev_price = timeseries[0].2;

    for row in timeseries.iter().skip(1) {
        let current_symbol = &row.0;
        let current_date = row.1;
        let current_price = row.2;

        if current_symbol == prev_symbol {
            let simple_ret = (current_price - prev_price) / prev_price;
            results.push((current_symbol.clone(), current_date, simple_ret));
        }

        prev_symbol = current_symbol;
        prev_price = current_price;
    }

    pgrx::iter::TableIterator::new(results)
}

/// Annualizes a daily simple return given a number of trading days in a year (e.g., 252).
#[pg_extern]
pub fn pgquant_annualize_simple(daily_return: f64, trading_days: i32) -> f64 {
    (1.0 + daily_return).powi(trading_days) - 1.0
}

/// Annualizes a daily log return given a number of trading days in a year (e.g., 252).
#[pg_extern]
pub fn pgquant_annualize_log(daily_return: f64, trading_days: i32) -> f64 {
    daily_return * (trading_days as f64)
}

/// Computes the cumulative simple return from an array of simple returns.
#[pg_extern]
pub fn pgquant_cumulative_simple_return(returns: Vec<f64>) -> f64 {
    returns.into_iter().fold(1.0, |acc, r| acc * (1.0 + r)) - 1.0
}

/// Computes the cumulative log return from an array of log returns.
#[pg_extern]
pub fn pgquant_cumulative_log_return(returns: Vec<f64>) -> f64 {
    returns.into_iter().sum()
}

#[cfg(any(test, feature = "pg_test"))]
#[pg_schema]
mod tests {
    use super::*;
    use pgrx::prelude::*;

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