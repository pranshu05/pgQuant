use pgrx::prelude::*;

/// Decomposes portfolio risk into per-asset contributions.
///
/// For a portfolio with weights `w` and covariance matrix `Σ`:
///
/// - **Portfolio volatility**: σ_p = √(w^T Σ w)
/// - **Marginal Risk Contribution (MRC)**: MRC_i = (Σ w)_i / σ_p
/// - **Component Risk Contribution (CRC)**: CRC_i = w_i × MRC_i
/// - **Percentage Contribution (PCT)**: PCT_i = CRC_i / σ_p
///
/// By Euler's theorem for homogeneous functions, Σ CRC_i = σ_p exactly.
///
/// # Arguments
/// * `weights` - Portfolio weight vector (length N). Must sum to 1.0.
/// * `cov_matrix` - Flattened N×N covariance matrix (row-major).
///
/// # Returns
/// A set of rows `(asset_index, weight, marginal_risk_contribution,
/// component_risk_contribution, pct_risk_contribution, portfolio_volatility)`.
///
/// # Example
/// ```sql
/// SELECT * FROM pgquant_risk_decomposition(
///     weights => ARRAY[0.6, 0.4],
///     cov_matrix => ARRAY[0.04, 0.01, 0.01, 0.09]
/// );
/// ```
#[pg_extern]
pub fn pgquant_risk_decomposition(
    weights: Vec<f64>,
    cov_matrix: Vec<f64>,
) -> Result<
    TableIterator<
        'static,
        (
            name!(asset_index, i32),
            name!(weight, f64),
            name!(marginal_risk_contribution, f64),
            name!(component_risk_contribution, f64),
            name!(pct_risk_contribution, f64),
            name!(portfolio_volatility, f64),
        ),
    >,
    pgrx::spi::Error,
> {
    let n = weights.len();

    if n == 0 {
        pgrx::error!("Weights array cannot be empty.");
    }
    if cov_matrix.len() != n * n {
        pgrx::error!(
            "Covariance matrix length must be N^2 ({}), got {}.",
            n * n,
            cov_matrix.len()
        );
    }

    // Validate weights sum to ~1.0 (tolerance for floating-point)
    let w_sum: f64 = weights.iter().sum();
    if (w_sum - 1.0).abs() > 1e-6 {
        pgrx::error!(
            "Weights must sum to 1.0, got {:.10}. Normalize before calling.",
            w_sum
        );
    }

    // Compute Σw  (matrix-vector product, O(N²))
    let mut sigma_w = vec![0.0; n];
    for i in 0..n {
        let mut s = 0.0;
        for j in 0..n {
            s += cov_matrix[i * n + j] * weights[j];
        }
        sigma_w[i] = s;
    }

    // Portfolio variance: w^T Σ w = w · (Σw)
    let port_var: f64 = weights
        .iter()
        .zip(sigma_w.iter())
        .map(|(&wi, &swi)| wi * swi)
        .sum();

    if port_var <= 0.0 {
        pgrx::error!(
            "Portfolio variance is non-positive ({:.2e}). Check covariance matrix is valid PSD.",
            port_var
        );
    }

    let port_vol = port_var.sqrt();

    // Build result rows
    let rows: Vec<_> = (0..n)
        .map(|i| {
            let mrc_i = sigma_w[i] / port_vol;
            let crc_i = weights[i] * mrc_i;
            let pct_i = crc_i / port_vol; // = w_i * (Σw)_i / port_var

            (
                (i + 1) as i32, // 1-indexed asset_index (SQL convention)
                weights[i],
                mrc_i,
                crc_i,
                pct_i,
                port_vol,
            )
        })
        .collect();

    Ok(TableIterator::new(rows))
}

#[cfg(any(test, feature = "pg_test"))]
#[pg_schema]
mod tests {
    use super::*;

