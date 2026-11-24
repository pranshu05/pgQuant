use nalgebra::DMatrix;
use pgrx::prelude::*;

/// Solves the Equal Risk Contribution (Risk Parity) problem via Cyclical Coordinate Descent (CCD).
///
/// Formulation:
///   min_{x > 0} 0.5 * x^T Sigma x - sum_i b_i * ln(x_i), where b_i = 1 / N
///
/// In each coordinate update, x_i is updated via the exact closed-form root of the
/// single-variable quadratic optimality condition:
///   Sigma_ii * x_i^2 + (sum_{j != i} Sigma_ij * x_j) * x_i - b_i = 0
///
/// Returns normalized weights w = x / sum(x).
pub fn risk_parity_ccd(cov: &DMatrix<f64>, max_iters: usize, tol: f64) -> Result<Vec<f64>, String> {
    let n = cov.nrows();
    if n == 0 {
        return Err("Covariance matrix cannot be empty".to_string());
    }
    if n == 1 {
        return Ok(vec![1.0]);
    }

    let b_i = 1.0 / (n as f64);

    // Initial point: inverse volatility x_i = 1 / sqrt(Sigma_ii)
    let mut x: Vec<f64> = (0..n).map(|i| 1.0 / cov[(i, i)].sqrt()).collect();

    // Maintain y = Sigma * x
    let mut y = vec![0.0; n];
    for i in 0..n {
        let mut s = 0.0;
        for j in 0..n {
            s += cov[(i, j)] * x[j];
        }
        y[i] = s;
    }

    for _ in 0..max_iters {
        #[cfg(not(test))]
        pgrx::check_for_interrupts!();
        let mut max_rel_diff = 0.0;

        for i in 0..n {
            let sigma_ii = cov[(i, i)];
            // beta_i = sum_{j != i} Sigma_ij * x_j = (Sigma * x)_i - Sigma_ii * x_i
            let beta_i = y[i] - sigma_ii * x[i];

            // Closed-form quadratic root:
            // sigma_ii * x_i^2 + beta_i * x_i - b_i = 0
            // Since sigma_ii > 0 and b_i > 0, the discriminant is strictly positive
            let disc = beta_i * beta_i + 4.0 * sigma_ii * b_i;
            let x_new = (-beta_i + disc.sqrt()) / (2.0 * sigma_ii);

            let delta_i = x_new - x[i];
            let rel_diff = delta_i.abs() / x[i];
            if rel_diff > max_rel_diff {
                max_rel_diff = rel_diff;
            }

            x[i] = x_new;

            // Incrementally update y_k += Sigma_{k, i} * delta_i
            for k in 0..n {
                y[k] += cov[(k, i)] * delta_i;
            }
        }

        if max_rel_diff < tol {
            let sum_x: f64 = x.iter().sum();
            return Ok(x.into_iter().map(|xi| xi / sum_x).collect());
        }
    }

    // Even if max_iters reached, normalize and return current best estimate
    let sum_x: f64 = x.iter().sum();
    Ok(x.into_iter().map(|xi| xi / sum_x).collect())
}

/// Standalone mathematical computation for Risk Parity (Equal Risk Contribution).
pub fn compute_risk_parity(cov_matrix: &[f64]) -> Result<Vec<f64>, String> {
    let total = cov_matrix.len();
    let n_f = (total as f64).sqrt();
    let n = n_f as usize;

    if n == 0 || n * n != total {
        return Err("Covariance matrix length must be a perfect square (N^2).".to_string());
    }
    if cov_matrix.iter().any(|c| c.is_nan() || c.is_infinite()) {
        return Err("Covariance matrix cannot contain NaN or infinite values.".to_string());
    }

    if n == 1 {
        return Ok(vec![1.0]);
    }

    let cov = DMatrix::from_row_slice(n, n, cov_matrix);

    // Validate symmetry
    for i in 0..n {
        for j in (i + 1)..n {
            if (cov[(i, j)] - cov[(j, i)]).abs() > 1e-10 {
                return Err("Covariance matrix must be symmetric.".to_string());
            }
        }
    }

    // Validate diagonal elements are strictly positive
    for i in 0..n {
        if cov[(i, i)] <= 0.0 || cov[(i, i)].is_nan() {
            return Err("Covariance matrix diagonal elements must be positive.".to_string());
        }
    }

    risk_parity_ccd(&cov, 1000, 1e-12)
}

