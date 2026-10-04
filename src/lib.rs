#![allow(unexpected_cfgs)]
use pgrx::prelude::*;

::pgrx::pg_module_magic!();

pub mod covariance;
pub mod factors;
pub mod returns;
pub mod risk;
pub mod spi_helpers;
pub mod volatility;

/// Returns the current pgquant version string.
///
/// # Example
/// ```sql
/// SELECT pgquant_version();
/// -- Returns: pgquant 0.1.0
/// ```
#[pg_extern]
fn pgquant_version() -> &'static str {
    concat!("pgquant ", env!("CARGO_PKG_VERSION"))
}

#[cfg(any(test, feature = "pg_test"))]
#[pg_schema]
mod tests {
    use pgrx::prelude::*;

    #[pg_test]
    fn test_pgquant_version() {
        let version = crate::pgquant_version();
        assert!(
            version.starts_with("pgquant "),
            "version string should start with 'pgquant '"
        );
        assert!(
            version.contains(env!("CARGO_PKG_VERSION")),
            "version string should contain the crate version. Got: {}",
            version
        );
    }
}

/// This module is required by `cargo pgrx test` invocations.
/// It must be visible at the root of your extension crate.
#[cfg(test)]
pub mod pg_test {
    pub fn setup(_options: Vec<&str>) {
        // perform one-off initialization when the pg_test framework starts
    }

    pub fn postgresql_conf_options() -> Vec<&'static str> {
        // return any postgresql.conf settings that are required for your tests
        vec![]
    }
}
