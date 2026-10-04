use pgrx::prelude::*;
use std::collections::BTreeMap;

/// Sorts and buckets stocks cross-sectionally by a characteristic.
/// The input query must return (symbol TEXT, date DATE, value DOUBLE PRECISION).
/// Returns a flattened table of (symbol, date, bucket_id).
/// Bucket IDs are 1-indexed, where 1 is the lowest characteristic value.
#[pg_extern]
pub fn pgquant_portfolio_sort(
    query: &str,
    num_buckets: i32,
) -> Result<
    TableIterator<
        'static,
        (
            name!(symbol, String),
            name!(date, pgrx::datum::Date),
            name!(bucket, i32),
        ),
    >,
    pgrx::spi::Error,
> {
    if num_buckets < 1 {
        pgrx::error!("num_buckets must be >= 1");
    }

    let data = crate::spi_helpers::fetch_timeseries(query)?;
    if data.is_empty() {
        return Ok(TableIterator::new(vec![]));
    }

    // Group by date: date -> Vec<(symbol, value)>
    let mut grouped_by_date: BTreeMap<pgrx::datum::Date, Vec<(String, f64)>> = BTreeMap::new();
    for (sym, date, val) in data {
        grouped_by_date.entry(date).or_default().push((sym, val));
    }

    let mut results = Vec::new();

    for (date, mut records) in grouped_by_date {
        // Sort by characteristic value ascending
        records.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        let n = records.len() as f64;

        for (i, (sym, _val)) in records.into_iter().enumerate() {
            // Linear bucket assignment: bucket 1 to num_buckets
            let bucket = ((i as f64 * num_buckets as f64) / n).floor() as i32 + 1;
            // Bound check just in case float math is weird
            let final_bucket = bucket.clamp(1, num_buckets);
            results.push((sym, date.clone(), final_bucket));
        }
    }

    Ok(TableIterator::new(results))
}

#[cfg(any(test, feature = "pg_test"))]
#[pg_schema]
mod tests {
    use super::*;

    #[pg_test]
    fn test_pgquant_portfolio_sort() {
        Spi::run("CREATE TEMP TABLE char_test (symbol text, date date, val double precision);")
            .unwrap();
        Spi::run(
            "INSERT INTO char_test VALUES 
            ('A', '2026-01-01', 10.0), 
            ('B', '2026-01-01', 20.0), 
            ('C', '2026-01-01', 30.0),
            ('D', '2026-01-01', 40.0)
        ;",
        )
        .unwrap();

        let query = "SELECT symbol, date, val FROM char_test ORDER BY symbol";

        // 2 buckets -> A, B in bucket 1 | C, D in bucket 2
        let result = pgquant_portfolio_sort(query, 2).unwrap();
        let mut rows: Vec<_> = result.collect();
        rows.sort_by_key(|r| r.0.clone());

        assert_eq!(rows.len(), 4);
        assert_eq!(rows[0].0, "A");
        assert_eq!(rows[0].2, 1);
        assert_eq!(rows[1].0, "B");
        assert_eq!(rows[1].2, 1);
        assert_eq!(rows[2].0, "C");
        assert_eq!(rows[2].2, 2);
        assert_eq!(rows[3].0, "D");
        assert_eq!(rows[3].2, 2);
    }
}
