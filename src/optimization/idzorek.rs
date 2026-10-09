use nalgebra::DMatrix;
use pgrx::prelude::*;

/// Calculates the Black-Litterman view uncertainty matrix (Omega) using Idzorek's method.
///
/// Formulation:
/// For each view i:
/// alpha_i = (1 - c_i) / c_i
/// Omega_{ii} = tau * alpha_i * P_i * Sigma * P_i^T
/// If c_i == 0, Omega_{ii} = 1e6
///
/// # Arguments
/// * `market_weights` - Array of market capitalization weights (length N). (Ignored in closed-form)
/// * `cov_matrix` - Flattened covariance matrix (length N * N, row-major).
/// * `risk_aversion` - The risk aversion parameter (delta). (Ignored in closed-form)
/// * `tau` - Weight-on-views scalar (typically 0.05).
/// * `p_matrix` - Flattened views matrix P (length K * N, row-major).
/// * `view_confidences` - Array of confidence percentages for each view [0, 1] (length K).
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
    if cov_matrix.len() != n * n {
        pgrx::error!("Covariance matrix length must be N^2.");
    }
    if p_matrix.len() != k * n {
        pgrx::error!("P matrix length must be K*N.");
    }

    let cov = DMatrix::from_row_slice(n, n, &cov_matrix);
    let p = DMatrix::from_row_slice(k, n, &p_matrix);

    let mut omega = DMatrix::<f64>::zeros(k, k);

    for i in 0..k {
        let conf = view_confidences[i];
        if conf < 0.0 || conf > 1.0 {
            pgrx::error!("View confidences must be between 0 and 1");
        }

        if conf == 0.0 {
            omega[(i, i)] = 1e6;
            continue;
        }

        let p_view = p.row(i); // 1 x N
        let alpha = (1.0 - conf) / conf;

        // p_view * cov * p_view^T
        let variance = (&p_view * &cov * p_view.transpose())[(0, 0)];
        omega[(i, i)] = tau * alpha * variance;
    }

    Ok(omega.as_slice().to_vec())
}

#[cfg(any(test, feature = "pg_test"))]
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
}
