use model::{
    Capability, CapabilityId, CapabilityMatrix, CapabilityState, EnvironmentSnapshot, Error,
    Reason, ReasonCode, ToolState,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Implementation {
    PlatformProbe,
    AvdFiles,
    AndroidCli,
    InstalledSdkFiles,
    RuntimeDiscovery,
    EmulatorBinary,
    AdbEmuKill,
}

impl Implementation {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::PlatformProbe => "platform_probe",
            Self::AvdFiles => "avd_files",
            Self::AndroidCli => "android_cli",
            Self::InstalledSdkFiles => "installed_sdk_files",
            Self::RuntimeDiscovery => "runtime_discovery",
            Self::EmulatorBinary => "emulator_binary",
            Self::AdbEmuKill => "adb_emu_kill",
        }
    }
}

/// A static condition that must hold before an implementation can run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Requirement {
    AndroidCli,
    Emulator,
    Adb,
    /// The Android CLI ignores a separate `ANDROID_AVD_HOME`, so AVDs it
    /// creates or removes must live under the Android user directory.
    AndroidCliAvdLayout,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Candidate {
    pub(crate) implementation: Implementation,
    pub(crate) requirements: &'static [Requirement],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Route {
    pub(crate) candidates: Vec<Candidate>,
    pub(crate) implemented: bool,
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Router;

impl Router {
    pub(crate) fn route(self, id: CapabilityId) -> Route {
        use CapabilityId as Id;
        use Implementation as Impl;
        use Requirement as Req;

        let candidate = |implementation, requirements| Candidate {
            implementation,
            requirements,
        };
        let (candidates, implemented) = match id {
            Id::Environment | Id::Capabilities | Id::Refresh => {
                (vec![candidate(Impl::PlatformProbe, &[])], true)
            }
            Id::PackagesListInstalled => (vec![candidate(Impl::InstalledSdkFiles, &[])], true),
            Id::PackagesListAvailable
            | Id::PackagesInstall
            | Id::PackagesRemove
            | Id::PackagesUpdate => (vec![candidate(Impl::AndroidCli, &[Req::AndroidCli])], false),
            Id::DevicesList | Id::DevicesGet => (vec![candidate(Impl::AvdFiles, &[])], true),
            Id::DevicesProfiles => (vec![candidate(Impl::AndroidCli, &[Req::AndroidCli])], true),
            Id::DevicesPlanCreate => (
                vec![candidate(
                    Impl::AndroidCli,
                    &[
                        Req::AndroidCli,
                        Req::Emulator,
                        Req::Adb,
                        Req::AndroidCliAvdLayout,
                    ],
                )],
                true,
            ),
            Id::DevicesDelete => (
                vec![candidate(
                    Impl::AndroidCli,
                    &[Req::AndroidCli, Req::Adb, Req::AndroidCliAvdLayout],
                )],
                true,
            ),
            Id::RuntimeRunning | Id::RuntimeBootStatus => {
                (vec![candidate(Impl::RuntimeDiscovery, &[Req::Adb])], true)
            }
            Id::RuntimeStart => (
                vec![candidate(Impl::EmulatorBinary, &[Req::Emulator, Req::Adb])],
                true,
            ),
            Id::RuntimeStartCustom => (
                vec![candidate(Impl::EmulatorBinary, &[Req::Emulator, Req::Adb])],
                false,
            ),
            Id::RuntimeStop => (
                vec![
                    candidate(Impl::AdbEmuKill, &[Req::Adb]),
                    candidate(Impl::AndroidCli, &[Req::AndroidCli]),
                ],
                true,
            ),
            _ => (Vec::new(), false),
        };
        Route {
            candidates,
            implemented,
        }
    }

    pub(crate) fn matrix(self, snapshot: &EnvironmentSnapshot) -> CapabilityMatrix {
        CapabilityMatrix {
            capabilities: CapabilityId::ALL
                .iter()
                .copied()
                .map(|id| Capability {
                    id,
                    state: self.state(id, snapshot),
                })
                .collect(),
        }
    }

    pub(crate) fn selected(
        self,
        id: CapabilityId,
        snapshot: &EnvironmentSnapshot,
    ) -> Result<Implementation, Error> {
        self.route(id)
            .candidates
            .iter()
            .find(|candidate| is_available(candidate, snapshot))
            .map(|candidate| candidate.implementation)
            .ok_or_else(|| Error::not_implemented(id.as_str()))
    }

    fn state(self, id: CapabilityId, snapshot: &EnvironmentSnapshot) -> CapabilityState {
        let route = self.route(id);
        let selected = route
            .candidates
            .iter()
            .find(|candidate| is_available(candidate, snapshot));
        if let (true, Some(candidate)) = (route.implemented, selected) {
            return CapabilityState::Available {
                implementation: candidate.implementation.as_str().into(),
            };
        }

        let mut reasons = Vec::new();
        let probe_only = route
            .candidates
            .iter()
            .all(|candidate| candidate.implementation == Implementation::PlatformProbe);
        if !snapshot.host.supported && !probe_only {
            reasons.push(Reason::new(
                ReasonCode::PlatformNotSupported,
                "this host platform is not implemented yet",
            ));
        }
        if snapshot.host.supported && selected.is_none() {
            for candidate in &route.candidates {
                for requirement in candidate.requirements {
                    for reason in unmet_reasons(*requirement, snapshot) {
                        if !reasons.contains(&reason) {
                            reasons.push(reason);
                        }
                    }
                }
            }
        }
        if !route.implemented || reasons.is_empty() {
            reasons.push(Reason::not_implemented());
        }
        CapabilityState::Unavailable { reasons }
    }
}

fn is_available(candidate: &Candidate, snapshot: &EnvironmentSnapshot) -> bool {
    if candidate.implementation == Implementation::PlatformProbe {
        return true;
    }
    snapshot.host.supported
        && candidate
            .requirements
            .iter()
            .all(|requirement| unmet_reasons(*requirement, snapshot).is_empty())
}

fn unmet_reasons(requirement: Requirement, snapshot: &EnvironmentSnapshot) -> Vec<Reason> {
    let name = match requirement {
        Requirement::AndroidCli => &snapshot.tool_names.android,
        Requirement::Emulator => &snapshot.tool_names.emulator,
        Requirement::Adb => &snapshot.tool_names.adb,
        Requirement::AndroidCliAvdLayout => {
            if snapshot.paths.avd_root == snapshot.paths.user_root.join("avd") {
                return Vec::new();
            }
            return vec![Reason::new(
                ReasonCode::NotImplemented,
                "Android CLI does not support a separate AVD directory",
            )];
        }
    };
    match snapshot.tools.iter().find(|tool| &tool.name == name) {
        Some(tool) if tool.state == ToolState::Available => Vec::new(),
        Some(tool) if !tool.reasons.is_empty() => tool.reasons.clone(),
        Some(tool) if tool.state == ToolState::Unavailable => vec![Reason::new(
            ReasonCode::ToolNotReady,
            format!("{name} is not ready"),
        )],
        _ => vec![Reason::new(
            ReasonCode::ToolNotFound,
            format!("{name} was not found"),
        )],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use model::{
        CpuArchitecture, Field, Host, Platform, PlatformPaths, ToolNames, ToolSource, ToolStatus,
        ValueSource,
    };
    use std::path::PathBuf;

    fn snapshot(tool_names_present: &[&str]) -> EnvironmentSnapshot {
        let tool_names = ToolNames {
            android: "android".into(),
            adb: "adb".into(),
            emulator: "emulator".into(),
            sdkmanager: "sdkmanager".into(),
            avdmanager: "avdmanager".into(),
        };
        EnvironmentSnapshot {
            host: Host {
                platform: Platform::MacOs,
                architecture: CpuArchitecture::Arm64,
                android_abi: Field::present("arm64-v8a".into()),
                supported: true,
            },
            paths: PlatformPaths {
                sdk_root: PathBuf::from("/sdk"),
                user_root: PathBuf::from("/user"),
                avd_root: PathBuf::from("/user/avd"),
                runtime_root: PathBuf::from("/runtime"),
                data_root: PathBuf::from("/data"),
            },
            sdk_root_source: ValueSource::PlatformDefault,
            environment: Vec::new(),
            tools: ["android", "adb", "emulator", "sdkmanager", "avdmanager"]
                .into_iter()
                .map(|name| ToolStatus {
                    name: name.into(),
                    path: tool_names_present
                        .contains(&name)
                        .then(|| PathBuf::from(format!("/bin/{name}"))),
                    version: Field::Unavailable,
                    state: if tool_names_present.contains(&name) {
                        ToolState::Available
                    } else {
                        ToolState::Missing
                    },
                    source: Some(ToolSource::SearchPath),
                    package: None,
                    reasons: Vec::new(),
                    diagnostic: None,
                })
                .collect(),
            legacy_tools: Vec::new(),
            installed_packages: Vec::new(),
            diagnostics: Vec::new(),
            tool_names,
        }
    }

    fn implementations(id: CapabilityId) -> Vec<Implementation> {
        Router
            .route(id)
            .candidates
            .into_iter()
            .map(|candidate| candidate.implementation)
            .collect()
    }

    fn state(snapshot: &EnvironmentSnapshot, id: CapabilityId) -> CapabilityState {
        Router
            .matrix(snapshot)
            .capability(id)
            .unwrap()
            .state
            .clone()
    }

    #[test]
    fn fixed_routes_match_the_p0_preferences() {
        assert_eq!(
            implementations(CapabilityId::DevicesList),
            [Implementation::AvdFiles]
        );
        assert_eq!(
            implementations(CapabilityId::DevicesProfiles),
            [Implementation::AndroidCli]
        );
        assert_eq!(
            implementations(CapabilityId::RuntimeStop),
            [Implementation::AdbEmuKill, Implementation::AndroidCli]
        );
        assert_eq!(
            implementations(CapabilityId::DevicesPlanCreate),
            [Implementation::AndroidCli]
        );
        assert_eq!(
            implementations(CapabilityId::DevicesDelete),
            [Implementation::AndroidCli]
        );
        assert_eq!(
            implementations(CapabilityId::PackagesListInstalled),
            [Implementation::InstalledSdkFiles]
        );
    }

    #[test]
    fn create_capability_reports_every_tool_that_execution_needs() {
        assert!(matches!(
            state(&snapshot(&["android"]), CapabilityId::DevicesPlanCreate),
            CapabilityState::Unavailable { reasons }
                if reasons.len() == 2
                    && reasons.iter().all(|reason| reason.code == ReasonCode::ToolNotFound)
        ));
        assert!(matches!(
            state(
                &snapshot(&["android", "emulator", "adb"]),
                CapabilityId::DevicesPlanCreate
            ),
            CapabilityState::Available { implementation } if implementation == "android_cli"
        ));
    }

    #[test]
    fn implemented_file_routes_do_not_require_tools() {
        let matrix = Router.matrix(&snapshot(&[]));
        for id in [
            CapabilityId::PackagesListInstalled,
            CapabilityId::DevicesList,
            CapabilityId::DevicesGet,
        ] {
            assert!(matches!(
                matrix.capability(id).map(|entry| &entry.state),
                Some(CapabilityState::Available { .. })
            ));
        }
    }

    #[test]
    fn android_cli_must_be_ready_for_profiles() {
        let missing = Router.matrix(&snapshot(&[]));
        assert!(matches!(
            &missing
                .capability(CapabilityId::DevicesProfiles)
                .unwrap()
                .state,
            CapabilityState::Unavailable { reasons }
                if reasons.iter().any(|reason| reason.code == ReasonCode::ToolNotFound)
        ));

        let available = Router.matrix(&snapshot(&["android"]));
        assert!(matches!(
            &available
                .capability(CapabilityId::DevicesProfiles)
                .unwrap()
                .state,
            CapabilityState::Available { implementation } if implementation == "android_cli"
        ));
    }

    #[test]
    fn runtime_capabilities_require_their_complete_tool_sets() {
        let adb_only = Router.matrix(&snapshot(&["adb"]));
        assert!(matches!(
            &adb_only
                .capability(CapabilityId::RuntimeRunning)
                .unwrap()
                .state,
            CapabilityState::Available { .. }
        ));
        assert!(matches!(
            &adb_only
                .capability(CapabilityId::RuntimeStart)
                .unwrap()
                .state,
            CapabilityState::Unavailable { reasons }
                if reasons.iter().any(|reason| reason.code == ReasonCode::ToolNotFound)
        ));

        let complete = Router.matrix(&snapshot(&["adb", "emulator"]));
        assert!(matches!(
            &complete
                .capability(CapabilityId::RuntimeStart)
                .unwrap()
                .state,
            CapabilityState::Available { implementation }
                if implementation == "emulator_binary"
        ));

        let delete = Router.matrix(&snapshot(&["android", "adb"]));
        assert!(matches!(
            &delete
                .capability(CapabilityId::DevicesDelete)
                .unwrap()
                .state,
            CapabilityState::Available { implementation }
                if implementation == "android_cli"
        ));
    }

    #[test]
    fn android_cli_writes_reject_an_avd_root_it_ignores() {
        let mut snapshot = snapshot(&["android", "emulator", "adb"]);
        snapshot.paths.avd_root = PathBuf::from("/separate-avd");
        for id in [CapabilityId::DevicesPlanCreate, CapabilityId::DevicesDelete] {
            assert!(matches!(
                state(&snapshot, id),
                CapabilityState::Unavailable { reasons }
                    if reasons.len() == 1 && reasons[0].code == ReasonCode::NotImplemented
            ));
        }
    }

    #[test]
    fn unsupported_platforms_keep_probe_access_and_reject_domain_routes() {
        let mut snapshot = snapshot(&["android", "adb", "emulator"]);
        snapshot.host.platform = Platform::Linux;
        snapshot.host.supported = false;
        let matrix = Router.matrix(&snapshot);

        assert!(matches!(
            &matrix.capability(CapabilityId::Environment).unwrap().state,
            CapabilityState::Available { .. }
        ));
        assert!(matches!(
            &matrix
                .capability(CapabilityId::DevicesList)
                .unwrap()
                .state,
            CapabilityState::Unavailable { reasons }
                if reasons.iter().any(|reason| {
                    reason.code == ReasonCode::PlatformNotSupported
                })
        ));
    }
}
