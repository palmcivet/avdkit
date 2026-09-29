//! Thin adapters around the official Android command-line tools.

#![allow(clippy::result_large_err)]

pub mod adb;
pub mod android;
pub mod emulator;
#[cfg(test)]
mod fixture;
mod output;

use std::{collections::BTreeMap, path::PathBuf};

use model::{AndroidCliMetrics, Error, PlatformPaths};
use process::{CancellationToken, CommandSpec, Output, Runner};

pub use output::NormalizedOutput;

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
    pub environment: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolEnvironment {
    pub sdk_root: PathBuf,
    pub user_root: PathBuf,
    pub avd_root: PathBuf,
    pub java_home: Option<PathBuf>,
}

impl From<&PlatformPaths> for ToolEnvironment {
    fn from(paths: &PlatformPaths) -> Self {
        Self {
            sdk_root: paths.sdk_root.clone(),
            user_root: paths.user_root.clone(),
            avd_root: paths.avd_root.clone(),
            java_home: None,
        }
    }
}

impl ToolEnvironment {
    fn variables(&self) -> BTreeMap<String, String> {
        let mut variables = BTreeMap::from([
            (
                "ANDROID_SDK_ROOT".into(),
                self.sdk_root.to_string_lossy().into_owned(),
            ),
            (
                "ANDROID_USER_HOME".into(),
                self.user_root.to_string_lossy().into_owned(),
            ),
            (
                "ANDROID_AVD_HOME".into(),
                self.avd_root.to_string_lossy().into_owned(),
            ),
        ]);
        if let Some(java_home) = &self.java_home {
            variables.insert("JAVA_HOME".into(), java_home.to_string_lossy().into_owned());
        }
        variables
    }
}

impl Invocation {
    pub fn android_discovery(executable: PathBuf, args: impl IntoIterator<Item = String>) -> Self {
        Self {
            tool: Tool::Android,
            executable,
            args: args.into_iter().collect(),
            environment: BTreeMap::new(),
        }
    }

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
            environment: BTreeMap::new(),
        }
    }

    pub fn emulator(executable: PathBuf, args: impl IntoIterator<Item = String>) -> Self {
        Self {
            tool: Tool::Emulator,
            executable,
            args: args.into_iter().collect(),
            environment: BTreeMap::new(),
        }
    }

    pub fn adb(executable: PathBuf, args: impl IntoIterator<Item = String>) -> Self {
        Self {
            tool: Tool::Adb,
            executable,
            args: args.into_iter().collect(),
            environment: BTreeMap::new(),
        }
    }

    pub fn with_environment(mut self, environment: &ToolEnvironment) -> Self {
        self.environment = environment.variables();
        self
    }

    pub fn with_variables(mut self, variables: impl IntoIterator<Item = (String, String)>) -> Self {
        self.environment.extend(variables);
        self
    }

    pub fn command(self) -> CommandSpec {
        CommandSpec {
            program: self.executable,
            args: self.args,
            environment: self.environment,
            ..CommandSpec::default()
        }
    }
}

pub async fn execute(runner: &Runner, invocation: Invocation) -> Result<Output, Error> {
    runner.run(invocation.command()).await
}

pub async fn execute_cancellable(
    runner: &Runner,
    invocation: Invocation,
    cancellation: &CancellationToken,
) -> Result<Output, Error> {
    runner
        .run_cancellable(invocation.command(), cancellation)
        .await
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

    #[test]
    fn tool_environment_is_injected_per_command() {
        let environment = ToolEnvironment {
            sdk_root: PathBuf::from("/sdk"),
            user_root: PathBuf::from("/user"),
            avd_root: PathBuf::from("/avd"),
            java_home: Some(PathBuf::from("/java")),
        };
        let command = Invocation::adb(PathBuf::from("/bin/adb"), ["devices".into()])
            .with_environment(&environment)
            .command();
        assert_eq!(command.environment["ANDROID_SDK_ROOT"], "/sdk");
        assert_eq!(command.environment["ANDROID_USER_HOME"], "/user");
        assert_eq!(command.environment["ANDROID_AVD_HOME"], "/avd");
        assert_eq!(command.environment["JAVA_HOME"], "/java");
    }
}
