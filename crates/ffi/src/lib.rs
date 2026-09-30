//! UniFFI boundary shared by Swift and future foreign-language outlets.
//!
//! Core enums are `#[non_exhaustive]`, while foreign enums are frozen. Values
//! added by a newer core map to an existing fallback where one is meaningful,
//! and otherwise to an explicit `Unknown` case.

use std::{path::PathBuf, sync::Arc};

use kit as core;
use thiserror::Error as ThisError;

uniffi::setup_scaffolding!();

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
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

#[derive(Debug, ThisError, uniffi::Error)]
pub enum BindingError {
    #[error("{0}")]
    ToolNotFound(String),
    #[error("{0}")]
    CapabilityUnavailable(String),
    #[error("{0}")]
    PreconditionFailed(String),
    #[error("{0}")]
    DeviceRunning(String),
    #[error("{0}")]
    DeviceNotFound(String),
    #[error("{0}")]
    NameConflict(String),
    #[error("{0}")]
    PackageNotFound(String),
    #[error("{0}")]
    LaunchFailed(String),
    #[error("{0}")]
    ToolOutputUnrecognized(String),
    #[error("{0}")]
    Timeout(String),
    #[error("{0}")]
    Cancelled(String),
    #[error("{0}")]
    PlatformNotSupported(String),
    #[error("{0}")]
    InvalidInput(String),
    #[error("{0}")]
    Internal(String),
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct ErrorRecord {
    pub code: ErrorCode,
    pub message: String,
    pub details_json: String,
}

#[derive(Debug, Clone, Copy, uniffi::Enum)]
pub enum FieldState {
    Present,
    Unavailable,
    Inapplicable,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct StringField {
    pub state: FieldState,
    pub value: Option<String>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct U32Field {
    pub state: FieldState,
    pub value: Option<u32>,
}

#[derive(Debug, Clone, Copy, uniffi::Enum)]
pub enum Platform {
    MacOs,
    Linux,
    Windows,
    Unsupported,
}

#[derive(Debug, Clone, Copy, uniffi::Enum)]
pub enum CpuArchitecture {
    Arm64,
    X86_64,
    Other,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct Host {
    pub platform: Platform,
    pub architecture: CpuArchitecture,
    pub android_abi: StringField,
    pub supported: bool,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct PlatformPaths {
    pub sdk_root: String,
    pub user_root: String,
    pub avd_root: String,
    pub runtime_root: String,
    pub data_root: String,
}

#[derive(Debug, Clone, Copy, uniffi::Enum)]
pub enum ValueSource {
    CallerOverride,
    ProcessEnvironment,
    LoginShell,
    AndroidCli,
    PlatformDefault,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct EnvironmentValue {
    pub name: String,
    pub value: String,
    pub source: ValueSource,
}

#[derive(Debug, Clone, Copy, uniffi::Enum)]
pub enum ToolState {
    Missing,
    Available,
    Unavailable,
    ReportOnly,
}

#[derive(Debug, Clone, Copy, uniffi::Enum)]
pub enum ToolSource {
    SearchPath,
    SdkPackage,
    LegacyToolsBin,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct Diagnostic {
    pub command: Option<String>,
    pub stdout: Option<String>,
    pub stderr: Option<String>,
    pub exit_status: Option<i32>,
}

#[derive(Debug, Clone, Copy, uniffi::Enum)]
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
    Unknown,
}

#[derive(Debug, Clone, Copy, uniffi::Enum)]
pub enum ReasonCode {
    NotImplemented,
    ToolNotFound,
    ToolNotReady,
    PlatformNotSupported,
    CapabilityUnavailable,
    Unknown,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct Reason {
    pub code: ReasonCode,
    pub message: String,
}

#[derive(Debug, Clone, uniffi::Enum)]
pub enum CapabilityState {
    Available { implementation: String },
    Unavailable { reasons: Vec<Reason> },
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct Capability {
    pub id: CapabilityId,
    pub state: CapabilityState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum PackageKind {
    SystemImage,
    Platform,
    BuildTools,
    Emulator,
    PlatformTools,
    CommandLineTools,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct PackageId {
    pub kind: PackageKind,
    pub api: Option<String>,
    pub tag: Option<String>,
    pub abi: Option<String>,
    pub qualifier: Option<String>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct Package {
    pub id: PackageId,
    pub revision: StringField,
    pub installed: bool,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct ToolStatus {
    pub name: String,
    pub path: Option<String>,
    pub version: StringField,
    pub state: ToolState,
    pub source: Option<ToolSource>,
    pub package: Option<Package>,
    pub reasons: Vec<Reason>,
    pub diagnostic: Option<Diagnostic>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct EnvironmentSnapshot {
    pub host: Host,
    pub paths: PlatformPaths,
    pub sdk_root_source: ValueSource,
    pub environment: Vec<EnvironmentValue>,
    pub tools: Vec<ToolStatus>,
    pub legacy_tools: Vec<ToolStatus>,
    pub installed_packages: Vec<Package>,
    pub diagnostics_json: String,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct EnvironmentReport {
    pub snapshot: EnvironmentSnapshot,
    pub capabilities: Vec<Capability>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct Device {
    pub id: String,
    pub display_name: StringField,
    pub profile: StringField,
    pub image: StringField,
    pub target: StringField,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct Profile {
    pub id: String,
    pub display_name: StringField,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct RunningInstance {
    pub id: String,
    pub serial: StringField,
    pub pid: U32Field,
}

#[derive(Debug, Clone, Copy, uniffi::Enum)]
pub enum BootStatus {
    Offline,
    Booting,
    Ready,
    Stuck,
    Unknown,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct HardwareConfig {
    pub ram_mib: Option<u32>,
    pub cpu_count: Option<u32>,
    pub screen_width: Option<u32>,
    pub screen_height: Option<u32>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct CreateDeviceDraft {
    pub id: String,
    pub profile: String,
    pub image: PackageId,
    pub display_name: Option<String>,
    pub hardware: HardwareConfig,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct StartOptions {
    pub cold: bool,
    pub wipe_data: bool,
    pub headless: bool,
    pub multi_instance: bool,
}

#[derive(Debug, Clone, Copy, uniffi::Enum)]
pub enum PlanKind {
    CreateDevice,
    Unknown,
}

#[derive(Debug, Clone, Copy, uniffi::Enum)]
pub enum PlanStepKind {
    ToolCall,
    FileRewrite,
    CallerCustom,
    Unknown,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct PlanStep {
    pub id: String,
    pub kind: PlanStepKind,
    pub description: String,
    pub compensation: Option<String>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct Plan {
    pub id: String,
    pub kind: PlanKind,
    pub steps: Vec<PlanStep>,
    pub json: String,
}

#[derive(Debug, Clone, Copy, uniffi::Enum)]
pub enum LogStream {
    Stdout,
    Stderr,
}

#[derive(Debug, Clone, uniffi::Enum)]
pub enum Event {
    StepStarted {
        step: String,
        message: String,
    },
    StepFinished {
        step: String,
        message: String,
    },
    CompensationStarted {
        step: String,
        message: String,
    },
    CompensationFinished {
        step: String,
        succeeded: bool,
        message: String,
    },
    Progress {
        ratio: Option<f64>,
    },
    Log {
        stream: LogStream,
        line: String,
    },
    Warning {
        message: String,
    },
    Unknown {
        json: String,
    },
}

#[derive(Debug, Clone, uniffi::Enum)]
pub enum OperationResult {
    PackageInstalled { package: PackageId },
    PackageRemoved { package: PackageId },
    DeviceDeleted { id: String },
    DeviceStarted { instance: RunningInstance },
    DeviceStopped { id: String },
    PlanCompleted { plan_id: String },
    Unknown { json: String },
}

#[derive(Debug, Clone, Copy, uniffi::Enum)]
pub enum LicenseAcceptance {
    Never,
    Allow,
}

#[derive(Debug, Clone, Copy, uniffi::Enum)]
pub enum AndroidCliMetrics {
    Inherit,
    NoMetrics,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct Policy {
    pub implicit_install: bool,
    pub explicit_install: bool,
    pub license_acceptance: LicenseAcceptance,
    pub file_operations: bool,
    pub android_cli_metrics: AndroidCliMetrics,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct KitConfig {
    pub sdk_root: Option<String>,
    pub policy: Policy,
    pub tool_preference: Vec<String>,
    pub command_timeout_seconds: Option<u64>,
    pub backup_dir: Option<String>,
}

#[uniffi::export]
pub fn default_kit_config() -> KitConfig {
    KitConfig::from(core::KitConfig::default())
}

#[derive(uniffi::Object)]
pub struct Kit {
    inner: core::Kit,
}

#[uniffi::export(async_runtime = "tokio")]
impl Kit {
    #[uniffi::constructor]
    pub fn new(config: KitConfig) -> Result<Arc<Self>, BindingError> {
        Ok(Arc::new(Self {
            inner: core::Kit::new(config.try_into()?).map_err(BindingError::from)?,
        }))
    }

    pub async fn environment(&self) -> Result<EnvironmentReport, BindingError> {
        self.inner
            .environment()
            .await
            .map(EnvironmentReport::from)
            .map_err(BindingError::from)
    }

    pub async fn capabilities(&self) -> Result<Vec<Capability>, BindingError> {
        self.inner
            .capabilities()
            .await
            .map(|values| values.into_iter().map(Capability::from).collect())
            .map_err(BindingError::from)
    }

    pub async fn refresh(&self) -> Result<EnvironmentReport, BindingError> {
        self.inner
            .refresh()
            .await
            .map(EnvironmentReport::from)
            .map_err(BindingError::from)
    }

    pub async fn list_installed(&self) -> Result<Vec<Package>, BindingError> {
        self.inner
            .list_installed()
            .await
            .map(|values| values.into_iter().map(Package::from).collect())
            .map_err(BindingError::from)
    }

    pub async fn list_available(&self) -> Result<Vec<Package>, BindingError> {
        self.inner
            .list_available()
            .await
            .map(|values| values.into_iter().map(Package::from).collect())
            .map_err(BindingError::from)
    }

    pub async fn list_devices(&self) -> Result<Vec<Device>, BindingError> {
        self.inner
            .list_devices()
            .await
            .map(|values| values.into_iter().map(Device::from).collect())
            .map_err(BindingError::from)
    }

    pub async fn get_device(&self, id: String) -> Result<Device, BindingError> {
        let id = core::AvdId::new(id).map_err(core::Error::from)?;
        self.inner
            .get_device(&id)
            .await
            .map(Device::from)
            .map_err(BindingError::from)
    }

    pub async fn profiles(&self) -> Result<Vec<Profile>, BindingError> {
        self.inner
            .profiles()
            .await
            .map(|values| values.into_iter().map(Profile::from).collect())
            .map_err(BindingError::from)
    }

    pub fn plan_create(&self, draft: CreateDeviceDraft) -> Result<Plan, BindingError> {
        self.inner
            .plan_create(draft.try_into()?)
            .map(Plan::from)
            .map_err(BindingError::from)
    }

    pub async fn execute_plan(&self, plan: Plan) -> Result<Arc<Operation>, BindingError> {
        let plan = serde_json::from_str(&plan.json).map_err(|error| {
            BindingError::InvalidInput(format!("invalid serialized plan: {error}"))
        })?;
        Ok(Operation::new(self.inner.execute_plan(plan)))
    }

    pub async fn install(&self, package: PackageId) -> Arc<Operation> {
        Operation::new(self.inner.install(package.into()))
    }

    pub async fn remove(&self, package: PackageId) -> Arc<Operation> {
        Operation::new(self.inner.remove(package.into()))
    }

    pub async fn delete_device(&self, id: String) -> Result<Arc<Operation>, BindingError> {
        let id = core::AvdId::new(id).map_err(core::Error::from)?;
        Ok(Operation::new(self.inner.delete_device(id)))
    }

    pub async fn running(&self) -> Result<Vec<RunningInstance>, BindingError> {
        self.inner
            .running()
            .await
            .map(|values| values.into_iter().map(RunningInstance::from).collect())
            .map_err(BindingError::from)
    }

    pub async fn boot_status(&self, id: String) -> Result<BootStatus, BindingError> {
        let id = core::AvdId::new(id).map_err(core::Error::from)?;
        self.inner
            .boot_status(&id)
            .await
            .map(BootStatus::from)
            .map_err(BindingError::from)
    }

    pub async fn start(
        &self,
        id: String,
        options: StartOptions,
    ) -> Result<Arc<Operation>, BindingError> {
        let id = core::AvdId::new(id).map_err(core::Error::from)?;
        Ok(Operation::new(self.inner.start(id, options.into())))
    }

    pub async fn stop(&self, id: String) -> Result<Arc<Operation>, BindingError> {
        let id = core::AvdId::new(id).map_err(core::Error::from)?;
        Ok(Operation::new(self.inner.stop(id)))
    }
}

#[derive(uniffi::Object)]
pub struct Operation {
    inner: core::Operation,
}

impl Operation {
    fn new(inner: core::Operation) -> Arc<Self> {
        Arc::new(Self { inner })
    }
}

#[uniffi::export(async_runtime = "tokio")]
impl Operation {
    pub async fn next_event(&self) -> Option<Event> {
        self.inner.next_event().await.map(Event::from)
    }

    pub fn cancel(&self) {
        self.inner.cancel();
    }

    pub async fn result(&self) -> Result<OperationResult, BindingError> {
        self.inner
            .result()
            .await
            .map(OperationResult::from)
            .map_err(BindingError::from)
    }
}

impl From<core::ErrorCode> for ErrorCode {
    fn from(value: core::ErrorCode) -> Self {
        match value {
            core::ErrorCode::ToolNotFound => Self::ToolNotFound,
            core::ErrorCode::CapabilityUnavailable => Self::CapabilityUnavailable,
            core::ErrorCode::PreconditionFailed => Self::PreconditionFailed,
            core::ErrorCode::DeviceRunning => Self::DeviceRunning,
            core::ErrorCode::DeviceNotFound => Self::DeviceNotFound,
            core::ErrorCode::NameConflict => Self::NameConflict,
            core::ErrorCode::PackageNotFound => Self::PackageNotFound,
            core::ErrorCode::LaunchFailed => Self::LaunchFailed,
            core::ErrorCode::ToolOutputUnrecognized => Self::ToolOutputUnrecognized,
            core::ErrorCode::Timeout => Self::Timeout,
            core::ErrorCode::Cancelled => Self::Cancelled,
            core::ErrorCode::PlatformNotSupported => Self::PlatformNotSupported,
            core::ErrorCode::InvalidInput => Self::InvalidInput,
            _ => Self::Internal,
        }
    }
}

impl From<core::Error> for BindingError {
    fn from(value: core::Error) -> Self {
        let details = serde_json::to_string(&value).unwrap_or_else(|_| value.message.clone());
        match value.code {
            core::ErrorCode::ToolNotFound => Self::ToolNotFound(details),
            core::ErrorCode::CapabilityUnavailable => Self::CapabilityUnavailable(details),
            core::ErrorCode::PreconditionFailed => Self::PreconditionFailed(details),
            core::ErrorCode::DeviceRunning => Self::DeviceRunning(details),
            core::ErrorCode::DeviceNotFound => Self::DeviceNotFound(details),
            core::ErrorCode::NameConflict => Self::NameConflict(details),
            core::ErrorCode::PackageNotFound => Self::PackageNotFound(details),
            core::ErrorCode::LaunchFailed => Self::LaunchFailed(details),
            core::ErrorCode::ToolOutputUnrecognized => Self::ToolOutputUnrecognized(details),
            core::ErrorCode::Timeout => Self::Timeout(details),
            core::ErrorCode::Cancelled => Self::Cancelled(details),
            core::ErrorCode::PlatformNotSupported => Self::PlatformNotSupported(details),
            core::ErrorCode::InvalidInput => Self::InvalidInput(details),
            _ => Self::Internal(details),
        }
    }
}

impl From<core::Error> for ErrorRecord {
    fn from(value: core::Error) -> Self {
        Self {
            code: value.code.into(),
            message: value.message.clone(),
            details_json: serde_json::to_string(&value).unwrap_or(value.message),
        }
    }
}

fn string_field<T: ToString>(value: core::Field<T>) -> StringField {
    match value {
        core::Field::Present { value } => StringField {
            state: FieldState::Present,
            value: Some(value.to_string()),
        },
        core::Field::Unavailable => StringField {
            state: FieldState::Unavailable,
            value: None,
        },
        core::Field::Inapplicable => StringField {
            state: FieldState::Inapplicable,
            value: None,
        },
    }
}

fn u32_field(value: core::Field<u32>) -> U32Field {
    match value {
        core::Field::Present { value } => U32Field {
            state: FieldState::Present,
            value: Some(value),
        },
        core::Field::Unavailable => U32Field {
            state: FieldState::Unavailable,
            value: None,
        },
        core::Field::Inapplicable => U32Field {
            state: FieldState::Inapplicable,
            value: None,
        },
    }
}

fn revision_field(value: core::Field<core::Revision>) -> StringField {
    match value {
        core::Field::Present { value } => {
            let mut rendered = value
                .components
                .iter()
                .map(u64::to_string)
                .collect::<Vec<_>>()
                .join(".");
            if let Some(suffix) = value.suffix {
                rendered.push('-');
                rendered.push_str(&suffix);
            }
            StringField {
                state: FieldState::Present,
                value: Some(rendered),
            }
        }
        core::Field::Unavailable => StringField {
            state: FieldState::Unavailable,
            value: None,
        },
        core::Field::Inapplicable => StringField {
            state: FieldState::Inapplicable,
            value: None,
        },
    }
}

impl From<core::Platform> for Platform {
    fn from(value: core::Platform) -> Self {
        match value {
            core::Platform::MacOs => Self::MacOs,
            core::Platform::Linux => Self::Linux,
            core::Platform::Windows => Self::Windows,
            _ => Self::Unsupported,
        }
    }
}

impl From<core::CpuArchitecture> for CpuArchitecture {
    fn from(value: core::CpuArchitecture) -> Self {
        match value {
            core::CpuArchitecture::Arm64 => Self::Arm64,
            core::CpuArchitecture::X86_64 => Self::X86_64,
            _ => Self::Other,
        }
    }
}

impl From<core::Host> for Host {
    fn from(value: core::Host) -> Self {
        Self {
            platform: value.platform.into(),
            architecture: value.architecture.into(),
            android_abi: string_field(value.android_abi),
            supported: value.supported,
        }
    }
}

impl From<core::PlatformPaths> for PlatformPaths {
    fn from(value: core::PlatformPaths) -> Self {
        Self {
            sdk_root: value.sdk_root.to_string_lossy().into_owned(),
            user_root: value.user_root.to_string_lossy().into_owned(),
            avd_root: value.avd_root.to_string_lossy().into_owned(),
            runtime_root: value.runtime_root.to_string_lossy().into_owned(),
            data_root: value.data_root.to_string_lossy().into_owned(),
        }
    }
}

impl From<core::ValueSource> for ValueSource {
    fn from(value: core::ValueSource) -> Self {
        match value {
            core::ValueSource::CallerOverride => Self::CallerOverride,
            core::ValueSource::ProcessEnvironment => Self::ProcessEnvironment,
            core::ValueSource::LoginShell => Self::LoginShell,
            core::ValueSource::AndroidCli => Self::AndroidCli,
            core::ValueSource::PlatformDefault => Self::PlatformDefault,
        }
    }
}

impl From<core::EnvironmentValue> for EnvironmentValue {
    fn from(value: core::EnvironmentValue) -> Self {
        Self {
            name: value.name,
            value: value.value,
            source: value.source.into(),
        }
    }
}

impl From<core::ToolState> for ToolState {
    fn from(value: core::ToolState) -> Self {
        match value {
            core::ToolState::Missing => Self::Missing,
            core::ToolState::Available => Self::Available,
            core::ToolState::Unavailable => Self::Unavailable,
            core::ToolState::ReportOnly => Self::ReportOnly,
        }
    }
}

impl From<core::ToolSource> for ToolSource {
    fn from(value: core::ToolSource) -> Self {
        match value {
            core::ToolSource::SearchPath => Self::SearchPath,
            core::ToolSource::SdkPackage => Self::SdkPackage,
            core::ToolSource::LegacyToolsBin => Self::LegacyToolsBin,
        }
    }
}

impl From<core::Diagnostic> for Diagnostic {
    fn from(value: core::Diagnostic) -> Self {
        Self {
            command: value.command,
            stdout: value.stdout,
            stderr: value.stderr,
            exit_status: value.exit_status,
        }
    }
}

impl From<core::CapabilityId> for CapabilityId {
    fn from(value: core::CapabilityId) -> Self {
        use core::CapabilityId as Id;
        match value {
            Id::Environment => Self::Environment,
            Id::Capabilities => Self::Capabilities,
            Id::Refresh => Self::Refresh,
            Id::PackagesListInstalled => Self::PackagesListInstalled,
            Id::PackagesListAvailable => Self::PackagesListAvailable,
            Id::PackagesInstall => Self::PackagesInstall,
            Id::PackagesRemove => Self::PackagesRemove,
            Id::PackagesUpdate => Self::PackagesUpdate,
            Id::DevicesList => Self::DevicesList,
            Id::DevicesGet => Self::DevicesGet,
            Id::DevicesProfiles => Self::DevicesProfiles,
            Id::DevicesPlanCreate => Self::DevicesPlanCreate,
            Id::DevicesPlanCreateCustomHardware => Self::DevicesPlanCreateCustomHardware,
            Id::DevicesPlanEdit => Self::DevicesPlanEdit,
            Id::DevicesPlanMove => Self::DevicesPlanMove,
            Id::DevicesPlanDuplicate => Self::DevicesPlanDuplicate,
            Id::DevicesDelete => Self::DevicesDelete,
            Id::RuntimeRunning => Self::RuntimeRunning,
            Id::RuntimeBootStatus => Self::RuntimeBootStatus,
            Id::RuntimeStart => Self::RuntimeStart,
            Id::RuntimeStartCustom => Self::RuntimeStartCustom,
            Id::RuntimeStop => Self::RuntimeStop,
            Id::RuntimeStopAll => Self::RuntimeStopAll,
            Id::SnapshotsList => Self::SnapshotsList,
            Id::SnapshotsSave => Self::SnapshotsSave,
            Id::SnapshotsLoad => Self::SnapshotsLoad,
            Id::SnapshotsDelete => Self::SnapshotsDelete,
            _ => Self::Unknown,
        }
    }
}

impl From<core::ReasonCode> for ReasonCode {
    fn from(value: core::ReasonCode) -> Self {
        match value {
            core::ReasonCode::NotImplemented => Self::NotImplemented,
            core::ReasonCode::ToolNotFound => Self::ToolNotFound,
            core::ReasonCode::ToolNotReady => Self::ToolNotReady,
            core::ReasonCode::PlatformNotSupported => Self::PlatformNotSupported,
            core::ReasonCode::CapabilityUnavailable => Self::CapabilityUnavailable,
            _ => Self::Unknown,
        }
    }
}

impl From<core::Reason> for Reason {
    fn from(value: core::Reason) -> Self {
        Self {
            code: value.code.into(),
            message: value.message,
        }
    }
}

impl From<core::CapabilityState> for CapabilityState {
    fn from(value: core::CapabilityState) -> Self {
        match value {
            core::CapabilityState::Available { implementation } => {
                Self::Available { implementation }
            }
            core::CapabilityState::Unavailable { reasons } => Self::Unavailable {
                reasons: reasons.into_iter().map(Reason::from).collect(),
            },
        }
    }
}

impl From<core::Capability> for Capability {
    fn from(value: core::Capability) -> Self {
        Self {
            id: value.id.into(),
            state: value.state.into(),
        }
    }
}

impl From<core::PackageKind> for PackageKind {
    fn from(value: core::PackageKind) -> Self {
        match value {
            core::PackageKind::SystemImage => Self::SystemImage,
            core::PackageKind::Platform => Self::Platform,
            core::PackageKind::BuildTools => Self::BuildTools,
            core::PackageKind::Emulator => Self::Emulator,
            core::PackageKind::PlatformTools => Self::PlatformTools,
            core::PackageKind::CommandLineTools => Self::CommandLineTools,
            _ => Self::Other,
        }
    }
}

impl From<PackageKind> for core::PackageKind {
    fn from(value: PackageKind) -> Self {
        match value {
            PackageKind::SystemImage => Self::SystemImage,
            PackageKind::Platform => Self::Platform,
            PackageKind::BuildTools => Self::BuildTools,
            PackageKind::Emulator => Self::Emulator,
            PackageKind::PlatformTools => Self::PlatformTools,
            PackageKind::CommandLineTools => Self::CommandLineTools,
            PackageKind::Other => Self::Other,
        }
    }
}

impl From<core::PackageId> for PackageId {
    fn from(value: core::PackageId) -> Self {
        Self {
            kind: value.kind.into(),
            api: value.api,
            tag: value.tag,
            abi: value.abi,
            qualifier: value.qualifier,
        }
    }
}

impl From<PackageId> for core::PackageId {
    fn from(value: PackageId) -> Self {
        Self {
            kind: value.kind.into(),
            api: value.api,
            tag: value.tag,
            abi: value.abi,
            qualifier: value.qualifier,
        }
    }
}

impl From<core::Package> for Package {
    fn from(value: core::Package) -> Self {
        Self {
            id: value.id.into(),
            revision: revision_field(value.revision),
            installed: value.installed,
        }
    }
}

impl From<core::ToolStatus> for ToolStatus {
    fn from(value: core::ToolStatus) -> Self {
        Self {
            name: value.name,
            path: value.path.map(|path| path.to_string_lossy().into_owned()),
            version: revision_field(value.version),
            state: value.state.into(),
            source: value.source.map(ToolSource::from),
            package: value.package.map(Package::from),
            reasons: value.reasons.into_iter().map(Reason::from).collect(),
            diagnostic: value.diagnostic.map(Diagnostic::from),
        }
    }
}

impl From<core::EnvironmentSnapshot> for EnvironmentSnapshot {
    fn from(value: core::EnvironmentSnapshot) -> Self {
        let diagnostics_json =
            serde_json::to_string(&value.diagnostics).unwrap_or_else(|_| "[]".into());
        Self {
            host: value.host.into(),
            paths: value.paths.into(),
            sdk_root_source: value.sdk_root_source.into(),
            environment: value
                .environment
                .into_iter()
                .map(EnvironmentValue::from)
                .collect(),
            tools: value.tools.into_iter().map(ToolStatus::from).collect(),
            legacy_tools: value
                .legacy_tools
                .into_iter()
                .map(ToolStatus::from)
                .collect(),
            installed_packages: value
                .installed_packages
                .into_iter()
                .map(Package::from)
                .collect(),
            diagnostics_json,
        }
    }
}

impl From<core::EnvironmentReport> for EnvironmentReport {
    fn from(value: core::EnvironmentReport) -> Self {
        Self {
            snapshot: value.snapshot.into(),
            capabilities: value
                .capabilities
                .capabilities
                .into_iter()
                .map(Capability::from)
                .collect(),
        }
    }
}

impl From<core::Device> for Device {
    fn from(value: core::Device) -> Self {
        Self {
            id: value.id.to_string(),
            display_name: string_field(value.display_name),
            profile: string_field(value.profile),
            image: match value.image {
                core::Field::Present { value } => StringField {
                    state: FieldState::Present,
                    value: Some(value.render_semicolon()),
                },
                core::Field::Unavailable => StringField {
                    state: FieldState::Unavailable,
                    value: None,
                },
                core::Field::Inapplicable => StringField {
                    state: FieldState::Inapplicable,
                    value: None,
                },
            },
            target: string_field(value.target),
        }
    }
}

impl From<core::Profile> for Profile {
    fn from(value: core::Profile) -> Self {
        Self {
            id: value.id.to_string(),
            display_name: string_field(value.display_name),
        }
    }
}

impl From<core::RunningInstance> for RunningInstance {
    fn from(value: core::RunningInstance) -> Self {
        Self {
            id: value.id.to_string(),
            serial: string_field(value.serial),
            pid: u32_field(value.pid),
        }
    }
}

impl From<core::BootStatus> for BootStatus {
    fn from(value: core::BootStatus) -> Self {
        match value {
            core::BootStatus::Offline => Self::Offline,
            core::BootStatus::Booting => Self::Booting,
            core::BootStatus::Ready => Self::Ready,
            core::BootStatus::Stuck => Self::Stuck,
            _ => Self::Unknown,
        }
    }
}

impl TryFrom<CreateDeviceDraft> for core::CreateDeviceDraft {
    type Error = BindingError;

    fn try_from(value: CreateDeviceDraft) -> Result<Self, Self::Error> {
        Ok(Self {
            id: core::AvdId::new(value.id).map_err(core::Error::from)?,
            profile: core::ProfileId::new(value.profile).map_err(core::Error::from)?,
            image: value.image.into(),
            display_name: value.display_name,
            hardware: core::HardwareConfig {
                ram_mib: value.hardware.ram_mib,
                cpu_count: value.hardware.cpu_count,
                screen_width: value.hardware.screen_width,
                screen_height: value.hardware.screen_height,
            },
        })
    }
}

impl From<StartOptions> for core::StartOptions {
    fn from(value: StartOptions) -> Self {
        Self {
            cold: value.cold,
            wipe_data: value.wipe_data,
            headless: value.headless,
            multi_instance: value.multi_instance,
        }
    }
}

impl From<core::PlanKind> for PlanKind {
    fn from(value: core::PlanKind) -> Self {
        match value {
            core::PlanKind::CreateDevice => Self::CreateDevice,
            _ => Self::Unknown,
        }
    }
}

impl From<core::PlanStepKind> for PlanStepKind {
    fn from(value: core::PlanStepKind) -> Self {
        match value {
            core::PlanStepKind::ToolCall => Self::ToolCall,
            core::PlanStepKind::FileRewrite => Self::FileRewrite,
            core::PlanStepKind::CallerCustom => Self::CallerCustom,
            _ => Self::Unknown,
        }
    }
}

impl From<core::Plan> for Plan {
    fn from(value: core::Plan) -> Self {
        let json = serde_json::to_string(&value).expect("public plan must serialize");
        Self {
            id: value.id,
            kind: value.kind.into(),
            steps: value
                .steps
                .into_iter()
                .map(|step| PlanStep {
                    id: step.id,
                    kind: step.kind.into(),
                    description: step.description,
                    compensation: step
                        .compensation
                        .map(|compensation| compensation.description),
                })
                .collect(),
            json,
        }
    }
}

impl From<core::LogStream> for LogStream {
    fn from(value: core::LogStream) -> Self {
        match value {
            core::LogStream::Stdout => Self::Stdout,
            core::LogStream::Stderr => Self::Stderr,
        }
    }
}

impl From<core::Event> for Event {
    fn from(value: core::Event) -> Self {
        match value {
            core::Event::StepStarted { step, message } => Self::StepStarted { step, message },
            core::Event::StepFinished { step, message } => Self::StepFinished { step, message },
            core::Event::CompensationStarted { step, message } => {
                Self::CompensationStarted { step, message }
            }
            core::Event::CompensationFinished {
                step,
                succeeded,
                message,
            } => Self::CompensationFinished {
                step,
                succeeded,
                message,
            },
            core::Event::Progress { ratio } => Self::Progress { ratio },
            core::Event::Log { stream, line } => Self::Log {
                stream: stream.into(),
                line,
            },
            core::Event::Warning { message } => Self::Warning { message },
            _ => Self::Unknown {
                json: serde_json::to_string(&value).expect("public events must serialize"),
            },
        }
    }
}

impl From<core::OperationResult> for OperationResult {
    fn from(value: core::OperationResult) -> Self {
        match value {
            core::OperationResult::PackageInstalled { package } => Self::PackageInstalled {
                package: package.into(),
            },
            core::OperationResult::PackageRemoved { package } => Self::PackageRemoved {
                package: package.into(),
            },
            core::OperationResult::DeviceDeleted { id } => {
                Self::DeviceDeleted { id: id.to_string() }
            }
            core::OperationResult::DeviceStarted { instance } => Self::DeviceStarted {
                instance: instance.into(),
            },
            core::OperationResult::DeviceStopped { id } => {
                Self::DeviceStopped { id: id.to_string() }
            }
            core::OperationResult::PlanCompleted { plan_id } => Self::PlanCompleted { plan_id },
            _ => Self::Unknown {
                json: serde_json::to_string(&value).expect("public results must serialize"),
            },
        }
    }
}

impl From<core::LicenseAcceptance> for LicenseAcceptance {
    fn from(value: core::LicenseAcceptance) -> Self {
        match value {
            core::LicenseAcceptance::Never => Self::Never,
            core::LicenseAcceptance::Allow => Self::Allow,
        }
    }
}

impl From<LicenseAcceptance> for core::LicenseAcceptance {
    fn from(value: LicenseAcceptance) -> Self {
        match value {
            LicenseAcceptance::Never => Self::Never,
            LicenseAcceptance::Allow => Self::Allow,
        }
    }
}

impl From<core::AndroidCliMetrics> for AndroidCliMetrics {
    fn from(value: core::AndroidCliMetrics) -> Self {
        match value {
            core::AndroidCliMetrics::Inherit => Self::Inherit,
            core::AndroidCliMetrics::NoMetrics => Self::NoMetrics,
        }
    }
}

impl From<AndroidCliMetrics> for core::AndroidCliMetrics {
    fn from(value: AndroidCliMetrics) -> Self {
        match value {
            AndroidCliMetrics::Inherit => Self::Inherit,
            AndroidCliMetrics::NoMetrics => Self::NoMetrics,
        }
    }
}

impl From<core::KitConfig> for KitConfig {
    fn from(value: core::KitConfig) -> Self {
        Self {
            sdk_root: value
                .sdk_root
                .map(|path| path.to_string_lossy().into_owned()),
            policy: Policy {
                implicit_install: value.policy.implicit_install,
                explicit_install: value.policy.explicit_install,
                license_acceptance: value.policy.license_acceptance.into(),
                file_operations: value.policy.file_operations,
                android_cli_metrics: value.policy.android_cli_metrics.into(),
            },
            tool_preference: value.tool_preference.entries,
            command_timeout_seconds: value.timeouts.command_seconds,
            backup_dir: value
                .backup_dir
                .map(|path| path.to_string_lossy().into_owned()),
        }
    }
}

impl TryFrom<KitConfig> for core::KitConfig {
    type Error = BindingError;

    fn try_from(value: KitConfig) -> Result<Self, Self::Error> {
        Ok(Self {
            sdk_root: value.sdk_root.map(PathBuf::from),
            policy: core::Policy {
                implicit_install: value.policy.implicit_install,
                explicit_install: value.policy.explicit_install,
                license_acceptance: value.policy.license_acceptance.into(),
                file_operations: value.policy.file_operations,
                android_cli_metrics: value.policy.android_cli_metrics.into(),
            },
            tool_preference: core::ToolPreference {
                entries: value.tool_preference,
            },
            timeouts: core::Timeouts {
                command_seconds: value.command_timeout_seconds,
            },
            backup_dir: value.backup_dir.map(PathBuf::from),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_send_sync<T: Send + Sync>() {}

    #[test]
    fn binding_objects_are_thread_safe() {
        assert_send_sync::<Kit>();
        assert_send_sync::<Operation>();
    }

    #[test]
    fn default_configuration_round_trips() {
        let ffi = default_kit_config();
        let core: core::KitConfig = ffi.try_into().unwrap();
        assert_eq!(core, core::KitConfig::default());
    }

    #[test]
    fn every_current_capability_has_a_foreign_case() {
        for id in core::CapabilityId::ALL {
            assert!(!matches!(CapabilityId::from(*id), CapabilityId::Unknown));
        }
    }

    #[tokio::test]
    async fn operation_exports_events_cancellation_and_result() {
        let operation = Operation::new(core::Operation::not_implemented("ffi probe"));
        assert!(operation.next_event().await.is_none());
        let error = operation.result().await.unwrap_err();
        assert!(matches!(error, BindingError::CapabilityUnavailable(_)));
        operation.cancel();
    }
}
