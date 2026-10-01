# Returns Functions

`pgQuant` provides functions to compute daily simple and logarithmic returns directly from timeseries queries, as well as utilities to annualize and compute cumulative returns.

## Time-Series Returns

These functions take a SQL query string as input. The query MUST return three columns: `symbol` (TEXT), `date` (DATE), and `price` (DOUBLE PRECISION), ordered appropriately (typically by `symbol`, then `date`).

### `pgquant_simple_returns(query TEXT)`
Computes the daily simple return for each row in the input query. The first date for each symbol will be dropped since a return requires a previous day's price.
- **Returns:** `TABLE(symbol TEXT, date DATE, simple_return DOUBLE PRECISION)`
- **Example:**
  ```sql
  SELECT * FROM pgquant_simple_returns(
      'SELECT symbol, date, price FROM prices_long WHERE symbol = ''ADANIENT'' ORDER BY date'
  );
  ```

### `pgquant_log_returns(query TEXT)`
Computes the daily logarithmic return ($\ln(P_t / P_{t-1})$) for each row.
- **Returns:** `TABLE(symbol TEXT, date DATE, log_return DOUBLE PRECISION)`
- **Example:**
  ```sql
  SELECT * FROM pgquant_log_returns('SELECT symbol, date, price FROM prices_long ORDER BY symbol, date');
  ```

## Aggregations & Utilities

These functions operate on scalar values or arrays of returns.

### `pgquant_cumulative_simple_return(returns DOUBLE PRECISION[])`
Computes the total cumulative return from an array of sequential daily simple returns.
- **Returns:** `DOUBLE PRECISION`
- **Example:**
  ```sql
  WITH rets AS (SELECT simple_return FROM pgquant_simple_returns('...'))
  SELECT pgquant_cumulative_simple_return(array_agg(simple_return)) FROM rets;
  ```

### `pgquant_cumulative_log_return(returns DOUBLE PRECISION[])`
Computes the total cumulative return from an array of sequential daily log returns (which is simply the sum of the log returns).
- **Returns:** `DOUBLE PRECISION`

### `pgquant_annualize_simple(avg_daily_return DOUBLE PRECISION, trading_days INTEGER)`
Annualizes an average daily simple return over the specified number of trading days (e.g., 252).
- **Returns:** `DOUBLE PRECISION`

### `pgquant_annualize_log(avg_daily_return DOUBLE PRECISION, trading_days INTEGER)`
Annualizes an average daily logarithmic return.
- **Returns:** `DOUBLE PRECISION`
