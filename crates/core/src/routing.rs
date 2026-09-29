use model::{
    Capability, CapabilityId, CapabilityMatrix, CapabilityState, EnvironmentSnapshot, Error,
    PlanKind, Reason, ReasonCode, ToolState,
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
    PlanCompiler,
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
            Self::PlanCompiler => "plan_compiler",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Route {
    pub(crate) implementations: Vec<Implementation>,
    pub(crate) implemented: bool,
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Router;

impl Router {
    pub(crate) fn plan_tool(self, kind: PlanKind) -> Implementation {
        match kind {
            PlanKind::CreateDevice => Implementation::AndroidCli,
        }
    }

    pub(crate) fn route(self, id: CapabilityId) -> Route {
        use CapabilityId as Id;
        use Implementation as Impl;

        let (implementations, implemented) = match id {
            Id::Environment | Id::Capabilities | Id::Refresh => (vec![Impl::PlatformProbe], true),
            Id::PackagesListInstalled => (vec![Impl::InstalledSdkFiles], true),
            Id::PackagesListAvailable
            | Id::PackagesInstall
            | Id::PackagesRemove
            | Id::PackagesUpdate => (vec![Impl::AndroidCli], false),
            Id::DevicesList | Id::DevicesGet => (vec![Impl::AvdFiles], true),
            Id::DevicesProfiles => (vec![Impl::AndroidCli], true),
            Id::DevicesPlanCreate => (vec![Impl::PlanCompiler], true),
            Id::DevicesDelete => (vec![Impl::AndroidCli], false),
            Id::RuntimeRunning | Id::RuntimeBootStatus => (vec![Impl::RuntimeDiscovery], false),
            Id::RuntimeStart | Id::RuntimeStartCustom => (vec![Impl::EmulatorBinary], false),
            Id::RuntimeStop => (vec![Impl::AdbEmuKill, Impl::AndroidCli], false),
            _ => (Vec::new(), false),
        };
        Route {
            implementations,
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
        let route = self.route(id);
        route
            .implementations
            .into_iter()
            .find(|implementation| self.is_available(*implementation, snapshot))
            .ok_or_else(|| Error::not_implemented(id.as_str()))
    }

    fn state(self, id: CapabilityId, snapshot: &EnvironmentSnapshot) -> CapabilityState {
        let route = self.route(id);
        let selected = route
            .implementations
            .iter()
            .copied()
            .find(|implementation| self.is_available(*implementation, snapshot));
        if route.implemented {
            if let Some(implementation) = selected {
                return CapabilityState::Available {
                    implementation: implementation.as_str().into(),
                };
            }
        }

        let mut reasons = Vec::new();
        if !snapshot.host.supported
            && !route
                .implementations
                .contains(&Implementation::PlatformProbe)
        {
            reasons.push(Reason::new(
                ReasonCode::PlatformNotSupported,
                "this host platform is not implemented yet",
            ));
        }
        if !route.implementations.is_empty()
            && selected.is_none()
            && snapshot.host.supported
            && route
                .implementations
                .iter()
                .any(|implementation| self.requires_tool(*implementation))
        {
            reasons.extend(self.tool_reasons(&route, snapshot));
        }
        if !route.implemented {
            reasons.push(Reason::not_implemented());
        }
        if reasons.is_empty() {
            reasons.push(Reason::not_implemented());
        }
        CapabilityState::Unavailable { reasons }
    }

    fn is_available(self, implementation: Implementation, snapshot: &EnvironmentSnapshot) -> bool {
        if implementation == Implementation::PlatformProbe {
            return true;
        }
        if !snapshot.host.supported {
            return false;
        }
        match implementation {
            Implementation::AndroidCli => tool_available(snapshot, &snapshot.tool_names.android),
            Implementation::RuntimeDiscovery | Implementation::AdbEmuKill => {
                tool_available(snapshot, &snapshot.tool_names.adb)
            }
            Implementation::EmulatorBinary => {
                tool_available(snapshot, &snapshot.tool_names.emulator)
            }
            Implementation::AvdFiles
            | Implementation::InstalledSdkFiles
            | Implementation::PlanCompiler => true,
            Implementation::PlatformProbe => true,
        }
    }

    fn requires_tool(self, implementation: Implementation) -> bool {
        matches!(
            implementation,
            Implementation::AndroidCli
                | Implementation::RuntimeDiscovery
                | Implementation::EmulatorBinary
                | Implementation::AdbEmuKill
        )
    }

    fn tool_reasons(self, route: &Route, snapshot: &EnvironmentSnapshot) -> Vec<Reason> {
        let names = route
            .implementations
            .iter()
            .filter_map(|implementation| match implementation {
                Implementation::AndroidCli => Some(&snapshot.tool_names.android),
                Implementation::RuntimeDiscovery | Implementation::AdbEmuKill => {
                    Some(&snapshot.tool_names.adb)
                }
                Implementation::EmulatorBinary => Some(&snapshot.tool_names.emulator),
                _ => None,
            })
            .collect::<Vec<_>>();
        let mut reasons = Vec::new();
        for name in names {
            match snapshot.tools.iter().find(|tool| &tool.name == name) {
                Some(tool) if !tool.reasons.is_empty() => reasons.extend(tool.reasons.clone()),
                Some(tool) if tool.state == ToolState::Unavailable => reasons.push(Reason::new(
                    ReasonCode::ToolNotReady,
                    format!("{name} is not ready"),
                )),
                _ => reasons.push(Reason::new(
                    ReasonCode::ToolNotFound,
                    format!("{name} was not found"),
                )),
            }
        }
        reasons
    }
}

fn tool_available(snapshot: &EnvironmentSnapshot, name: &str) -> bool {
    snapshot
        .tools
        .iter()
        .any(|tool| tool.name == name && tool.state == ToolState::Available)
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
                avd_root: PathBuf::from("/avd"),
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

    #[test]
    fn fixed_routes_match_the_p0_preferences() {
        let router = Router;
        assert_eq!(
            router.route(CapabilityId::DevicesList).implementations,
            [Implementation::AvdFiles]
        );
        assert_eq!(
            router.route(CapabilityId::DevicesProfiles).implementations,
            [Implementation::AndroidCli]
        );
        assert_eq!(
            router.route(CapabilityId::RuntimeStop).implementations,
            [Implementation::AdbEmuKill, Implementation::AndroidCli]
        );
        assert_eq!(
            router
                .route(CapabilityId::PackagesListInstalled)
                .implementations,
            [Implementation::InstalledSdkFiles]
        );
        assert_eq!(
            router.plan_tool(PlanKind::CreateDevice),
            Implementation::AndroidCli
        );
    }

    #[test]
    fn implemented_file_routes_do_not_require_tools() {
        let matrix = Router.matrix(&snapshot(&[]));
        for id in [
            CapabilityId::PackagesListInstalled,
            CapabilityId::DevicesList,
            CapabilityId::DevicesGet,
            CapabilityId::DevicesPlanCreate,
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
