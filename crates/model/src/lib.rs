//! Serializable public data types shared by every outlet.

#![allow(clippy::result_large_err)]
#![deny(missing_docs)]

/// Product-name-derived identifiers.
pub mod brand;
/// Capability identifiers, states, reasons, and remedies.
pub mod capability;
/// Caller configuration and policy.
pub mod config;
/// Device, profile, package, and runtime records.
pub mod device;
/// Versioned serialized response wrapper.
pub mod envelope;
/// Environment snapshots, tool status, and capability matrices.
pub mod environment;
/// Stable errors and diagnostics.
pub mod error;
/// Events emitted by long-running operations.
pub mod event;
/// Explicit three-state fields.
pub mod field;
/// Host platform, architecture, paths, and tool names.
pub mod host;
/// Validated identifiers and revisions.
pub mod ids;
/// Serializable plans and operation results.
pub mod plan;

pub use brand::{environment_prefix, test_avd_prefix, PRODUCT_NAME};
pub use capability::{
    Capability, CapabilityId, CapabilityState, Reason, ReasonCode, Remedy, RemedyKind,
};
pub use config::{
    AndroidCliMetrics, KitConfig, LicenseAcceptance, Policy, Timeouts, ToolPreference,
};
pub use device::{
    BootStatus, Device, HardwareConfig, Package, Profile, RunningInstance, StartOptions,
};
pub use envelope::{Envelope, SCHEMA_VERSION};
pub use environment::{
    CapabilityMatrix, EnvironmentDiagnostic, EnvironmentDiagnosticCode, EnvironmentReport,
    EnvironmentSnapshot, EnvironmentValue, ToolSource, ToolState, ToolStatus, ValueSource,
};
pub use error::{CompensationResult, Diagnostic, Error, ErrorCode, ModelError};
pub use event::{Event, LogStream};
pub use field::Field;
pub use host::{CpuArchitecture, Description, Host, Platform, PlatformPaths, ToolNames};
pub use ids::{AvdId, PackageId, PackageKind, ProfileId, Revision, Serial};
pub use plan::{
    Compensation, CreateDeviceDraft, OperationResult, Plan, PlanIntent, PlanKind, PlanStep,
    PlanStepKind,
};
