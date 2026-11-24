use nalgebra::{DMatrix, DVector};
use pgrx::prelude::*;

/// Calculates the Black-Litterman posterior expected returns and covariance matrix.
///
/// Formulation:
/// Pi = risk_aversion * Cov * market_weights
/// E[R] = [(\tau \Sigma)^{-1} + P^T \Omega^{-1} P]^{-1} [(\tau \Sigma)^{-1} \Pi + P^T \Omega^{-1} Q]
/// \Sigma_p = \Sigma + [(\tau \Sigma)^{-1} + P^T \Omega^{-1} P]^{-1}
///
/// # Arguments
/// * `market_weights` - Array of market capitalization weights (length N).
/// * `cov_matrix` - Flattened covariance matrix (length N * N).
/// * `risk_aversion` - The risk aversion parameter (delta).
/// * `tau` - Weight-on-views scalar (typically 0.05).
/// * `p_matrix` - Flattened views matrix P (length K * N).
/// * `q_vector` - Array of view expected returns Q (length K).
/// * `omega_matrix` - Flattened uncertainty matrix of views Omega (length K * K).
///
/// # Returns
/// A composite type containing `(expected_returns, posterior_cov_matrix)`.
/// We will implement this as returning `(Vec<f64>, Vec<f64>)` directly via pgrx.
type BlackLittermanResult = Result<
    TableIterator<
        'static,
        (
            name!(expected_returns, Vec<f64>),
            name!(posterior_covariance, Vec<f64>),
        ),
    >,
    pgrx::spi::Error,
>;

/// Standalone mathematical computation for Black-Litterman model.
///
/// Implements:
/// - Prior: \(\Pi = \delta \Sigma w_{mkt}\)
/// - Posterior Mean: \(E[R] = [(\tau \Sigma)^{-1} + P^T \Omega^{-1} P]^{-1} [(\tau \Sigma)^{-1} \Pi + P^T \Omega^{-1} Q]\)
/// - Posterior Covariance: \(M = [(\tau \Sigma)^{-1} + P^T \Omega^{-1} P]^{-1}\), \(\Sigma_p = \Sigma + M\)
pub fn compute_black_litterman(
    market_weights: &[f64],
    cov_matrix: &[f64],
    risk_aversion: f64,
    tau: f64,
    p_matrix: &[f64],
    q_vector: &[f64],
    omega_matrix: &[f64],
) -> Result<(Vec<f64>, Vec<f64>), &'static str> {
    let n = market_weights.len();
    let k = q_vector.len();

    if n == 0 {
        return Err("Market weights array cannot be empty.");
    }
    if cov_matrix.len() != n * n {
        return Err("Covariance matrix length must be N^2.");
    }
    if p_matrix.len() != k * n {
        return Err("P matrix length must be K*N.");
    }
    if omega_matrix.len() != k * k {
        return Err("Omega matrix length must be K^2.");
    }
    if risk_aversion <= 0.0 || risk_aversion.is_nan() || risk_aversion.is_infinite() {
        return Err("Risk aversion must be > 0 and finite.");
    }
    if tau <= 0.0 || tau.is_nan() || tau.is_infinite() {
        return Err("Tau must be > 0 and finite.");
    }
    if market_weights.iter().any(|w| w.is_nan() || w.is_infinite()) {
        return Err("Market weights cannot contain NaN or infinite values.");
    }
    if cov_matrix.iter().any(|c| c.is_nan() || c.is_infinite()) {
        return Err("Covariance matrix cannot contain NaN or infinite values.");
    }
    if p_matrix.iter().any(|p| p.is_nan() || p.is_infinite()) {
        return Err("P matrix cannot contain NaN or infinite values.");
    }
    if q_vector.iter().any(|q| q.is_nan() || q.is_infinite()) {
        return Err("Q vector cannot contain NaN or infinite values.");
    }
    if omega_matrix.iter().any(|o| o.is_nan() || o.is_infinite()) {
        return Err("Omega matrix cannot contain NaN or infinite values.");
    }

    let w_mkt = DVector::from_row_slice(market_weights);
    let cov = DMatrix::from_row_slice(n, n, cov_matrix);
    let p = DMatrix::from_row_slice(k, n, p_matrix);
    let q = DVector::from_row_slice(q_vector);
    let omega = DMatrix::from_row_slice(k, k, omega_matrix);

    // Pi = delta * Cov * w_mkt
    let pi = risk_aversion * &cov * w_mkt;

    // tau_cov = tau * Cov
    let tau_cov = tau * &cov;

    // We need (tau * Cov)^{-1}
    let tau_cov_inv = tau_cov
        .clone()
        .cholesky()
        .map(|c| c.inverse())
        .or_else(|| tau_cov.pseudo_inverse(1e-9).ok())
        .ok_or("Failed to invert tau * Cov")?;

    // We need Omega^{-1}
    let omega_inv = omega
        .clone()
        .cholesky()
        .map(|c| c.inverse())
        .or_else(|| omega.pseudo_inverse(1e-9).ok())
        .ok_or("Failed to invert Omega")?;

    // P^T * Omega^{-1} * P
    let p_t = p.transpose();
    let pt_omegainv_p = &p_t * &omega_inv * &p;

    // inner_term = (tau_cov_inv + P^T * Omega^{-1} * P)
    let inner_term = &tau_cov_inv + pt_omegainv_p;

    // inner_term_inv = inner_term^{-1}
    let inner_term_inv = inner_term
        .clone()
        .cholesky()
        .map(|c| c.inverse())
        .or_else(|| inner_term.pseudo_inverse(1e-9).ok())
        .ok_or("Failed to invert inner term")?;

    // rhs = tau_cov_inv * Pi + P^T * Omega^{-1} * Q
    let rhs = &tau_cov_inv * pi + &p_t * &omega_inv * q;

    // E[R] = inner_term_inv * rhs
    let expected_returns = &inner_term_inv * rhs;

    // Sigma_p = Cov + inner_term_inv
    let posterior_covariance = &cov + &inner_term_inv;

    Ok((
        expected_returns.as_slice().to_vec(),
        posterior_covariance.as_slice().to_vec(),
    ))
}

