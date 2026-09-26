//! Process execution primitives shared by tool drivers.

#![allow(clippy::result_large_err)]

use std::{collections::BTreeMap, path::PathBuf, time::Duration};

use model::{Error, ErrorCode};
use tokio::{
    io::AsyncWriteExt,
    process::Command,
    time::{error::Elapsed, timeout},
};

#[derive(Debug, Clone, Default)]
pub struct CommandSpec {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub environment: BTreeMap<String, String>,
    pub current_dir: Option<PathBuf>,
    pub stdin: Option<Vec<u8>>,
}

#[derive(Debug, Clone)]
pub struct Output {
    pub status: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Clone)]
pub struct Runner {
    pub default_timeout: Duration,
}

impl Default for Runner {
    fn default() -> Self {
        Self {
            default_timeout: Duration::from_secs(30),
        }
    }
}

impl Runner {
    pub async fn run(&self, spec: CommandSpec) -> Result<Output, Error> {
        self.run_with_timeout(spec, self.default_timeout).await
    }

    pub async fn run_with_timeout(
        &self,
        spec: CommandSpec,
        duration: Duration,
    ) -> Result<Output, Error> {
        let mut command = Command::new(&spec.program);
        command
            .args(&spec.args)
            .envs(&spec.environment)
            .kill_on_drop(true);
        if let Some(current_dir) = &spec.current_dir {
            command.current_dir(current_dir);
        }

        let mut child = command
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .stdin(if spec.stdin.is_some() {
                std::process::Stdio::piped()
            } else {
                std::process::Stdio::null()
            })
            .spawn()
            .map_err(|error| Error::new(ErrorCode::ToolNotFound, error.to_string()))?;

        if let Some(input) = spec.stdin {
            if let Some(mut stdin) = child.stdin.take() {
                stdin
                    .write_all(&input)
                    .await
                    .map_err(|error| Error::new(ErrorCode::Internal, error.to_string()))?;
            }
        }

        let result = timeout(duration, child.wait_with_output()).await;
        match result {
            Ok(result) => {
                let output =
                    result.map_err(|error| Error::new(ErrorCode::Internal, error.to_string()))?;
                Ok(Output {
                    status: output.status.code(),
                    stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                    stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
                })
            }
            Err(Elapsed { .. }) => Err(Error::new(
                ErrorCode::Timeout,
                format!("command timed out after {} seconds", duration.as_secs()),
            )),
        }
    }
}
