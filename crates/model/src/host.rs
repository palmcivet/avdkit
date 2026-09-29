use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::Field;

/// Operating-system family detected at runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Platform {
    /// Apple macOS.
    MacOs,
    /// Linux.
    Linux,
    /// Microsoft Windows.
    Windows,
    /// An unrecognized operating system.
    Unsupported,
}

/// CPU architecture detected at runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CpuArchitecture {
    /// 64-bit Arm.
    Arm64,
    /// 64-bit x86.
    X86_64,
    /// An architecture without a dedicated representation.
    Other,
}

/// Host identity and implementation support state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Host {
    /// Operating-system family.
    pub platform: Platform,
    /// CPU architecture.
    pub architecture: CpuArchitecture,
    /// Android guest ABI compatible with the detected host.
    pub android_abi: Field<String>,
    /// Whether this platform and architecture have an implementation.
    pub supported: bool,
}

/// Selected filesystem roots for Android tools and library data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlatformPaths {
    /// Android SDK root.
    pub sdk_root: PathBuf,
    /// Android user configuration root.
    pub user_root: PathBuf,
    /// AVD index directory.
    pub avd_root: PathBuf,
    /// Library-owned data directory.
    pub data_root: PathBuf,
}

/// Platform-specific names of official Android executables.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolNames {
    /// Legacy Android CLI executable.
    pub android: String,
    /// Android Debug Bridge executable.
    pub adb: String,
    /// Android emulator executable.
    pub emulator: String,
    /// SDK package manager executable.
    pub sdkmanager: String,
    /// AVD manager executable.
    pub avdmanager: String,
}

/// Host-specific defaults consumed by environment discovery.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Description {
    /// Host identity.
    pub host: Host,
    /// Default or environment-selected paths.
    pub paths: PlatformPaths,
    /// Executable names for the host.
    pub tools: ToolNames,
}
