//! Public Rust facade.

#![allow(clippy::result_large_err)]
#![deny(missing_docs)]

mod executor;
mod operation;
mod planner;
mod routing;

use std::sync::{Arc, RwLock};

pub use model::{
    environment_prefix, test_avd_prefix, AndroidCliMetrics, AvdId, BootStatus, Capability,
    CapabilityId, CapabilityMatrix, CapabilityState, Compensation, CompensationResult,
    CpuArchitecture, CreateDeviceDraft, Description, Device, Diagnostic, Envelope,
    EnvironmentDiagnostic, EnvironmentDiagnosticCode, EnvironmentReport, EnvironmentSnapshot,
    EnvironmentValue, Error, ErrorCode, Event, Field, HardwareConfig, Host, KitConfig,
    LicenseAcceptance, LogStream, ModelError, OperationResult, Package, PackageId, PackageKind,
    Plan, PlanIntent, PlanKind, PlanStep, PlanStepKind, Platform, PlatformPaths, Policy, Profile,
    ProfileId, Reason, ReasonCode, Remedy, RemedyKind, Revision, RunningInstance, Serial,
    StartOptions, Timeouts, ToolNames, ToolPreference, ToolSource, ToolState, ToolStatus,
    ValueSource, PRODUCT_NAME, SCHEMA_VERSION,
};
pub use operation::Operation;
use routing::{Implementation, Router};

/// Thread-safe entry point for environment queries and AVD operations.
#[derive(Debug, Clone)]
pub struct Kit {
    config: KitConfig,
    report: Arc<RwLock<EnvironmentReport>>,
}

impl Kit {
    /// Constructs a facade and probes the environment on the current thread.
    ///
    /// Async callers should prefer [`Kit::new_async`] so filesystem discovery
    /// runs on the runtime's blocking pool.
    pub fn new(config: KitConfig) -> Result<Self, Error> {
        config.validate()?;
        let snapshot = probe_environment_sync(config.sdk_root.clone())?;
        let report = report_from_snapshot(snapshot);
        Ok(Self {
            config,
            report: Arc::new(RwLock::new(report)),
        })
    }

    /// Constructs a facade without blocking an async worker on discovery I/O.
    pub async fn new_async(config: KitConfig) -> Result<Self, Error> {
        config.validate()?;
        let snapshot = ::environment::discovery::probe(config.sdk_root.clone()).await?;
        let report = report_from_snapshot(snapshot);
        Ok(Self {
            config,
            report: Arc::new(RwLock::new(report)),
        })
    }

    /// Returns the validated configuration.
    pub fn config(&self) -> &KitConfig {
        &self.config
    }

    /// Returns the cached environment report.
    pub async fn environment(&self) -> Result<EnvironmentReport, Error> {
        self.read_report()
    }

    /// Returns the cached capability entries.
    pub async fn capabilities(&self) -> Result<Vec<Capability>, Error> {
        Ok(self.read_report()?.capabilities.capabilities)
    }

    /// Re-runs environment discovery on the async runtime's blocking pool.
    pub async fn refresh(&self) -> Result<EnvironmentReport, Error> {
        let snapshot = ::environment::discovery::probe(self.config.sdk_root.clone()).await?;
        let report = report_from_snapshot(snapshot);
        *self.write_report()? = report;
        self.read_report()
    }

    /// Lists installed SDK packages.
    pub async fn list_installed(&self) -> Result<Vec<Package>, Error> {
        self.require_capability(CapabilityId::PackagesListInstalled)?;
        let report = self.read_report()?;
        debug_assert_eq!(
            Router.selected(CapabilityId::PackagesListInstalled, &report.snapshot)?,
            Implementation::InstalledSdkFiles
        );
        Ok(report.snapshot.installed_packages)
    }

    /// Lists SDK packages available for installation.
    pub async fn list_available(&self) -> Result<Vec<Package>, Error> {
        self.require_capability(CapabilityId::PackagesListAvailable)?;
        Ok(Vec::new())
    }

    /// Starts an SDK package installation operation.
    pub fn install(&self, _package: PackageId) -> Operation {
        match self.require_capability(CapabilityId::PackagesInstall) {
            Ok(()) => Operation::not_implemented("packages.install"),
            Err(error) => Operation::failed(error),
        }
    }

