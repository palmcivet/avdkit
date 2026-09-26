/// Temporary product name. Other identifiers are derived from this value.
pub const PRODUCT_NAME: &str = "avdkit";

pub fn environment_prefix() -> String {
    format!("{}_", PRODUCT_NAME.to_ascii_uppercase())
}

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
