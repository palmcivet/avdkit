use std::{
    future::Future,
    path::{Path, PathBuf},
    pin::Pin,
    time::Duration,
};

use model::{EnvironmentSnapshot, Error, ErrorCode, ToolState};
use process::{CancellationToken, CommandSpec, Runner};

pub(crate) const COMMAND_TIMEOUT: Duration = Duration::from_secs(30);

pub(crate) type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub(crate) trait ChildProcess: Send {
    fn id(&self) -> u32;
    fn try_exit(&mut self) -> Result<Option<Option<i32>>, Error>;
}

/// Side-effecting host access shared by plan execution and the runtime lifecycle.
pub(crate) trait Backend: Send + Sync {
    fn execute<'a>(
        &'a self,
        invocation: drivers::Invocation,
        cancellation: &'a CancellationToken,
        timeout: Duration,
    ) -> BoxFuture<'a, Result<process::Output, Error>>;

    fn spawn_detached(
        &self,
        spec: CommandSpec,
        stdout_path: &Path,
        stderr_path: &Path,
        cancellation: &CancellationToken,
    ) -> Result<Box<dyn ChildProcess>, Error>;

    fn process_executable(&self, pid: u32) -> Result<Option<PathBuf>, Error>;

    fn signal(&self, pid: u32, signal: platform::ProcessSignal) -> Result<(), Error>;
}

pub(crate) struct RealBackend {
    runner: Runner,
}

impl RealBackend {
    pub(crate) fn new() -> Self {
        Self {
            runner: Runner::default(),
        }
    }
}

struct RealChild(process::DetachedProcess);

impl ChildProcess for RealChild {
    fn id(&self) -> u32 {
        self.0.id()
    }

    fn try_exit(&mut self) -> Result<Option<Option<i32>>, Error> {
        self.0.try_wait()
    }
}

impl Backend for RealBackend {
    fn execute<'a>(
        &'a self,
        invocation: drivers::Invocation,
        cancellation: &'a CancellationToken,
        timeout: Duration,
    ) -> BoxFuture<'a, Result<process::Output, Error>> {
        Box::pin(self.runner.run_cancellable_with_timeout(
            invocation.command(),
            timeout,
            cancellation,
        ))
    }

    fn spawn_detached(
        &self,
        spec: CommandSpec,
        stdout_path: &Path,
        stderr_path: &Path,
        cancellation: &CancellationToken,
    ) -> Result<Box<dyn ChildProcess>, Error> {
        self.runner
            .spawn_detached(spec, stdout_path, stderr_path, cancellation)
            .map(|process| Box::new(RealChild(process)) as Box<dyn ChildProcess>)
    }

    fn process_executable(&self, pid: u32) -> Result<Option<PathBuf>, Error> {
        platform::process_executable(pid).map_err(process::platform_error)
    }

    fn signal(&self, pid: u32, signal: platform::ProcessSignal) -> Result<(), Error> {
        platform::signal_process_group(pid, signal).map_err(process::platform_error)
    }
}

pub(crate) fn tool_path(snapshot: &EnvironmentSnapshot, name: &str) -> Result<PathBuf, Error> {
    snapshot
        .tools
        .iter()
        .find(|tool| tool.name == name && tool.state == ToolState::Available)
        .and_then(|tool| tool.path.clone())
        .ok_or_else(|| Error::new(ErrorCode::ToolNotFound, format!("{name} was not found")))
}

pub(crate) async fn blocking<T, F>(work: F) -> Result<T, Error>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, Error> + Send + 'static,
{
    tokio::task::spawn_blocking(work).await.map_err(|error| {
        Error::new(
            ErrorCode::Internal,
            format!("blocking task failed: {error}"),
        )
    })?
}
