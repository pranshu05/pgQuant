use crate::mle::optimize_mle_nd;
use pgrx::prelude::*;

/// Estimates GARCH(1,1) parameters (omega, alpha, beta) and the final log-likelihood
/// using MLE with normal innovations.
#[pg_extern]
pub fn pgquant_garch11_normal(
    returns: Vec<f64>,
) -> Result<
    TableIterator<
        'static,
        (
            name!(omega, f64),
            name!(alpha, f64),
            name!(beta, f64),
            name!(loglik, f64),
        ),
    >,
    pgrx::spi::Error,
> {
    if returns.len() < 10 {
        pgrx::error!("Need at least 10 observations for GARCH(1,1) estimation");
    }

    let mut mean = 0.0;
    let mut m2 = 0.0;
    for (i, &r) in returns.iter().enumerate() {
        let n_f = i as f64 + 1.0;
        let delta = r - mean;
        mean += delta / n_f;
        m2 += delta * (r - mean);
    }
    let sample_var = m2 / (returns.len() as f64 - 1.0);

    let init_state = |p: &[f64]| {
        let omega = p[0];
        let alpha = p[1];
        let beta = p[2];
        if omega <= 0.0 || alpha < 0.0 || beta < 0.0 || (alpha + beta) >= 1.0 {
            None
        } else {
            Some(sample_var)
        }
    };

    let ll_obs = |current_var: &mut f64, r: f64, p: &[f64]| {
        let omega = p[0];
        let alpha = p[1];
        let beta = p[2];

        if *current_var <= 0.0 {
            *current_var = 1e-12;
        }
        let ll = -0.5 * (current_var.ln() + (r * r) / *current_var);
        *current_var = omega + alpha * (r * r) + beta * *current_var;
        ll
    };

    let init_param = vec![sample_var * 0.05, 0.05, 0.90];
    let initial_simplex = vec![
        init_param.clone(),
        vec![sample_var * 0.1, 0.05, 0.90],
        vec![sample_var * 0.05, 0.1, 0.90],
        vec![sample_var * 0.05, 0.05, 0.85],
    ];

    let result = optimize_mle_nd(&returns, ll_obs, init_state, initial_simplex, 1000, 1e-6)
        .unwrap_or_else(|e| pgrx::error!("Optimization failed: {}", e));

    let best_param = result.0;
    let loglik = result.1;

    let omega = best_param[0];
    let alpha = best_param[1];
    let beta = best_param[2];

    Ok(TableIterator::new(vec![(omega, alpha, beta, loglik)]))
}

/// Estimates GARCH(1,1) parameters (omega, alpha, beta, nu) and the final log-likelihood
/// using MLE with Student-t innovations.
#[pg_extern]
pub fn pgquant_garch11_t(
    returns: Vec<f64>,
) -> Result<
    TableIterator<
        'static,
        (
            name!(omega, f64),
            name!(alpha, f64),
            name!(beta, f64),
            name!(nu, f64),
            name!(loglik, f64),
        ),
    >,
    pgrx::spi::Error,
