use pgrx::prelude::*;

/// Computes the Exponentially Weighted Moving Average (EWMA) volatility series.
/// Given an array of returns and a decay factor lambda (e.g., 0.94 for RiskMetrics).
/// Returns an array of volatilities of the same length.
#[pg_extern]
pub fn pgquant_ewma_vol(returns: Vec<f64>, lambda: f64) -> Vec<f64> {
    if returns.is_empty() {
        return Vec::new();
    }

    if lambda <= 0.0 || lambda >= 1.0 {
        pgrx::error!("Lambda must be between 0 and 1 (exclusive).");
    }

    // Initialize with sample variance of the provided series
    // If n=1, use return^2
    let initial_var = if returns.len() > 1 {
        let n = returns.len() as f64;
        let sum: f64 = returns.iter().sum();
        let sum_sq: f64 = returns.iter().map(|r| r * r).sum();
        let var = (sum_sq - (sum * sum) / n) / (n - 1.0);
        var
    } else {
        returns[0] * returns[0]
    };

    let mut vol_series = Vec::with_capacity(returns.len());
    let mut current_var = initial_var;

    for r in returns {
        current_var = lambda * current_var + (1.0 - lambda) * (r * r);
        vol_series.push(current_var.sqrt());
    }

    vol_series
}

/// Computes the EWMA negative log-likelihood for a given lambda.
/// Uses Gaussian assumption: r_t ~ N(0, sigma_t^2)
/// where sigma_t^2 is the PRIOR forecast before observing r_t,
/// updated as: sigma_{t+1}^2 = lambda * sigma_t^2 + (1-lambda) * r_t^2
/// NLL = 0.5 * sum_t [ ln(sigma_t^2) + r_t^2 / sigma_t^2 ]
/// (constant ln(2*pi) terms dropped since they don't affect the argmin)
fn ewma_nll(returns: &[f64], lambda: f64) -> f64 {
    if returns.len() < 2 {
        return f64::INFINITY;
    }

    // Initialize variance as sample variance of all returns
    let n = returns.len() as f64;
    let sum: f64 = returns.iter().sum();
    let sum_sq: f64 = returns.iter().map(|r| r * r).sum();
    let initial_var = (sum_sq - (sum * sum) / n) / (n - 1.0);

    let mut current_var = initial_var;
    let mut nll = 0.0;

    for &r in returns {
        // Guard against zero/negative variance (numerical edge case)
        if current_var <= 0.0 {
            current_var = 1e-12;
        }
        // Evaluate likelihood using the PRIOR forecast (before seeing r_t)
        nll += current_var.ln() + (r * r) / current_var;
        // Now update variance for the next period
        current_var = lambda * current_var + (1.0 - lambda) * (r * r);
    }

    0.5 * nll
}

/// Estimates the optimal EWMA lambda via MLE using Brent's method
/// (golden-section bounded scalar optimization on [0.01, 0.9999]).
/// Returns the lambda that maximizes the Gaussian log-likelihood
/// of the EWMA variance model given observed returns.
#[pg_extern]
pub fn pgquant_ewma_lambda_mle(returns: Vec<f64>) -> f64 {
    if returns.len() < 3 {
        pgrx::error!("Need at least 3 observations for lambda estimation");
    }

    // Brent's method on the interval [lo, hi] — a simple bounded
    // 1-D scalar minimiser. We use the textbook implementation
    // (golden-section + parabolic interpolation) to avoid pulling
    // in an additional argmin solver dependency for a scalar problem.
    let lo = 0.01_f64;
    let hi = 0.9999_f64;

    let result = brent_min(|lam| ewma_nll(&returns, lam), lo, hi, 1e-8, 200);
    result
}

/// Brent's method for finding the minimum of a unimodal function f on [a, b].
/// `tol` is the desired precision, `max_iter` caps iterations.
/// Returns the x value that minimises f.
fn brent_min<F: Fn(f64) -> f64>(f: F, mut a: f64, mut b: f64, tol: f64, max_iter: usize) -> f64 {
    let golden: f64 = 0.381966011250105; // (3 - sqrt(5)) / 2

    let mut x = a + golden * (b - a);
    let mut w = x;
    let mut v = x;
    let mut fx = f(x);
    let mut fw = fx;
    let mut fv = fx;
    let mut d = 0.0_f64;
    let mut e = 0.0_f64;

    for _ in 0..max_iter {
        let midpoint = 0.5 * (a + b);
        let tol1 = tol * x.abs() + 1e-10;
        let tol2 = 2.0 * tol1;

        if (x - midpoint).abs() <= tol2 - 0.5 * (b - a) {
            return x;
        }

        // Try parabolic interpolation
        let mut use_golden = true;
        let mut u = 0.0_f64;

        if e.abs() > tol1 {
            // Fit parabola
            let r = (x - w) * (fx - fv);
            let q = (x - v) * (fx - fw);
            let p = (x - v) * q - (x - w) * r;
            let q = 2.0 * (q - r);
            let (p, q) = if q > 0.0 { (-p, q) } else { (p, -q) };

            if p.abs() < (0.5 * q * e).abs() && p > q * (a - x) && p < q * (b - x) {
                // Parabolic step
                let step = p / q;
                u = x + step;
                if (u - a) < tol2 || (b - u) < tol2 {
                    u = if x < midpoint { x + tol1 } else { x - tol1 };
                }
                use_golden = false;
                d = step;
            }
        }

        if use_golden {
            e = if x < midpoint { b - x } else { a - x };
            d = golden * e;
            u = x + d;
        }

        // Ensure u differs from x by at least tol1
        let u = if (u - x).abs() >= tol1 {
            u
        } else if d > 0.0 {
            x + tol1
        } else {
            x - tol1
        };

        let fu = f(u);

        // Update brackets
        if fu <= fx {
            if u < x {
                b = x;
            } else {
                a = x;
            }
            v = w;
            fv = fw;
            w = x;
            fw = fx;
            x = u;
            fx = fu;
        } else {
            if u < x {
                a = u;
            } else {
                b = u;
            }
            if fu <= fw || (w - x).abs() < 1e-15 {
                v = w;
                fv = fw;
                w = u;
                fw = fu;
            } else if fu <= fv || (v - x).abs() < 1e-15 || (v - w).abs() < 1e-15 {
                v = u;
                fv = fu;
            }
        }
    }

    x
}

