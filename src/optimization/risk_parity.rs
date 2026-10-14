use argmin::core::{CostFunction, Executor, State};
use argmin::solver::neldermead::NelderMead;
use nalgebra::DMatrix;
use pgrx::prelude::*;

/// Internal cost function for the risk parity optimization problem.

/// The risk parity objective minimizes:
///   f(y) = sum_i (RC_i - target)^2

/// where RC_i = w_i * (Sigma * w)_i is the risk contribution of asset i,
/// target = (w^T Sigma w) / N is the equal risk budget, and
/// w = softmax(y) maps unconstrained parameters y to valid portfolio weights.

/// The softmax reparameterization ensures:
/// - All weights are strictly positive (no need for bound constraints)
/// - Weights always sum to exactly 1.0 (no equality constraint)
/// This reduces the problem to an unconstrained optimization that Nelder-Mead
/// handles naturally.
struct RiskParityCost {
    cov: DMatrix<f64>,
    n: usize,
}

impl RiskParityCost {
    /// Convert unconstrained parameters y to portfolio weights via softmax.
    /// Uses the log-sum-exp trick for numerical stability:
    ///   w_i = exp(y_i - max(y)) / sum_j exp(y_j - max(y))
    fn softmax(y: &[f64]) -> Vec<f64> {
        let max_y = y.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let exps: Vec<f64> = y.iter().map(|&yi| (yi - max_y).exp()).collect();
        let sum: f64 = exps.iter().sum();
        exps.iter().map(|&e| e / sum).collect()
    }
}

impl CostFunction for RiskParityCost {
    type Param = Vec<f64>;
    type Output = f64;

    fn cost(&self, y: &Self::Param) -> Result<Self::Output, argmin::core::Error> {
        let w = Self::softmax(y);

        // Compute Sigma * w
        let mut sigma_w = vec![0.0; self.n];
        for i in 0..self.n {
            let mut s = 0.0;
            for j in 0..self.n {
                s += self.cov[(i, j)] * w[j];
            }
            sigma_w[i] = s;
        }

        // Portfolio variance: w^T Sigma w
        let port_var: f64 = w
            .iter()
            .zip(sigma_w.iter())
            .map(|(&wi, &swi)| wi * swi)
            .sum();

        // Target risk contribution per asset
        let target = port_var / self.n as f64;

        // Sum of squared deviations of risk contributions from target
        let mut obj = 0.0;
        for i in 0..self.n {
            let rc_i = w[i] * sigma_w[i];
            let diff = rc_i - target;
            obj += diff * diff;
        }
        Ok(obj)
    }
}

/// Calculates risk parity (Equal Risk Contribution) portfolio weights.

/// Risk parity allocates capital such that the marginal risk contribution of each
/// asset is equal, ensuring true diversification by risk rather than capital

/// Mathematical Formulation
/// Minimizes: Σ (w_i × (Σw)_i − w^T Σ w / N)²
/// Subject to: w_i ≥ 0, Σw_i = 1

/// Uses Nelder-Mead optimization on a softmax reparameterization of weights
/// to naturally enforce positivity and sum-to-one constraints
/// Arguments: cov_matrix
/// Returns: An array of optimal risk parity weights

