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

### `pgquant_construct_smb(query TEXT)`
Constructs the Fama-French SMB (Small Minus Big) factor return from a double-sort panel (Size and Book-to-Market). 

- **Arguments:**
  - `query`: A SQL query returning exactly `(symbol TEXT, date DATE, return DOUBLE PRECISION, size_bucket INT, btm_bucket INT, weight DOUBLE PRECISION)`.
    - `size_bucket`: 1 (Small) or 2 (Big).
    - `btm_bucket`: 1 (Growth), 2 (Neutral), or 3 (Value).
    - `weight`: The weight of the asset in the bucket (e.g. market cap for value-weighted, or `1.0` for equal-weighted).
- **Returns:** A table of `(date DATE, smb_return DOUBLE PRECISION)`.

### `pgquant_construct_hml(query TEXT)`
Constructs the Fama-French HML (High Minus Low) factor return. It uses the exact same input query structure as `pgquant_construct_smb` but outputs the HML portfolio return.

- **Arguments:** Same as `pgquant_construct_smb`.
- **Returns:** A table of `(date DATE, hml_return DOUBLE PRECISION)`.
