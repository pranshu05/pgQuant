# Portfolio Optimization

This module provides tools for mean-variance portfolio optimization, allowing you to construct optimal weights based on expected returns and covariance matrices.

### `pgquant_mv_optimize_unconstrained(returns FLOAT[], cov_matrix FLOAT[], risk_free_rate FLOAT, risk_aversion FLOAT)`
Calculates the analytical (closed-form) optimal weights for the risky assets in a mean-variance framework without non-negativity (long-only) constraints.

The solution is computed via:
$$ w = \frac{1}{\lambda} \Sigma^{-1} (\mu - r_f \mathbf{1}) $$
Where $\lambda$ is risk aversion, $\Sigma$ is the covariance matrix, $\mu$ is the expected returns vector, and $r_f$ is the risk-free rate.

- **Arguments:**
  - `returns`: A 1D array of expected returns for the $N$ assets.
  - `cov_matrix`: A flattened 1D array of the $N \times N$ covariance matrix (row-major).
  - `risk_free_rate`: The baseline risk-free rate ($r_f$).
  - `risk_aversion`: The investor's risk aversion parameter ($\lambda > 0$).
- **Returns:** A 1D array (`DOUBLE PRECISION[]`) containing the optimal weights for the risky assets.
- **Note:** Because this is an *unconstrained* optimization, the sum of the returned weights is not guaranteed to be $1.0$, and the weights can be negative (implying short selling). The residual weight $1 - \sum w_i$ is implicitly allocated to the risk-free asset.
