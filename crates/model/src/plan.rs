use serde::{Deserialize, Serialize};

use crate::{
    device::HardwareConfig,
    ids::{AvdId, PackageId, ProfileId},
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateDeviceDraft {
    pub id: AvdId,
    pub profile: ProfileId,
    pub image: PackageId,
    pub display_name: Option<String>,
    pub hardware: HardwareConfig,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanKind {
    CreateDevice,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanStepKind {
    ToolCall,
    FileRewrite,
    CallerCustom,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Compensation {
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanStep {
    pub id: String,
    pub kind: PlanStepKind,
    pub description: String,
    pub compensation: Option<Compensation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Plan {
    pub id: String,
    pub kind: PlanKind,
    pub steps: Vec<PlanStep>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OperationResult {
    PackageInstalled {
        package: PackageId,
    },
    PackageRemoved {
        package: PackageId,
    },
    DeviceDeleted {
        id: AvdId,
    },
    DeviceStarted {
        instance: crate::device::RunningInstance,
    },
    DeviceStopped {
        id: AvdId,
    },
    PlanCompleted {
        plan_id: String,
    },
}
