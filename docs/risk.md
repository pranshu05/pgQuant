# Risk Metrics (VaR & ES)

`pgQuant` provides Value at Risk (VaR) and Expected Shortfall (ES, also known as Conditional VaR) estimation natively inside Postgres.

Both historical (non-parametric) and Gaussian (parametric) methods are supported. The functions accept an `alpha` parameter representing the confidence level (e.g. `0.95` for 95% confidence).

## Historical Method

The historical method computes VaR and ES by looking at the empirical distribution of past returns.

### `pgquant_var_historical(returns DOUBLE PRECISION[], alpha DOUBLE PRECISION)`
Computes the historical Value at Risk at the given confidence level `alpha`.
- **Arguments:**
  - `returns`: Array of historical daily returns.
  - `alpha`: Confidence level (e.g., 0.95).
- **Returns:** `DOUBLE PRECISION`
- **Example:**
  ```sql
  WITH rets AS (SELECT simple_return FROM pgquant_simple_returns('...'))
  SELECT pgquant_var_historical(array_agg(simple_return), 0.95) AS var_95 FROM rets;
  ```

### `pgquant_es_historical(returns DOUBLE PRECISION[], alpha DOUBLE PRECISION)`
Computes the historical Expected Shortfall, which is the average of all returns that fall beyond the historical VaR threshold.
- **Returns:** `DOUBLE PRECISION`

## Parametric (Gaussian) Method

The parametric method assumes returns are normally distributed. You must compute the mean (`mu`) and standard deviation (`sigma`) of the returns in SQL beforehand.

### `pgquant_var_gaussian(mu DOUBLE PRECISION, sigma DOUBLE PRECISION, alpha DOUBLE PRECISION)`
Computes the Gaussian Value at Risk using the inverse cumulative distribution function (CDF) of the normal distribution.
- **Arguments:**
  - `mu`: The mean of the return series.
  - `sigma`: The standard deviation of the return series.
  - `alpha`: Confidence level (e.g., 0.95).
- **Returns:** `DOUBLE PRECISION`
- **Example:**
  ```sql
  WITH rets AS (SELECT simple_return FROM pgquant_simple_returns('...')),
       stats AS (SELECT avg(simple_return) as mu, stddev(simple_return) as sigma FROM rets)
  SELECT pgquant_var_gaussian(mu, sigma, 0.95) AS var_95 FROM stats;
  ```

### `pgquant_es_gaussian(mu DOUBLE PRECISION, sigma DOUBLE PRECISION, alpha DOUBLE PRECISION)`
Computes the Gaussian Expected Shortfall.
- **Returns:** `DOUBLE PRECISION`
