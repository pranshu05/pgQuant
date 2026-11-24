use crate::spi_helpers::fetch_timeseries;
use pgrx::prelude::*;
use std::collections::VecDeque;

/// Running Welford accumulator for incremental mean and sample variance computation.
#[derive(Debug, Clone, Default)]
pub struct WelfordAccumulator {
    pub count: usize,
    pub mean: f64,
    pub m2: f64,
}

impl WelfordAccumulator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update(&mut self, x: f64) {
        self.count += 1;
        let delta = x - self.mean;
        self.mean += delta / (self.count as f64);
        let delta2 = x - self.mean;
        self.m2 += delta * delta2;
    }

    pub fn variance(&self) -> f64 {
        if self.count < 2 {
            0.0
        } else {
            (self.m2 / (self.count as f64 - 1.0)).max(0.0)
        }
    }

    pub fn std_dev(&self) -> f64 {
        self.variance().sqrt()
    }
}

/// Computes rolling volatility over a slice of f64 values using sliding Welford
/// with periodic exact recalculation every 500 steps to prevent floating-point drift.
pub fn compute_rolling_vol_series(values: &[f64], window: usize) -> Vec<f64> {
    if window <= 1 || values.len() < window {
        return Vec::new();
    }

    let mut results = Vec::with_capacity(values.len() - window + 1);
    let mut window_vals: VecDeque<f64> = VecDeque::with_capacity(window);
    let mut mean = 0.0;
    let mut m2 = 0.0;
    let mut steps_since_recalc: usize = 0;

    for &val in values {
        if window_vals.len() < window {
            window_vals.push_back(val);
            let old_mean = mean;
            mean += (val - old_mean) / window_vals.len() as f64;
            m2 += (val - old_mean) * (val - mean);

            if window_vals.len() == window {
                if m2 < 0.0 {
                    m2 = 0.0;
                }
                let variance = m2 / (window as f64 - 1.0);
                results.push(variance.sqrt());
            }
        } else {
            let x_old = window_vals.pop_front().unwrap_or(0.0);
            let x_new = val;
            window_vals.push_back(val);

            let old_mean = mean;
            mean += (x_new - x_old) / window as f64;
            m2 += (x_new - old_mean) * (x_new - mean) - (x_old - old_mean) * (x_old - mean);

            steps_since_recalc += 1;
            if steps_since_recalc >= 500 {
                // Periodic exact recalculation to eliminate accumulated floating-point drift
                let n_w = window_vals.len() as f64;
                let exact_mean = window_vals.iter().sum::<f64>() / n_w;
                let mut exact_m2 = 0.0;
                for &x in &window_vals {
                    let diff = x - exact_mean;
                    exact_m2 += diff * diff;
                }
                mean = exact_mean;
                m2 = exact_m2;
                steps_since_recalc = 0;
            }

            if m2 < 0.0 {
                m2 = 0.0;
            }

            let variance = m2 / (window as f64 - 1.0);
            results.push(variance.sqrt());
        }
    }

    results
}

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
    let mut steps_since_recalc: usize = 0;

    for (sym, date, val) in timeseries {
        pgrx::check_for_interrupts!();
        if sym != current_symbol {
            current_symbol = sym.clone();
            window_vals.clear();
            mean = 0.0;
            m2 = 0.0;
            steps_since_recalc = 0;
        }

        if window_vals.len() < window as usize {
            window_vals.push_back(val);
            let old_mean = mean;
            mean += (val - old_mean) / window_vals.len() as f64;
            m2 += (val - old_mean) * (val - mean);

            if window_vals.len() == window as usize {
                if m2 < 0.0 {
                    m2 = 0.0;
                }
                let n = window as f64;
                let variance = m2 / (n - 1.0);
                let vol = variance.sqrt();
                results.push((sym, date, vol));
            }
        } else {
            let x_old = window_vals.pop_front().unwrap_or(0.0);
            let x_new = val;
            window_vals.push_back(val);

            let old_mean = mean;
            mean += (x_new - x_old) / window as f64;
            m2 += (x_new - old_mean) * (x_new - mean) - (x_old - old_mean) * (x_old - mean);

            steps_since_recalc += 1;
            if steps_since_recalc >= 500 {
                // Periodic exact recalculation to eliminate accumulated floating-point drift
                let n_w = window_vals.len() as f64;
                let exact_mean = window_vals.iter().sum::<f64>() / n_w;
                let mut exact_m2 = 0.0;
                for &x in &window_vals {
                    let diff = x - exact_mean;
                    exact_m2 += diff * diff;
                }
                mean = exact_mean;
                m2 = exact_m2;
                steps_since_recalc = 0;
            }

            if m2 < 0.0 {
                m2 = 0.0;
            }

            let n = window as f64;
            let variance = m2 / (n - 1.0);
            let vol = variance.sqrt();
            results.push((sym, date, vol));
        }
    }

    Ok(pgrx::iter::TableIterator::new(results))
}

#[cfg(feature = "pg_test")]
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

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn test_welford_accumulator() {
        let mut acc = WelfordAccumulator::new();
        let samples = [1.0, 2.0, 3.0, 4.0, 5.0];
        for &x in &samples {
            acc.update(x);
        }
        assert_eq!(acc.count, 5);
        assert!((acc.mean - 3.0).abs() < 1e-12);
        // sample variance for [1, 2, 3, 4, 5] is 10 / 4 = 2.5
        assert!((acc.variance() - 2.5).abs() < 1e-12);
        assert!((acc.std_dev() - (2.5_f64).sqrt()).abs() < 1e-12);
    }

    #[test]
    fn test_rolling_vol_short_or_empty() {
        assert!(compute_rolling_vol_series(&[], 10).is_empty());
        assert!(compute_rolling_vol_series(&[1.0, 2.0], 5).is_empty());
        assert!(compute_rolling_vol_series(&[1.0, 2.0], 1).is_empty());
    }

    #[test]
    fn test_rolling_vol_drift_guard_10k_steps() {
        let n = 10_000;
        let window = 50;
        let mut values = Vec::with_capacity(n);
        for i in 0..n {
            values.push(100.0 + (i as f64 * 0.01).sin() * 5.0 + ((i % 7) as f64) * 0.1);
        }

        let rolling = compute_rolling_vol_series(&values, window);
        assert_eq!(rolling.len(), n - window + 1);

        // Check the very last step against an exact calculation
        let last_window = &values[n - window..n];
        let exact_mean = last_window.iter().sum::<f64>() / (window as f64);
        let exact_m2: f64 = last_window.iter().map(|&x| (x - exact_mean).powi(2)).sum();
        let exact_vol = (exact_m2 / (window as f64 - 1.0)).sqrt();

        let rolling_vol = *rolling.last().unwrap();
        assert!(
            (rolling_vol - exact_vol).abs() < 1e-10,
            "Drift accumulated after 10,000 steps: rolling={}, exact={}",
            rolling_vol,
            exact_vol
        );
    }
}
