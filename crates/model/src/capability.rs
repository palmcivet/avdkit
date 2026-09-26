use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityId {
    Environment,
    Capabilities,
    Refresh,
    PackagesListInstalled,
    PackagesListAvailable,
    PackagesInstall,
    PackagesRemove,
    PackagesUpdate,
    DevicesList,
    DevicesGet,
    DevicesProfiles,
    DevicesPlanCreate,
    DevicesPlanCreateCustomHardware,
    DevicesPlanEdit,
    DevicesPlanMove,
    DevicesPlanDuplicate,
    DevicesDelete,
    RuntimeRunning,
    RuntimeBootStatus,
    RuntimeStart,
    RuntimeStartCustom,
    RuntimeStop,
    RuntimeStopAll,
    SnapshotsList,
    SnapshotsSave,
    SnapshotsLoad,
    SnapshotsDelete,
}

impl CapabilityId {
    pub const ALL: &'static [Self] = &[
        Self::Environment,
        Self::Capabilities,
        Self::Refresh,
        Self::PackagesListInstalled,
        Self::PackagesListAvailable,
        Self::PackagesInstall,
        Self::PackagesRemove,
        Self::PackagesUpdate,
        Self::DevicesList,
        Self::DevicesGet,
        Self::DevicesProfiles,
        Self::DevicesPlanCreate,
        Self::DevicesPlanCreateCustomHardware,
        Self::DevicesPlanEdit,
        Self::DevicesPlanMove,
        Self::DevicesPlanDuplicate,
        Self::DevicesDelete,
        Self::RuntimeRunning,
        Self::RuntimeBootStatus,
        Self::RuntimeStart,
        Self::RuntimeStartCustom,
        Self::RuntimeStop,
        Self::RuntimeStopAll,
        Self::SnapshotsList,
        Self::SnapshotsSave,
        Self::SnapshotsLoad,
        Self::SnapshotsDelete,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Environment => "environment",
            Self::Capabilities => "capabilities",
            Self::Refresh => "refresh",
            Self::PackagesListInstalled => "packages_list_installed",
            Self::PackagesListAvailable => "packages_list_available",
            Self::PackagesInstall => "packages_install",
            Self::PackagesRemove => "packages_remove",
            Self::PackagesUpdate => "packages_update",
            Self::DevicesList => "devices_list",
            Self::DevicesGet => "devices_get",
            Self::DevicesProfiles => "devices_profiles",
            Self::DevicesPlanCreate => "devices_plan_create",
            Self::DevicesPlanCreateCustomHardware => "devices_plan_create_custom_hardware",
            Self::DevicesPlanEdit => "devices_plan_edit",
            Self::DevicesPlanMove => "devices_plan_move",
            Self::DevicesPlanDuplicate => "devices_plan_duplicate",
            Self::DevicesDelete => "devices_delete",
            Self::RuntimeRunning => "runtime_running",
            Self::RuntimeBootStatus => "runtime_boot_status",
            Self::RuntimeStart => "runtime_start",
            Self::RuntimeStartCustom => "runtime_start_custom",
            Self::RuntimeStop => "runtime_stop",
            Self::RuntimeStopAll => "runtime_stop_all",
            Self::SnapshotsList => "snapshots_list",
            Self::SnapshotsSave => "snapshots_save",
            Self::SnapshotsLoad => "snapshots_load",
            Self::SnapshotsDelete => "snapshots_delete",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum CapabilityState {
    Available { implementation: String },
    Unavailable { reasons: Vec<Reason> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capability {
    pub id: CapabilityId,
    pub state: CapabilityState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReasonCode {
    NotImplemented,
    ToolNotFound,
    PlatformNotSupported,
    CapabilityUnavailable,
}

impl ReasonCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotImplemented => "not_implemented",
            Self::ToolNotFound => "tool_not_found",
            Self::PlatformNotSupported => "platform_not_supported",
            Self::CapabilityUnavailable => "capability_unavailable",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reason {
    pub code: ReasonCode,
    pub message: String,
    pub remedies: Vec<Remedy>,
}

impl Reason {
    pub fn new(code: ReasonCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            remedies: Vec::new(),
        }
    }

    pub fn not_implemented() -> Self {
        Self::new(
            ReasonCode::NotImplemented,
            "this operation is not implemented yet",
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemedyKind {
    LibraryOperation,
    Manual,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Remedy {
    pub message: String,
    pub command: Option<String>,
    pub kind: RemedyKind,
    pub operation: Option<CapabilityId>,
}
