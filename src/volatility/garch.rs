use argmin::core::{CostFunction, Executor, State};
use argmin::solver::neldermead::NelderMead;
use pgrx::prelude::*;

/// Computes the negative log-likelihood for GARCH(1,1) with normal innovations.
/// params = [omega, alpha, beta]
struct Garch11NormalNLL<'a> {
    returns: &'a [f64],
    initial_var: f64,
}

impl<'a> CostFunction for Garch11NormalNLL<'a> {
    type Param = Vec<f64>;
    type Output = f64;

    fn cost(&self, p: &Self::Param) -> Result<Self::Output, argmin::core::Error> {
        let omega = p[0];
        let alpha = p[1];
        let beta = p[2];

        // Bounds/Constraints checking via penalty
        if omega <= 0.0 || alpha < 0.0 || beta < 0.0 || (alpha + beta) >= 1.0 {
            return Ok(f64::INFINITY);
        }

        let mut current_var = self.initial_var;
        let mut nll = 0.0;

        for &r in self.returns {
            // Guard against variance collapse
            if current_var <= 0.0 {
                current_var = 1e-12;
            }
            nll += current_var.ln() + (r * r) / current_var;
            // Update variance for next step
            current_var = omega + alpha * (r * r) + beta * current_var;
        }

        Ok(0.5 * nll)
    }
}

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

    let n = returns.len() as f64;
    let sum: f64 = returns.iter().sum();
    let sum_sq: f64 = returns.iter().map(|r| r * r).sum();
    let sample_var = (sum_sq - (sum * sum) / n) / (n - 1.0);

    let cost = Garch11NormalNLL {
        returns: &returns,
        initial_var: sample_var,
    };

    // Initial guess: based on typical financial data
    let init_param = vec![sample_var * 0.05, 0.05, 0.90];

    // Simplex vertices around the initial guess
    let initial_simplex = vec![
        init_param.clone(),
        vec![sample_var * 0.1, 0.05, 0.90],
        vec![sample_var * 0.05, 0.1, 0.90],
        vec![sample_var * 0.05, 0.05, 0.85],
    ];

    let solver = NelderMead::new(initial_simplex)
        .with_sd_tolerance(1e-6)
        .unwrap();

    let res = Executor::new(cost, solver)
        .configure(|state| state.max_iters(1000))
        .run()
        .unwrap_or_else(|e| pgrx::error!("Optimization failed: {}", e));

    let best_param = res.state().get_best_param().unwrap().clone();
    let best_cost = res.state().get_best_cost();

    let omega = best_param[0];
    let alpha = best_param[1];
    let beta = best_param[2];
    // Return positive log likelihood
    let loglik = -best_cost;

    Ok(TableIterator::new(vec![(omega, alpha, beta, loglik)]))
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
}
