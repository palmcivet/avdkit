//! Environment snapshot and capability derivation.

use std::{
    fs,
    path::{Component, Path, PathBuf},
};

use model::{
    Capability, CapabilityId, CapabilityMatrix, CapabilityState, EnvironmentReport,
    EnvironmentSnapshot, Reason, ReasonCode, Remedy, RemedyKind, ToolStatus,
};

pub fn probe() -> EnvironmentReport {
    probe_with_sdk_root(None)
}

pub fn probe_with_sdk_root(sdk_root: Option<&Path>) -> EnvironmentReport {
    let mut description = platform::describe();
    if let Some(sdk_root) = sdk_root {
        description.paths.sdk_root = sdk_root.to_path_buf();
    }
    let tools = [
        description.tools.android.clone(),
        description.tools.adb.clone(),
        description.tools.emulator.clone(),
        description.tools.sdkmanager.clone(),
        description.tools.avdmanager.clone(),
    ]
    .into_iter()
    .map(|name| ToolStatus {
        path: locate(&name, &description.paths.sdk_root),
        name,
    })
    .collect::<Vec<_>>();

    let snapshot = EnvironmentSnapshot {
        host: description.host,
        paths: description.paths,
        tool_names: description.tools,
        tools,
    };
    EnvironmentReport {
        capabilities: CapabilityMatrix {
            capabilities: capabilities(&snapshot),
        },
        snapshot,
    }
}

/// Checks one known license hash without depending on command output or prompts.
///
/// Missing, unreadable, or malformed license files are treated as not accepted.
pub fn license_hash_accepted(sdk_root: &Path, license_id: &str, expected_hash: &str) -> bool {
    if expected_hash.is_empty()
        || Path::new(license_id).components().count() != 1
        || !matches!(
            Path::new(license_id).components().next(),
            Some(Component::Normal(_))
        )
    {
        return false;
    }
    fs::read_to_string(sdk_root.join("licenses").join(license_id))
        .map(|contents| {
            contents
                .lines()
                .map(str::trim)
                .any(|hash| hash.eq_ignore_ascii_case(expected_hash))
        })
        .unwrap_or(false)
}

fn locate(name: &str, sdk_root: &Path) -> Option<PathBuf> {
    let candidate = PathBuf::from(name);
    if candidate.is_absolute() && candidate.is_file() {
        return Some(candidate);
    }
    let path_candidate = std::env::var_os("PATH")
        .into_iter()
        .flat_map(|path| std::env::split_paths(&path).collect::<Vec<_>>())
        .map(|directory| directory.join(name))
        .find(|path| path.is_file());
    if path_candidate.is_some() {
        return path_candidate;
    }

    [
        sdk_root.join("cmdline-tools/latest/bin"),
        sdk_root.join("emulator"),
        sdk_root.join("platform-tools"),
    ]
    .into_iter()
    .map(|directory| directory.join(name))
    .find(|path| path.is_file())
}

fn tool_found(snapshot: &EnvironmentSnapshot, name: &str) -> bool {
    snapshot
        .tools
        .iter()
        .any(|tool| tool.name == name && tool.path.is_some())
}

fn required_tools_found(snapshot: &EnvironmentSnapshot) -> bool {
    [
        snapshot.tool_names.android.as_str(),
        snapshot.tool_names.adb.as_str(),
        snapshot.tool_names.emulator.as_str(),
    ]
    .into_iter()
    .all(|name| tool_found(snapshot, name))
}

fn capabilities(snapshot: &EnvironmentSnapshot) -> Vec<Capability> {
    CapabilityId::ALL
        .iter()
        .copied()
        .map(|id| Capability {
            id,
            state: state_for(id, snapshot),
        })
        .collect()
}

fn state_for(id: CapabilityId, snapshot: &EnvironmentSnapshot) -> CapabilityState {
    match id {
        CapabilityId::Environment | CapabilityId::Capabilities | CapabilityId::Refresh => {
            CapabilityState::Available {
                implementation: "platform_probe".into(),
            }
        }
        CapabilityId::DevicesProfiles
            if snapshot.host.supported && tool_found(snapshot, &snapshot.tool_names.android) =>
        {
            CapabilityState::Available {
                implementation: "android_cli".into(),
            }
        }
        CapabilityId::DevicesList | CapabilityId::DevicesGet if snapshot.host.supported => {
            CapabilityState::Available {
                implementation: "avd_files".into(),
            }
        }
        CapabilityId::DevicesPlanCreate if snapshot.host.supported => CapabilityState::Available {
            implementation: "plan_compiler".into(),
        },
        other => CapabilityState::Unavailable {
            reasons: unavailable_reasons(other, snapshot),
        },
    }
}

