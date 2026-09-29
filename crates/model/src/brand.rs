/// Temporary product name. Other identifiers are derived from this value.
pub const PRODUCT_NAME: &str = "avdkit";

/// Returns the environment-variable prefix derived from [`PRODUCT_NAME`].
pub fn environment_prefix() -> String {
    format!("{}_", PRODUCT_NAME.to_ascii_uppercase())
}

/// Returns the AVD identifier prefix reserved for tests.
pub fn test_avd_prefix() -> String {
    format!("{PRODUCT_NAME}_test_")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derived_names_use_the_single_product_name() {
        assert_eq!(environment_prefix(), "AVDKIT_");
        assert_eq!(test_avd_prefix(), "avdkit_test_");
    }
}
