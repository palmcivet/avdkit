use serde::{Deserialize, Serialize};

/// A stable identifier for an operation that callers can query before use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum CapabilityId {
    /// Read the cached environment report.
    Environment,
    /// Read the cached capability matrix.
    Capabilities,
    /// Refresh environment discovery.
    Refresh,
    /// List installed SDK packages.
    PackagesListInstalled,
    /// List SDK packages available for installation.
    PackagesListAvailable,
    /// Install an SDK package.
    PackagesInstall,
    /// Remove an SDK package.
    PackagesRemove,
    /// Update installed SDK packages.
    PackagesUpdate,
    /// List configured virtual devices.
    DevicesList,
    /// Read one configured virtual device.
    DevicesGet,
    /// List device profiles.
    DevicesProfiles,
    /// Create a device by executing a compiled plan with preset hardware.
    ///
    /// Compiling a plan is a pure operation and does not require this capability.
    DevicesPlanCreate,
    /// Compile a device-creation plan using custom hardware.
    DevicesPlanCreateCustomHardware,
    /// Compile a device-edit plan.
    DevicesPlanEdit,
    /// Compile a device-move plan.
    DevicesPlanMove,
    /// Compile a device-duplication plan.
    DevicesPlanDuplicate,
    /// Delete a configured virtual device.
    DevicesDelete,
    /// List running emulator instances.
    RuntimeRunning,
    /// Read an emulator's boot status.
    RuntimeBootStatus,
    /// Start an emulator with preset options.
    RuntimeStart,
    /// Start an emulator with custom options.
    RuntimeStartCustom,
    /// Stop one emulator.
    RuntimeStop,
    /// Stop every emulator managed by the SDK.
    RuntimeStopAll,
    /// List emulator snapshots.
    SnapshotsList,
    /// Save an emulator snapshot.
    SnapshotsSave,
    /// Load an emulator snapshot.
    SnapshotsLoad,
    /// Delete an emulator snapshot.
    SnapshotsDelete,
}

impl CapabilityId {
    /// Every capability identifier in stable declaration order.
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

    /// Returns the stable snake-case identifier used by text outlets.
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

/// Whether a capability is usable in the current environment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum CapabilityState {
    /// The operation can be routed to the named implementation.
    Available {
        /// Stable implementation identifier.
        implementation: String,
    },
    /// The operation cannot currently be performed.
    Unavailable {
        /// All known reasons preventing the operation.
        reasons: Vec<Reason>,
    },
}

/// One entry in the capability matrix.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capability {
    /// Operation described by this entry.
    pub id: CapabilityId,
    /// Current availability of the operation.
    pub state: CapabilityState,
}

/// A stable machine-readable reason for capability unavailability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ReasonCode {
    /// The library has no implementation yet.
    NotImplemented,
    /// A required executable was not found.
    ToolNotFound,
    /// A required executable exists but is not ready for use.
    ToolNotReady,
    /// The host platform is not supported.
    PlatformNotSupported,
    /// A required lower-level capability is unavailable.
    CapabilityUnavailable,
}

impl ReasonCode {
    /// Returns the stable snake-case code used by text outlets.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotImplemented => "not_implemented",
            Self::ToolNotFound => "tool_not_found",
            Self::ToolNotReady => "tool_not_ready",
            Self::PlatformNotSupported => "platform_not_supported",
            Self::CapabilityUnavailable => "capability_unavailable",
        }
    }
}

/// A structured explanation of why a capability is unavailable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reason {
    /// Stable reason code.
    pub code: ReasonCode,
    /// Human-readable explanation.
    pub message: String,
    /// Possible ways to resolve this reason.
    pub remedies: Vec<Remedy>,
}

impl Reason {
    /// Creates a reason without remedies.
    pub fn new(code: ReasonCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            remedies: Vec::new(),
        }
    }

    /// Creates the standard not-implemented reason.
    pub fn not_implemented() -> Self {
        Self::new(
            ReasonCode::NotImplemented,
            "this operation is not implemented yet",
        )
    }
}

/// How a caller can apply a remedy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum RemedyKind {
    /// Invoke another library operation.
    LibraryOperation,
    /// Perform the remedy outside the library.
    Manual,
}

/// A possible action that resolves an unavailable capability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Remedy {
    /// Human-readable remediation guidance.
    pub message: String,
    /// Optional shell command suitable for display to a user.
    pub command: Option<String>,
    /// How the remedy is applied.
    pub kind: RemedyKind,
    /// Library operation that applies the remedy, when available.
    pub operation: Option<CapabilityId>,
}
