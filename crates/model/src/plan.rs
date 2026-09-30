use serde::{Deserialize, Serialize};

use crate::{
    device::HardwareConfig,
    ids::{AvdId, PackageId, ProfileId},
};

/// Caller intent used to compile a device-creation plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateDeviceDraft {
    /// Identifier of the new AVD.
    pub id: AvdId,
    /// Device hardware profile.
    pub profile: ProfileId,
    /// Installed system image to use.
    pub image: PackageId,
    /// Optional user-facing name.
    pub display_name: Option<String>,
    /// Hardware overrides.
    pub hardware: HardwareConfig,
}

/// High-level operation represented by a plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PlanKind {
    /// Create a configured virtual device.
    CreateDevice,
}

/// Serializable domain input retained so an approved plan can be executed later.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[non_exhaustive]
pub enum PlanIntent {
    /// Create a configured virtual device.
    CreateDevice {
        /// Validated creation request.
        draft: CreateDeviceDraft,
    },
}

/// Execution mechanism used by a plan step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PlanStepKind {
    /// Invoke an official command-line tool.
    ToolCall,
    /// Rewrite library-managed metadata files.
    FileRewrite,
    /// Ask the caller to perform a custom action.
    CallerCustom,
}

/// Action that reverses a completed plan step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Compensation {
    /// Human-readable description of the reversal.
    pub description: String,
}

/// One ordered unit in a compiled plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanStep {
    /// Stable identifier within the plan.
    pub id: String,
    /// Execution mechanism.
    pub kind: PlanStepKind,
    /// Human-readable action description.
    pub description: String,
    /// Reversal to run if a later step fails.
    pub compensation: Option<Compensation>,
}

/// Serializable, reviewable representation of a composed operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Plan {
    /// Stable plan identifier.
    pub id: String,
    /// High-level operation.
    pub kind: PlanKind,
    /// Domain input used to recompile private executable steps.
    pub intent: PlanIntent,
    /// Ordered execution steps.
    pub steps: Vec<PlanStep>,
}

/// Successful final value of a long-running operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[non_exhaustive]
pub enum OperationResult {
    /// An SDK package was installed.
    PackageInstalled {
        /// Installed package.
        package: PackageId,
    },
    /// An SDK package was removed.
    PackageRemoved {
        /// Removed package.
        package: PackageId,
    },
    /// An AVD was deleted.
    DeviceDeleted {
        /// Deleted AVD identifier.
        id: AvdId,
    },
    /// An emulator reached the requested startup condition.
    DeviceStarted {
        /// Discovered running instance.
        instance: crate::device::RunningInstance,
    },
    /// An emulator stopped.
    DeviceStopped {
        /// Stopped AVD identifier.
        id: AvdId,
    },
    /// Every step of a compiled plan completed.
    PlanCompleted {
        /// Completed plan identifier.
        plan_id: String,
    },
}
