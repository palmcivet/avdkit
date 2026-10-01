//! Host-specific paths and executable names.
//!
//! Other crates consume this description instead of branching on the target
//! operating system themselves.

mod process;

use std::{
    collections::BTreeMap,
    path::PathBuf,
    process::{Command, Stdio},
};

use model::{CpuArchitecture, Field, Host, Platform, PlatformPaths, ToolNames};

pub use process::{
    prepare_child, prepare_detached_child, process_executable, process_exists,
    signal_process_group, ProcessGroup, ProcessSignal,
};

/// Host-specific defaults consumed by environment discovery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Description {
    /// Host identity.
    pub host: Host,
    /// Default or environment-selected paths.
    pub paths: PlatformPaths,
    /// Executable names for the host.
    pub tools: ToolNames,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginShell {
    pub program: PathBuf,
    pub args: Vec<String>,
}

pub fn process_environment() -> BTreeMap<String, String> {
    std::env::vars().collect()
}

pub fn login_shell(environment: &BTreeMap<String, String>) -> Option<LoginShell> {
    if current_host().platform != Platform::MacOs {
        return None;
    }
    let program = environment
        .get("SHELL")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/bin/zsh"));
    Some(LoginShell {
        program,
        args: vec!["-l".into(), "-c".into(), "env".into()],
    })
}

pub fn describe() -> Description {
    let environment = process_environment();
    let mut description = describe_with_environment(&environment);
    if let Some(sdk_root) = environment
        .get("ANDROID_HOME")
        .or_else(|| environment.get("ANDROID_SDK_ROOT"))
    {
        description.paths.sdk_root = PathBuf::from(sdk_root);
    }
    description
}

pub fn describe_with_environment(environment: &BTreeMap<String, String>) -> Description {
    let host = current_host();
    let home = environment
        .get("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));

    let sdk_root = default_sdk_root(&host, &home);
    let user_root = environment
        .get("ANDROID_USER_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".android"));
    let avd_root = environment
        .get("ANDROID_AVD_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| user_root.join("avd"));

    Description {
        host: host.clone(),
        paths: PlatformPaths {
            sdk_root,
            user_root,
            avd_root,
            runtime_root: runtime_root(&host, &home, environment),
            data_root: default_data_root(&host, &home, environment),
        },
        tools: tool_names(),
    }
}

fn current_host() -> Host {
    let platform = if cfg!(target_os = "macos") {
        Platform::MacOs
    } else if cfg!(target_os = "linux") {
        Platform::Linux
    } else if cfg!(target_os = "windows") {
        Platform::Windows
    } else {
        Platform::Unsupported
    };
    let architecture = runtime_architecture();
    let android_abi = match architecture {
        CpuArchitecture::Arm64 => Field::present("arm64-v8a".into()),
        CpuArchitecture::X86_64 => Field::present("x86_64".into()),
        _ => Field::Unavailable,
    };
    Host {
        supported: matches!(platform, Platform::MacOs)
            && matches!(architecture, CpuArchitecture::Arm64),
        platform,
        architecture,
        android_abi,
    }
}

fn runtime_architecture() -> CpuArchitecture {
    let command_architecture = Command::new("uname")
        .arg("-m")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|value| value.trim().to_owned());
    let architecture = command_architecture
        .unwrap_or_else(|| std::env::consts::ARCH.to_owned())
        .to_ascii_lowercase();
    match architecture.as_str() {
        "arm64" | "aarch64" => CpuArchitecture::Arm64,
        "x86_64" | "amd64" => CpuArchitecture::X86_64,
        _ => CpuArchitecture::Other,
    }
}

fn default_sdk_root(host: &Host, home: &std::path::Path) -> PathBuf {
    match host.platform {
        Platform::MacOs => home.join("Library/Android/sdk"),
        _ => home.join("Android/Sdk"),
    }
}

fn runtime_root(
    host: &Host,
    home: &std::path::Path,
    environment: &BTreeMap<String, String>,
) -> PathBuf {
    match host.platform {
        Platform::MacOs => home.join("Library/Caches/TemporaryItems/avd/running"),
        Platform::Linux => environment
            .get("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".cache"))
            .join("avd/running"),
        _ => home.join(".cache/avd/running"),
    }
}

fn default_data_root(
    host: &Host,
    home: &std::path::Path,
    environment: &BTreeMap<String, String>,
) -> PathBuf {
    match host.platform {
        Platform::MacOs => home
            .join("Library/Application Support")
            .join(model::PRODUCT_NAME),
        Platform::Linux => environment
            .get("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".local/share"))
            .join(model::PRODUCT_NAME),
        _ => home.join(".local/share").join(model::PRODUCT_NAME),
    }
}

fn tool_names() -> ToolNames {
    ToolNames {
        android: "android".into(),
        adb: "adb".into(),
        emulator: "emulator".into(),
        sdkmanager: "sdkmanager".into(),
        avdmanager: "avdmanager".into(),
    }
}
