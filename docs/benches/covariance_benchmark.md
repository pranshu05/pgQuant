# Covariance Matrix Benchmark Results

This benchmark rigorously compares the performance of manual mean-centering versus the matrix formula for computing the sample covariance matrix using Rust's `criterion` framework.

## Methodology
- **Dates (Observations, T):** 5,000
- **Symbols (Assets, N):** 500
- **Matrix Size:** 5000 x 500
- **Framework:** `criterion.rs` (10 samples, 3s warm-up, statistical outlier detection)

## Criterion Statistical Results

| Method | Avg Time (95% CI) | Description |
|--------|------|-------------|
| **Manual Centering** | 76.58 ms [75.30 ms - 77.96 ms] | Iterating through columns, calculating the mean, and subtracting it in-place using a nested loop. |
| **Matrix Formula** | 62.03 ms [60.55 ms - 63.94 ms] | Using the formula `(1/T) * (X^T * X - (1/T) * s * s^T)`, where `s` is the vector of column sums. |

## Conclusion
The matrix formula is **~19% faster** (62ms vs 76ms) than the manual mean-centering approach under rigorous optimized profiling.

[Benchmark Source Code](../../benches/cov_benchmark.rs)