#[pg_extern]
pub fn pgquant_risk_parity_optimize(cov_matrix: Vec<f64>) -> Result<Vec<f64>, pgrx::spi::Error> {
    let total = cov_matrix.len();
    let n_f = (total as f64).sqrt();
    let n = n_f as usize;

    if n == 0 || n * n != total {
        pgrx::error!("Covariance matrix length must be a perfect square (N^2).");
    }

    if n == 1 {
        // Trivial case: single asset gets 100% weight
        return Ok(vec![1.0]);
    }

    let cov = DMatrix::from_row_slice(n, n, &cov_matrix);

    // Validate the covariance matrix is symmetric (within tolerance)
    for i in 0..n {
        for j in (i + 1)..n {
            if (cov[(i, j)] - cov[(j, i)]).abs() > 1e-10 {
                pgrx::error!("Covariance matrix must be symmetric.");
            }
        }
    }

    // Validate diagonal elements are positive (non-degenerate assets)
    for i in 0..n {
        if cov[(i, i)] <= 0.0 {
            pgrx::error!("Covariance matrix diagonal elements must be positive.");
        }
    }

    let cost_fn = RiskParityCost {
        cov: cov.clone(),
        n,
    };

    // Initial guess: inverse-volatility heuristic in log space.
    // Start with y_i = -0.5 * ln(sigma_ii), which maps via softmax to
    // weights roughly proportional to 1/sigma_i — a good starting point
    // for risk parity since it already approximately equalizes risk.
    let y0: Vec<f64> = (0..n).map(|i| -0.5 * cov[(i, i)].ln()).collect();

    // Build initial simplex: y0 + n perturbation vectors
    let perturbation = 0.5;
    let mut simplex: Vec<Vec<f64>> = Vec::with_capacity(n + 1);
    simplex.push(y0.clone());
    for i in 0..n {
        let mut vertex = y0.clone();
        vertex[i] += perturbation;
        simplex.push(vertex);
    }

    let solver = NelderMead::new(simplex)
        .with_sd_tolerance(1e-14)
        .unwrap_or_else(|e| pgrx::error!("Failed to initialize Nelder-Mead solver: {}", e));

    let result = Executor::new(cost_fn, solver)
        .configure(|state| state.max_iters(10_000))
        .run()
        .unwrap_or_else(|e| pgrx::error!("Risk parity optimization failed: {}", e));

    let best_y = result
        .state()
        .get_best_param()
        .unwrap_or_else(|| pgrx::error!("No solution found by Nelder-Mead."));

    let weights = RiskParityCost::softmax(best_y);

    // Validate convergence: check that risk contributions are approximately equal
    let mut sigma_w = vec![0.0; n];
    for i in 0..n {
        for j in 0..n {
            sigma_w[i] += cov[(i, j)] * weights[j];
        }
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

    Ok(weights)
}

#[cfg(any(test, feature = "pg_test"))]
#[pg_schema]
mod tests {
    use super::*;

    /// Helper: compute risk contributions for a given set of weights and covariance.
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

    #[pg_test]
    fn test_risk_parity_2x2_equal_variance() {
        // Two uncorrelated assets with equal variance -> equal weights
        let cov = vec![0.04, 0.0, 0.0, 0.04];
        let w = pgquant_risk_parity_optimize(cov.clone()).unwrap();

        assert_eq!(w.len(), 2);
        let sum: f64 = w.iter().sum();
        assert!(
            (sum - 1.0).abs() < 1e-6,
            "Weights must sum to 1, got {}",
            sum
        );
        assert!(
            (w[0] - 0.5).abs() < 1e-4,
            "Equal variance uncorrelated assets should yield equal weights. Got {:?}",
            w
        );
        assert!(
            (w[1] - 0.5).abs() < 1e-4,
            "Equal variance uncorrelated assets should yield equal weights. Got {:?}",
            w
        );
    }

    #[pg_test]
    fn test_risk_parity_2x2_unequal_variance() {
        // Two uncorrelated assets with 4x variance difference.
        // Asset 1: sigma^2 = 0.04 (sigma = 0.20)
        // Asset 2: sigma^2 = 0.16 (sigma = 0.40)
        // For uncorrelated assets, risk parity -> w_i proportional to 1/sigma_i
        // w1 = (1/0.20) / (1/0.20 + 1/0.40) = 5/7.5 = 2/3
        // w2 = (1/0.40) / (1/0.20 + 1/0.40) = 2.5/7.5 = 1/3
        let cov = vec![0.04, 0.0, 0.0, 0.16];
        let w = pgquant_risk_parity_optimize(cov.clone()).unwrap();

        let rc = risk_contributions(&w, &cov);
        let rc_ratio = rc[0] / rc[1];
        assert!(
            (rc_ratio - 1.0).abs() < 1e-3,
            "Risk contributions should be equal. RC0={:.6}, RC1={:.6}, ratio={:.6}",
            rc[0],
            rc[1],
            rc_ratio
        );

        // Higher variance asset should get lower weight
        assert!(
            w[0] > w[1],
            "Lower-variance asset should get higher weight. w={:?}",
            w
        );
    }

    #[pg_test]
    fn test_risk_parity_3x3() {
        // 3-asset case with realistic covariance matrix
        let cov = vec![0.04, 0.006, 0.002, 0.006, 0.09, 0.009, 0.002, 0.009, 0.01];
        let w = pgquant_risk_parity_optimize(cov.clone()).unwrap();

        assert_eq!(w.len(), 3);
        let sum: f64 = w.iter().sum();
        assert!(
            (sum - 1.0).abs() < 1e-6,
            "Weights must sum to 1, got {}",
            sum
        );

        // All weights must be positive
        for (i, &wi) in w.iter().enumerate() {
            assert!(wi > 0.0, "Weight {} must be positive, got {}", i, wi);
        }

        // Risk contributions should be approximately equal
        let rc = risk_contributions(&w, &cov);
        let mean_rc: f64 = rc.iter().sum::<f64>() / rc.len() as f64;
        for (i, &rci) in rc.iter().enumerate() {
            let rel_error = (rci - mean_rc).abs() / mean_rc;
            assert!(
                rel_error < 0.01,
                "RC[{}]={:.8} deviates from mean={:.8} by {:.4}%",
                i,
                rci,
                mean_rc,
                rel_error * 100.0
            );
        }

        // Highest-variance asset (asset 1, sigma=0.30) should get lowest weight
        // Lowest-variance asset (asset 2, sigma=0.10) should get highest weight
        assert!(
            w[2] > w[0] && w[0] > w[1],
            "Weights should be inversely related to risk. w={:?}",
            w
        );
    }

    #[pg_test]
    fn test_risk_parity_single_asset() {
        let cov = vec![0.04];
        let w = pgquant_risk_parity_optimize(cov).unwrap();
        assert_eq!(w.len(), 1);
        assert!((w[0] - 1.0).abs() < 1e-10);
    }

    #[pg_test]
    fn test_risk_parity_4_asset_correlated() {
        // 4-asset correlated case
        let cov = vec![
            0.0225, 0.0045, 0.0030, 0.0015, 0.0045, 0.0400, 0.0060, 0.0020, 0.0030, 0.0060, 0.0100,
            0.0025, 0.0015, 0.0020, 0.0025, 0.0064,
        ];
        let w = pgquant_risk_parity_optimize(cov.clone()).unwrap();

        assert_eq!(w.len(), 4);
        let sum: f64 = w.iter().sum();
        assert!((sum - 1.0).abs() < 1e-6, "Weights must sum to 1");

        let rc = risk_contributions(&w, &cov);
        let mean_rc: f64 = rc.iter().sum::<f64>() / rc.len() as f64;
        for (i, &rci) in rc.iter().enumerate() {
            let rel_error = (rci - mean_rc).abs() / mean_rc;
            assert!(
                rel_error < 0.01,
                "RC[{}]={:.8} deviates from mean={:.8} by {:.4}%",
                i,
                rci,
                mean_rc,
                rel_error * 100.0
            );
        }
    }
}