    /// Starts an SDK package removal operation.
    pub fn remove(&self, _package: PackageId) -> Operation {
        match self.require_capability(CapabilityId::PackagesRemove) {
            Ok(()) => Operation::not_implemented("packages.remove"),
            Err(error) => Operation::failed(error),
        }
    }

    /// Scans AVD metadata files on the async runtime's blocking pool.
    pub async fn list_devices(&self) -> Result<Vec<Device>, Error> {
        self.require_capability(CapabilityId::DevicesList)?;
        let report = self.read_report()?;
        debug_assert_eq!(
            Router.selected(CapabilityId::DevicesList, &report.snapshot)?,
            Implementation::AvdFiles
        );
        tokio::task::spawn_blocking(move || {
            avdfs::AvdStore::new(
                report.snapshot.paths.avd_root,
                report.snapshot.paths.user_root,
            )
            .list()
            .map(|devices| devices.into_iter().map(device_from_metadata).collect())
        })
        .await
        .map_err(blocking_task_error)?
    }

    /// Reads one AVD's metadata files on the async runtime's blocking pool.
    pub async fn get_device(&self, id: &AvdId) -> Result<Device, Error> {
        self.require_capability(CapabilityId::DevicesGet)?;
        let report = self.read_report()?;
        debug_assert_eq!(
            Router.selected(CapabilityId::DevicesGet, &report.snapshot)?,
            Implementation::AvdFiles
        );
        let id = id.clone();
        tokio::task::spawn_blocking(move || {
            avdfs::AvdStore::new(
                report.snapshot.paths.avd_root,
                report.snapshot.paths.user_root,
            )
            .get(&id)
            .map(device_from_metadata)
        })
        .await
        .map_err(blocking_task_error)?
    }

    /// Queries the legacy Android CLI for its preset device profiles.
    pub async fn profiles(&self) -> Result<Vec<Profile>, Error> {
        self.require_capability(CapabilityId::DevicesProfiles)?;
        let report = self.read_report()?;
        debug_assert_eq!(
            Router.selected(CapabilityId::DevicesProfiles, &report.snapshot)?,
            Implementation::AndroidCli
        );
        let executable = report
            .snapshot
            .tools
            .iter()
            .find(|tool| tool.name == report.snapshot.tool_names.android)
            .filter(|tool| tool.state == ToolState::Available)
            .and_then(|tool| tool.path.clone())
            .ok_or_else(|| Error::new(ErrorCode::ToolNotFound, "Android CLI was not found"))?;
        let environment = drivers::ToolEnvironment::from(&report.snapshot.paths);
        let invocation = drivers::Invocation::android(
            executable,
            &report.snapshot.paths.sdk_root,
            self.config.policy.android_cli_metrics,
            ["emulator".into(), "create".into(), "--list-profiles".into()],
        )
        .with_environment(&environment);
        let output = drivers::execute(&process::Runner::default(), invocation).await?;
        Ok(profiles_from_ids(drivers::android::parse_profiles(
            &output,
        )?))
    }

    /// Compiles a serializable plan for creating an AVD.
    pub fn plan_create(&self, draft: CreateDeviceDraft) -> Result<Plan, Error> {
        if !draft.hardware.is_default() {
            return Err(Error::not_implemented("custom hardware"));
        }
        self.require_capability(CapabilityId::DevicesPlanCreate)?;
        debug_assert_eq!(
            Router.plan_tool(PlanKind::CreateDevice),
            Implementation::AndroidCli
        );
        Ok(planner::compile_create(&draft))
    }

    /// Starts execution of a previously compiled plan.
    pub fn execute_plan(&self, plan: Plan) -> Operation {
        if let Err(error) = self.require_capability(CapabilityId::DevicesPlanCreate) {
            return Operation::failed(error);
        }
        let draft = match planner::create_draft(&plan) {
            Ok(draft) => draft,
            Err(error) => return Operation::failed(error),
        };
        let snapshot = match self.read_report() {
            Ok(report) => report.snapshot,
            Err(error) => return Operation::failed(error),
        };
        let metrics = self.config.policy.android_cli_metrics;
        Operation::spawn(move |operation_id, events, cancellation| {
            Box::pin(executor::execute_create(
                operation_id,
                draft,
                snapshot,
                metrics,
                events,
                cancellation,
            ))
        })
    }

