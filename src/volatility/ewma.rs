use pgrx::prelude::*;

/// Computes the Exponentially Weighted Moving Average (EWMA) volatility series.
/// Given an array of returns and a decay factor lambda (e.g., 0.94 for RiskMetrics).
/// Returns an array of volatilities of the same length.
#[pg_extern]
pub fn pgquant_ewma_vol(returns: Vec<f64>, lambda: f64) -> Vec<f64> {
    if returns.is_empty() {
        return Vec::new();
    }

    if lambda <= 0.0 || lambda >= 1.0 {
        pgrx::error!("Lambda must be between 0 and 1 (exclusive).");
    }

    // Initialize with sample variance of the provided series
    // If n=1, use return^2
    let initial_var = if returns.len() > 1 {
        let n = returns.len() as f64;
        let sum: f64 = returns.iter().sum();
        let sum_sq: f64 = returns.iter().map(|r| r * r).sum();
        let var = (sum_sq - (sum * sum) / n) / (n - 1.0);
        var
    } else {
        returns[0] * returns[0]
    };

    let mut vol_series = Vec::with_capacity(returns.len());
    let mut current_var = initial_var;

    for r in returns {
        current_var = lambda * current_var + (1.0 - lambda) * (r * r);
        vol_series.push(current_var.sqrt());
    }

    vol_series
}

#[cfg(any(test, feature = "pg_test"))]
#[pg_schema]
mod tests {
    use super::*;

    #[pg_test]
    fn test_ewma_vol() {
        let returns = vec![0.01, -0.02, 0.015];
        let lambda = 0.94;

        let vols = pgquant_ewma_vol(returns.clone(), lambda);

        // Manual calculation
        // sum = 0.005, n = 3, sum_sq = 0.0001 + 0.0004 + 0.000225 = 0.000725
        // initial_var = (0.000725 - 0.000025 / 3) / 2 = (0.000725 - 0.000008333) / 2 = 0.000358333
        // var_1 = 0.94 * 0.000358333 + 0.06 * 0.0001 = 0.000336833 + 0.000006 = 0.000342833
        // vol_1 = sqrt(var_1) ≈ 0.0185157

        assert_eq!(vols.len(), 3);
        assert!((vols[0] - 0.0185157).abs() < 1e-6);
    }
}
