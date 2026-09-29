//! Environment discovery and SDK metadata inspection.

#![allow(clippy::result_large_err)]

use std::{
    fs,
    path::{Component, Path},
};

pub mod discovery;

/// Checks one known license hash without depending on command output or prompts.
///
/// Missing, unreadable, or malformed license files are treated as not accepted.
pub fn license_hash_accepted(sdk_root: &Path, license_id: &str, expected_hash: &str) -> bool {
    if expected_hash.is_empty()
        || Path::new(license_id).components().count() != 1
        || !matches!(
            Path::new(license_id).components().next(),
            Some(Component::Normal(_))
        )
    {
        return false;
    }
    fs::read_to_string(sdk_root.join("licenses").join(license_id))
        .map(|contents| {
            contents
                .lines()
                .map(str::trim)
                .any(|hash| hash.eq_ignore_ascii_case(expected_hash))
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    static NEXT_TEMP_DIR: AtomicU64 = AtomicU64::new(1);

    fn temp_sdk_root() -> PathBuf {
        std::env::temp_dir().join(format!(
            "avdkit-license-test-{}-{}",
            std::process::id(),
            NEXT_TEMP_DIR.fetch_add(1, Ordering::Relaxed)
        ))
    }

    #[test]
    fn accepted_licenses_are_read_from_hash_files() {
        let sdk_root = temp_sdk_root();
        let licenses = sdk_root.join("licenses");
        fs::create_dir_all(&licenses).unwrap();
        fs::write(
            licenses.join("android-sdk-license"),
            "first-hash\r\nEXPECTED-HASH\n",
        )
        .unwrap();

        assert!(license_hash_accepted(
            &sdk_root,
            "android-sdk-license",
            "expected-hash"
        ));
        assert!(!license_hash_accepted(
            &sdk_root,
            "android-sdk-license",
            "missing-hash"
        ));
        assert!(!license_hash_accepted(
            &sdk_root,
            "../android-sdk-license",
            "expected-hash"
        ));

        fs::remove_dir_all(sdk_root).unwrap();
    }

    #[test]
    fn missing_license_files_are_not_accepted() {
        assert!(!license_hash_accepted(
            &temp_sdk_root(),
            "android-sdk-license",
            "expected-hash"
        ));
    }
}
