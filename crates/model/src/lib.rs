//! Serializable public data types shared by every outlet.

use std::{cmp::Ordering, fmt, str::FromStr};

use serde::{Deserialize, Serialize};

/// Temporary product name. Other identifiers are derived from this value.
pub const PRODUCT_NAME: &str = "avdkit";

pub fn environment_prefix() -> String {
    format!("{}_", PRODUCT_NAME.to_ascii_uppercase())
}

pub fn test_avd_prefix() -> String {
    format!("{PRODUCT_NAME}_test_")
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AvdId(String);

impl AvdId {
    pub fn new(value: impl Into<String>) -> Result<Self, ModelError> {
        let value = value.into();
        if value.is_empty() {
            return Err(ModelError::EmptyIdentifier);
        }
        if value == "." || value == ".." || value.contains('/') || value.contains('\\') {
            return Err(ModelError::InvalidIdentifier(value));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AvdId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Serial(String);

impl Serial {
    pub fn new(value: impl Into<String>) -> Result<Self, ModelError> {
        let value = value.into();
        if value.is_empty() {
            return Err(ModelError::EmptyIdentifier);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Serial {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProfileId(String);

impl ProfileId {
    pub fn new(value: impl Into<String>) -> Result<Self, ModelError> {
        let value = value.into();
        if value.is_empty() {
            return Err(ModelError::EmptyIdentifier);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ProfileId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageId {
    pub kind: PackageKind,
    pub value: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PackageKind {
    SystemImage,
    Platform,
    BuildTools,
    Emulator,
    PlatformTools,
    CommandLineTools,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Revision {
    pub components: Vec<u64>,
    pub suffix: Option<String>,
}

impl FromStr for Revision {
    type Err = ModelError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let (numbers, suffix) = value
            .split_once('-')
            .map_or((value, None), |(a, b)| (a, Some(b)));
        let mut components = numbers
            .split('.')
            .map(|component| {
                component
                    .parse()
                    .map_err(|_| ModelError::InvalidRevision(value.into()))
            })
            .collect::<Result<Vec<u64>, _>>()?;
        while components.last() == Some(&0) && components.len() > 1 {
            components.pop();
        }
        if components.is_empty() {
            return Err(ModelError::InvalidRevision(value.into()));
        }
        Ok(Self {
            components,
            suffix: suffix.map(str::to_owned),
        })
    }
}

impl Ord for Revision {
    fn cmp(&self, other: &Self) -> Ordering {
        let length = self.components.len().max(other.components.len());
        for index in 0..length {
            let left = self.components.get(index).copied().unwrap_or_default();
            let right = other.components.get(index).copied().unwrap_or_default();
            match left.cmp(&right) {
                Ordering::Equal => continue,
                ordering => return ordering,
            }
        }
        self.suffix.cmp(&other.suffix)
    }
}

impl PartialOrd for Revision {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Platform {
    MacOs,
    Linux,
    Windows,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CpuArchitecture {
    Arm64,
    X86_64,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Host {
    pub platform: Platform,
    pub architecture: CpuArchitecture,
    pub supported: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityState {
    Available { implementation: String },
    Unavailable { reasons: Vec<Reason> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capability {
    pub operation: String,
    pub state: CapabilityState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reason {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    ToolNotFound,
    CapabilityUnavailable,
    PlatformNotSupported,
    InvalidInput,
    Timeout,
    Cancelled,
    Internal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Error {
    pub code: ErrorCode,
    pub message: String,
    pub reason: Option<String>,
}

impl Error {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            reason: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    StepStarted,
    StepFinished,
    Progress,
    Log,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Event {
    pub kind: EventKind,
    pub message: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ModelError {
    #[error("identifier must not be empty")]
    EmptyIdentifier,
    #[error("invalid identifier: {0}")]
    InvalidIdentifier(String),
    #[error("invalid revision: {0}")]
    InvalidRevision(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn revisions_compare_without_trailing_zero_significance() {
        let left: Revision = "7".parse().unwrap();
        let right: Revision = "7.0.0".parse().unwrap();
        assert_eq!(left, right);
    }

    #[test]
    fn derived_names_use_the_single_product_name() {
        assert_eq!(environment_prefix(), "AVDKIT_");
        assert_eq!(test_avd_prefix(), "avdkit_test_");
    }
}
