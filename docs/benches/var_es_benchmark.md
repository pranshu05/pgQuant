# Historical VaR & ES: O(N) vs O(N log N)

This benchmark compares the performance of two algorithms for computing Value at Risk (VaR) and Expected Shortfall (ES) at a 95% confidence level. The first algorithm uses a full sort of the returns array, while the second employs a quickselect partitioning method to find the $k$-th percentile in linear time.

## Methodology
- **Dataset:** 100,000 historical return observations
- **Objective:** Computing Value at Risk and Expected Shortfall at 95% confidence level.

## Criterion Statistical Results
| Algorithm | Average Time | Description |
|-----------|--------------|-------------|
| **`sort_by`** ($O(N \log N)$) | **1.7320 ms** | Fully sorts the entire returns array to find the $k$-th percentile. |
| **`select_nth_unstable_by`** ($O(N)$) | **141.10 µs** *(0.14 ms)* | Uses quickselect partitioning to find the $k$-th percentile. |

## Conclusion
The $O(N)$ quickselect approach is **~12x faster** (over 1200% speedup) for 100,000 observations. It avoids both full array sorting and the subsequent memory allocations required to filter tail returns.

[Benchmark Source Code](../../benches/var_es_benchmark.rs)