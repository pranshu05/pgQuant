# pgQuant

**pgQuant - the quant toolkit Postgres never had**

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

**Planned Features (WIP):**
- **Student-t VaR & ES**
- **Volatility Models:** Rolling volatility, EWMA, and GARCH(1,1) estimation via MLE.
- **Covariance:** Sample and Shrinkage covariance (Ledoit-Wolf style).
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

## Building and Installation

This extension is built with [pgrx](https://github.com/pgcentralfoundation/pgrx). 

Prerequisites:
- Rust toolchain
- PostgreSQL 15, 16, or 17
- `cargo-pgrx` installed and initialized

To build and run:
```bash
cargo pgrx run
```

To run tests:
```bash
cargo pgrx test
```

## License

MIT License
