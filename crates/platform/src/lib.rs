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

use model::{CpuArchitecture, Description, Field, Host, Platform, PlatformPaths, ToolNames};

pub use process::{prepare_child, ProcessGroup};

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
        .get("ANDROID_SDK_ROOT")
        .or_else(|| environment.get("ANDROID_HOME"))
    {
        description.paths.sdk_root = PathBuf::from(sdk_root);
    }
    description
}

pub fn describe_with_environment(environment: &BTreeMap<String, String>) -> Description {
    let host = current_host();
    let home = environment
        .get("HOME")
        .or_else(|| environment.get("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));

    let sdk_root = default_sdk_root(&host, &home, environment);
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
            data_root: default_data_root(&host, &home, environment),
        },
        tools: tool_names(&host),
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
        CpuArchitecture::Other => Field::Unavailable,
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
        .or_else(|| std::env::var("PROCESSOR_ARCHITECTURE").ok())
        .unwrap_or_else(|| std::env::consts::ARCH.to_owned())
        .to_ascii_lowercase();
    match architecture.as_str() {
        "arm64" | "aarch64" => CpuArchitecture::Arm64,
        "x86_64" | "amd64" => CpuArchitecture::X86_64,
        _ => CpuArchitecture::Other,
    }
}

fn default_sdk_root(
    host: &Host,
    home: &std::path::Path,
    environment: &BTreeMap<String, String>,
) -> PathBuf {
    match host.platform {
        Platform::MacOs => home.join("Library/Android/sdk"),
        Platform::Linux => home.join("Android/Sdk"),
        Platform::Windows => environment
            .get("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.to_path_buf())
            .join("Android/Sdk"),
        Platform::Unsupported => home.join("Android/Sdk"),
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
        Platform::Windows => environment
            .get("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.to_path_buf())
            .join(model::PRODUCT_NAME),
        Platform::Unsupported => home.join(".local/share").join(model::PRODUCT_NAME),
    }
}

fn tool_names(host: &Host) -> ToolNames {
    let executable_suffix = if matches!(host.platform, Platform::Windows) {
        ".exe"
    } else {
        ""
    };
    let script_suffix = if matches!(host.platform, Platform::Windows) {
        ".bat"
    } else {
        ""
    };
    ToolNames {
        android: format!("android{script_suffix}"),
        adb: format!("adb{executable_suffix}"),
        emulator: format!("emulator{executable_suffix}"),
        sdkmanager: format!("sdkmanager{script_suffix}"),
        avdmanager: format!("avdmanager{script_suffix}"),
    }
}
