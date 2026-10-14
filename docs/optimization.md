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

## Black-Litterman Model

The Black-Litterman model combines market equilibrium returns with subjective investor views to produce stable expected returns, resolving the sensitivity issues of standard mean-variance optimization.

### `pgquant_black_litterman`

Calculates the Black-Litterman posterior expected returns and covariance matrix.

```sql
SELECT * FROM pgquant_black_litterman(
    market_weights => ARRAY[0.5, 0.5],
    cov_matrix => ARRAY[0.04, 0.01, 0.01, 0.05],
    risk_aversion => 2.0,
    tau => 0.05,
    p_matrix => ARRAY[1.0, -1.0], -- 1x2 view matrix
    q_vector => ARRAY[0.05], -- 1x1 view returns
    omega_matrix => ARRAY[0.01] -- 1x1 uncertainty matrix
);
```
**Returns**: `(expected_returns double precision[], posterior_covariance double precision[])`

### `pgquant_bl_idzorek_omega`

Using Idzorek's method, automatically calibrates the uncertainty matrix ($\Omega$) given simple percentage confidence scores (e.g. 0.5 for 50% confident).

```sql
SELECT pgquant_bl_idzorek_omega(
    market_weights => ARRAY[0.5, 0.5],
    cov_matrix => ARRAY[0.04, 0.01, 0.01, 0.05],
    risk_aversion => 2.0,
    tau => 0.05,
    p_matrix => ARRAY[1.0, -1.0],
    view_confidences => ARRAY[0.5] -- 50% confidence in the view
);
```
**Returns**: `double precision[]` (A flattened KxK diagonal $\Omega$ matrix ready to be fed into `pgquant_black_litterman`).

## Risk Parity (Equal Risk Contribution)

Risk parity allocates capital such that the marginal risk contribution of each asset is equal, producing portfolios that are truly diversified by *risk* rather than by capital allocation.

### `pgquant_risk_parity_optimize(cov_matrix FLOAT[])`

Solves the nonlinear risk parity optimization problem:

$$\min_w \sum_{i=1}^{N} \left( w_i (\Sigma w)_i - \frac{w^T \Sigma w}{N} \right)^2$$

subject to $w_i \geq 0$ and $\sum_i w_i = 1$.

- **Arguments:**
  - `cov_matrix`: A flattened 1D array of the $N \times N$ covariance matrix (row-major).
- **Returns:** A 1D array (`DOUBLE PRECISION[]`) containing the risk parity weights.

**How it works:** Internally, the optimizer uses a softmax reparameterization to convert the constrained problem into an unconstrained one. Portfolio weights are expressed as $w_i = \exp(y_i) / \sum_j \exp(y_j)$, which guarantees positivity and sum-to-one. Nelder-Mead then minimizes the sum of squared deviations of risk contributions from the target (equal) budget.

```sql
-- 3-asset risk parity portfolio
SELECT pgquant_risk_parity_optimize(
    ARRAY[
        0.04, 0.006, 0.002,
        0.006, 0.09, 0.009,
        0.002, 0.009, 0.01
    ]
);

-- End-to-end: compute covariance from returns, then find risk parity weights
WITH cov AS (
    SELECT pgquant_sample_cov(
        'SELECT symbol, date, return FROM daily_returns ORDER BY symbol, date',
        ARRAY['AAPL', 'MSFT', 'GOOG']
    ) AS matrix
)
SELECT pgquant_risk_parity_optimize(matrix) FROM cov;
```

**Key properties:**
- Higher-variance assets receive lower weights
- Each asset contributes equally to total portfolio risk
- For uncorrelated assets, weights are proportional to $1/\sigma_i$
- The optimizer initializes from the inverse-volatility heuristic and converges to machine precision

