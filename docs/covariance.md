# Covariance

This module contains functions for computing the sample and shrinkage covariance matrices of an asset panel.

### `pgquant_sample_cov(query TEXT)`
Computes the sample covariance matrix for a panel of returns using list-wise deletion (only dates where all queried symbols have data are included). 

- **Arguments:**
  - `query`: A string containing a SQL query that returns exactly three columns: `(symbol TEXT, date DATE, return DOUBLE PRECISION)`.
- **Returns:** A flattened table with columns `(symbol1 TEXT, symbol2 TEXT, covariance DOUBLE PRECISION)`.
- **Example:**
  ```sql
  WITH rets AS (SELECT * FROM pgquant_log_returns('SELECT symbol, date, price FROM my_prices'))
  SELECT * FROM pgquant_sample_cov('SELECT symbol, date, log_return FROM rets');
  ```

### `pgquant_shrinkage_cov_lw(query TEXT)`
Computes the Ledoit-Wolf shrinkage covariance matrix towards a scaled identity target. This is useful when the number of observations is comparable to or smaller than the number of assets, making the sample covariance matrix ill-conditioned. The optimal shrinkage intensity is computed analytically.

- **Arguments:**
  - `query`: A string containing a SQL query that returns exactly three columns: `(symbol TEXT, date DATE, return DOUBLE PRECISION)`.
- **Returns:** A flattened table with columns `(symbol1 TEXT, symbol2 TEXT, covariance DOUBLE PRECISION)`.
- **Example:**
  ```sql
  WITH rets AS (SELECT * FROM pgquant_log_returns('SELECT symbol, date, price FROM my_prices'))
  SELECT * FROM pgquant_shrinkage_cov_lw('SELECT symbol, date, log_return FROM rets');
  ```