fn unavailable_reasons(id: CapabilityId, snapshot: &EnvironmentSnapshot) -> Vec<Reason> {
    let mut reasons = Vec::new();
    if !snapshot.host.supported {
        reasons.push(Reason::new(
            ReasonCode::PlatformNotSupported,
            "this host platform is not implemented yet",
        ));
    }
    if needs_official_tools(id) && !tools_found_for(id, snapshot) {
        let mut reason = Reason::new(
            ReasonCode::ToolNotFound,
            "one or more Android tools were not found",
        );
        reason.remedies.push(Remedy {
            message: "install the Android CLI, emulator, and platform-tools".into(),
            command: Some("android sdk install emulator platform-tools".into()),
            kind: RemedyKind::LibraryOperation,
            operation: Some(CapabilityId::PackagesInstall),
        });
        reasons.push(reason);
    }
    if !is_implemented(id) {
        reasons.push(Reason::not_implemented());
    }
    reasons
}

fn is_implemented(id: CapabilityId) -> bool {
    matches!(
        id,
        CapabilityId::DevicesList
            | CapabilityId::DevicesGet
            | CapabilityId::DevicesProfiles
            | CapabilityId::DevicesPlanCreate
    )
}

fn tools_found_for(id: CapabilityId, snapshot: &EnvironmentSnapshot) -> bool {
    if id == CapabilityId::DevicesProfiles {
        tool_found(snapshot, &snapshot.tool_names.android)
    } else {
        required_tools_found(snapshot)
    }
}