#[pg_extern]
pub fn pgquant_black_litterman(
    market_weights: Vec<f64>,
    cov_matrix: Vec<f64>,
    risk_aversion: f64,
    tau: f64,
    p_matrix: Vec<f64>,
    q_vector: Vec<f64>,
    omega_matrix: Vec<f64>,
) -> BlackLittermanResult {
    let (er, cov_p) = compute_black_litterman(
        &market_weights,
        &cov_matrix,
        risk_aversion,
        tau,
        &p_matrix,
        &q_vector,
        &omega_matrix,
    )
    .unwrap_or_else(|e| pgrx::error!("{}", e));

    Ok(TableIterator::new(std::iter::once((er, cov_p))))
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn test_black_litterman_analytical() {
        let market_weights = vec![0.5, 0.5];
        let cov_matrix = vec![0.04, 0.01, 0.01, 0.05];
        let risk_aversion = 2.0;
        let tau = 0.05;
        let p_matrix = vec![1.0, -1.0];
        let q_vector = vec![0.05];
        let omega_matrix = vec![0.01];

        let (er, cov_p) = compute_black_litterman(
            &market_weights,
            &cov_matrix,
            risk_aversion,
            tau,
            &p_matrix,
            &q_vector,
            &omega_matrix,
        )
        .unwrap();

        assert!((er[0] - 0.05666667).abs() < 1e-6);
        assert!((er[1] - 0.05111111).abs() < 1e-6);

        assert!((cov_p[0] - 0.04183333).abs() < 1e-6);
        assert!((cov_p[1] - 0.01072222).abs() < 1e-6);
        assert!((cov_p[2] - 0.01072222).abs() < 1e-6);
        assert!((cov_p[3] - 0.05220370).abs() < 1e-6);
    }

    #[test]
    fn test_black_litterman_validation() {
        assert!(compute_black_litterman(&[], &[], 2.0, 0.05, &[], &[], &[]).is_err());
        assert!(
            compute_black_litterman(&[0.5, 0.5], &[0.04], 2.0, 0.05, &[1.0], &[0.05], &[0.01])
                .is_err()
        );
        assert!(compute_black_litterman(
            &[0.5, 0.5],
            &[0.04, 0.0, 0.0, 0.04],
            -1.0,
            0.05,
            &[1.0, -1.0],
            &[0.05],
            &[0.01]
        )
        .is_err());
    }
}

#[cfg(feature = "pg_test")]
#[pg_schema]
mod tests {
    use super::*;

    #[pg_test]
    fn test_pgquant_black_litterman() {
        let market_weights = vec![0.5, 0.5];
        let cov_matrix = vec![0.04, 0.01, 0.01, 0.05];
        let risk_aversion = 2.0;
        let tau = 0.05;
        let p_matrix = vec![1.0, -1.0];
        let q_vector = vec![0.05];
        let omega_matrix = vec![0.01];

        let mut iter = pgquant_black_litterman(
            market_weights,
            cov_matrix,
            risk_aversion,
            tau,
            p_matrix,
            q_vector,
            omega_matrix,
        )
        .unwrap();
        let (er, cov_p) = iter.next().unwrap();

        assert!((er[0] - 0.05666667).abs() < 1e-6);
        assert!((er[1] - 0.05111111).abs() < 1e-6);

        assert!((cov_p[0] - 0.04183333).abs() < 1e-6);
        assert!((cov_p[1] - 0.01072222).abs() < 1e-6);
        assert!((cov_p[2] - 0.01072222).abs() < 1e-6);
        assert!((cov_p[3] - 0.05220370).abs() < 1e-6);
    }
}
