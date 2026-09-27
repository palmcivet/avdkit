use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::Error;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LicenseAcceptance {
    #[default]
    Never,
    Allow,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AndroidCliMetrics {
    #[default]
    Inherit,
    NoMetrics,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Policy {
    pub implicit_install: bool,
    pub explicit_install: bool,
    pub license_acceptance: LicenseAcceptance,
    pub file_operations: bool,
    pub android_cli_metrics: AndroidCliMetrics,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            implicit_install: false,
            explicit_install: true,
            license_acceptance: LicenseAcceptance::Never,
            file_operations: true,
            android_cli_metrics: AndroidCliMetrics::Inherit,
        }
    }
}

impl Policy {
    fn validate(&self) -> Result<(), Error> {
        let defaults = Self::default();
        if self.implicit_install != defaults.implicit_install {
            return Err(Error::not_implemented("policy.implicit_install"));
        }
        if self.explicit_install != defaults.explicit_install {
            return Err(Error::not_implemented("policy.explicit_install"));
        }
        if self.license_acceptance != defaults.license_acceptance {
            return Err(Error::not_implemented("policy.license_acceptance"));
        }
        if self.file_operations != defaults.file_operations {
            return Err(Error::not_implemented("policy.file_operations"));
        }
        if self.android_cli_metrics != defaults.android_cli_metrics {
            return Err(Error::not_implemented("policy.android_cli_metrics"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ToolPreference {
    pub entries: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Timeouts {
    pub command_seconds: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct KitConfig {
    pub sdk_root: Option<PathBuf>,
    pub policy: Policy,
    pub tool_preference: ToolPreference,
    pub timeouts: Timeouts,
    pub backup_dir: Option<PathBuf>,
}

impl KitConfig {
    pub fn validate(&self) -> Result<(), Error> {
        self.policy.validate()?;
        if self.tool_preference != ToolPreference::default() {
            return Err(Error::not_implemented("tool_preference"));
        }
        if self.timeouts != Timeouts::default() {
            return Err(Error::not_implemented("timeouts"));
        }
        if self.backup_dir.is_some() {
            return Err(Error::not_implemented("backup_dir"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ErrorCode, ReasonCode};

    fn assert_rejected(config: KitConfig, setting: &str) {
        let error = config.validate().unwrap_err();
        assert_eq!(error.code, ErrorCode::CapabilityUnavailable);
        assert_eq!(error.message, format!("{setting} is not implemented"));
        assert_eq!(error.reasons[0].code, ReasonCode::NotImplemented);
    }

    #[test]
    fn default_policy_is_conservative_and_supported() {
        let policy = Policy::default();
        assert!(!policy.implicit_install);
        assert!(policy.explicit_install);
        assert_eq!(policy.license_acceptance, LicenseAcceptance::Never);
        assert!(policy.file_operations);
        assert_eq!(policy.android_cli_metrics, AndroidCliMetrics::Inherit);
        KitConfig::default().validate().unwrap();
    }

    #[test]
    fn non_default_policy_settings_are_rejected_individually() {
        let mut config = KitConfig::default();
        config.policy.implicit_install = true;
        assert_rejected(config, "policy.implicit_install");

        let mut config = KitConfig::default();
        config.policy.explicit_install = false;
        assert_rejected(config, "policy.explicit_install");

        let mut config = KitConfig::default();
        config.policy.license_acceptance = LicenseAcceptance::Allow;
        assert_rejected(config, "policy.license_acceptance");

        let mut config = KitConfig::default();
        config.policy.file_operations = false;
        assert_rejected(config, "policy.file_operations");

        let mut config = KitConfig::default();
        config.policy.android_cli_metrics = AndroidCliMetrics::NoMetrics;
        assert_rejected(config, "policy.android_cli_metrics");
    }

    #[test]
    fn policy_values_have_stable_json_names() {
        assert_eq!(
            serde_json::to_value(LicenseAcceptance::Allow).unwrap(),
            "allow"
        );
        assert_eq!(
            serde_json::to_value(AndroidCliMetrics::NoMetrics).unwrap(),
            "no_metrics"
        );
    }
}
