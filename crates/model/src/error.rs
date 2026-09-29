use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error as ThisError;

use crate::capability::Reason;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    ToolNotFound,
    CapabilityUnavailable,
    PreconditionFailed,
    DeviceRunning,
    DeviceNotFound,
    NameConflict,
    PackageNotFound,
    LaunchFailed,
    ToolOutputUnrecognized,
    Timeout,
    Cancelled,
    PlatformNotSupported,
    InvalidInput,
    Internal,
}

impl ErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ToolNotFound => "tool_not_found",
            Self::CapabilityUnavailable => "capability_unavailable",
            Self::PreconditionFailed => "precondition_failed",
            Self::DeviceRunning => "device_running",
            Self::DeviceNotFound => "device_not_found",
            Self::NameConflict => "name_conflict",
            Self::PackageNotFound => "package_not_found",
            Self::LaunchFailed => "launch_failed",
            Self::ToolOutputUnrecognized => "tool_output_unrecognized",
            Self::Timeout => "timeout",
            Self::Cancelled => "cancelled",
            Self::PlatformNotSupported => "platform_not_supported",
            Self::InvalidInput => "invalid_input",
            Self::Internal => "internal",
        }
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub command: Option<String>,
    pub stdout: Option<String>,
    pub stderr: Option<String>,
    pub exit_status: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Error {
    pub code: ErrorCode,
    pub message: String,
    pub reasons: Vec<Reason>,
    pub failed_step: Option<String>,
    pub diagnostic: Option<Diagnostic>,
}

impl Error {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            reasons: Vec::new(),
            failed_step: None,
            diagnostic: None,
        }
    }

    pub fn capability_unavailable(message: impl Into<String>, reasons: Vec<Reason>) -> Self {
        Self {
            code: ErrorCode::CapabilityUnavailable,
            message: message.into(),
            reasons,
            failed_step: None,
            diagnostic: None,
        }
    }

    pub fn not_implemented(what: impl AsRef<str>) -> Self {
        Self::capability_unavailable(
            format!("{} is not implemented", what.as_ref()),
            vec![Reason::not_implemented()],
        )
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for Error {}

impl From<ModelError> for Error {
    fn from(error: ModelError) -> Self {
        Self::new(ErrorCode::InvalidInput, error.to_string())
    }
}

#[derive(Debug, ThisError)]
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
    fn error_codes_serialize_as_snake_case() {
        let value = serde_json::to_value(ErrorCode::CapabilityUnavailable).unwrap();
        assert_eq!(value, "capability_unavailable");
        let value = serde_json::to_value(ErrorCode::DeviceNotFound).unwrap();
        assert_eq!(value, "device_not_found");
    }
}