    /// Starts an operation that deletes an AVD.
    pub fn delete_device(&self, _id: AvdId) -> Operation {
        match self.require_capability(CapabilityId::DevicesDelete) {
            Ok(()) => Operation::not_implemented("devices.delete"),
            Err(error) => Operation::failed(error),
        }
    }

    /// Lists discovered running emulator instances.
    pub async fn running(&self) -> Result<Vec<RunningInstance>, Error> {
        self.require_capability(CapabilityId::RuntimeRunning)?;
        Ok(Vec::new())
    }

    /// Returns the current boot state of an AVD.
    pub async fn boot_status(&self, _id: &AvdId) -> Result<BootStatus, Error> {
        self.require_capability(CapabilityId::RuntimeBootStatus)?;
        Ok(BootStatus::Offline)
    }

    /// Starts an emulator operation.
    pub fn start(&self, _id: AvdId, options: StartOptions) -> Operation {
        if !options.is_default() {
            return Operation::not_implemented("custom start options");
        }
        match self.require_capability(CapabilityId::RuntimeStart) {
            Ok(()) => Operation::not_implemented("runtime.start"),
            Err(error) => Operation::failed(error),
        }
    }

    /// Starts an operation that stops an emulator.
    pub fn stop(&self, _id: AvdId) -> Operation {
        match self.require_capability(CapabilityId::RuntimeStop) {
            Ok(()) => Operation::not_implemented("runtime.stop"),
            Err(error) => Operation::failed(error),
        }
    }

    fn read_report(&self) -> Result<EnvironmentReport, Error> {
        self.report
            .read()
            .map(|report| report.clone())
            .map_err(|_| Error::new(ErrorCode::Internal, "environment lock is poisoned"))
    }

    fn write_report(&self) -> Result<std::sync::RwLockWriteGuard<'_, EnvironmentReport>, Error> {
        self.report
            .write()
            .map_err(|_| Error::new(ErrorCode::Internal, "environment lock is poisoned"))
    }

    fn require_capability(&self, id: CapabilityId) -> Result<(), Error> {
        let report = self.read_report()?;
        match report
            .capabilities
            .capability(id)
            .map(|capability| &capability.state)
        {
            Some(CapabilityState::Available { .. }) => Ok(()),
            Some(CapabilityState::Unavailable { reasons }) => Err(Error::capability_unavailable(
                format!("{} is unavailable", id.as_str()),
                reasons.clone(),
            )),
            None => Err(Error::not_implemented(id.as_str())),
        }
    }
}

fn report_from_snapshot(snapshot: EnvironmentSnapshot) -> EnvironmentReport {
    EnvironmentReport {
        capabilities: Router.matrix(&snapshot),
        snapshot,
    }
}

fn blocking_task_error(error: tokio::task::JoinError) -> Error {
    Error::new(
        ErrorCode::Internal,
        format!("blocking task failed: {error}"),
    )
}

fn probe_environment_sync(
    sdk_root: Option<std::path::PathBuf>,
) -> Result<EnvironmentSnapshot, Error> {
    let probe = move || {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| {
                Error::new(
                    ErrorCode::Internal,
                    format!("create environment probe runtime: {error}"),
                )
            })?
            .block_on(::environment::discovery::probe(sdk_root))
    };
    if tokio::runtime::Handle::try_current().is_ok() {
        std::thread::spawn(probe)
            .join()
            .map_err(|_| Error::new(ErrorCode::Internal, "environment probe thread panicked"))?
    } else {
        probe()
    }
}

fn profiles_from_ids(ids: Vec<ProfileId>) -> Vec<Profile> {
    ids.into_iter()
        .map(|id| Profile {
            id,
            display_name: Field::Unavailable,
        })
        .collect()
}

