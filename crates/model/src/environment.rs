use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{
    capability::{Capability, CapabilityId},
    host::{Host, PlatformPaths, ToolNames},
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolStatus {
    pub name: String,
    pub path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvironmentSnapshot {
    pub host: Host,
    pub paths: PlatformPaths,
    pub tool_names: ToolNames,
    pub tools: Vec<ToolStatus>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityMatrix {
    pub capabilities: Vec<Capability>,
}

impl CapabilityMatrix {
    pub fn capability(&self, id: CapabilityId) -> Option<&Capability> {
        self.capabilities
            .iter()
            .find(|capability| capability.id == id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvironmentReport {
    pub snapshot: EnvironmentSnapshot,
    pub capabilities: CapabilityMatrix,
}
