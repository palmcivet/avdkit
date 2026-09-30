use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{
    capability::{Capability, CapabilityId},
    device::Package,
    error::Diagnostic,
    field::Field,
    host::{Host, PlatformPaths, ToolNames},
    ids::Revision,
    Reason,
};

/// Origin of a discovered environment value or selected path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValueSource {
    /// Explicit caller configuration.
    CallerOverride,
    /// Environment inherited by the current process.
    ProcessEnvironment,
    /// Environment loaded from the platform login shell.
    LoginShell,
    /// Value reported by the Android CLI (`android`).
    AndroidCli,
    /// Platform-specific fallback.
    PlatformDefault,
}

/// One environment value together with its origin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvironmentValue {
    /// Environment variable name or logical value name.
    pub name: String,
    /// Discovered value.
    pub value: String,
    /// Origin of the value.
    pub source: ValueSource,
}

/// How an official executable was discovered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolSource {
    /// Found through the merged executable search path.
    SearchPath,
    /// Found in a package under the selected SDK root.
    SdkPackage,
    /// Found under the deprecated SDK `tools/bin` directory.
    LegacyToolsBin,
}

/// Readiness of a discovered executable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolState {
    /// No executable was found.
    Missing,
    /// The executable is ready for routing.
    Available,
    /// The executable exists but cannot currently be used.
    Unavailable,
    /// The executable belongs to a layout that is only reported.
    ReportOnly,
}

/// Discovery result for one official executable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolStatus {
    /// Platform-specific executable name.
    pub name: String,
    /// Resolved executable path, or `None` when not found.
    pub path: Option<PathBuf>,
    /// Discovered executable or package revision.
    pub version: Field<Revision>,
    /// Current readiness.
    pub state: ToolState,
    /// Discovery mechanism.
    pub source: Option<ToolSource>,
    /// SDK package that supplied the executable, when applicable.
    pub package: Option<Package>,
    /// Structured reasons for an unavailable executable.
    pub reasons: Vec<Reason>,
    /// Optional command output retained for troubleshooting.
    pub diagnostic: Option<Diagnostic>,
}

/// Category of a non-fatal environment probe diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum EnvironmentDiagnosticCode {
    /// Multiple sources supplied different values.
    SourceConflict,
    /// A probe command or filesystem scan failed.
    ProbeFailed,
}

/// Non-fatal issue found while constructing an environment snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvironmentDiagnostic {
    /// Stable diagnostic category.
    pub code: EnvironmentDiagnosticCode,
    /// Human-readable explanation.
    pub message: String,
    /// Values involved in the diagnostic.
    pub values: Vec<EnvironmentValue>,
}

/// Immutable inputs used to derive a capability matrix.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvironmentSnapshot {
    /// Host platform and architecture.
    pub host: Host,
    /// Selected filesystem roots.
    pub paths: PlatformPaths,
    /// Origin of the selected SDK root.
    pub sdk_root_source: ValueSource,
    /// Environment values considered by discovery.
    pub environment: Vec<EnvironmentValue>,
    /// Executable names for this platform.
    pub tool_names: ToolNames,
    /// Discovery state of each relevant executable.
    pub tools: Vec<ToolStatus>,
    /// Deprecated executables that are reported but never routed.
    pub legacy_tools: Vec<ToolStatus>,
    /// SDK packages discovered from `source.properties`.
    pub installed_packages: Vec<Package>,
    /// Non-fatal conflicts and probe failures.
    pub diagnostics: Vec<EnvironmentDiagnostic>,
}

/// Availability of every declared operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityMatrix {
    /// Capability entries in declaration order.
    pub capabilities: Vec<Capability>,
}

impl CapabilityMatrix {
    /// Finds the entry for a capability identifier.
    pub fn capability(&self, id: CapabilityId) -> Option<&Capability> {
        self.capabilities
            .iter()
            .find(|capability| capability.id == id)
    }
}

/// Environment snapshot and capabilities derived from the same probe.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvironmentReport {
    /// Discovered host, path, and tool state.
    pub snapshot: EnvironmentSnapshot,
    /// Capabilities derived from `snapshot`.
    pub capabilities: CapabilityMatrix,
}
