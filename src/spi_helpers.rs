use pgrx::prelude::*;

/// Executes a SQL query that returns columns (symbol text, date date, value double precision).
/// Returns a vector of tuples.
pub fn fetch_timeseries(
    query: &str,
) -> Result<Vec<(String, pgrx::datum::Date, f64)>, pgrx::spi::Error> {
    Spi::connect(|client| {
        let mut results = Vec::new();
        let table = client.select(query, None, None)?;
        for row in table {
            // pgrx SPI tuples are 1-indexed. We expect:
            // 1: symbol (text)
            // 2: date (date)
            // 3: value (double precision)
            let symbol: String = row
                .get(1)?
                .unwrap_or_else(|| pgrx::error!("symbol column cannot be null"));
            let date: pgrx::datum::Date = row
                .get(2)?
                .unwrap_or_else(|| pgrx::error!("date column cannot be null"));
            let value: f64 = row
                .get(3)?
                .unwrap_or_else(|| pgrx::error!("value column cannot be null"));

            results.push((symbol, date, value));
        }
        Ok(results)
    })
}

#[cfg(any(test, feature = "pg_test"))]
#[pg_schema]
mod tests {
    use super::fetch_timeseries;
    use pgrx::prelude::*;

    #[pg_test]
    fn test_fetch_timeseries() {
        // Create a temporary table for testing
        Spi::run("CREATE TEMP TABLE test_prices (symbol text, date date, price double precision);")
            .unwrap();
        Spi::run("INSERT INTO test_prices VALUES ('AAPL', '2026-10-01', 150.5), ('MSFT', '2026-10-01', 300.0);").unwrap();

        let query = "SELECT symbol, date, price FROM test_prices ORDER BY symbol;";
        let results = fetch_timeseries(query).expect("Failed to fetch timeseries");

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0, "AAPL");
        assert_eq!(results[0].2, 150.5);
        assert_eq!(results[1].0, "MSFT");
        assert_eq!(results[1].2, 300.0);
    }
}