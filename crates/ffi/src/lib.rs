//! Foreign-function interface boundary.
//!
//! The UniFFI surface is intentionally not generated yet. This crate re-exports
//! the public facade and model types so bindings do not invent a second schema.

pub use kit::{
    AvdId, BootStatus, Capability, CapabilityId, CreateDeviceDraft, Device, Envelope,
    EnvironmentReport, Error, Event, Kit, KitConfig, Operation, OperationResult, Package,
    PackageId, Plan, Profile, RunningInstance, StartOptions,
};
