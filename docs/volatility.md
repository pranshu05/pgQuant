# Volatility Models

`pgQuant` brings standard quantitative volatility tracking functions natively into SQL.

## Rolling Volatility

### `pgquant_rolling_vol(query TEXT, window INTEGER DEFAULT 50)`
Computes the rolling sample standard deviation of a return series. It utilizes an incremental algorithm over a specified rolling window.
- **Arguments:**
  - `query`: A SQL query returning `(symbol TEXT, date DATE, return_value DOUBLE PRECISION)`. This uses the `pgQuant` data access convention.
  - `window`: The lookback window size (in periods/days). Default is 50.
- **Returns:** `SETOF (symbol TEXT, date DATE, vol DOUBLE PRECISION)`
- **Note:** Incomplete windows at the start of a symbol's time series will not emit rows (similar to Pandas' default `NaN` handling for rolling standard deviation).
- **Example:**
  ```sql
  SELECT * FROM pgquant_rolling_vol(
      'SELECT symbol, date, simple_return FROM pgquant_simple_returns(''SELECT symbol, date, price FROM my_prices'')',
      50
  );
  ```
## Exponentially Weighted Moving Average (EWMA) Volatility

### `pgquant_ewma_vol(returns DOUBLE PRECISION[], lambda DOUBLE PRECISION)`
Computes the EWMA volatility series for an array of returns. It returns an array of volatilities of the same length, where each entry represents the volatility estimate at that point in time. 
- **Arguments:**
  - `returns`: Array of daily returns.
  - `lambda`: The decay factor (e.g., `0.94` as popularized by RiskMetrics).
- **Returns:** `DOUBLE PRECISION[]`
- **Note:** The internal variance series is initialized to the sample variance of the entire array to prevent a cold-start bias.
- **Example:**
  ```sql
  WITH rets AS (SELECT simple_return FROM pgquant_simple_returns('...'))
  SELECT pgquant_ewma_vol(array_agg(simple_return), 0.94) AS ewma_vols FROM rets;
  ```

## EWMA Lambda Estimation (MLE)

### `pgquant_ewma_lambda_mle(returns DOUBLE PRECISION[])`
Estimates the optimal EWMA decay factor (lambda) via Maximum Likelihood Estimation. Under a Gaussian assumption ($r_t \sim N(0, \sigma_t^2)$), finds the lambda in $(0, 1)$ that minimises the negative log-likelihood of the observed return series.
- **Arguments:**
  - `returns`: Array of at least 3 daily returns.
- **Returns:** `DOUBLE PRECISION` — the estimated lambda.
- **Method:** Brent's method (bounded 1-D minimisation on $[0.01, 0.9999]$).
- **Example:**
  ```sql
  WITH rets AS (SELECT simple_return FROM pgquant_simple_returns('...'))
  SELECT pgquant_ewma_lambda_mle(array_agg(simple_return)) AS optimal_lambda FROM rets;
  ```
- **Typical usage:** Feed the estimated lambda back into `pgquant_ewma_vol` for an optimally-calibrated volatility series:
  ```sql
  WITH rets AS (SELECT simple_return FROM pgquant_simple_returns('...')),
       lam AS (SELECT pgquant_ewma_lambda_mle(array_agg(simple_return)) AS l FROM rets)
  SELECT pgquant_ewma_vol(array_agg(rets.simple_return), lam.l) FROM rets, lam;
  ```

## GARCH(1,1) Parameter Estimation

### `pgquant_garch11_normal(returns DOUBLE PRECISION[])`
Estimates the GARCH(1,1) model parameters ($\omega$, $\alpha$, $\beta$) using Maximum Likelihood Estimation (MLE) assuming normally distributed innovations.
- **Arguments:**
  - `returns`: Array of daily returns (requires at least 10 observations).
- **Returns:** A single row containing:
  - `omega`: The constant term in the variance equation.
  - `alpha`: The ARCH term (reaction to past squared returns).
  - `beta`: The GARCH term (persistence of past variance).
  - `loglik`: The final maximized log-likelihood value.
- **Example:**
  ```sql
  WITH rets AS (SELECT simple_return FROM pgquant_simple_returns('...'))
  SELECT * FROM pgquant_garch11_normal(array_agg(simple_return)) FROM rets;
  ```

### `pgquant_garch11_t(returns DOUBLE PRECISION[])`
Estimates the GARCH(1,1) model parameters ($\omega$, $\alpha$, $\beta$) along with the degrees of freedom ($\nu$) using Maximum Likelihood Estimation (MLE) assuming Student-t distributed innovations. This model accounts for fatter tails in financial returns compared to the normal distribution.
- **Arguments:**
  - `returns`: Array of daily returns (requires at least 10 observations).
- **Returns:** A single row containing:
  - `omega`: The constant term in the variance equation.
  - `alpha`: The ARCH term (reaction to past squared returns).
  - `beta`: The GARCH term (persistence of past variance).
  - `nu`: The estimated degrees of freedom of the Student-t distribution (captures tail thickness).
  - `loglik`: The final maximized log-likelihood value.
- **Example:**
  ```sql
  WITH rets AS (SELECT log_return FROM pgquant_log_returns('...'))
  SELECT * FROM pgquant_garch11_t(array_agg(log_return)) FROM rets;
  ```
