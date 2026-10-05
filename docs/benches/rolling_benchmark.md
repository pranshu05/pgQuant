# Rolling Volatility: Naive vs Welford's Algorithm

This benchmark compares the performance of two algorithms for computing rolling sample standard deviation over a 50-day window. The first algorithm uses a naive sum of squares method, while the second employs Welford's algorithm for numerically stable updates.

## Methodology
- **Dataset:** 1,000,000 observations (50-day rolling window)
- **Objective:** Computing continuous rolling sample standard deviation.

## Criterion Statistical Results
| Algorithm | Average Time | Description |
|-----------|--------------|-------------|
| **Naive Sum of Squares** | **5.8062 ms** | Uses `(sum_sq - (sum^2 / n)) / (n-1)`. Very fast but extremely vulnerable to catastrophic cancellation (numerical drift leading to negative variances). |
| **Sliding Welford's Algorithm** | **6.9286 ms** | Exact updates of running mean and $M_2$. Numerically stable, immune to precision drift. |

## Conclusion
Welford's sliding window algorithm is **~19% slower** (~1.1 ms slower over 1 million data points). In quantitative finance, taking a sub-millisecond penalty to guarantee precision and eliminate catastrophic cancellation across millions of floating-point operations is universally considered a mandatory trade-off.

[Benchmark Source Code](../../benches/rolling_benchmark.rs)