> {
    if returns.len() < 10 {
        pgrx::error!("Need at least 10 observations for GARCH(1,1) estimation");
    }

    let mut mean = 0.0;
    let mut m2 = 0.0;
    for (i, &r) in returns.iter().enumerate() {
        let n_f = i as f64 + 1.0;
        let delta = r - mean;
        mean += delta / n_f;
        m2 += delta * (r - mean);
    }
    let sample_var = m2 / (returns.len() as f64 - 1.0);

    let init_state = |p: &[f64]| {
        let omega = p[0];
        let alpha = p[1];
        let beta = p[2];
        let nu = p[3];
        if omega <= 0.0 || alpha < 0.0 || beta < 0.0 || (alpha + beta) >= 1.0 || nu <= 2.01 {
            None
        } else {
            // Also return the precomputed constant term for the state so we don't recalculate it
            let half_nu = nu * 0.5;
            let log_gamma_diff = statrs::function::gamma::ln_gamma(half_nu + 0.5)
                - statrs::function::gamma::ln_gamma(half_nu);
            let const_term = log_gamma_diff - 0.5 * (std::f64::consts::PI * (nu - 2.0)).ln();
            Some((sample_var, const_term))
        }
    };

    let ll_obs = |state: &mut (f64, f64), r: f64, p: &[f64]| {
        let omega = p[0];
        let alpha = p[1];
        let beta = p[2];
        let nu = p[3];

        let current_var = &mut state.0;
        let const_term = state.1;

        if *current_var <= 0.0 {
            *current_var = 1e-12;
        }

        let half_nu = nu * 0.5;
        let term1 = -0.5 * current_var.ln();
        let term2 = -(half_nu + 0.5) * (1.0 + (r * r) / (*current_var * (nu - 2.0))).ln();

        let ll_t = const_term + term1 + term2;

        *current_var = omega + alpha * (r * r) + beta * *current_var;
        ll_t
    };

    let init_param = vec![sample_var * 0.05, 0.05, 0.90, 5.0];
    let initial_simplex = vec![
        init_param.clone(),
        vec![sample_var * 0.1, 0.05, 0.90, 5.0],
        vec![sample_var * 0.05, 0.1, 0.90, 5.0],
        vec![sample_var * 0.05, 0.05, 0.85, 5.0],
        vec![sample_var * 0.05, 0.05, 0.90, 8.0],
    ];

    let result = optimize_mle_nd(&returns, ll_obs, init_state, initial_simplex, 2000, 1e-6)
        .unwrap_or_else(|e| pgrx::error!("Optimization failed: {}", e));

    let best_param = result.0;
    let loglik = result.1;

    let omega = best_param[0];
    let alpha = best_param[1];
    let beta = best_param[2];
    let nu = best_param[3];

    Ok(TableIterator::new(vec![(omega, alpha, beta, nu, loglik)]))
}
#[cfg(any(test, feature = "pg_test"))]
#[pg_schema]
mod tests {
    use super::*;

    #[pg_test]
    fn test_pgquant_garch11_normal() {
        // Generate synthetic GARCH(1,1) returns
        let true_omega = 0.00001;
        let true_alpha = 0.1;
        let true_beta = 0.8;

        let mut seed: u64 = 54321;
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

        let mut returns = Vec::with_capacity(1000);
        let mut var: f64 = true_omega / (1.0 - true_alpha - true_beta);
        for _ in 0..1000 {
            let z = next_normal();
            let r = var.sqrt() * z;
            returns.push(r);
            var = true_omega + true_alpha * (r * r) + true_beta * var;
        }

        let result = pgquant_garch11_normal(returns).unwrap();
        let res_vec: Vec<_> = result.collect();
        assert_eq!(res_vec.len(), 1);

        let (omega, alpha, beta, _loglik) = res_vec[0];

        assert!(omega > 0.0);
        assert!(alpha >= 0.0);
        assert!(beta >= 0.0);
        assert!(alpha + beta < 1.0);
        // Shouldn't assert too strictly on exactly matching true parameters for Nelder-Mead with 1000 samples,
        // but it should successfully converge without error.
    }

    #[pg_test]
    fn test_pgquant_garch11_t() {
        // Generate synthetic GARCH(1,1)-t returns
        let true_omega = 0.00001;
        let true_alpha = 0.1;
        let true_beta = 0.8;
        let true_nu = 6.0;

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

        let mut next_t = || -> f64 {
            let z = next_normal();
            let mut v = 0.0;
            for _ in 0..(true_nu as usize) {
                let n = next_normal();
                v += n * n;
            }
            z / (v / true_nu).sqrt()
        };

        let mut returns = Vec::with_capacity(1000);
        let mut var: f64 = true_omega / (1.0 - true_alpha - true_beta);
        for _ in 0..1000 {
            // Standardized t has variance nu/(nu-2). To make variance exactly 1, we scale it.
            let z = next_t() * ((true_nu - 2.0) / true_nu).sqrt();
            let r = var.sqrt() * z;
            returns.push(r);
            var = true_omega + true_alpha * (r * r) + true_beta * var;
        }

        let result = super::pgquant_garch11_t(returns).unwrap();
        let res_vec: Vec<_> = result.collect();
        assert_eq!(res_vec.len(), 1);

        let (omega, alpha, beta, nu, _loglik) = res_vec[0];

        assert!(omega > 0.0);
        assert!(alpha >= 0.0);
        assert!(beta >= 0.0);
        assert!(alpha + beta < 1.0);
        assert!(nu > 2.0);
    }
}
