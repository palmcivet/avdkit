use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error as ThisError;

use crate::capability::Reason;

/// Stable machine-readable category for an operation failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// A required executable was not found.
    ToolNotFound,
    /// The requested operation is not currently available.
    CapabilityUnavailable,
    /// Request-specific prerequisites were not met.
    PreconditionFailed,
    /// The requested AVD must be stopped first.
    DeviceRunning,
    /// The requested AVD does not exist.
    DeviceNotFound,
    /// The requested name is already in use.
    NameConflict,
    /// The requested SDK package does not exist.
    PackageNotFound,
    /// An emulator process failed during startup.
    LaunchFailed,
    /// Tool output did not match a known contract.
    ToolOutputUnrecognized,
    /// The operation exceeded its time limit.
    Timeout,
    /// The caller cancelled the operation.
    Cancelled,
    /// The host platform has no implementation.
    PlatformNotSupported,
    /// Caller input is invalid.
    InvalidInput,
    /// An invariant, synchronization, or low-level I/O operation failed.
    Internal,
}

impl ErrorCode {
    /// Returns the stable snake-case error code used by text outlets.
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

/// Optional low-level context retained for troubleshooting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    /// Rendered command, when safe and available.
    pub command: Option<String>,
    /// Captured standard output.
    pub stdout: Option<String>,
    /// Captured standard error.
    pub stderr: Option<String>,
    /// Process exit status, when the process exited normally.
    pub exit_status: Option<i32>,
}

/// Stable operation error shared by every outlet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Error {
    /// Machine-readable failure category.
    pub code: ErrorCode,
    /// Human-readable summary.
    pub message: String,
    /// Structured availability reasons, when applicable.
    pub reasons: Vec<Reason>,
    /// Plan step that failed, when applicable.
    pub failed_step: Option<String>,
    /// Optional low-level troubleshooting context.
    pub diagnostic: Option<Diagnostic>,
}

impl Error {
    /// Creates an error without reasons or diagnostics.
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            reasons: Vec::new(),
            failed_step: None,
            diagnostic: None,
        }
    }

    /// Creates a capability-unavailable error with structured reasons.
    pub fn capability_unavailable(message: impl Into<String>, reasons: Vec<Reason>) -> Self {
        Self {
            code: ErrorCode::CapabilityUnavailable,
            message: message.into(),
            reasons,
            failed_step: None,
            diagnostic: None,
        }
    }

    /// Creates the standard not-implemented capability error.
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

/// Validation failure produced while constructing a public model value.
#[derive(Debug, ThisError)]
pub enum ModelError {
    /// An identifier was empty.
    #[error("identifier must not be empty")]
    EmptyIdentifier,
    /// An identifier contained forbidden path syntax.
    #[error("invalid identifier: {0}")]
    InvalidIdentifier(String),
    /// A revision string could not be parsed.
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