fn needs_official_tools(id: CapabilityId) -> bool {
    !matches!(
        id,
        CapabilityId::Environment
            | CapabilityId::Capabilities
            | CapabilityId::Refresh
            | CapabilityId::DevicesList
            | CapabilityId::DevicesGet
            | CapabilityId::DevicesPlanCreate
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use model::{CpuArchitecture, Host, Platform, PlatformPaths, ToolNames};
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEMP_DIR: AtomicU64 = AtomicU64::new(1);

    fn temp_sdk_root() -> PathBuf {
        std::env::temp_dir().join(format!(
            "avdkit-license-test-{}-{}",
            std::process::id(),
            NEXT_TEMP_DIR.fetch_add(1, Ordering::Relaxed)
        ))
    }

    fn snapshot(supported: bool, tools_present: bool) -> EnvironmentSnapshot {
        let tool_names = ToolNames {
            android: "android".into(),
            adb: "adb".into(),
            emulator: "emulator".into(),
            sdkmanager: "sdkmanager".into(),
            avdmanager: "avdmanager".into(),
        };
        let path = tools_present.then(|| PathBuf::from("/bin/true"));
        EnvironmentSnapshot {
            host: Host {
                platform: Platform::MacOs,
                architecture: CpuArchitecture::Arm64,
                supported,
            },
            paths: PlatformPaths {
                sdk_root: PathBuf::from("/sdk"),
                user_root: PathBuf::from("/user"),
                avd_root: PathBuf::from("/avd"),
                data_root: PathBuf::from("/data"),
            },
            tools: [
                tool_names.android.clone(),
                tool_names.adb.clone(),
                tool_names.emulator.clone(),
                tool_names.sdkmanager.clone(),
                tool_names.avdmanager.clone(),
            ]
            .into_iter()
            .map(|name| ToolStatus {
                path: path.clone(),
                name,
            })
            .collect(),
            tool_names,
        }
    }

    #[test]
    fn matrix_covers_every_registered_capability() {
        let capabilities = capabilities(&snapshot(true, true));
        assert_eq!(capabilities.len(), CapabilityId::ALL.len());
        for id in CapabilityId::ALL {
            assert!(capabilities.iter().any(|capability| capability.id == *id));
        }
    }

    #[test]
    fn implemented_queries_are_available() {
        let capabilities = capabilities(&snapshot(true, false));
        for id in [
            CapabilityId::Environment,
            CapabilityId::Capabilities,
            CapabilityId::Refresh,
            CapabilityId::DevicesList,
            CapabilityId::DevicesGet,
            CapabilityId::DevicesPlanCreate,
        ] {
            assert!(matches!(
                capabilities
                    .iter()
                    .find(|capability| capability.id == id)
                    .map(|capability| &capability.state),
                Some(CapabilityState::Available { .. })
            ));
        }
    }

    #[test]
    fn profiles_are_available_when_android_cli_is_present() {
        let capabilities = capabilities(&snapshot(true, true));
        assert!(matches!(
            capabilities
                .iter()
                .find(|capability| capability.id == CapabilityId::DevicesProfiles)
                .map(|capability| &capability.state),
            Some(CapabilityState::Available { implementation }) if implementation == "android_cli"
        ));
    }

    #[test]
    fn implemented_profiles_do_not_report_not_implemented() {
        let capabilities = capabilities(&snapshot(true, false));
        let profiles = capabilities
            .iter()
            .find(|capability| capability.id == CapabilityId::DevicesProfiles)
            .unwrap();
        match &profiles.state {
            CapabilityState::Unavailable { reasons } => {
                assert!(reasons
                    .iter()
                    .any(|reason| reason.code == ReasonCode::ToolNotFound));
                assert!(!reasons
                    .iter()
                    .any(|reason| reason.code == ReasonCode::NotImplemented));
            }
            CapabilityState::Available { .. } => panic!("profiles should be unavailable"),
        }
    }

    #[test]
    fn file_queries_only_require_a_supported_platform() {
        let capabilities = capabilities(&snapshot(true, false));
        for id in [CapabilityId::DevicesList, CapabilityId::DevicesGet] {
            assert!(matches!(
                capabilities
                    .iter()
                    .find(|capability| capability.id == id)
                    .map(|capability| &capability.state),
                Some(CapabilityState::Available { implementation }) if implementation == "avd_files"
            ));
        }
    }

    #[test]
    fn plan_compilation_requires_a_supported_platform_but_no_tools() {
        let supported = capabilities(&snapshot(true, false));
        assert!(matches!(
            supported
                .iter()
                .find(|capability| capability.id == CapabilityId::DevicesPlanCreate)
                .map(|capability| &capability.state),
            Some(CapabilityState::Available { implementation }) if implementation == "plan_compiler"
        ));

        let unsupported = capabilities(&snapshot(false, false));
        let plan_create = unsupported
            .iter()
            .find(|capability| capability.id == CapabilityId::DevicesPlanCreate)
            .unwrap();
        match &plan_create.state {
            CapabilityState::Unavailable { reasons } => {
                assert_eq!(reasons.len(), 1);
                assert_eq!(reasons[0].code, ReasonCode::PlatformNotSupported);
            }
            CapabilityState::Available { .. } => {
                panic!("plan compilation should be unavailable on unsupported platforms");
            }
        }
    }

    #[test]
    fn unimplemented_operations_include_stable_reasons() {
        let capabilities = capabilities(&snapshot(false, false));
        let start = capabilities
            .iter()
            .find(|capability| capability.id == CapabilityId::RuntimeStart)
            .unwrap();
        match &start.state {
            CapabilityState::Unavailable { reasons } => {
                let codes: Vec<_> = reasons.iter().map(|reason| reason.code).collect();
                assert!(codes.contains(&ReasonCode::PlatformNotSupported));
                assert!(codes.contains(&ReasonCode::ToolNotFound));
                assert!(codes.contains(&ReasonCode::NotImplemented));
            }
            CapabilityState::Available { .. } => panic!("start should be unavailable"),
        }
    }

    #[test]
    fn accepted_licenses_are_read_from_hash_files() {
        let sdk_root = temp_sdk_root();
        let licenses = sdk_root.join("licenses");
        fs::create_dir_all(&licenses).unwrap();
        fs::write(
            licenses.join("android-sdk-license"),
            "first-hash\r\nEXPECTED-HASH\n",
        )
        .unwrap();

        assert!(license_hash_accepted(
            &sdk_root,
            "android-sdk-license",
            "expected-hash"
        ));
        assert!(!license_hash_accepted(
            &sdk_root,
            "android-sdk-license",
            "missing-hash"
        ));
        assert!(!license_hash_accepted(
            &sdk_root,
            "../android-sdk-license",
            "expected-hash"
        ));

        fs::remove_dir_all(sdk_root).unwrap();
    }

    #[test]
    fn missing_license_files_are_not_accepted() {
        assert!(!license_hash_accepted(
            &temp_sdk_root(),
            "android-sdk-license",
            "expected-hash"
        ));
    }
}
