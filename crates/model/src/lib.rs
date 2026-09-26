//! Serializable public data types shared by every outlet.

#![allow(clippy::result_large_err)]

pub mod brand;
pub mod capability;
pub mod config;
pub mod device;
pub mod envelope;
pub mod environment;
pub mod error;
pub mod event;
pub mod field;
pub mod host;
pub mod ids;
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
pub use environment::{CapabilityMatrix, EnvironmentReport, EnvironmentSnapshot, ToolStatus};
pub use error::{Diagnostic, Error, ErrorCode, ModelError};
pub use event::{Event, LogStream};
pub use field::Field;
pub use host::{CpuArchitecture, Description, Host, Platform, PlatformPaths, ToolNames};
pub use ids::{AvdId, PackageId, PackageKind, ProfileId, Revision, Serial};
pub use plan::{
    Compensation, CreateDeviceDraft, OperationResult, Plan, PlanKind, PlanStep, PlanStepKind,
};
