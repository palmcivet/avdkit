//! Thin adapters around the official Android command-line tools.

use std::path::PathBuf;

use model::Error;
use process::{CommandSpec, Output, Runner};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    Android,
    SdkManager,
    AvdManager,
    Emulator,
    Adb,
}

#[derive(Debug, Clone)]
pub struct Invocation {
    pub tool: Tool,
    pub executable: PathBuf,
    pub args: Vec<String>,
}

impl Invocation {
    pub fn command(self) -> CommandSpec {
        CommandSpec {
            program: self.executable,
            args: self.args,
            ..CommandSpec::default()
        }
    }
}

pub async fn execute(runner: &Runner, invocation: Invocation) -> Result<Output, Error> {
    runner.run(invocation.command()).await
}
