use argmin::core::{CostFunction, Executor, State};
use argmin::solver::brent::BrentOpt;
use argmin::solver::neldermead::NelderMead;

/// 1D MLE using Brent's method
struct Mle1D<'a, F, S, I> {
    data: &'a [f64],
    ll_obs: F,
    init_state: I,
    _marker: std::marker::PhantomData<S>,
}

impl<'a, F, S, I> CostFunction for Mle1D<'a, F, S, I>
where
    F: Fn(&mut S, f64, f64) -> f64,
    I: Fn(f64) -> Option<S>,
{
    type Param = f64;
    type Output = f64;

    fn cost(&self, p: &Self::Param) -> Result<Self::Output, argmin::core::Error> {
        let mut state = match (self.init_state)(*p) {
            Some(s) => s,
            None => return Ok(f64::INFINITY),
        };

        let mut nll = 0.0;
        for &obs in self.data {
            let ll = (self.ll_obs)(&mut state, obs, *p);
            nll -= ll;
        }
        Ok(nll)
    }
}

pub fn optimize_mle_1d<F, S, I>(
    data: &[f64],
    ll_obs: F,
    init_state: I,
    min_param: f64,
    max_param: f64,
    max_iters: u64,
) -> Result<(f64, f64), argmin::core::Error>
where
    F: Fn(&mut S, f64, f64) -> f64,
    I: Fn(f64) -> Option<S>,
{
    let cost = Mle1D {
        data,
        ll_obs,
        init_state,
        _marker: std::marker::PhantomData,
    };

    let solver = BrentOpt::new(min_param, max_param);

    let res = Executor::new(cost, solver)
        .configure(|state| state.max_iters(max_iters))
        .run()?;

    let best_param = res.state().get_best_param().unwrap();
    let best_cost = res.state().get_best_cost();

    Ok((*best_param, -best_cost))
}

/// ND MLE using Nelder-Mead
struct MleND<'a, F, S, I> {
    data: &'a [f64],
    ll_obs: F,
    init_state: I,
    _marker: std::marker::PhantomData<S>,
}

impl<'a, F, S, I> CostFunction for MleND<'a, F, S, I>
where
    F: Fn(&mut S, f64, &[f64]) -> f64,
    I: Fn(&[f64]) -> Option<S>,
{
    type Param = Vec<f64>;
    type Output = f64;

    fn cost(&self, p: &Self::Param) -> Result<Self::Output, argmin::core::Error> {
        let mut state = match (self.init_state)(p) {
            Some(s) => s,
            None => return Ok(f64::INFINITY),
        };

        let mut nll = 0.0;
        for &obs in self.data {
            let ll = (self.ll_obs)(&mut state, obs, p);
            nll -= ll;
        }
        Ok(nll)
    }
}

pub fn optimize_mle_nd<F, S, I>(
    data: &[f64],
    ll_obs: F,
    init_state: I,
    initial_simplex: Vec<Vec<f64>>,
    max_iters: u64,
    sd_tolerance: f64,
) -> Result<(Vec<f64>, f64), argmin::core::Error>
where
    F: Fn(&mut S, f64, &[f64]) -> f64,
    I: Fn(&[f64]) -> Option<S>,
{
    let cost = MleND {
        data,
        ll_obs,
        init_state,
        _marker: std::marker::PhantomData,
    };

    let solver = NelderMead::new(initial_simplex).with_sd_tolerance(sd_tolerance)?;

    let res = Executor::new(cost, solver)
        .configure(|state| state.max_iters(max_iters))
        .run()?;

    let best_param = res.state().get_best_param().unwrap().clone();
    let best_cost = res.state().get_best_cost();

    Ok((best_param.clone(), -best_cost))
}
