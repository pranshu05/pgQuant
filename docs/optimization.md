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

Calibrates the Black-Litterman view uncertainty matrix ($\Omega$) from percentage confidence scores using the closed-form proportional variance method (He & Litterman, 1999; Idzorek, 2005):

$$\Omega_{ii} = \max\left( \tau \frac{1 - c_i}{c_i} (P \Sigma P^T)_{ii}, \, \epsilon \right), \quad \epsilon = 10^{-8}$$

**Boundary Handling & Numerical Stability:**
- **$c_i = 1.0$ (100% confidence):** $\Omega_{ii}$ is bounded below by $\epsilon = 10^{-8}$. This prevents the covariance matrix from becoming singular and ensures Black-Litterman does not discard the view via pseudo-inverse zeroing.
- **$c_i = 0.0$ (0% confidence):** $\Omega_{ii} = 10^6$ (uninformative prior, minimal weight on view).
- **$c_i \in (0.0, 1.0)$:** $\Omega_{ii}$ scales inversely with confidence.
- `market_weights` and `risk_aversion` are accepted for API compatibility with the full numerical implied tilt calibration framework.

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

Solves the strictly convex Equal Risk Contribution (ERC) formulation:

$$\min_{x > 0} \left( \frac{1}{2} x^T \Sigma x - \sum_{i=1}^N \frac{1}{N} \ln x_i \right)$$

Once optimal $x^*$ is found, the normalized portfolio weights are $w_i = x_i^* / \sum_{j=1}^N x_j^*$.

- **Arguments:**
  - `cov_matrix`: A flattened 1D array of the $N \times N$ covariance matrix (row-major).
- **Returns:** A 1D array (`DOUBLE PRECISION[]`) containing the risk parity weights.

**How it works:** Internally, the optimizer uses **Cyclical Coordinate Descent (CCD)** (Griveau-Billion et al., 2013). Each asset's coordinate update has a closed-form quadratic root:
$$x_i = \frac{-\sum_{j \ne i} \Sigma_{ij} x_j + \sqrt{\left(\sum_{j \ne i} \Sigma_{ij} x_j\right)^2 + 4 \Sigma_{ii} / N}}{2 \Sigma_{ii}}$$
This guarantees global convergence in 10–20 iterations without numerical stagnation, scaling efficiently to $N = 100+$ assets.

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


## Portfolio Risk Decomposition & Attribution

After constructing any portfolio (mean-variance, risk parity, or custom), this function decomposes total portfolio risk into per-asset contributions.

### `pgquant_risk_decomposition(weights FLOAT[], cov_matrix FLOAT[])`

For a portfolio with weights $w$ and covariance matrix $\Sigma$:

| Metric | Formula | Interpretation |
|--------|---------|----------------|
| **Portfolio Volatility** | $\sigma_p = \sqrt{w^T \Sigma w}$ | Total portfolio risk |
| **Marginal Risk Contribution (MRC)** | $\text{MRC}_i = \frac{(\Sigma w)_i}{\sigma_p}$ | How much an infinitesimal increase in $w_i$ increases $\sigma_p$ |
| **Component Risk Contribution (CRC)** | $\text{CRC}_i = w_i \times \text{MRC}_i$ | Absolute contribution of asset $i$ to $\sigma_p$ |
| **Percentage Contribution (PCT)** | $\text{PCT}_i = \frac{\text{CRC}_i}{\sigma_p}$ | Fraction of total risk attributable to asset $i$ |

**Euler Identity:** By Euler's theorem for homogeneous functions, $\sum_i \text{CRC}_i = \sigma_p$ exactly.

- **Arguments:**
  - `weights`: A 1D array of portfolio weights (length $N$, must sum to 1.0).
  - `cov_matrix`: A flattened 1D array of the $N \times N$ covariance matrix (row-major).
- **Returns:** A set of rows with columns `(asset_index, weight, marginal_risk_contribution, component_risk_contribution, pct_risk_contribution, portfolio_volatility)`.

```sql
-- Basic 2-asset decomposition
SELECT * FROM pgquant_risk_decomposition(
    weights => ARRAY[0.6, 0.4],
    cov_matrix => ARRAY[0.04, 0.015, 0.015, 0.09]
);

-- Verify Euler identity
SELECT
    SUM(component_risk_contribution) AS sum_crc,
    MAX(portfolio_volatility) AS port_vol
FROM pgquant_risk_decomposition(
    ARRAY[0.6, 0.4],
    ARRAY[0.04, 0.015, 0.015, 0.09]
);
-- sum_crc ≈ port_vol (exact equality to machine precision)

-- Verify risk parity produces equal contributions
WITH rp AS (
    SELECT pgquant_risk_parity_optimize(
        ARRAY[0.04, 0.006, 0.002, 0.006, 0.09, 0.009, 0.002, 0.009, 0.01]
    ) AS weights
)
SELECT * FROM pgquant_risk_decomposition(
    (SELECT weights FROM rp),
    ARRAY[0.04, 0.006, 0.002, 0.006, 0.09, 0.009, 0.002, 0.009, 0.01]
);
-- All pct_risk_contribution values should be ≈ 0.333 (1/3)

-- Find concentrated risk positions
SELECT asset_index, pct_risk_contribution
FROM pgquant_risk_decomposition(
    ARRAY[0.25, 0.25, 0.25, 0.25],
    ARRAY[0.04, 0.01, 0.005, 0.002,
          0.01, 0.09, 0.008, 0.003,
          0.005, 0.008, 0.01, 0.002,
          0.002, 0.003, 0.002, 0.0064]
)
WHERE pct_risk_contribution > 0.30
ORDER BY pct_risk_contribution DESC;
```
