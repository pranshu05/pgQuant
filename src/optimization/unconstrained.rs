use nalgebra::{DMatrix, DVector};
use pgrx::prelude::*;

/// Calculates the unconstrained mean-variance optimal portfolio weights.
///
/// Formula: w = (1 / risk_aversion) * Sigma^{-1} * (mu - r_f * 1)
///
/// # Arguments
/// * `returns` - An array of expected returns (length N).
/// * `cov_matrix` - A flattened 1D array of the covariance matrix (length N * N, row-major).
/// * `risk_free_rate` - The risk-free rate.
/// * `risk_aversion` - The risk aversion parameter (lambda).
///
/// # Returns
/// An array of optimal weights for the risky assets.
#[pg_extern]
pub fn pgquant_mv_optimize_unconstrained(
    returns: Vec<f64>,
    cov_matrix: Vec<f64>,
    risk_free_rate: f64,
    risk_aversion: f64,
) -> Result<Vec<f64>, pgrx::spi::Error> {
    let n = returns.len();
    if n == 0 {
        pgrx::error!("Returns array cannot be empty.");
    }
    if cov_matrix.len() != n * n {
        pgrx::error!("Covariance matrix length must be N^2 where N is the length of returns.");
    }
    if risk_aversion <= 0.0 {
        pgrx::error!("Risk aversion must be > 0.");
    }

    let mu = DVector::from_vec(returns);
    let cov = DMatrix::from_row_slice(n, n, &cov_matrix);

    let mut excess_returns = mu;
    for i in 0..n {
        excess_returns[i] -= risk_free_rate;
    }

    // We want to solve: cov * w_unscaled = excess_returns
    // Try Cholesky first, fallback to pseudo-inverse if not positive definite
    let w_unscaled = match cov.clone().cholesky() {
        Some(chol) => chol.solve(&excess_returns),
        None => {
            let pinv = cov.pseudo_inverse(1e-9).unwrap_or_else(|_| {
                pgrx::error!("Failed to compute pseudo-inverse of covariance matrix.")
            });
            pinv * excess_returns
        }
    };

    let mut w = w_unscaled;
    for i in 0..n {
        w[i] /= risk_aversion;
    }

    Ok(w.as_slice().to_vec())
}

#[cfg(any(test, feature = "pg_test"))]
#[pg_schema]
mod tests {
    use super::*;

    #[pg_test]
    fn test_pgquant_mv_optimize_unconstrained() {
        let returns = vec![0.10, 0.12];
        let cov = vec![0.04, 0.01, 0.01, 0.05];
        let rf = 0.02;
        let lambda = 2.0;

        let w = pgquant_mv_optimize_unconstrained(returns, cov, rf, lambda).unwrap();

        assert!((w[0] - 0.7894736842105263).abs() < 1e-6);
        assert!((w[1] - 0.8421052631578947).abs() < 1e-6);
    }
}