    #[pg_test]
    fn test_risk_decomposition_2x2_basic() {
        // Two assets: σ1=0.20, σ2=0.30, ρ=0.25
        // Cov = [[0.04, 0.015], [0.015, 0.09]]
        let weights = vec![0.6, 0.4];
        let cov = vec![0.04, 0.015, 0.015, 0.09];

        let mut iter = pgquant_risk_decomposition(weights.clone(), cov.clone()).unwrap();

        // Hand-calculate:
        // Σw = [0.04*0.6 + 0.015*0.4, 0.015*0.6 + 0.09*0.4]
        //    = [0.024 + 0.006, 0.009 + 0.036]
        //    = [0.030, 0.045]
        // port_var = 0.6*0.030 + 0.4*0.045 = 0.018 + 0.018 = 0.036
        // port_vol = sqrt(0.036) = 0.189736659...
        let expected_port_vol = 0.036f64.sqrt();

        // MRC = Σw / port_vol
        let expected_mrc_0 = 0.030 / expected_port_vol;
        let expected_mrc_1 = 0.045 / expected_port_vol;

        // CRC = w * MRC
        let expected_crc_0 = 0.6 * expected_mrc_0;
        let expected_crc_1 = 0.4 * expected_mrc_1;

        // PCT = CRC / port_vol
        let expected_pct_0 = expected_crc_0 / expected_port_vol;
        let expected_pct_1 = expected_crc_1 / expected_port_vol;

        // Verify Euler decomposition: CRC_0 + CRC_1 = port_vol
        assert!(
            (expected_crc_0 + expected_crc_1 - expected_port_vol).abs() < 1e-12,
            "Euler decomposition identity must hold"
        );
        // PCT must sum to 1.0
        assert!(
            (expected_pct_0 + expected_pct_1 - 1.0).abs() < 1e-12,
            "Percentage contributions must sum to 1.0"
        );

        let row0 = iter.next().unwrap();
        assert_eq!(row0.0, 1); // 1-indexed
        assert!((row0.1 - 0.6).abs() < 1e-12);
        assert!((row0.2 - expected_mrc_0).abs() < 1e-10, "MRC[0] mismatch");
        assert!((row0.3 - expected_crc_0).abs() < 1e-10, "CRC[0] mismatch");
        assert!((row0.4 - expected_pct_0).abs() < 1e-10, "PCT[0] mismatch");
        assert!(
            (row0.5 - expected_port_vol).abs() < 1e-10,
            "port_vol mismatch"
        );

        let row1 = iter.next().unwrap();
        assert_eq!(row1.0, 2);
        assert!((row1.1 - 0.4).abs() < 1e-12);
        assert!((row1.2 - expected_mrc_1).abs() < 1e-10, "MRC[1] mismatch");
        assert!((row1.3 - expected_crc_1).abs() < 1e-10, "CRC[1] mismatch");
        assert!((row1.4 - expected_pct_1).abs() < 1e-10, "PCT[1] mismatch");
        assert!(
            (row1.5 - expected_port_vol).abs() < 1e-10,
            "port_vol mismatch"
        );

        assert!(iter.next().is_none());
    }

    #[pg_test]
    fn test_risk_decomposition_euler_identity_3x3() {
        // 3-asset portfolio — verify Euler decomposition: Σ CRC_i = σ_p
        let weights = vec![0.5, 0.3, 0.2];
        let cov = vec![0.04, 0.006, 0.002, 0.006, 0.09, 0.009, 0.002, 0.009, 0.01];

        let rows: Vec<_> = pgquant_risk_decomposition(weights, cov).unwrap().collect();

        assert_eq!(rows.len(), 3);

        let port_vol = rows[0].5;
        let crc_sum: f64 = rows.iter().map(|r| r.3).sum();
        let pct_sum: f64 = rows.iter().map(|r| r.4).sum();

        // Euler identity: sum of CRCs = portfolio vol
        assert!(
            (crc_sum - port_vol).abs() < 1e-10,
            "Euler identity violated: sum(CRC)={:.12}, port_vol={:.12}",
            crc_sum,
            port_vol
        );

        // Percentage contributions must sum to 1.0
        assert!(
            (pct_sum - 1.0).abs() < 1e-10,
            "PCT sum must be 1.0, got {:.12}",
            pct_sum
        );

        // All portfolio_volatility values should be identical
        for row in &rows {
            assert!(
                (row.5 - port_vol).abs() < 1e-15,
                "Portfolio vol must be same in every row"
            );
        }
    }

