use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::Error;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LicenseAcceptance {
    #[default]
    Never,
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
        if self.policy != Policy::default() {
            return Err(Error::not_implemented("policy"));
        }
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
