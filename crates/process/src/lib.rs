//! Process execution primitives shared by tool drivers.

#![allow(clippy::result_large_err)]

use std::{
    collections::BTreeMap,
    fs::OpenOptions,
    path::{Path, PathBuf},
    time::Duration,
};

use model::{Error, ErrorCode};
use tokio::{io::AsyncWriteExt, process::Command, sync::watch, time::sleep};

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

#[derive(Debug)]
pub struct DetachedProcess {
    child: tokio::process::Child,
    pid: u32,
}

impl DetachedProcess {
    pub fn id(&self) -> u32 {
        self.pid
    }

    pub fn try_wait(&mut self) -> Result<Option<Option<i32>>, Error> {
        self.child
            .try_wait()
            .map(|status| status.map(|status| status.code()))
            .map_err(wait_error)
    }
}

#[derive(Debug, Clone)]
pub struct CancellationToken {
    cancelled: watch::Sender<bool>,
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self::new()
    }
}

impl CancellationToken {
    pub fn new() -> Self {
        let (cancelled, _) = watch::channel(false);
        Self { cancelled }
    }

    pub fn cancel(&self) {
        self.cancelled.send_replace(true);
    }

    pub fn is_cancelled(&self) -> bool {
        *self.cancelled.borrow()
    }

    pub async fn cancelled(&self) {
        let mut cancelled = self.cancelled.subscribe();
        if *cancelled.borrow() {
            return;
        }
        let _ = cancelled.changed().await;
    }
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

    pub fn spawn_detached(
        &self,
        spec: CommandSpec,
        stdout_path: &Path,
        stderr_path: &Path,
        cancellation: &CancellationToken,
    ) -> Result<DetachedProcess, Error> {
        if cancellation.is_cancelled() {
            return Err(cancelled_error());
        }
        let stdout = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(stdout_path)
            .map_err(|error| Error::new(ErrorCode::Internal, error.to_string()))?;
        let stderr = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(stderr_path)
            .map_err(|error| Error::new(ErrorCode::Internal, error.to_string()))?;
        let mut command = Command::new(&spec.program);
        command
            .args(&spec.args)
            .envs(&spec.environment)
            .stdin(std::process::Stdio::null())
            .stdout(stdout)
            .stderr(stderr)
            .kill_on_drop(false);
        platform::prepare_detached_child(command.as_std_mut());
        if let Some(current_dir) = &spec.current_dir {
            command.current_dir(current_dir);
        }
        let child = command.spawn().map_err(spawn_error)?;
        let pid = child
            .id()
            .ok_or_else(|| Error::new(ErrorCode::Internal, "detached process has no identifier"))?;
        Ok(DetachedProcess { child, pid })
    }

    pub async fn run_cancellable(
        &self,
        spec: CommandSpec,
        cancellation: &CancellationToken,
    ) -> Result<Output, Error> {
        self.run_cancellable_with_timeout(spec, self.default_timeout, cancellation)
            .await
    }

    pub async fn run_with_timeout(
        &self,
        spec: CommandSpec,
        duration: Duration,
    ) -> Result<Output, Error> {
        self.run_controlled(spec, duration, None).await
    }

    pub async fn run_cancellable_with_timeout(
        &self,
        spec: CommandSpec,
        duration: Duration,
        cancellation: &CancellationToken,
    ) -> Result<Output, Error> {
        self.run_controlled(spec, duration, Some(cancellation))
            .await
    }

    async fn run_controlled(
        &self,
        spec: CommandSpec,
        duration: Duration,
        cancellation: Option<&CancellationToken>,
    ) -> Result<Output, Error> {
        if cancellation.is_some_and(CancellationToken::is_cancelled) {
            return Err(cancelled_error());
        }

        let mut command = Command::new(&spec.program);
        command
            .args(&spec.args)
            .envs(&spec.environment)
            .kill_on_drop(true);
        platform::prepare_child(command.as_std_mut());
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
            .map_err(spawn_error)?;
        let mut process_group = child.id().map(platform::ProcessGroup::for_child);

        if let Some(input) = spec.stdin {
            if let Some(mut stdin) = child.stdin.take() {
                stdin
                    .write_all(&input)
                    .await
                    .map_err(|error| Error::new(ErrorCode::Internal, error.to_string()))?;
            }
        }

        let result = if let Some(cancellation) = cancellation {
            tokio::select! {
                biased;
                _ = cancellation.cancelled() => Err(cancelled_error()),
                _ = sleep(duration) => Err(timeout_error(duration)),
                result = child.wait_with_output() => result.map_err(wait_error),
            }
        } else {
            tokio::select! {
                biased;
                _ = sleep(duration) => Err(timeout_error(duration)),
                result = child.wait_with_output() => result.map_err(wait_error),
            }
        };

        match result {
            Ok(output) => {
                if let Some(group) = &mut process_group {
                    group.disarm();
                }
                Ok(Output {
                    status: output.status.code(),
                    stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                    stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
                })
            }
            Err(error) => Err(error),
        }
    }
}

