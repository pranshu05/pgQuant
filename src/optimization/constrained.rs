use clarabel::algebra::*;
use clarabel::solver::*;
use pgrx::prelude::*;

/// Calculates the long-only constrained mean-variance optimal portfolio weights.
///
/// Formulation:
/// Minimize 1/2 w^T \Sigma w - 1/\lambda (\mu - r_f 1)^T w
/// Subject to:
///     w^T 1 = 1   (sum of weights is 1)
///     w >= 0      (no short selling)
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
pub fn pgquant_mv_optimize_constrained(
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

    // Build upper triangular P matrix in CSC format
    let mut p_colptr = vec![0; n + 1];
    let mut p_rowval = Vec::new();
    let mut p_nzval = Vec::new();

    let mut nnz = 0;
    for j in 0..n {
        p_colptr[j] = nnz;
        for i in 0..=j {
            let val = cov_matrix[i * n + j];
            if val.abs() > 1e-12 {
                p_rowval.push(i);
                p_nzval.push(val);
                nnz += 1;
            }
        }
    }
    p_colptr[n] = nnz;
    let p_mat = CscMatrix::new(n, n, p_colptr, p_rowval, p_nzval);

    // Build q vector: - (mu - r_f) / lambda
    let mut q = vec![0.0; n];
    for i in 0..n {
        q[i] = -(returns[i] - risk_free_rate) / risk_aversion;
    }

    // Build constraint matrix A
    // Row 0: sum(w) = 1
    // Rows 1..=N: -w <= 0 (i.e. w >= 0)
    let mut a_colptr = vec![0; n + 1];
    let mut a_rowval = Vec::new();
    let mut a_nzval = Vec::new();

    let mut a_nnz = 0;
    for j in 0..n {
        a_colptr[j] = a_nnz;

        // Sum to 1 constraint
        a_rowval.push(0);
        a_nzval.push(1.0);
        a_nnz += 1;

        // Non-negativity constraint
        a_rowval.push(j + 1);
        a_nzval.push(-1.0);
        a_nnz += 1;
    }
    a_colptr[n] = a_nnz;
    let a_mat = CscMatrix::new(1 + n, n, a_colptr, a_rowval, a_nzval);

    // b vector
    let mut b = vec![0.0; n + 1];
    b[0] = 1.0;

    // Cones definition
    let cones = vec![
        ZeroConeT(1),        // For sum(w) = 1
        NonnegativeConeT(n), // For w >= 0
    ];

    let mut settings = DefaultSettings::default();
    settings.verbose = false;

    let mut solver = DefaultSolver::new(&p_mat, &q, &a_mat, &b, &cones, settings)
        .unwrap_or_else(|_| pgrx::error!("Failed to initialize Clarabel QP solver."));

    solver.solve();

    if solver.solution.status == SolverStatus::Solved
        || solver.solution.status == SolverStatus::AlmostSolved
    {
        Ok(solver.solution.x.clone())
    } else {
        pgrx::error!("QP solver failed to converge: {:?}", solver.solution.status);
    }
}

#[cfg(any(test, feature = "pg_test"))]
#[pg_schema]
mod tests {
    use super::*;

    #[pg_test]
    fn test_pgquant_mv_optimize_constrained() {
        let returns = vec![0.10, 0.12];
        let cov = vec![0.04, 0.01, 0.01, 0.05];
        let rf = 0.02;
        let lambda = 2.0;

        let w = pgquant_mv_optimize_constrained(returns, cov, rf, lambda).unwrap();

        assert!((w[0] - 0.4285714).abs() < 1e-4);
        assert!((w[1] - 0.5714285).abs() < 1e-4);
    }
}
