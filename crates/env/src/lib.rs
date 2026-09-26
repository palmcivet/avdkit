//! Environment snapshot and capability derivation.

use std::{
    path::{Path, PathBuf},
    process::Command,
};

use model::{Capability, CapabilityState, Reason};
use platform::Description;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolStatus {
    pub name: String,
    pub path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    pub platform: Description,
    pub tools: Vec<ToolStatus>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Matrix {
    pub capabilities: Vec<Capability>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Report {
    pub snapshot: Snapshot,
    pub capabilities: Matrix,
}

pub fn probe() -> Report {
    probe_with_sdk_root(None)
}

pub fn probe_with_sdk_root(sdk_root: Option<&Path>) -> Report {
    let mut platform = platform::describe();
    if let Some(sdk_root) = sdk_root {
        platform.paths.sdk_root = sdk_root.to_path_buf();
    }
    let tools = [
        platform.tools.android.clone(),
        platform.tools.adb.clone(),
        platform.tools.emulator.clone(),
        platform.tools.sdkmanager.clone(),
        platform.tools.avdmanager.clone(),
    ]
    .into_iter()
    .map(|name| ToolStatus {
        path: locate(&name, &platform),
        name,
    })
    .collect::<Vec<_>>();

    let snapshot = Snapshot { platform, tools };
    let capabilities = Matrix {
        capabilities: capabilities(&snapshot),
    };
    Report {
        snapshot,
        capabilities,
    }
}

fn locate(name: &str, description: &platform::Description) -> Option<PathBuf> {
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

    let sdk_root = &description.paths.sdk_root;
    [
        sdk_root.join("cmdline-tools/latest/bin"),
        sdk_root.join("emulator"),
        sdk_root.join("platform-tools"),
    ]
    .into_iter()
    .map(|directory| directory.join(name))
    .find(|path| path.is_file())
}

fn capabilities(snapshot: &Snapshot) -> Vec<Capability> {
    let required_for_listing = [
        snapshot.platform.tools.android.as_str(),
        snapshot.platform.tools.adb.as_str(),
        snapshot.platform.tools.emulator.as_str(),
    ];
    let required_tools_found = required_for_listing.iter().all(|required| {
        snapshot
            .tools
            .iter()
            .any(|tool| tool.name == *required && tool.path.is_some())
    });
    let base_state = if !snapshot.platform.host.supported {
        CapabilityState::Unavailable {
            reasons: vec![Reason {
                code: "platform_not_supported".into(),
                message: "this host platform is not implemented yet".into(),
            }],
        }
    } else if required_tools_found {
        CapabilityState::Available {
            implementation: "official_tools".into(),
        }
    } else {
        CapabilityState::Unavailable {
            reasons: vec![Reason {
                code: "tool_not_found".into(),
                message: "one or more Android tools were not found".into(),
            }],
        }
    };
    vec![
        Capability {
            operation: "environment".into(),
            state: CapabilityState::Available {
                implementation: "platform_probe".into(),
            },
        },
        Capability {
            operation: "devices.list".into(),
            state: base_state,
        },
    ]
}

#[allow(dead_code)]
fn command_exists(name: &str) -> bool {
    Command::new(name).arg("--version").output().is_ok()
}