fn device_from_metadata(metadata: avdfs::AvdMetadata) -> Device {
    Device {
        id: metadata.id,
        display_name: metadata
            .display_name
            .map_or(Field::Unavailable, Field::present),
        profile: metadata.profile.map_or(Field::Unavailable, Field::present),
        image: metadata.image.map_or(Field::Unavailable, Field::present),
        target: metadata.target.map_or(Field::Unavailable, Field::present),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use model::{HardwareConfig, PackageKind, ProfileId};

    fn assert_send_sync<T: Send + Sync>() {}

    fn kit() -> Kit {
        Kit::new(KitConfig::default()).unwrap()
    }

    fn draft() -> CreateDeviceDraft {
        CreateDeviceDraft {
            id: AvdId::new("phone_one").unwrap(),
            profile: ProfileId::new("medium_phone").unwrap(),
            image: PackageId {
                kind: PackageKind::SystemImage,
                api: Some("36".into()),
                tag: Some("google_apis".into()),
                abi: Some("arm64-v8a".into()),
                qualifier: None,
            },
            display_name: Some("Phone One".into()),
            hardware: HardwareConfig::default(),
        }
    }

    #[test]
    fn kit_is_send_sync() {
        assert_send_sync::<Kit>();
    }

    #[test]
    fn rejects_non_default_configuration() {
        let mut config = KitConfig::default();
        config.policy.implicit_install = true;
        let error = Kit::new(config).unwrap_err();
        assert_eq!(error.code, ErrorCode::CapabilityUnavailable);
        assert_eq!(error.reasons[0].code, ReasonCode::NotImplemented);
    }

    #[test]
    fn plan_create_compiles_a_serializable_plan() {
        let plan = kit().plan_create(draft()).unwrap();
        assert_eq!(plan.id, "create:phone_one");
        assert_eq!(plan.steps.len(), 4);
        let encoded = serde_json::to_value(&plan).unwrap();
        assert_eq!(encoded["kind"], "create_device");
        assert!(encoded["steps"].as_array().unwrap()[0]["compensation"].is_object());
    }

    #[test]
    fn plan_create_rejects_custom_hardware() {
        let mut draft = draft();
        draft.hardware.ram_mib = Some(4096);
        let error = kit().plan_create(draft).unwrap_err();
        assert_eq!(error.code, ErrorCode::CapabilityUnavailable);
        assert_eq!(error.reasons[0].code, ReasonCode::NotImplemented);
    }

    #[tokio::test]
    async fn execute_plan_rejects_a_plan_changed_after_compilation() {
        let kit = kit();
        let mut plan = kit.plan_create(draft()).unwrap();
        plan.steps[0].description = "different action".into();
        let operation = kit.execute_plan(plan);
        let error = operation.result().await.unwrap_err();
        assert_eq!(error.code, ErrorCode::InvalidInput);
    }

    #[test]
    fn profile_without_a_tool_display_name_is_explicitly_unavailable() {
        let profiles = profiles_from_ids(vec![ProfileId::new("medium_phone").unwrap()]);
        assert_eq!(profiles[0].id.as_str(), "medium_phone");
        assert!(profiles[0].display_name.as_value().is_none());
    }

    #[test]
    fn avd_metadata_maps_missing_values_to_unavailable_fields() {
        let device = device_from_metadata(avdfs::AvdMetadata {
            id: AvdId::new("phone").unwrap(),
            directory: "/unused".into(),
            display_name: Some("Phone".into()),
            profile: None,
            image: None,
            target: Some("android-36".into()),
        });
        assert_eq!(
            device.display_name.as_value().map(String::as_str),
            Some("Phone")
        );
        assert!(device.profile.as_value().is_none());
        assert!(device.image.as_value().is_none());
        assert_eq!(
            device.target.as_value().map(String::as_str),
            Some("android-36")
        );
    }

    #[tokio::test]
    async fn unavailable_operation_completes() {
        let mut operation = kit().install(draft().image);
        let event = operation.next_event().await;
        assert!(matches!(event, Some(Event::Warning { .. })));
        assert!(operation.next_event().await.is_none());
        let error = operation.result().await.unwrap_err();
        assert_eq!(error.code, ErrorCode::CapabilityUnavailable);
    }

    #[tokio::test]
    async fn operation_can_be_cancelled() {
        let operation = Operation::cancellable();
        operation.cancel();
        let error = operation.result().await.unwrap_err();
        assert_eq!(error.code, ErrorCode::Cancelled);
    }

    #[tokio::test]
    async fn environment_queries_are_available() {
        let kit = kit();
        let report = kit.environment().await.unwrap();
        let capabilities = kit.capabilities().await.unwrap();
        assert_eq!(capabilities.len(), report.capabilities.capabilities.len());
        assert!(CapabilityId::ALL
            .iter()
            .all(|id| report.capabilities.capability(*id).is_some()));
        kit.refresh().await.unwrap();
    }
}
