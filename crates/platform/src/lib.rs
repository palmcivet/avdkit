//! Host-specific paths and executable names.
//!
//! Other crates consume this description instead of branching on the target
//! operating system themselves.

mod process;

use std::path::PathBuf;

use model::{CpuArchitecture, Description, Host, Platform, PlatformPaths, ToolNames};

pub use process::{prepare_child, ProcessGroup};

pub fn describe() -> Description {
    let host = current_host();
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));

    let sdk_root = std::env::var_os("ANDROID_SDK_ROOT")
        .or_else(|| std::env::var_os("ANDROID_HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|| default_sdk_root(&host, &home));
    let user_root = std::env::var_os("ANDROID_USER_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".android"));
    let avd_root = std::env::var_os("ANDROID_AVD_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| user_root.join("avd"));

    Description {
        host: host.clone(),
        paths: PlatformPaths {
            sdk_root,
            user_root,
            avd_root,
            data_root: default_data_root(&host, &home),
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
    let architecture = if cfg!(target_arch = "aarch64") {
        CpuArchitecture::Arm64
    } else if cfg!(target_arch = "x86_64") {
        CpuArchitecture::X86_64
    } else {
        CpuArchitecture::Other
    };
    Host {
        supported: matches!(platform, Platform::MacOs)
            && matches!(architecture, CpuArchitecture::Arm64),
        platform,
        architecture,
    }
}

fn default_sdk_root(host: &Host, home: &std::path::Path) -> PathBuf {
    match host.platform {
        Platform::MacOs => home.join("Library/Android/sdk"),
        Platform::Linux => home.join("Android/Sdk"),
        Platform::Windows => std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.to_path_buf())
            .join("Android/Sdk"),
        Platform::Unsupported => home.join("Android/Sdk"),
    }
}

fn default_data_root(host: &Host, home: &std::path::Path) -> PathBuf {
    match host.platform {
        Platform::MacOs => home
            .join("Library/Application Support")
            .join(model::PRODUCT_NAME),
        Platform::Linux => std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".local/share"))
            .join(model::PRODUCT_NAME),
        Platform::Windows => std::env::var_os("LOCALAPPDATA")
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
