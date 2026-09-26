use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Platform {
    MacOs,
    Linux,
    Windows,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CpuArchitecture {
    Arm64,
    X86_64,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Host {
    pub platform: Platform,
    pub architecture: CpuArchitecture,
    pub supported: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlatformPaths {
    pub sdk_root: PathBuf,
    pub avd_root: PathBuf,
    pub data_root: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolNames {
    pub android: String,
    pub adb: String,
    pub emulator: String,
    pub sdkmanager: String,
    pub avdmanager: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Description {
    pub host: Host,
    pub paths: PlatformPaths,
    pub tools: ToolNames,
}
