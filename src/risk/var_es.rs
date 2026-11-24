use pgrx::prelude::*;
use statrs::distribution::{Continuous, ContinuousCDF, Normal, StudentsT};

/// Standalone mathematical computation for historical VaR.
pub fn compute_var_historical(mut returns: Vec<f64>, confidence: f64) -> Result<f64, &'static str> {
    if confidence.is_nan() || confidence <= 0.0 || confidence >= 1.0 {
        return Err("Confidence must be strictly between 0.0 and 1.0 (e.g., 0.95)");
    }
    if returns.is_empty() {
        return Err("Returns array cannot be empty");
    }
    if returns.iter().any(|&r| r.is_nan() || r.is_infinite()) {
        return Err("Returns array cannot contain NaN or infinite values");
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

    Ok(-var_threshold)
}

/// Standalone mathematical computation for historical Expected Shortfall.
pub fn compute_es_historical(mut returns: Vec<f64>, confidence: f64) -> Result<f64, &'static str> {
    if confidence.is_nan() || confidence <= 0.0 || confidence >= 1.0 {
        return Err("Confidence must be strictly between 0.0 and 1.0 (e.g., 0.95)");
    }
    if returns.is_empty() {
        return Err("Returns array cannot be empty");
    }
    if returns.iter().any(|&r| r.is_nan() || r.is_infinite()) {
        return Err("Returns array cannot contain NaN or infinite values");
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

    let tail_len = index + 1;
    if tail_len == 0 {
        return Ok(0.0);
    }

    let left_sum: f64 = left.iter().sum();
    let total_sum = left_sum + var_threshold;
    let avg = total_sum / (tail_len as f64);

    Ok(-avg)
}

/// Standalone mathematical computation for Gaussian VaR.
pub fn compute_var_gaussian(mean: f64, stddev: f64, confidence: f64) -> Result<f64, &'static str> {
    if confidence.is_nan() || confidence <= 0.0 || confidence >= 1.0 {
        return Err("Confidence must be strictly between 0.0 and 1.0 (e.g., 0.95)");
    }
    if mean.is_nan() || mean.is_infinite() {
        return Err("Mean cannot be NaN or infinite");
    }
    if stddev <= 0.0 || stddev.is_nan() || stddev.is_infinite() {
        return Err("Standard deviation must be strictly positive and finite");
    }
    let n = Normal::new(mean, stddev).map_err(|_| "Failed to initialize Normal distribution")?;
    let alpha = 1.0 - confidence;
    Ok(-n.inverse_cdf(alpha))
}

/// Standalone mathematical computation for Gaussian Expected Shortfall.
pub fn compute_es_gaussian(mean: f64, stddev: f64, confidence: f64) -> Result<f64, &'static str> {
    if confidence.is_nan() || confidence <= 0.0 || confidence >= 1.0 {
        return Err("Confidence must be strictly between 0.0 and 1.0 (e.g., 0.95)");
    }
    if mean.is_nan() || mean.is_infinite() {
        return Err("Mean cannot be NaN or infinite");
    }
    if stddev <= 0.0 || stddev.is_nan() || stddev.is_infinite() {
        return Err("Standard deviation must be strictly positive and finite");
    }
    let alpha = 1.0 - confidence;
    let standard_normal =
        Normal::new(0.0, 1.0).map_err(|_| "Failed to initialize standard Normal distribution")?;
    let z_alpha = standard_normal.inverse_cdf(alpha);
    let phi_z = standard_normal.pdf(z_alpha);
    Ok(-mean + stddev * (phi_z / alpha))
}

/// Standalone mathematical computation for Student-t VaR.
pub fn compute_var_t(
    mean: f64,
    stddev: f64,
    dof: f64,
    confidence: f64,
) -> Result<f64, &'static str> {
    if confidence.is_nan() || confidence <= 0.0 || confidence >= 1.0 {
        return Err("Confidence must be strictly between 0.0 and 1.0 (e.g., 0.95)");
    }
    if mean.is_nan() || mean.is_infinite() {
        return Err("Mean cannot be NaN or infinite");
    }
    if stddev <= 0.0 || stddev.is_nan() || stddev.is_infinite() {
        return Err("Standard deviation must be strictly positive and finite");
    }
    if dof <= 0.0 || dof.is_nan() || dof.is_infinite() {
        return Err("Degrees of freedom must be strictly positive and finite");
    }
    let t_dist = StudentsT::new(mean, stddev, dof)
        .map_err(|_| "Failed to initialize Student-t distribution")?;
    let alpha = 1.0 - confidence;
    Ok(-t_dist.inverse_cdf(alpha))
}

/// Standalone mathematical computation for Student-t Expected Shortfall.
pub fn compute_es_t(
    mean: f64,
    stddev: f64,
    dof: f64,
    confidence: f64,
) -> Result<f64, &'static str> {
    if confidence.is_nan() || confidence <= 0.0 || confidence >= 1.0 {
        return Err("Confidence must be strictly between 0.0 and 1.0 (e.g., 0.95)");
    }
    if mean.is_nan() || mean.is_infinite() {
        return Err("Mean cannot be NaN or infinite");
    }
    if stddev <= 0.0 || stddev.is_nan() || stddev.is_infinite() {
        return Err("Standard deviation must be strictly positive and finite");
    }
    if dof <= 1.0 || dof.is_nan() || dof.is_infinite() {
        return Err("Degrees of freedom must be > 1 for Expected Shortfall to be defined");
    }
    let alpha = 1.0 - confidence;
    let standard_t = StudentsT::new(0.0, 1.0, dof)
        .map_err(|_| "Failed to initialize standard Student-t distribution")?;
    let t_alpha = standard_t.inverse_cdf(alpha);
    let f_t_alpha = standard_t.pdf(t_alpha);
    let es_standard = (f_t_alpha / alpha) * ((dof + t_alpha.powi(2)) / (dof - 1.0));
    Ok(-mean + stddev * es_standard)
}

