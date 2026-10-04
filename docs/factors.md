# Factor Construction

This module provides infrastructure to construct custom financial factors (like the Fama-French factors) directly within PostgreSQL.

### `pgquant_portfolio_sort(query TEXT, num_buckets INT)`
Sorts and buckets stocks cross-sectionally for each date based on a characteristic value. This is the foundational plumbing for constructing Long-Short portfolios.

- **Arguments:**
  - `query`: A SQL query returning exactly `(symbol TEXT, date DATE, characteristic_value DOUBLE PRECISION)`.
  - `num_buckets`: The number of quantiles to divide the cross-section into (e.g. 2 for median split, 5 for quintiles, 10 for deciles).
- **Returns:** A flattened table with columns `(symbol TEXT, date DATE, bucket INT)`.
- **Note:** Bucket 1 always contains the lowest characteristic values, up to `num_buckets` for the highest values.
- **Example:**
  ```sql
  -- Create 5 buckets based on Market Capitalization for each day
  SELECT * FROM pgquant_portfolio_sort(
    'SELECT symbol, date, market_cap FROM daily_characteristics',
    5
  );
  ```
