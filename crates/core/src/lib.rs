//! Public Rust facade.

use std::{
    path::PathBuf,
    sync::{Arc, RwLock},
};

use environment::Report;
use model::{Capability, Error, ErrorCode};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct KitConfig {
    pub sdk_root: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct Kit {
    config: KitConfig,
    report: Arc<RwLock<Report>>,
}

impl Kit {
    pub fn new(config: KitConfig) -> Result<Self, Error> {
        let report = environment::probe_with_sdk_root(config.sdk_root.as_deref());
        Ok(Self {
            config,
            report: Arc::new(RwLock::new(report)),
        })
    }

    pub fn config(&self) -> &KitConfig {
        &self.config
    }

    pub fn environment(&self) -> Report {
        self.report
            .read()
            .expect("environment lock is not poisoned")
            .clone()
    }

    pub fn capabilities(&self) -> Vec<Capability> {
        self.environment().capabilities.capabilities
    }

    pub fn refresh(&self) -> Result<(), Error> {
        let report = environment::probe_with_sdk_root(self.config.sdk_root.as_deref());
        *self
            .report
            .write()
            .map_err(|_| Error::new(ErrorCode::Internal, "environment lock is poisoned"))? = report;
        Ok(())
    }
}