#[cfg(any(test, feature = "pg_test"))]
#[pg_schema]
mod tests {
    use super::*;

    #[pg_test]
    fn test_ewma_vol() {
        let returns = vec![0.01, -0.02, 0.015];
        let lambda = 0.94;

        let vols = pgquant_ewma_vol(returns.clone(), lambda);

        // Manual calculation
        // sum = 0.005, n = 3, sum_sq = 0.0001 + 0.0004 + 0.000225 = 0.000725
        // initial_var = (0.000725 - 0.000025 / 3) / 2 = (0.000725 - 0.000008333) / 2 = 0.000358333
        // var_1 = 0.94 * 0.000358333 + 0.06 * 0.0001 = 0.000336833 + 0.000006 = 0.000342833
        // vol_1 = sqrt(var_1) ≈ 0.0185157

        assert_eq!(vols.len(), 3);
        assert!((vols[0] - 0.0185157).abs() < 1e-6);
    }

    #[pg_test]
    fn test_ewma_lambda_mle_recovers_known_lambda() {
        // Generate synthetic returns from an EWMA process with known lambda.
        // We use a deterministic pseudo-random sequence (simple LCG) to
        // create realistic-looking returns without depending on rand.
        let true_lambda = 0.94;
        let n = 1000;

        // Simple LCG for deterministic pseudo-random normals (Box-Muller)
        let mut seed: u64 = 12345;
        let mut next_uniform = || -> f64 {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (seed >> 11) as f64 / (1u64 << 53) as f64
        };
        let mut next_normal = || -> f64 {
            let u1 = next_uniform().max(1e-15);
            let u2 = next_uniform();
            (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()
        };

        // Simulate: r_t = sigma_t * z_t, where z_t ~ N(0,1)
        // sigma_t^2 = lambda * sigma_{t-1}^2 + (1-lambda) * r_{t-1}^2
        let mut var: f64 = 0.0002; // initial daily variance (~1.4% vol)
        let mut returns = Vec::with_capacity(n);
        for _ in 0..n {
            let z = next_normal();
            let r = var.sqrt() * z;
            returns.push(r);
            var = true_lambda * var + (1.0 - true_lambda) * r * r;
        }

        let estimated = pgquant_ewma_lambda_mle(returns);

        // With 1000 observations generated from lambda=0.94,
        // the MLE should recover something close (within ±0.05).
        assert!(
            (estimated - true_lambda).abs() < 0.05,
            "Expected lambda near {}, got {}",
            true_lambda,
            estimated
        );
    }

    #[pg_test]
    fn test_ewma_nll_is_minimised_interior() {
        // Verify the NLL function actually has an interior minimum
        // (not at the boundary) for the synthetic data, by checking
        // that the estimated lambda gives a lower NLL than the boundaries.
        let mut seed: u64 = 67890;
        let mut next_uniform = || -> f64 {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (seed >> 11) as f64 / (1u64 << 53) as f64
        };
        let mut next_normal = || -> f64 {
            let u1 = next_uniform().max(1e-15);
            let u2 = next_uniform();
            (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()
        };

        let mut var: f64 = 0.0001;
        let mut returns = Vec::with_capacity(500);
        for _ in 0..500 {
            let z = next_normal();
            let r = var.sqrt() * z;
            returns.push(r);
            var = 0.92 * var + 0.08 * r * r;
        }

        let est = pgquant_ewma_lambda_mle(returns.clone());
        let nll_est = super::ewma_nll(&returns, est);
        let nll_low = super::ewma_nll(&returns, 0.05);
        let nll_high = super::ewma_nll(&returns, 0.999);

        assert!(
            nll_est <= nll_low,
            "NLL at estimated lambda ({} at {}) should be <= NLL at boundary ({} at 0.05)",
            nll_est,
            est,
            nll_low
        );
        assert!(
            nll_est <= nll_high,
            "NLL at estimated lambda ({} at {}) should be <= NLL at boundary ({} at 0.999)",
            nll_est,
            est,
            nll_high
        );
    }
}
