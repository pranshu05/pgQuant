use crate::spi_helpers::fetch_timeseries;
use pgrx::prelude::*;
use std::collections::VecDeque;

#[pg_extern]
pub fn pgquant_rolling_vol(
    query: &str,
    window: default!(i32, 50),
) -> Result<
    pgrx::iter::TableIterator<
        'static,
        (
            name!(symbol, String),
            name!(date, pgrx::datum::Date),
            name!(vol, f64),
        ),
    >,
    pgrx::spi::Error,
> {
    if window <= 1 {
        pgrx::error!("Window size must be greater than 1");
    }

    let mut timeseries = fetch_timeseries(query)?;

    // Sort by symbol and date
    timeseries.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));

    let mut results = Vec::new();
    let mut current_symbol = String::new();
    let mut window_vals: VecDeque<f64> = VecDeque::new();
    let mut mean = 0.0;
    let mut m2 = 0.0;

    for (sym, date, val) in timeseries {
        if sym != current_symbol {
            current_symbol = sym.clone();
            window_vals.clear();
            mean = 0.0;
            m2 = 0.0;
        }

        if window_vals.len() < window as usize {
            window_vals.push_back(val);
            let old_mean = mean;
            mean += (val - old_mean) / window_vals.len() as f64;
            m2 += (val - old_mean) * (val - mean);

            if window_vals.len() == window as usize {
                let n = window as f64;
                let mut variance = m2 / (n - 1.0);
                if variance < 0.0 {
                    variance = 0.0;
                }
                let vol = variance.sqrt();
                results.push((sym, date, vol));
            }
        } else {
            let x_old = window_vals.pop_front().unwrap();
            let x_new = val;
            window_vals.push_back(val);

            let old_mean = mean;
            mean += (x_new - x_old) / window as f64;
            m2 += (x_new - old_mean) * (x_new - mean) - (x_old - old_mean) * (x_old - mean);

            let n = window as f64;
            let mut variance = m2 / (n - 1.0);
            if variance < 0.0 {
                variance = 0.0;
            }
            let vol = variance.sqrt();
            results.push((sym, date, vol));
        }
    }

    Ok(pgrx::iter::TableIterator::new(results))
}

#[cfg(any(test, feature = "pg_test"))]
#[pg_schema]
mod tests {
    use pgrx::prelude::*;

    #[pg_test]
    fn test_pgquant_rolling_vol() {
        Spi::run("CREATE TEMP TABLE test_rolling_returns (symbol text, date date, return_val double precision);").unwrap();
        Spi::run(
            "INSERT INTO test_rolling_returns VALUES 
            ('AAPL', '2026-10-01', 0.01), 
            ('AAPL', '2026-10-02', 0.02),
            ('AAPL', '2026-10-03', -0.01),
            ('AAPL', '2026-10-04', 0.03);",
        )
        .unwrap();

        // 3-day rolling window
        let query = "SELECT * FROM pgquant_rolling_vol('SELECT symbol, date, return_val FROM test_rolling_returns', 3);";

        let results = Spi::connect(|client| {
            let table = client.select(query, None, None).unwrap();
            let mut vec = Vec::new();
            for row in table {
                let sym: String = row.get(1).unwrap().unwrap();
                let vol: f64 = row.get(3).unwrap().unwrap();
                vec.push((sym, vol));
            }
            Ok::<_, pgrx::spi::Error>(vec)
        })
        .unwrap();

        // Window is 3, so we should get results only for 10-03 and 10-04
        assert_eq!(results.len(), 2);

        // AAPL days 1-3: 0.01, 0.02, -0.01
        // mean = 0.02/3 = 0.006666...
        // var = ((0.01-0.00666)^2 + (0.02-0.00666)^2 + (-0.01-0.00666)^2) / 2
        // sum = 0.02, sum_sq = 0.0001 + 0.0004 + 0.0001 = 0.0006
        // var = (0.0006 - 0.0004 / 3) / 2 = (0.0006 - 0.0001333) / 2 = 0.0004666 / 2 = 0.0002333
        // vol = sqrt(0.0002333) ≈ 0.01527525
        assert!((results[0].1 - 0.01527525).abs() < 1e-6);

        // AAPL days 2-4: 0.02, -0.01, 0.03
        // mean = 0.04/3 = 0.013333...
        // sum = 0.04, sum_sq = 0.0004 + 0.0001 + 0.0009 = 0.0014
        // var = (0.0014 - 0.0016 / 3) / 2 = (0.0014 - 0.0005333) / 2 = 0.0008666 / 2 = 0.0004333
        // vol = sqrt(0.0004333) ≈ 0.02081666
        assert!((results[1].1 - 0.02081666).abs() < 1e-6);
    }
}