    #[pg_test]
    fn test_risk_decomposition_equal_weight_uncorrelated() {
        // 2 uncorrelated assets with equal variance, equal weights
        // Each should contribute exactly 50% of risk
        let weights = vec![0.5, 0.5];
        let cov = vec![0.04, 0.0, 0.0, 0.04];

        let rows: Vec<_> = pgquant_risk_decomposition(weights, cov).unwrap().collect();

        assert!((rows[0].4 - 0.5).abs() < 1e-10, "PCT should be 50% each");
        assert!((rows[1].4 - 0.5).abs() < 1e-10, "PCT should be 50% each");
    }

    #[pg_test]
    fn test_risk_decomposition_single_asset() {
        let weights = vec![1.0];
        let cov = vec![0.04]; // σ = 0.2

        let rows: Vec<_> = pgquant_risk_decomposition(weights, cov).unwrap().collect();

        assert_eq!(rows.len(), 1);
        assert!((rows[0].2 - 0.2).abs() < 1e-10, "MRC should equal σ");
        assert!((rows[0].3 - 0.2).abs() < 1e-10, "CRC should equal σ");
        assert!((rows[0].4 - 1.0).abs() < 1e-10, "PCT should be 100%");
        assert!(
            (rows[0].5 - 0.2).abs() < 1e-10,
            "Portfolio vol should be 0.2"
        );
    }

    #[pg_test]
    fn test_risk_decomposition_4_asset() {
        // 4-asset correlated portfolio
        let weights = vec![0.25, 0.25, 0.25, 0.25];
        let cov = vec![
            0.0225, 0.0045, 0.0030, 0.0015, 0.0045, 0.0400, 0.0060, 0.0020, 0.0030, 0.0060, 0.0100,
            0.0025, 0.0015, 0.0020, 0.0025, 0.0064,
        ];

        let rows: Vec<_> = pgquant_risk_decomposition(weights, cov).unwrap().collect();

        assert_eq!(rows.len(), 4);

        let port_vol = rows[0].5;
        let crc_sum: f64 = rows.iter().map(|r| r.3).sum();
        let pct_sum: f64 = rows.iter().map(|r| r.4).sum();

        assert!(
            (crc_sum - port_vol).abs() < 1e-10,
            "Euler identity: sum(CRC)={:.12} vs port_vol={:.12}",
            crc_sum,
            port_vol
        );
        assert!((pct_sum - 1.0).abs() < 1e-10, "PCT sum = {:.12}", pct_sum);

        // Higher-variance asset (asset 2, σ²=0.04) should have highest pct contribution
        // Lower-variance asset (asset 4, σ²=0.0064) should have lowest pct contribution
        assert!(
            rows[1].4 > rows[0].4,
            "Asset 2 (σ²=0.04) should contribute more risk than asset 1 (σ²=0.0225)"
        );
        assert!(
            rows[0].4 > rows[3].4,
            "Asset 1 (σ²=0.0225) should contribute more risk than asset 4 (σ²=0.0064)"
        );
    }

    #[pg_test]
    fn test_risk_decomposition_risk_parity_verification() {
        // After running risk parity, all PCT contributions should be equal (~1/N).
        // This verifies the integration between risk_parity and risk_decomposition.
        let cov = vec![0.04, 0.006, 0.002, 0.006, 0.09, 0.009, 0.002, 0.009, 0.01];
        let rp_weights =
            crate::optimization::risk_parity::pgquant_risk_parity_optimize(cov.clone()).unwrap();

        let rows: Vec<_> = pgquant_risk_decomposition(rp_weights, cov)
            .unwrap()
            .collect();

        let n = rows.len();
        let target_pct = 1.0 / n as f64;
        for row in &rows {
            assert!(
                (row.4 - target_pct).abs() < 0.01,
                "Risk parity asset {} PCT={:.6} should be ~{:.6}",
                row.0,
                row.4,
                target_pct
            );
        }
    }
}
