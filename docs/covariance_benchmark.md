# Covariance Matrix Benchmark Results

This benchmark compares the performance of manual mean-centering versus the matrix formula `(1/T) * (X^T * X - (1/T) * s * s^T)` for computing the sample covariance matrix.

## Methodology
- **Dates (Observations, T):** 5,000
- **Symbols (Assets, N):** 500
- **Matrix Size:** 5000 x 500
- **Iterations:** 1 run

## Results

| Method | Time | Description |
|--------|------|-------------|
| **Manual Centering** | 81.00 ms | Iterating through columns, calculating the mean, and subtracting it in-place using a nested loop. |
| **Matrix Formula** | 72.26 ms | Using the formula `(1/T) * (X^T * X - (1/T) * s * s^T)`, where `s` is the vector of column sums. |


## Conclusion

The matrix formula is ~10.8% faster than the manual mean-centering approach. 