//! Thin adapters around the official Android command-line tools.

#![allow(clippy::result_large_err)]

use std::path::PathBuf;

use model::{AndroidCliMetrics, Error};
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
    pub fn android(
        executable: PathBuf,
        sdk_root: &std::path::Path,
        metrics: AndroidCliMetrics,
        args: impl IntoIterator<Item = String>,
    ) -> Self {
        let mut command_args = vec![format!("--sdk={}", sdk_root.to_string_lossy())];
        if metrics == AndroidCliMetrics::NoMetrics {
            command_args.push("--no-metrics".into());
        }
        command_args.extend(args);
        Self {
            tool: Tool::Android,
            executable,
            args: command_args,
        }
    }

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn android_invocation_always_pins_the_sdk_root() {
        let invocation = Invocation::android(
            PathBuf::from("/bin/android"),
            Path::new("/sdk"),
            AndroidCliMetrics::Inherit,
            ["sdk".into(), "list".into()],
        );
        assert_eq!(
            invocation.args,
            ["--sdk=/sdk", "sdk", "list"]
                .into_iter()
                .map(String::from)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn android_invocation_only_disables_metrics_when_requested() {
        let invocation = Invocation::android(
            PathBuf::from("/bin/android"),
            Path::new("/sdk"),
            AndroidCliMetrics::NoMetrics,
            ["info".into()],
        );
        assert_eq!(
            invocation.args,
            ["--sdk=/sdk", "--no-metrics", "info"]
                .into_iter()
                .map(String::from)
                .collect::<Vec<_>>()
        );
    }
}
