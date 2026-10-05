use pgrx::prelude::*;

/// Computes the historical Value at Risk (VaR) for a given array of returns.
/// `confidence` is typically 0.95 or 0.99.
/// Returns a positive number representing the loss amount (e.g. 0.05 for 5% loss).
#[pg_extern]
pub fn pgquant_var_historical(mut returns: Vec<f64>, confidence: f64) -> f64 {
    if returns.is_empty() {
        return 0.0;
    }

    let alpha = 1.0 - confidence;
    let mut index = (alpha * returns.len() as f64).round() as usize;
    if index == 0 {
        index = 1;
    }
    let index = index.min(returns.len()) - 1;

    let (_, &mut var_threshold, _) = returns.select_nth_unstable_by(index, |a, b| {
        a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal)
    });

    -var_threshold
}

/// Computes the historical Expected Shortfall (ES) / CVaR.
/// Averages all returns that are worse than or equal to the VaR.
/// Returns a positive number representing the expected loss magnitude.
#[pg_extern]
pub fn pgquant_es_historical(mut returns: Vec<f64>, confidence: f64) -> f64 {
    if returns.is_empty() {
        return 0.0;
    }

    let alpha = 1.0 - confidence;
    let mut index = (alpha * returns.len() as f64).round() as usize;
    if index == 0 {
        index = 1;
    }
    let index = index.min(returns.len()) - 1;

    let (left, &mut var_threshold, _) = returns.select_nth_unstable_by(index, |a, b| {
        a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal)
    });

    // The elements <= var_threshold are in `left` and the `var_threshold` itself.
    // So there are exactly `index + 1` elements in the tail.
    let tail_len = index + 1;
    if tail_len == 0 {
        return 0.0;
    }

    let left_sum: f64 = left.iter().sum();
    let total_sum = left_sum + var_threshold;
    let avg = total_sum / (tail_len as f64);

    -avg
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

use statrs::distribution::StudentsT;

/// Computes parametric Value at Risk (VaR) assuming a Student-t distribution.
/// `mean`, `stddev` (scale), and `dof` (degrees of freedom) are the distribution parameters.
#[pg_extern]
pub fn pgquant_var_t(mean: f64, stddev: f64, dof: f64, confidence: f64) -> f64 {
    if stddev <= 0.0 {
        pgrx::error!("Standard deviation must be strictly positive");
    }
    if dof <= 0.0 {
        pgrx::error!("Degrees of freedom must be strictly positive");
    }

    // statrs StudentsT takes (location, scale, dof)
    let t_dist = StudentsT::new(mean, stddev, dof).unwrap();
    let alpha = 1.0 - confidence;

    -t_dist.inverse_cdf(alpha)
}

/// Computes parametric Expected Shortfall (ES) assuming a Student-t distribution.
/// Note: dof must be > 1 for the mean (and thus ES) to exist.
#[pg_extern]
pub fn pgquant_es_t(mean: f64, stddev: f64, dof: f64, confidence: f64) -> f64 {
    if stddev <= 0.0 {
        pgrx::error!("Standard deviation must be strictly positive");
    }
    if dof <= 1.0 {
        pgrx::error!("Degrees of freedom must be > 1 for Expected Shortfall to be defined");
    }

    let alpha = 1.0 - confidence;

    // Use a standard Student-t (location=0, scale=1) for the core analytical formula
    let standard_t = StudentsT::new(0.0, 1.0, dof).unwrap();

    // Quantile of the standard t-distribution
    let t_alpha = standard_t.inverse_cdf(alpha);

    // PDF at the quantile
    let f_t_alpha = standard_t.pdf(t_alpha);

    // Analytic ES for standard t:
    // ES_standard = ( f(t_alpha) / alpha ) * ( (nu + t_alpha^2) / (nu - 1) )
    let es_standard = (f_t_alpha / alpha) * ((dof + t_alpha.powi(2)) / (dof - 1.0));

    // Transform back to the generalized location-scale family
    // ES = -mu + sigma * ES_standard
    -mean + stddev * es_standard
}

#[cfg(any(test, feature = "pg_test"))]
#[pg_schema]
mod tests {
    use super::*;

    #[pg_test]
    fn test_historical_var_es() {
        let mut returns = vec![0.01; 95];
        returns.extend_from_slice(&[-0.01, -0.02, -0.03, -0.04, -0.05]);

        let var = pgquant_var_historical(returns.clone(), 0.95);
        assert!((var - 0.01).abs() < 1e-6);

        let es = pgquant_es_historical(returns.clone(), 0.95);
        assert!((es - 0.03).abs() < 1e-6);
    }

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

    #[pg_test]
    fn test_t_var_es() {
        let mean = 0.0;
        let stddev = 1.0;
        let dof = 5.0;

        let var = pgquant_var_t(mean, stddev, dof, 0.95);
        // For standard t with dof=5, 95% VaR (t_0.05, 5) ≈ 2.015
        assert!((var - 2.01504).abs() < 1e-4);

        let es = pgquant_es_t(mean, stddev, dof, 0.95);
        // Analytical ES for t(0, 1, 5) at alpha=0.05
        // t_alpha = -2.01504
        // f(t_alpha) = 0.065298
        // ES = (0.065298 / 0.05) * ((5 + 2.01504^2) / 4)
        // ES = 1.30596 * (9.0604 / 4) = 1.30596 * 2.2651 = 2.958
        assert!((es - 2.8901).abs() < 1e-3);
    }
}