fn spawn_error(error: std::io::Error) -> Error {
    let code = if error.kind() == std::io::ErrorKind::NotFound {
        ErrorCode::ToolNotFound
    } else {
        ErrorCode::Internal
    };
    Error::new(code, error.to_string())
}

fn wait_error(error: std::io::Error) -> Error {
    Error::new(ErrorCode::Internal, error.to_string())
}

fn cancelled_error() -> Error {
    Error::new(ErrorCode::Cancelled, "command cancelled")
}

fn timeout_error(duration: Duration) -> Error {
    Error::new(
        ErrorCode::Timeout,
        format!("command timed out after {duration:?}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{env, thread};

    const HELPER_MODE: &str = "PROCESS_TEST_MODE";

    fn helper_spec(mode: &str) -> CommandSpec {
        CommandSpec {
            program: env::current_exe().unwrap(),
            args: vec![
                "--exact".into(),
                "tests::process_helper".into(),
                "--nocapture".into(),
            ],
            environment: [(HELPER_MODE.into(), mode.into())].into(),
            ..CommandSpec::default()
        }
    }

    #[test]
    fn process_helper() {
        match env::var(HELPER_MODE).as_deref() {
            Ok("output") => {
                println!("helper stdout");
                eprintln!("helper stderr");
            }
            Ok("sleep") => thread::sleep(Duration::from_secs(120)),
            _ => {}
        }
    }

    #[tokio::test]
    async fn captures_output_and_status() {
        let output = Runner::default().run(helper_spec("output")).await.unwrap();
        assert_eq!(output.status, Some(0));
        assert!(output.stdout.contains("helper stdout"));
        assert!(output.stderr.contains("helper stderr"));
    }

    #[tokio::test]
    async fn rejects_a_cancelled_command_before_spawning() {
        let cancellation = CancellationToken::new();
        cancellation.cancel();
        let error = Runner::default()
            .run_cancellable(
                CommandSpec {
                    program: PathBuf::from("definitely-does-not-exist"),
                    ..CommandSpec::default()
                },
                &cancellation,
            )
            .await
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::Cancelled);
    }

    #[tokio::test]
    async fn cancels_a_running_command() {
        let runner = Runner::default();
        let cancellation = CancellationToken::new();
        let cancellation_for_task = cancellation.clone();
        let task = tokio::spawn(async move {
            runner
                .run_cancellable(helper_spec("sleep"), &cancellation_for_task)
                .await
        });
        sleep(Duration::from_millis(100)).await;
        cancellation.cancel();

        let error = task.await.unwrap().unwrap_err();
        assert_eq!(error.code, ErrorCode::Cancelled);
    }

    #[tokio::test]
    async fn times_out_a_running_command() {
        let error = Runner::default()
            .run_with_timeout(helper_spec("sleep"), Duration::from_millis(50))
            .await
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::Timeout);
    }

    #[tokio::test]
    async fn detached_process_writes_logs_without_pipes() {
        let root = env::temp_dir().join(format!("detached-process-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let stdout = root.join("stdout.log");
        let stderr = root.join("stderr.log");
        let cancellation = CancellationToken::new();
        let mut process = Runner::default()
            .spawn_detached(helper_spec("output"), &stdout, &stderr, &cancellation)
            .unwrap();
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        while process.try_wait().unwrap().is_none() {
            assert!(tokio::time::Instant::now() < deadline);
            sleep(Duration::from_millis(10)).await;
        }
        assert!(std::fs::read_to_string(stdout)
            .unwrap()
            .contains("helper stdout"));
        assert!(std::fs::read_to_string(stderr)
            .unwrap()
            .contains("helper stderr"));
        std::fs::remove_dir_all(root).unwrap();
    }
}
