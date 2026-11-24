use nalgebra::DMatrix;
use pgrx::prelude::*;

/// Calculates the Black-Litterman view uncertainty matrix (Omega) using Idzorek's proportional variance method.
///
/// Formulation:
/// This implements the closed-form proportional variance view uncertainty approximation
/// (He & Litterman, 1999; Idzorek, 2005):
///   alpha_i = (1 - c_i) / c_i
///   Omega_{ii} = max(tau * alpha_i * (P_i * Sigma * P_i^T), epsilon)
///
/// Boundary handling:
/// - If c_i == 0.0 (0% confidence / uninformative): Omega_{ii} = 1e6
/// - If c_i == 1.0 (100% confidence / absolute certainty): alpha_i = 0, bounded below by
///   epsilon = 1e-8 to ensure numerical stability and prevent pseudo-inverse zeroing in Black-Litterman.
///
/// Note:
/// `_market_weights` and `_risk_aversion` are accepted for API compatibility with the full
/// numerical implied tilt calibration framework.
///
/// # Arguments
/// * `_market_weights` - Array of market capitalization weights (length N).
/// * `cov_matrix` - Flattened covariance matrix (length N * N, row-major).
/// * `_risk_aversion` - The risk aversion parameter (delta).
/// * `tau` - Weight-on-views scalar (typically 0.05).
/// * `p_matrix` - Flattened views matrix P (length K * N, row-major).
/// * `view_confidences` - Array of confidence percentages for each view in [0.0, 1.0] (length K).
///
/// # Returns
/// An array representing the flattened K x K diagonal uncertainty matrix Omega.
#[pg_extern]
pub fn pgquant_bl_idzorek_omega(
    _market_weights: Vec<f64>,
    cov_matrix: Vec<f64>,
    _risk_aversion: f64,
    tau: f64,
    p_matrix: Vec<f64>,
    view_confidences: Vec<f64>,
) -> Result<Vec<f64>, pgrx::spi::Error> {
    let k = view_confidences.len();
    let n = _market_weights.len();

    if k == 0 {
        pgrx::error!("View confidences array cannot be empty.");
    }
    if n == 0 {
        pgrx::error!("Market weights array cannot be empty.");
    }
    if cov_matrix.len() != n * n {
        pgrx::error!("Covariance matrix length must be N^2.");
    }
    if p_matrix.len() != k * n {
        pgrx::error!("P matrix length must be K*N.");
    }
    if tau <= 0.0 || tau.is_nan() || tau.is_infinite() {
        pgrx::error!("Tau must be strictly positive and finite.");
    }
    if cov_matrix.iter().any(|c| c.is_nan() || c.is_infinite()) {
        pgrx::error!("Covariance matrix cannot contain NaN or infinite values.");
    }
    if p_matrix.iter().any(|p| p.is_nan() || p.is_infinite()) {
        pgrx::error!("P matrix cannot contain NaN or infinite values.");
    }
    if view_confidences
        .iter()
        .any(|c| c.is_nan() || *c < 0.0 || *c > 1.0)
    {
        pgrx::error!("View confidences must be between 0.0 and 1.0.");
    }

    let cov = DMatrix::from_row_slice(n, n, &cov_matrix);
    let p = DMatrix::from_row_slice(k, n, &p_matrix);

    let mut omega = DMatrix::<f64>::zeros(k, k);
    const MIN_UNCERTAINTY: f64 = 1e-8;

    for i in 0..k {
        let conf = view_confidences[i];

        if conf == 0.0 {
            omega[(i, i)] = 1e6;
            continue;
        }

        let p_view = p.row(i); // 1 x N
        let alpha = (1.0 - conf) / conf;

        // p_view * cov * p_view^T
        let variance = (p_view * &cov * p_view.transpose())[(0, 0)];
        let calculated_omega = tau * alpha * variance;
        omega[(i, i)] = calculated_omega.max(MIN_UNCERTAINTY);
    }

    Ok(omega.as_slice().to_vec())
}

#[cfg(feature = "pg_test")]
#[pg_schema]
mod tests {
    use super::*;

    #[pg_test]
    fn test_pgquant_bl_idzorek_omega() {
        let market_weights = vec![0.5, 0.5];
        let cov_matrix = vec![0.04, 0.01, 0.01, 0.05];
        let risk_aversion = 2.0;
        let tau = 0.05;
        let p_matrix = vec![1.0, -1.0];
        // Test confidence = 0.5
        let view_confidences = vec![0.5];

        let omega = pgquant_bl_idzorek_omega(
            market_weights,
            cov_matrix,
            risk_aversion,
            tau,
            p_matrix,
            view_confidences,
        )
        .unwrap();

        // alpha = (1 - 0.5) / 0.5 = 1.0
        // variance = P * Cov * P^T = 0.04 + 0.05 - 2(0.01) = 0.07
        // omega = tau * alpha * variance = 0.05 * 1.0 * 0.07 = 0.0035
        assert!((omega[0] - 0.0035).abs() < 1e-6);
    }

    #[pg_test]
    fn test_pgquant_bl_idzorek_boundary_confidences() {
        let market_weights = vec![0.5, 0.5];
        let cov_matrix = vec![0.04, 0.01, 0.01, 0.05];
        let risk_aversion = 2.0;
        let tau = 0.05;
        let p_matrix = vec![1.0, -1.0, 1.0, 0.0];
        // View 0: 100% confidence (should have floor 1e-8)
        // View 1: 0% confidence (should have 1e6)
        let view_confidences = vec![1.0, 0.0];

        let omega = pgquant_bl_idzorek_omega(
            market_weights,
            cov_matrix,
            risk_aversion,
            tau,
            p_matrix,
            view_confidences,
        )
        .unwrap();

        // 2x2 matrix: omega[0,0] is omega[0], omega[1,1] is omega[3]
        assert_eq!(omega[0], 1e-8);
        assert_eq!(omega[3], 1e6);
    }
}
