//! Public Rust facade.

#![allow(clippy::result_large_err)]

mod operation;
mod planner;

use std::sync::{Arc, RwLock};

pub use model::{
    environment_prefix, test_avd_prefix, AndroidCliMetrics, AvdId, BootStatus, Capability,
    CapabilityId, CapabilityMatrix, CapabilityState, Compensation, CpuArchitecture,
    CreateDeviceDraft, Description, Device, Diagnostic, Envelope, EnvironmentReport,
    EnvironmentSnapshot, Error, ErrorCode, Event, Field, HardwareConfig, Host, KitConfig,
    LicenseAcceptance, LogStream, ModelError, OperationResult, Package, PackageId, PackageKind,
    Plan, PlanKind, PlanStep, PlanStepKind, Platform, PlatformPaths, Policy, Profile, ProfileId,
    Reason, ReasonCode, Remedy, RemedyKind, Revision, RunningInstance, Serial, StartOptions,
    Timeouts, ToolNames, ToolPreference, ToolStatus, PRODUCT_NAME, SCHEMA_VERSION,
};
pub use operation::Operation;

#[derive(Debug, Clone)]
pub struct Kit {
    config: KitConfig,
    report: Arc<RwLock<EnvironmentReport>>,
}

impl Kit {
    pub fn new(config: KitConfig) -> Result<Self, Error> {
        config.validate()?;
        let report = ::environment::probe_with_sdk_root(config.sdk_root.as_deref());
        Ok(Self {
            config,
            report: Arc::new(RwLock::new(report)),
        })
    }

    pub fn config(&self) -> &KitConfig {
        &self.config
    }

    pub async fn environment(&self) -> Result<EnvironmentReport, Error> {
        self.read_report()
    }

    pub async fn capabilities(&self) -> Result<Vec<Capability>, Error> {
        Ok(self.read_report()?.capabilities.capabilities)
    }

    pub async fn refresh(&self) -> Result<EnvironmentReport, Error> {
        let report = ::environment::probe_with_sdk_root(self.config.sdk_root.as_deref());
        *self.write_report()? = report;
        self.read_report()
    }

    pub async fn list_installed(&self) -> Result<Vec<Package>, Error> {
        self.require_capability(CapabilityId::PackagesListInstalled)?;
        Ok(Vec::new())
    }

    pub async fn list_available(&self) -> Result<Vec<Package>, Error> {
        self.require_capability(CapabilityId::PackagesListAvailable)?;
        Ok(Vec::new())
    }

    pub fn install(&self, _package: PackageId) -> Operation {
        match self.require_capability(CapabilityId::PackagesInstall) {
            Ok(()) => Operation::not_implemented("packages.install"),
            Err(error) => Operation::failed(error),
        }
    }

    pub fn remove(&self, _package: PackageId) -> Operation {
        match self.require_capability(CapabilityId::PackagesRemove) {
            Ok(()) => Operation::not_implemented("packages.remove"),
            Err(error) => Operation::failed(error),
        }
    }

    pub async fn list_devices(&self) -> Result<Vec<Device>, Error> {
        self.require_capability(CapabilityId::DevicesList)?;
        Ok(Vec::new())
    }

    pub async fn get_device(&self, _id: &AvdId) -> Result<Device, Error> {
        self.require_capability(CapabilityId::DevicesGet)
            .and(Err(Error::not_implemented("devices.get")))
    }

    pub async fn profiles(&self) -> Result<Vec<Profile>, Error> {
        self.require_capability(CapabilityId::DevicesProfiles)?;
        let report = self.read_report()?;
        let executable = report
            .snapshot
            .tools
            .iter()
            .find(|tool| tool.name == report.snapshot.tool_names.android)
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

    pub fn plan_create(&self, draft: CreateDeviceDraft) -> Result<Plan, Error> {
        if !draft.hardware.is_default() {
            return Err(Error::not_implemented("custom hardware"));
        }
        self.require_capability(CapabilityId::DevicesPlanCreate)?;
        Ok(planner::compile_create(&draft))
    }

    pub fn execute_plan(&self, plan: Plan) -> Operation {
        Operation::not_implemented(format!("execute plan {}", plan.id))
    }

    pub fn delete_device(&self, _id: AvdId) -> Operation {
        match self.require_capability(CapabilityId::DevicesDelete) {
            Ok(()) => Operation::not_implemented("devices.delete"),
            Err(error) => Operation::failed(error),
        }
    }

    pub async fn running(&self) -> Result<Vec<RunningInstance>, Error> {
        self.require_capability(CapabilityId::RuntimeRunning)?;
        Ok(Vec::new())
    }

    pub async fn boot_status(&self, _id: &AvdId) -> Result<BootStatus, Error> {
        self.require_capability(CapabilityId::RuntimeBootStatus)?;
        Ok(BootStatus::Offline)
    }

    pub fn start(&self, _id: AvdId, options: StartOptions) -> Operation {
        if !options.is_default() {
            return Operation::not_implemented("custom start options");
        }
        match self.require_capability(CapabilityId::RuntimeStart) {
            Ok(()) => Operation::not_implemented("runtime.start"),
            Err(error) => Operation::failed(error),
        }
    }

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

fn profiles_from_ids(ids: Vec<ProfileId>) -> Vec<Profile> {
    ids.into_iter()
        .map(|id| Profile {
            id,
            display_name: Field::Unavailable,
        })
        .collect()
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

    #[test]
    fn profile_without_a_tool_display_name_is_explicitly_unavailable() {
        let profiles = profiles_from_ids(vec![ProfileId::new("medium_phone").unwrap()]);
        assert_eq!(profiles[0].id.as_str(), "medium_phone");
        assert!(profiles[0].display_name.as_value().is_none());
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
