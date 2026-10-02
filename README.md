<div align="center">
  <img src="assets/logo.jpg" alt="pgQuant Logo" width="300"/>
  <h1>pgQuant</h1>
  <p><strong>The quant toolkit Postgres never had</strong></p>
  
  [![Release](https://img.shields.io/github/v/release/pranshu05/pgQuant?style=flat-square)](https://github.com/pranshu05/pgQuant/releases)
  [![Build Status](https://img.shields.io/github/actions/workflow/status/pranshu05/pgQuant/release.yml?branch=main&style=flat-square)](https://github.com/pranshu05/pgQuant/actions)
  [![PostgreSQL](https://img.shields.io/badge/PostgreSQL-14%20%7C%2015%20%7C%2016%20%7C%2017-336791?style=flat-square&logo=postgresql)](https://postgresql.org)
  [![Rust](https://img.shields.io/badge/Rust-1.84+-black?style=flat-square&logo=rust)](https://rust-lang.org)
  [![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg?style=flat-square)](https://opensource.org/licenses/MIT)
</div>

`pgQuant` is a PostgreSQL extension written in Rust (using `pgrx`) that brings quantitative finance primitives - returns, risk (VaR/ES), volatility (rolling/EWMA/GARCH), covariance (sample/shrinkage), factor construction (HML/SMB/WML), and mean-variance portfolio optimization directly into SQL. It operates directly on your own price and returns tables.

## Inspiration

The inspiration for starting this project was the course I have taken currently, **SC453: Applied Quantitative Finance** under [Prof. Jayanth R Varma](https://github.com/jrvarma), combined with my experience working on PostgreSQL extensions. This extension aims to take the quantitative models from the classroom and bring them to production data directly inside the database.

## Features

**Currently Implemented:**
- **[Returns (docs/returns.md)](docs/returns.md):** 
  - Simple & log returns over timeseries price queries (SRF).
  - Cumulative returns and annualization functions.
- **[Risk Metrics (docs/risk.md)](docs/risk.md):**
  - Historical Value at Risk (VaR) & Expected Shortfall (ES)
  - Gaussian (parametric) VaR & ES
  - Student-t (parametric) VaR & ES
- **[Volatility Models (docs/volatility.md)](docs/volatility.md):**
  - Rolling Volatility (configurable window)
  - Exponentially Weighted Moving Average (EWMA) Volatility
  - EWMA Lambda Estimation via MLE
  - GARCH(1,1) parameter estimation with normal and Student-t innovations
- **[Covariance (docs/covariance.md)](docs/covariance.md):**
  - Sample Covariance Matrix (flattened output)
  - Shrinkage Covariance Matrix (Ledoit-Wolf style)

**Planned Features (WIP):**
- **Factor Construction:** SMB, HML, and WML (momentum) construction.
- **Portfolio Optimization:** Unconstrained and constrained (long-only) mean-variance optimization.

## Data Access Convention

Rather than owning a schema, `pgQuant` is designed to be flexible. Functions accept either:
1. A **SQL text query parameter** describing how to pull the data:
   ```sql
   SELECT * FROM pgquant_log_returns(
     'SELECT symbol, date, price FROM my_prices ORDER BY symbol, date'
   );
   ```
2. Or a plain `double precision[]` array for simpler aggregate-style functions that you can extract via a normal SQL query.

## Installation

### 1. Pre-Compiled Release (Recommended)
You can download the pre-compiled binary for your specific PostgreSQL version directly from the [GitHub Releases](https://github.com/pranshu05/pgQuant/releases) page.

1. Download the ZIP file for your PostgreSQL version (e.g., `pgquant-v0.1.2-pg14-linux-amd64.zip`).
2. Unzip the file:
   ```bash
   unzip pgquant-v0.1.2-pg14-linux-amd64.zip
   ```
3. Copy the library and extension files into your PostgreSQL installation directories:
   ```bash
   # Find your PostgreSQL library and extension directories
   PG_LIB=$(pg_config --pkglibdir)
   PG_EXT=$(pg_config --sharedir)/extension
   
   # Copy the shared library
   sudo cp pgquant.so $PG_LIB/
   
   # Copy the control and SQL schema files
   sudo cp pgquant.control pgquant--*.sql $PG_EXT/
   ```
4. Finally, connect to your PostgreSQL database and enable the extension:
   ```sql
   CREATE EXTENSION pgquant;
   ```

### 2. Compiling from Source
If you prefer to compile the extension yourself or are using a different OS/architecture, you will need the Rust toolchain and `cargo-pgrx` (version `0.12.9`).

```bash
# Clone the repository
git clone https://github.com/pranshu05/pgQuant.git
cd pgQuant

# Install cargo-pgrx (0.12.9)
cargo install --locked cargo-pgrx --version "=0.12.9"
cargo pgrx init

# Compile and install directly into your local PostgreSQL
cargo pgrx install --release
```

## License

MIT License
