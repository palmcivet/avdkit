use serde::{Deserialize, Serialize};

use crate::{
    field::Field,
    ids::{AvdId, PackageId, ProfileId, Revision, Serial},
};

/// Configuration metadata for one Android virtual device.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Device {
    /// Stable AVD identifier.
    pub id: AvdId,
    /// User-facing display name.
    pub display_name: Field<String>,
    /// Hardware profile used by the AVD.
    pub profile: Field<ProfileId>,
    /// System image selected by the AVD.
    pub image: Field<PackageId>,
    /// Android target recorded by the AVD.
    pub target: Field<String>,
}

/// A device profile offered by an Android tool.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile {
    /// Stable profile identifier.
    pub id: ProfileId,
    /// User-facing profile name, when supplied by the tool.
    pub display_name: Field<String>,
}

/// A discovered running emulator process.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunningInstance {
    /// Configured AVD identifier.
    pub id: AvdId,
    /// adb serial associated with the emulator.
    pub serial: Field<Serial>,
    /// Emulator process identifier.
    pub pid: Field<u32>,
}

/// High-level boot state of an emulator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum BootStatus {
    /// No reachable instance is running.
    Offline,
    /// The instance is running but Android is not ready.
    Booting,
    /// Android has completed the required readiness checks.
    Ready,
    /// The instance stopped making boot progress.
    Stuck,
}

/// An Android SDK package and its installation state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Package {
    /// Stable package identifier.
    pub id: PackageId,
    /// Installed or advertised package revision.
    pub revision: Field<Revision>,
    /// Whether the package is installed in the selected SDK.
    pub installed: bool,
}

/// Optional hardware overrides for a device-creation request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct HardwareConfig {
    /// Guest RAM in mebibytes.
    pub ram_mib: Option<u32>,
    /// Number of guest CPU cores.
    pub cpu_count: Option<u32>,
    /// Screen width in pixels.
    pub screen_width: Option<u32>,
    /// Screen height in pixels.
    pub screen_height: Option<u32>,
}

impl HardwareConfig {
    /// Returns whether no hardware overrides were selected.
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// Options that alter emulator startup behavior.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct StartOptions {
    /// Disable loading a saved quick-boot state.
    pub cold: bool,
    /// Reset writable device data before startup.
    pub wipe_data: bool,
    /// Start without a graphical window.
    pub headless: bool,
    /// Permit another instance of the same AVD.
    pub multi_instance: bool,
}

impl StartOptions {
    /// Returns whether no startup overrides were selected.
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}