/// Calculates risk parity (Equal Risk Contribution) portfolio weights.
///
/// Risk parity allocates capital such that the marginal risk contribution of each
/// asset is equal, ensuring true diversification by risk rather than capital.
///
/// Mathematical Formulation
/// Solves the strictly convex Equal Risk Contribution problem:
///   min_{x > 0} (1/2 * x^T Σ x - sum_{i=1}^N b_i * ln(x_i)), where b_i = 1/N
///
/// Uses Cyclical Coordinate Descent (CCD, Griveau-Billion et al., 2013) with
/// closed-form single-coordinate quadratic updates for guaranteed global convergence
/// in 10-20 iterations.
///
/// Arguments: cov_matrix
/// Returns: An array of optimal risk parity weights
#[pg_extern]
pub fn pgquant_risk_parity_optimize(cov_matrix: Vec<f64>) -> Result<Vec<f64>, pgrx::spi::Error> {
    let weights = compute_risk_parity(&cov_matrix).unwrap_or_else(|e| {
        pgrx::error!("{}", e);
    });

    let n = weights.len();
    if n > 1 {
        let cov = DMatrix::from_row_slice(n, n, &cov_matrix);
        let mut sigma_w = vec![0.0; n];
        for (i, elem) in sigma_w.iter_mut().enumerate().take(n) {
            let mut s = 0.0;
            for (j, &wj) in weights.iter().enumerate().take(n) {
                s += cov[(i, j)] * wj;
            }
            *elem = s;
        }

        let port_var: f64 = weights
            .iter()
            .zip(sigma_w.iter())
            .map(|(&wi, &swi)| wi * swi)
            .sum();
        let target_rc = port_var / n as f64;

        let max_deviation = (0..n)
            .map(|i| {
                let rc_i = weights[i] * sigma_w[i];
                (rc_i - target_rc).abs()
            })
            .fold(0.0f64, f64::max);

        // If the maximum deviation in risk contribution is more than 1% of
        // portfolio variance, the optimizer has not converged well enough.
        if max_deviation > 0.01 * port_var {
            pgrx::warning!(
                "Risk parity optimizer converged with residual deviation {:.2e}. Results may be approximate.",
                max_deviation
            );
        }
    }

    Ok(weights)
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    fn risk_contributions(weights: &[f64], cov: &[f64]) -> Vec<f64> {
        let n = weights.len();
        let mut sigma_w = vec![0.0; n];
        for i in 0..n {
            for j in 0..n {
                sigma_w[i] += cov[i * n + j] * weights[j];
            }
        }
        weights
            .iter()
            .zip(sigma_w.iter())
            .map(|(&w, &sw)| w * sw)
            .collect()
    }

    #[test]
    fn test_risk_parity_2x2_equal_variance() {
        let cov = vec![0.04, 0.0, 0.0, 0.04];
        let w = compute_risk_parity(&cov).unwrap();

        assert_eq!(w.len(), 2);
        let sum: f64 = w.iter().sum();
        assert!((sum - 1.0).abs() < 1e-6);
        assert!((w[0] - 0.5).abs() < 1e-4);
        assert!((w[1] - 0.5).abs() < 1e-4);
    }

    #[test]
    fn test_risk_parity_2x2_unequal_variance() {
        let cov = vec![0.04, 0.0, 0.0, 0.16];
        let w = compute_risk_parity(&cov).unwrap();

        let rc = risk_contributions(&w, &cov);
        let rc_ratio = rc[0] / rc[1];
        assert!((rc_ratio - 1.0).abs() < 1e-3);
        assert!(w[0] > w[1]);
    }

    #[test]
    fn test_risk_parity_3x3() {
        let cov = vec![0.04, 0.006, 0.002, 0.006, 0.09, 0.009, 0.002, 0.009, 0.01];
        let w = compute_risk_parity(&cov).unwrap();

        assert_eq!(w.len(), 3);
        let sum: f64 = w.iter().sum();
        assert!((sum - 1.0).abs() < 1e-6);

        for (i, &wi) in w.iter().enumerate() {
            assert!(wi > 0.0, "Weight {} must be positive, got {}", i, wi);
        }

        let rc = risk_contributions(&w, &cov);
        let mean_rc: f64 = rc.iter().sum::<f64>() / rc.len() as f64;
        for &rci in &rc {
            let rel_error = (rci - mean_rc).abs() / mean_rc;
            assert!(rel_error < 0.01);
        }

        assert!(w[2] > w[0] && w[0] > w[1]);
    }

    #[test]
    fn test_risk_parity_single_asset() {
        let cov = vec![0.04];
        let w = compute_risk_parity(&cov).unwrap();
        assert_eq!(w.len(), 1);
        assert!((w[0] - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_risk_parity_4_asset_correlated() {
        let cov = vec![
            0.0225, 0.0045, 0.0030, 0.0015, 0.0045, 0.0400, 0.0060, 0.0020, 0.0030, 0.0060, 0.0100,
            0.0025, 0.0015, 0.0020, 0.0025, 0.0064,
        ];
        let w = compute_risk_parity(&cov).unwrap();

        assert_eq!(w.len(), 4);
        let sum: f64 = w.iter().sum();
        assert!((sum - 1.0).abs() < 1e-6);

        let rc = risk_contributions(&w, &cov);
        let mean_rc: f64 = rc.iter().sum::<f64>() / rc.len() as f64;
        for &rci in &rc {
            let rel_error = (rci - mean_rc).abs() / mean_rc;
            assert!(rel_error < 0.01);
        }
    }

    #[test]
    fn test_risk_parity_validation() {
        assert!(compute_risk_parity(&[]).is_err());
        assert!(compute_risk_parity(&[0.04, 0.01]).is_err());
        assert!(compute_risk_parity(&[0.04, 0.02, 0.01, 0.04]).is_err());
        assert!(compute_risk_parity(&[-0.04, 0.0, 0.0, 0.04]).is_err());
    }

    #[test]
    fn test_risk_parity_ccd_100_assets() {
        let n = 100;
        let mut cov_data = vec![0.01; n * n];
        for i in 0..n {
            cov_data[i * n + i] = 0.04 + 0.001 * (i as f64);
        }
        let cov = DMatrix::from_row_slice(n, n, &cov_data);
        let w = risk_parity_ccd(&cov, 500, 1e-12).expect("CCD should converge on 100 assets");

        assert_eq!(w.len(), n);
        let sum_w: f64 = w.iter().sum();
        assert!((sum_w - 1.0).abs() < 1e-9);

        let mut sigma_w = vec![0.0; n];
        for i in 0..n {
            for j in 0..n {
                sigma_w[i] += cov[(i, j)] * w[j];
            }
        }
        let rc: Vec<f64> = (0..n).map(|i| w[i] * sigma_w[i]).collect();
        let mean_rc: f64 = rc.iter().sum::<f64>() / (n as f64);
        for &rci in &rc {
            let rel_diff = (rci - mean_rc).abs() / mean_rc;
            assert!(rel_diff < 1e-6);
        }
    }
}
