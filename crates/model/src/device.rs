use serde::{Deserialize, Serialize};

use crate::{
    field::Field,
    ids::{AvdId, PackageId, ProfileId, Revision, Serial},
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Device {
    pub id: AvdId,
    pub display_name: Field<String>,
    pub profile: Field<ProfileId>,
    pub image: Field<PackageId>,
    pub target: Field<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile {
    pub id: ProfileId,
    pub display_name: Field<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunningInstance {
    pub id: AvdId,
    pub serial: Field<Serial>,
    pub pid: Field<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BootStatus {
    Offline,
    Booting,
    Ready,
    Stuck,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Package {
    pub id: PackageId,
    pub revision: Field<Revision>,
    pub installed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct HardwareConfig {
    pub ram_mib: Option<u32>,
    pub cpu_count: Option<u32>,
    pub screen_width: Option<u32>,
    pub screen_height: Option<u32>,
}

impl HardwareConfig {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct StartOptions {
    pub cold: bool,
    pub wipe_data: bool,
    pub headless: bool,
    pub multi_instance: bool,
}

impl StartOptions {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}