/// Computes the historical Value at Risk (VaR) for a given array of returns.
/// `confidence` is typically 0.95 or 0.99.
/// Returns a positive number representing the loss amount (e.g. 0.05 for 5% loss).
#[pg_extern]
pub fn pgquant_var_historical(returns: Vec<f64>, confidence: f64) -> f64 {
    compute_var_historical(returns, confidence).unwrap_or_else(|e| pgrx::error!("{}", e))
}

/// Computes the historical Expected Shortfall (ES) / CVaR.
/// Averages all returns that are worse than or equal to the VaR.
/// Returns a positive number representing the expected loss magnitude.
#[pg_extern]
pub fn pgquant_es_historical(returns: Vec<f64>, confidence: f64) -> f64 {
    compute_es_historical(returns, confidence).unwrap_or_else(|e| pgrx::error!("{}", e))
}

/// Computes parametric Value at Risk (VaR) assuming a normal distribution.
/// `mean` and `stddev` are the distribution parameters (e.g. daily mean and stddev).
/// Returns a positive number representing the expected loss magnitude.
#[pg_extern]
pub fn pgquant_var_gaussian(mean: f64, stddev: f64, confidence: f64) -> f64 {
    compute_var_gaussian(mean, stddev, confidence).unwrap_or_else(|e| pgrx::error!("{}", e))
}

/// Computes parametric Expected Shortfall (ES) assuming a normal distribution.
/// Returns a positive number representing the conditional expected loss magnitude.
#[pg_extern]
pub fn pgquant_es_gaussian(mean: f64, stddev: f64, confidence: f64) -> f64 {
    compute_es_gaussian(mean, stddev, confidence).unwrap_or_else(|e| pgrx::error!("{}", e))
}

/// Computes parametric Value at Risk (VaR) assuming a Student-t distribution.
/// `mean`, `stddev` (scale), and `dof` (degrees of freedom) are the distribution parameters.
#[pg_extern]
pub fn pgquant_var_t(mean: f64, stddev: f64, dof: f64, confidence: f64) -> f64 {
    compute_var_t(mean, stddev, dof, confidence).unwrap_or_else(|e| pgrx::error!("{}", e))
}

/// Computes parametric Expected Shortfall (ES) assuming a Student-t distribution.
/// Note: dof must be > 1 for the mean (and thus ES) to exist.
#[pg_extern]
pub fn pgquant_es_t(mean: f64, stddev: f64, dof: f64, confidence: f64) -> f64 {
    compute_es_t(mean, stddev, dof, confidence).unwrap_or_else(|e| pgrx::error!("{}", e))
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn test_historical_var_es() {
        let mut returns = vec![0.01; 95];
        returns.extend_from_slice(&[-0.01, -0.02, -0.03, -0.04, -0.05]);

        let var = compute_var_historical(returns.clone(), 0.95).unwrap();
        assert!((var - 0.01).abs() < 1e-6);

        let es = compute_es_historical(returns.clone(), 0.95).unwrap();
        assert!((es - 0.03).abs() < 1e-6);
    }

    #[test]
    fn test_historical_validation() {
        assert!(compute_var_historical(vec![], 0.95).is_err());
        assert!(compute_var_historical(vec![0.01, f64::NAN], 0.95).is_err());
        assert!(compute_var_historical(vec![0.01], 1.5).is_err());
        assert!(compute_var_historical(vec![0.01], -0.1).is_err());
    }

    #[test]
    fn test_gaussian_var_es() {
        let mean = 0.0;
        let stddev = 1.0;

        let var = compute_var_gaussian(mean, stddev, 0.95).unwrap();
        assert!((var - 1.64485).abs() < 1e-4);

        let es = compute_es_gaussian(mean, stddev, 0.95).unwrap();
        assert!((es - 2.0627).abs() < 1e-3);
    }

    #[test]
    fn test_gaussian_validation() {
        assert!(compute_var_gaussian(0.0, -1.0, 0.95).is_err());
        assert!(compute_var_gaussian(f64::NAN, 1.0, 0.95).is_err());
        assert!(compute_var_gaussian(0.0, 1.0, 0.0).is_err());
    }

    #[test]
    fn test_t_var_es() {
        let mean = 0.0;
        let stddev = 1.0;
        let dof = 5.0;

        let var = compute_var_t(mean, stddev, dof, 0.95).unwrap();
        assert!((var - 2.01504).abs() < 1e-4);

        let es = compute_es_t(mean, stddev, dof, 0.95).unwrap();
        assert!((es - 2.8901).abs() < 1e-3);
    }

    #[test]
    fn test_t_validation() {
        assert!(compute_var_t(0.0, 1.0, 0.0, 0.95).is_err());
        assert!(compute_es_t(0.0, 1.0, 1.0, 0.95).is_err()); // dof must be > 1 for ES
    }
}

#[cfg(feature = "pg_test")]
#[pg_schema]
mod tests {
    use super::*;

    #[pg_test]
    fn test_pg_var_es() {
        let mut returns = vec![0.01; 95];
        returns.extend_from_slice(&[-0.01, -0.02, -0.03, -0.04, -0.05]);
        let var = pgquant_var_historical(returns.clone(), 0.95);
        assert!((var - 0.01).abs() < 1e-6);
    }
}
