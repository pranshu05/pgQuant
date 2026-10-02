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
