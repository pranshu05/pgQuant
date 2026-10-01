use pgrx::prelude::*;

/// Computes the historical Value at Risk (VaR) for a given array of returns.
/// `confidence` is typically 0.95 or 0.99.
/// Returns a positive number representing the loss amount (e.g. 0.05 for 5% loss).
#[pg_extern]
pub fn pgquant_var_historical(mut returns: Vec<f64>, confidence: f64) -> f64 {
    if returns.is_empty() {
        return 0.0;
    }

    // Sort returns ascending (worst losses first)
    returns.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let alpha = 1.0 - confidence;
    let mut index = (alpha * returns.len() as f64).ceil() as usize;
    if index == 0 {
        index = 1;
    }
    let index = index.min(returns.len()) - 1;

    -returns[index]
}

/// Computes the historical Expected Shortfall (ES) / CVaR.
/// Averages all returns that are worse than or equal to the VaR.
/// Returns a positive number representing the expected loss magnitude.
#[pg_extern]
pub fn pgquant_es_historical(mut returns: Vec<f64>, confidence: f64) -> f64 {
    if returns.is_empty() {
        return 0.0;
    }

    returns.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let alpha = 1.0 - confidence;
    let mut index = (alpha * returns.len() as f64).ceil() as usize;
    if index == 0 {
        index = 1;
    }
    let index = index.min(returns.len()) - 1;

    let var_threshold = returns[index];

    let tail_returns: Vec<f64> = returns
        .into_iter()
        .filter(|&r| r <= var_threshold)
        .collect();

    if tail_returns.is_empty() {
        return 0.0;
    }

    let sum: f64 = tail_returns.iter().sum();
    let avg = sum / tail_returns.len() as f64;

    -avg
}

#[cfg(any(test, feature = "pg_test"))]
#[pg_schema]
mod tests {
    use super::*;
    use pgrx::prelude::*;

    #[pg_test]
    fn test_historical_var_es() {
        let mut returns = vec![0.01; 95];
        returns.extend_from_slice(&[-0.01, -0.02, -0.03, -0.04, -0.05]);

        let var = pgquant_var_historical(returns.clone(), 0.95);
        assert!((var - 0.01).abs() < 1e-6);

        let es = pgquant_es_historical(returns.clone(), 0.95);
        assert!((es - 0.03).abs() < 1e-6);
    }
}

use statrs::distribution::{Continuous, ContinuousCDF, Normal};

/// Computes parametric Value at Risk (VaR) assuming a normal distribution.
/// `mean` and `stddev` are the distribution parameters (e.g. daily mean and stddev).
/// Returns a positive number representing the expected loss magnitude.
#[pg_extern]
pub fn pgquant_var_gaussian(mean: f64, stddev: f64, confidence: f64) -> f64 {
    if stddev <= 0.0 {
        pgrx::error!("Standard deviation must be strictly positive");
    }
    let n = Normal::new(mean, stddev).unwrap();
    let alpha = 1.0 - confidence;
    -n.inverse_cdf(alpha)
}

/// Computes parametric Expected Shortfall (ES) assuming a normal distribution.
/// Returns a positive number representing the conditional expected loss magnitude.
#[pg_extern]
pub fn pgquant_es_gaussian(mean: f64, stddev: f64, confidence: f64) -> f64 {
    if stddev <= 0.0 {
        pgrx::error!("Standard deviation must be strictly positive");
    }
    let alpha = 1.0 - confidence;
    let standard_normal = Normal::new(0.0, 1.0).unwrap();
    let z_alpha = standard_normal.inverse_cdf(alpha);
    let phi_z = standard_normal.pdf(z_alpha);

    // ES_alpha = -mean + stddev * (phi(z_alpha) / alpha)
    -mean + stddev * (phi_z / alpha)
}

#[cfg(any(test, feature = "pg_test"))]
#[pg_schema]
mod gaussian_tests {
    use super::*;
    use pgrx::prelude::*;

    #[pg_test]
    fn test_gaussian_var_es() {
        let mean = 0.0;
        let stddev = 1.0;

        let var = pgquant_var_gaussian(mean, stddev, 0.95);
        // For standard normal, 95% VaR is ~1.64485
        assert!((var - 1.64485).abs() < 1e-4);

        let es = pgquant_es_gaussian(mean, stddev, 0.95);
        // For standard normal, 95% ES is phi(-1.645)/0.05 ≈ 0.1031 / 0.05 = 2.0627
        assert!((es - 2.0627).abs() < 1e-3);
    }
}
