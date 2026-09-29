use std::{
    fs,
    future::Future,
    path::{Path, PathBuf},
    pin::Pin,
    sync::Arc,
    time::Duration,
};

use model::{
    AvdId, BootStatus, Diagnostic, EnvironmentSnapshot, Error, ErrorCode, Event, Field,
    OperationResult, RunningInstance, Serial, ToolState,
};
use process::{CancellationToken, CommandSpec, Runner};
use tokio::{sync::mpsc, time::Instant};

const ADB_QUERY_TIMEOUT: Duration = Duration::from_secs(5);
const DISCOVERY_TIMEOUT: Duration = Duration::from_secs(15);
const ADB_CONNECT_TIMEOUT: Duration = Duration::from_secs(60);
const BOOT_TIMEOUT: Duration = Duration::from_secs(300);
const STOP_TIMEOUT: Duration = Duration::from_secs(10);
const SIGNAL_TIMEOUT: Duration = Duration::from_secs(5);
const POLL_INTERVAL: Duration = Duration::from_millis(250);

#[derive(Clone, Copy)]
struct LifecycleTimeouts {
    discovery: Duration,
    adb_connect: Duration,
    boot: Duration,
    stop: Duration,
    signal: Duration,
    poll: Duration,
}

const PRESET_TIMEOUTS: LifecycleTimeouts = LifecycleTimeouts {
    discovery: DISCOVERY_TIMEOUT,
    adb_connect: ADB_CONNECT_TIMEOUT,
    boot: BOOT_TIMEOUT,
    stop: STOP_TIMEOUT,
    signal: SIGNAL_TIMEOUT,
    poll: POLL_INTERVAL,
};

trait ChildProcess: Send {
    fn id(&self) -> u32;
    fn try_exit(&mut self) -> Result<Option<Option<i32>>, Error>;
}

trait Backend: Send + Sync {
    fn execute<'a>(
        &'a self,
        invocation: drivers::Invocation,
        cancellation: &'a CancellationToken,
        timeout: Duration,
    ) -> Pin<Box<dyn Future<Output = Result<process::Output, Error>> + Send + 'a>>;

    fn spawn_detached(
        &self,
        spec: CommandSpec,
        stdout_path: &Path,
        stderr_path: &Path,
        cancellation: &CancellationToken,
    ) -> Result<Box<dyn ChildProcess>, Error>;

    fn process_exists(&self, pid: u32) -> bool;

    fn signal(&self, pid: u32, signal: platform::ProcessSignal) -> Result<(), Error>;
}

struct RealBackend {
    runner: Runner,
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
    ) -> Pin<Box<dyn Future<Output = Result<process::Output, Error>> + Send + 'a>> {
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

    fn process_exists(&self, pid: u32) -> bool {
        platform::process_exists(pid)
    }

    fn signal(&self, pid: u32, signal: platform::ProcessSignal) -> Result<(), Error> {
        platform::signal_process_group(pid, signal)
            .map_err(|error| Error::new(ErrorCode::Internal, error.to_string()))
    }
}

#[derive(Debug, Clone)]
struct DiscoveryRecord {
    id: AvdId,
    serial: String,
    pid: u32,
}

#[derive(Debug, Clone)]
struct DiscoveredInstance {
    public: RunningInstance,
    state: String,
}

#[derive(Clone, Copy)]
struct StopStrategy {
    use_adb: bool,
    metrics: model::AndroidCliMetrics,
}

struct StartedProcessGuard {
    backend: Arc<dyn Backend>,
    pid: u32,
    armed: bool,
}

impl StartedProcessGuard {
    fn new(backend: Arc<dyn Backend>, pid: u32) -> Self {
        Self {
            backend,
            pid,
            armed: true,
        }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for StartedProcessGuard {
    fn drop(&mut self) {
        if self.armed {
            let _ = self.backend.signal(self.pid, platform::ProcessSignal::Kill);
        }
    }
}

pub(crate) async fn running(snapshot: &EnvironmentSnapshot) -> Result<Vec<RunningInstance>, Error> {
    let cancellation = CancellationToken::new();
    let backend = RealBackend {
        runner: Runner::default(),
    };
    discover(&backend, snapshot, &cancellation)
        .await
        .map(|instances| {
            instances
                .into_iter()
                .map(|instance| instance.public)
                .collect()
        })
}

pub(crate) async fn boot_status(
    snapshot: &EnvironmentSnapshot,
    id: &AvdId,
) -> Result<BootStatus, Error> {
    let cancellation = CancellationToken::new();
    let backend = RealBackend {
        runner: Runner::default(),
    };
    boot_status_with_backend(&backend, snapshot, id, &cancellation).await
}

pub(crate) async fn start(
    id: AvdId,
    snapshot: EnvironmentSnapshot,
    events: mpsc::UnboundedSender<Event>,
    cancellation: CancellationToken,
) -> Result<OperationResult, Error> {
    start_with_backend(
        id,
        snapshot,
        events,
        cancellation,
        Arc::new(RealBackend {
            runner: Runner::default(),
        }),
        PRESET_TIMEOUTS,
    )
    .await
}

pub(crate) async fn stop(
    id: AvdId,
    snapshot: EnvironmentSnapshot,
    use_adb: bool,
    metrics: model::AndroidCliMetrics,
    events: mpsc::UnboundedSender<Event>,
    cancellation: CancellationToken,
) -> Result<OperationResult, Error> {
    stop_with_backend(
        id,
        snapshot,
        StopStrategy { use_adb, metrics },
        events,
        cancellation,
        Arc::new(RealBackend {
            runner: Runner::default(),
        }),
        PRESET_TIMEOUTS,
    )
    .await
}

async fn discover(
    backend: &dyn Backend,
    snapshot: &EnvironmentSnapshot,
    cancellation: &CancellationToken,
) -> Result<Vec<DiscoveredInstance>, Error> {
    let adb = tool_path(snapshot, &snapshot.tool_names.adb)?;
    let environment = drivers::ToolEnvironment::from(&snapshot.paths);
    let output = backend
        .execute(
            drivers::Invocation::adb(adb.clone(), ["devices".into(), "-l".into()])
                .with_environment(&environment),
            cancellation,
            ADB_QUERY_TIMEOUT,
        )
        .await?;
    let devices = drivers::adb::parse_devices(&output)?;
    let records = discovery_records(snapshot, backend).await?;
    let mut instances = Vec::new();
    for device in devices
        .into_iter()
        .filter(|device| device.serial.starts_with("emulator-"))
    {
        let record = records
            .iter()
            .find(|record| record.serial == device.serial)
            .cloned();
        let identity = if let Some(record) = record {
            Some((record.id, Field::present(record.pid)))
        } else if device.state == "device" {
            resolve_name(backend, &adb, &environment, &device.serial, cancellation)
                .await?
                .map(|id| (id, Field::Unavailable))
        } else {
            None
        };
        let Some((id, pid)) = identity else {
            continue;
        };
        let serial = Serial::new(device.serial).map_err(Error::from)?;
        instances.push(DiscoveredInstance {
            public: RunningInstance {
                id,
                serial: Field::present(serial),
                pid,
            },
            state: device.state,
        });
    }
    instances.sort_by(|left, right| {
        left.public
            .id
            .as_str()
            .cmp(right.public.id.as_str())
            .then_with(|| field_serial(&left.public.serial).cmp(field_serial(&right.public.serial)))
    });
    Ok(instances)
}

async fn resolve_name(
    backend: &dyn Backend,
    adb: &Path,
    environment: &drivers::ToolEnvironment,
    serial: &str,
    cancellation: &CancellationToken,
) -> Result<Option<AvdId>, Error> {
    let property = backend
        .execute(
            drivers::Invocation::adb(
                adb.to_path_buf(),
                [
                    "-s".into(),
                    serial.into(),
                    "shell".into(),
                    "getprop".into(),
                    "ro.boot.qemu.avd_name".into(),
                ],
            )
            .with_environment(environment),
            cancellation,
            ADB_QUERY_TIMEOUT,
        )
        .await;
    match property {
        Ok(output) => {
            if let Some(id) = drivers::adb::parse_avd_name(&output)? {
                return Ok(Some(id));
            }
        }
        Err(error) if error.code == ErrorCode::Timeout => {}
        Err(error) => return Err(error),
    }
    let console = backend
        .execute(
            drivers::Invocation::adb(
                adb.to_path_buf(),
                [
                    "-s".into(),
                    serial.into(),
                    "emu".into(),
                    "avd".into(),
                    "name".into(),
                ],
            )
            .with_environment(environment),
            cancellation,
            Duration::from_secs(2),
        )
        .await;
    match console {
        Ok(output) => drivers::adb::parse_avd_name(&output),
        Err(error) if matches!(error.code, ErrorCode::Timeout) => Ok(None),
        Err(error) => Err(error),
    }
}

async fn boot_status_with_backend(
    backend: &dyn Backend,
    snapshot: &EnvironmentSnapshot,
    id: &AvdId,
    cancellation: &CancellationToken,
) -> Result<BootStatus, Error> {
    let instances = discover(backend, snapshot, cancellation).await?;
    let Some(instance) = instances
        .into_iter()
        .find(|instance| &instance.public.id == id)
    else {
        return Ok(BootStatus::Offline);
    };
    if instance.state != "device" {
        return Ok(BootStatus::Booting);
    }
    if readiness(backend, snapshot, &instance.public, cancellation).await? {
        Ok(BootStatus::Ready)
    } else {
        Ok(BootStatus::Booting)
    }
}

async fn readiness(
    backend: &dyn Backend,
    snapshot: &EnvironmentSnapshot,
    instance: &RunningInstance,
    cancellation: &CancellationToken,
) -> Result<bool, Error> {
    let adb = tool_path(snapshot, &snapshot.tool_names.adb)?;
    let serial = field_serial_value(&instance.serial)
        .ok_or_else(|| Error::new(ErrorCode::Internal, "running emulator has no adb serial"))?;
    let environment = drivers::ToolEnvironment::from(&snapshot.paths);
    let boot = backend
        .execute(
            drivers::Invocation::adb(
                adb.clone(),
                [
                    "-s".into(),
                    serial.into(),
                    "shell".into(),
                    "getprop".into(),
                    "sys.boot_completed".into(),
                ],
            )
            .with_environment(&environment),
            cancellation,
            ADB_QUERY_TIMEOUT,
        )
        .await;
    let boot = match boot {
        Ok(output) => output,
        Err(error) if error.code == ErrorCode::Cancelled => return Err(error),
        Err(_) => return Ok(false),
    };
    if !drivers::adb::boot_completed(&boot) {
        return Ok(false);
    }
    let packages = backend
        .execute(
            drivers::Invocation::adb(
                adb,
                [
                    "-s".into(),
                    serial.into(),
                    "shell".into(),
                    "pm".into(),
                    "path".into(),
                    "android".into(),
                ],
            )
            .with_environment(&environment),
            cancellation,
            ADB_QUERY_TIMEOUT,
        )
        .await;
    match packages {
        Ok(output) => Ok(drivers::adb::package_manager_ready(&output)),
        Err(error) if error.code == ErrorCode::Cancelled => Err(error),
        Err(_) => Ok(false),
    }
}

async fn start_with_backend(
    id: AvdId,
    snapshot: EnvironmentSnapshot,
    events: mpsc::UnboundedSender<Event>,
    cancellation: CancellationToken,
    backend: Arc<dyn Backend>,
    timeouts: LifecycleTimeouts,
) -> Result<OperationResult, Error> {
    if let Some(instance) = discover(backend.as_ref(), &snapshot, &cancellation)
        .await?
        .into_iter()
        .find(|instance| instance.public.id == id)
    {
        return Ok(OperationResult::DeviceStarted {
            instance: instance.public,
        });
    }
    let avd_root = snapshot.paths.avd_root.clone();
    let user_root = snapshot.paths.user_root.clone();
    let id_for_check = id.clone();
    blocking(move || {
        avdfs::AvdStore::new(avd_root, user_root)
            .get(&id_for_check)
            .map(|_| ())
    })
    .await?;

    let emulator = tool_path(&snapshot, &snapshot.tool_names.emulator)?;
    let environment = drivers::ToolEnvironment::from(&snapshot.paths);
    let log_directory = snapshot.paths.data_root.join("logs").join(id.as_str());
    let stdout_path = log_directory.join("stdout.log");
    let stderr_path = log_directory.join("stderr.log");
    blocking({
        let log_directory = log_directory.clone();
        move || fs::create_dir_all(&log_directory).map_err(|error| io_error(&log_directory, error))
    })
    .await?;

    send_started(&events, "launch", "launch emulator in a detached session");
    let invocation =
        drivers::Invocation::emulator(emulator, ["-avd".into(), id.as_str().to_owned()])
            .with_environment(&environment);
    let mut child = backend.spawn_detached(
        invocation.command(),
        &stdout_path,
        &stderr_path,
        &cancellation,
    )?;
    let launched_pid = child.id();
    let mut process_guard = StartedProcessGuard::new(Arc::clone(&backend), launched_pid);
    send_finished(
        &events,
        "launch",
        format!("emulator process {launched_pid} started"),
    );

    let started = Instant::now();
    let mut discovery_finished = false;
    let mut adb_finished = false;
    send_started(
        &events,
        "wait_discovery",
        "wait for emulator discovery file",
    );
    loop {
        if cancellation.is_cancelled() {
            return Err(Error::new(ErrorCode::Cancelled, "emulator start cancelled"));
        }
        if let Some(status) = child.try_exit()? {
            return Err(launch_error(&id, status, &stdout_path, &stderr_path));
        }

        if !discovery_finished {
            let records = discovery_records(&snapshot, backend.as_ref()).await?;
            if records
                .iter()
                .any(|record| record.id == id && record.pid == launched_pid)
            {
                discovery_finished = true;
                send_finished(&events, "wait_discovery", "discovery file is live");
                send_started(&events, "wait_adb", "wait for adb connection");
            } else if started.elapsed() >= timeouts.discovery {
                return Err(Error::new(
                    ErrorCode::Timeout,
                    "timed out waiting for emulator discovery",
                ));
            }
        }

        if discovery_finished {
            let instances = discover(backend.as_ref(), &snapshot, &cancellation).await?;
            if let Some(instance) = instances
                .into_iter()
                .find(|instance| instance.public.id == id)
            {
                if !adb_finished {
                    adb_finished = true;
                    send_finished(&events, "wait_adb", "emulator appeared in adb");
                    send_started(&events, "wait_boot", "wait for Android boot readiness");
                }
                if readiness(backend.as_ref(), &snapshot, &instance.public, &cancellation).await? {
                    send_finished(&events, "wait_boot", "Android is ready");
                    process_guard.disarm();
                    return Ok(OperationResult::DeviceStarted {
                        instance: instance.public,
                    });
                }
            } else if !adb_finished && started.elapsed() >= timeouts.adb_connect {
                return Err(Error::new(
                    ErrorCode::Timeout,
                    "timed out waiting for emulator adb connection",
                ));
            }
        }

        if adb_finished && started.elapsed() >= timeouts.boot {
            return Err(Error::new(
                ErrorCode::Timeout,
                "timed out waiting for Android to boot",
            ));
        }
        tokio::time::sleep(timeouts.poll).await;
    }
}

async fn stop_with_backend(
    id: AvdId,
    snapshot: EnvironmentSnapshot,
    strategy: StopStrategy,
    events: mpsc::UnboundedSender<Event>,
    cancellation: CancellationToken,
    backend: Arc<dyn Backend>,
    timeouts: LifecycleTimeouts,
) -> Result<OperationResult, Error> {
    let records = discovery_records(&snapshot, backend.as_ref()).await?;
    let matching_records = records
        .into_iter()
        .filter(|record| record.id == id)
        .collect::<Vec<_>>();
    if strategy.use_adb {
        let instances = discover(backend.as_ref(), &snapshot, &cancellation)
            .await?
            .into_iter()
            .filter(|instance| instance.public.id == id)
            .collect::<Vec<_>>();
        if instances.is_empty() && matching_records.is_empty() {
            return Ok(OperationResult::DeviceStopped { id });
        }
        send_started(&events, "stop", "request emulator shutdown through adb");
        let adb = tool_path(&snapshot, &snapshot.tool_names.adb)?;
        let environment = drivers::ToolEnvironment::from(&snapshot.paths);
        for instance in &instances {
            let Some(serial) = field_serial_value(&instance.public.serial) else {
                continue;
            };
            let output = backend
                .execute(
                    drivers::Invocation::adb(
                        adb.clone(),
                        ["-s".into(), serial.into(), "emu".into(), "kill".into()],
                    )
                    .with_environment(&environment),
                    &cancellation,
                    ADB_QUERY_TIMEOUT,
                )
                .await;
            match output {
                Ok(output) if drivers::adb::emu_kill_succeeded(&output) => {}
                Ok(_) => {
                    let _ = events.send(Event::Warning {
                        message: format!("adb did not confirm shutdown for {serial}"),
                    });
                }
                Err(error) if error.code == ErrorCode::Cancelled => return Err(error),
                Err(error) => {
                    let _ = events.send(Event::Warning {
                        message: format!("adb shutdown request failed for {serial}: {error}"),
                    });
                }
            }
        }
        send_finished(&events, "stop", "shutdown request sent");
        let serials = instances
            .iter()
            .filter_map(|instance| field_serial_value(&instance.public.serial).map(str::to_owned))
            .collect::<Vec<_>>();
        let pids = matching_records
            .iter()
            .map(|record| record.pid)
            .collect::<Vec<_>>();
        wait_then_force(
            backend.as_ref(),
            &snapshot,
            &serials,
            &pids,
            &events,
            &cancellation,
            timeouts,
        )
        .await?;
    } else {
        if matching_records.is_empty() {
            return Ok(OperationResult::DeviceStopped { id });
        }
        send_started(
            &events,
            "stop",
            "request emulator shutdown through Android CLI",
        );
        let android = tool_path(&snapshot, &snapshot.tool_names.android)?;
        let environment = drivers::ToolEnvironment::from(&snapshot.paths);
        let output = backend
            .execute(
                drivers::Invocation::android(
                    android,
                    &snapshot.paths.sdk_root,
                    strategy.metrics,
                    ["emulator".into(), "stop".into(), id.as_str().into()],
                )
                .with_environment(&environment),
                &cancellation,
                STOP_TIMEOUT,
            )
            .await;
        match output {
            Ok(output) => {
                if let Err(error) = drivers::android::require_success(
                    &output,
                    "Android CLI failed to stop the emulator",
                ) {
                    let _ = events.send(Event::Warning {
                        message: error.to_string(),
                    });
                }
            }
            Err(error) if error.code == ErrorCode::Cancelled => return Err(error),
            Err(error) => {
                let _ = events.send(Event::Warning {
                    message: format!("Android CLI shutdown request failed: {error}"),
                });
            }
        }
        send_finished(&events, "stop", "shutdown request completed");
        let pids = matching_records
            .iter()
            .map(|record| record.pid)
            .collect::<Vec<_>>();
        wait_then_force(
            backend.as_ref(),
            &snapshot,
            &[],
            &pids,
            &events,
            &cancellation,
            timeouts,
        )
        .await?;
    }
    Ok(OperationResult::DeviceStopped { id })
}

async fn wait_then_force(
    backend: &dyn Backend,
    snapshot: &EnvironmentSnapshot,
    serials: &[String],
    pids: &[u32],
    events: &mpsc::UnboundedSender<Event>,
    cancellation: &CancellationToken,
    timeouts: LifecycleTimeouts,
) -> Result<(), Error> {
    send_started(
        events,
        "wait_stop",
        "wait for adb and emulator process exit",
    );
    if wait_stopped(
        backend,
        snapshot,
        serials,
        pids,
        timeouts.stop,
        timeouts.poll,
        cancellation,
    )
    .await?
    {
        send_finished(events, "wait_stop", "emulator stopped");
        return Ok(());
    }
    let _ = events.send(Event::Warning {
        message: "graceful stop timed out; forcing the emulator may damage snapshots".into(),
    });
    for pid in pids {
        backend.signal(*pid, platform::ProcessSignal::Terminate)?;
    }
    if wait_stopped(
        backend,
        snapshot,
        serials,
        pids,
        timeouts.signal,
        timeouts.poll,
        cancellation,
    )
    .await?
    {
        send_finished(events, "wait_stop", "emulator terminated");
        return Ok(());
    }
    for pid in pids {
        backend.signal(*pid, platform::ProcessSignal::Kill)?;
    }
    if wait_stopped(
        backend,
        snapshot,
        serials,
        pids,
        timeouts.signal,
        timeouts.poll,
        cancellation,
    )
    .await?
    {
        send_finished(events, "wait_stop", "emulator killed");
        Ok(())
    } else {
        Err(Error::new(
            ErrorCode::Timeout,
            "emulator remained alive after forced stop",
        ))
    }
}

async fn wait_stopped(
    backend: &dyn Backend,
    snapshot: &EnvironmentSnapshot,
    serials: &[String],
    pids: &[u32],
    timeout: Duration,
    poll: Duration,
    cancellation: &CancellationToken,
) -> Result<bool, Error> {
    let deadline = Instant::now() + timeout;
    loop {
        if cancellation.is_cancelled() {
            return Err(Error::new(ErrorCode::Cancelled, "emulator stop cancelled"));
        }
        let processes_stopped = pids.iter().all(|pid| !backend.process_exists(*pid));
        let serials_stopped = if serials.is_empty() {
            true
        } else {
            let adb = tool_path(snapshot, &snapshot.tool_names.adb)?;
            let environment = drivers::ToolEnvironment::from(&snapshot.paths);
            let output = backend
                .execute(
                    drivers::Invocation::adb(adb, ["devices".into(), "-l".into()])
                        .with_environment(&environment),
                    cancellation,
                    ADB_QUERY_TIMEOUT,
                )
                .await?;
            let devices = drivers::adb::parse_devices(&output)?;
            serials
                .iter()
                .all(|serial| devices.iter().all(|device| &device.serial != serial))
        };
        if processes_stopped && serials_stopped {
            return Ok(true);
        }
        if Instant::now() >= deadline {
            return Ok(false);
        }
        tokio::time::sleep(poll).await;
    }
}

async fn discovery_records(
    snapshot: &EnvironmentSnapshot,
    backend: &dyn Backend,
) -> Result<Vec<DiscoveryRecord>, Error> {
    let root = snapshot.paths.runtime_root.clone();
    let records = blocking(move || read_discovery_records(&root)).await?;
    Ok(records
        .into_iter()
        .filter(|record| backend.process_exists(record.pid))
        .collect())
}

fn read_discovery_records(root: &Path) -> Result<Vec<DiscoveryRecord>, Error> {
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(io_error(root, error)),
    };
    let mut records = Vec::new();
    for entry in entries {
        let path = entry.map_err(|error| io_error(root, error))?.path();
        let Some(pid) = path
            .file_stem()
            .and_then(|name| name.to_str())
            .and_then(|name| name.strip_prefix("pid_"))
            .and_then(|pid| pid.parse::<u32>().ok())
        else {
            continue;
        };
        let contents = match fs::read_to_string(&path) {
            Ok(contents) => contents,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(io_error(&path, error)),
        };
        let document = avdfs::IniDocument::parse(&contents);
        let Some(id) = document
            .get("avd.id")
            .and_then(|value| AvdId::new(value.to_owned()).ok())
        else {
            continue;
        };
        let Some(serial) = document.get("port.serial").map(normalize_serial) else {
            continue;
        };
        records.push(DiscoveryRecord { id, serial, pid });
    }
    records.sort_by_key(|record| record.pid);
    Ok(records)
}

fn normalize_serial(value: &str) -> String {
    if value.starts_with("emulator-") {
        value.to_owned()
    } else {
        format!("emulator-{value}")
    }
}

fn field_serial(field: &Field<Serial>) -> &str {
    field_serial_value(field).unwrap_or_default()
}

fn field_serial_value(field: &Field<Serial>) -> Option<&str> {
    field.as_value().map(Serial::as_str)
}

fn tool_path(snapshot: &EnvironmentSnapshot, name: &str) -> Result<PathBuf, Error> {
    snapshot
        .tools
        .iter()
        .find(|tool| tool.name == name && tool.state == ToolState::Available)
        .and_then(|tool| tool.path.clone())
        .ok_or_else(|| Error::new(ErrorCode::ToolNotFound, format!("{name} was not found")))
}

fn launch_error(id: &AvdId, status: Option<i32>, stdout_path: &Path, stderr_path: &Path) -> Error {
    let mut error = Error::new(
        ErrorCode::LaunchFailed,
        format!("emulator for {id} exited before Android was ready"),
    );
    error.diagnostic = Some(Diagnostic {
        command: Some(format!("emulator -avd {id}")),
        stdout: fs::read_to_string(stdout_path).ok(),
        stderr: fs::read_to_string(stderr_path).ok(),
        exit_status: status,
    });
    error
}

fn send_started(events: &mpsc::UnboundedSender<Event>, step: &str, message: impl Into<String>) {
    let _ = events.send(Event::StepStarted {
        step: step.into(),
        message: message.into(),
    });
}

fn send_finished(events: &mpsc::UnboundedSender<Event>, step: &str, message: impl Into<String>) {
    let _ = events.send(Event::StepFinished {
        step: step.into(),
        message: message.into(),
    });
}

async fn blocking<T, F>(work: F) -> Result<T, Error>
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

fn io_error(path: &Path, error: std::io::Error) -> Error {
    Error::new(
        ErrorCode::Internal,
        format!(
            "runtime file operation failed for {}: {error}",
            path.display()
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use model::{
        AndroidCliMetrics, CpuArchitecture, Host, Platform, PlatformPaths, ToolNames, ToolSource,
        ToolStatus, ValueSource,
    };
    use std::sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    };

    static NEXT_ROOT: AtomicU64 = AtomicU64::new(1);

    #[derive(Default)]
    struct FakeState {
        alive: bool,
        serial_visible: bool,
        ready: bool,
        exit_immediately: bool,
        require_kill: bool,
        spawn_count: usize,
        signals: Vec<platform::ProcessSignal>,
    }

    struct FakeBackend {
        root: PathBuf,
        id: AvdId,
        pid: u32,
        state: Arc<Mutex<FakeState>>,
    }

    struct FakeChild {
        pid: u32,
        state: Arc<Mutex<FakeState>>,
    }

    impl ChildProcess for FakeChild {
        fn id(&self) -> u32 {
            self.pid
        }

        fn try_exit(&mut self) -> Result<Option<Option<i32>>, Error> {
            let state = self.state.lock().unwrap();
            Ok(state.exit_immediately.then_some(Some(1)))
        }
    }

    impl Backend for FakeBackend {
        fn execute<'a>(
            &'a self,
            invocation: drivers::Invocation,
            cancellation: &'a CancellationToken,
            _timeout: Duration,
        ) -> Pin<Box<dyn Future<Output = Result<process::Output, Error>> + Send + 'a>> {
            Box::pin(async move {
                if cancellation.is_cancelled() {
                    return Err(Error::new(ErrorCode::Cancelled, "cancelled"));
                }
                let args = invocation.args;
                let mut state = self.state.lock().unwrap();
                if args == ["devices", "-l"] {
                    let device = if state.serial_visible {
                        "emulator-5554 device transport_id:1\n"
                    } else {
                        ""
                    };
                    return Ok(output(format!("List of devices attached\n{device}")));
                }
                if args.ends_with(&[
                    "shell".into(),
                    "getprop".into(),
                    "ro.boot.qemu.avd_name".into(),
                ]) {
                    return Ok(output(format!("{}\n", self.id)));
                }
                if args.ends_with(&[
                    "shell".into(),
                    "getprop".into(),
                    "sys.boot_completed".into(),
                ]) {
                    return Ok(output(if state.ready { "1\n" } else { "\n" }));
                }
                if args.ends_with(&["shell".into(), "pm".into(), "path".into(), "android".into()]) {
                    return Ok(output(if state.ready {
                        "package:/system/framework/framework-res.apk\n"
                    } else {
                        "\n"
                    }));
                }
                if args.ends_with(&["emu".into(), "kill".into()]) {
                    if !state.require_kill {
                        state.alive = false;
                        state.serial_visible = false;
                    }
                    return Ok(output("OK: killing emulator, bye bye\n"));
                }
                if args.iter().any(|argument| argument == "stop") {
                    state.alive = false;
                    state.serial_visible = false;
                    return Ok(output("stopped\n"));
                }
                Ok(output(""))
            })
        }

        fn spawn_detached(
            &self,
            _spec: CommandSpec,
            stdout_path: &Path,
            stderr_path: &Path,
            _cancellation: &CancellationToken,
        ) -> Result<Box<dyn ChildProcess>, Error> {
            fs::write(stdout_path, "emulator stdout\n").unwrap();
            fs::write(stderr_path, "emulator stderr\n").unwrap();
            fs::create_dir_all(&self.root).unwrap();
            fs::write(
                self.root.join(format!("pid_{}.ini", self.pid)),
                format!("avd.id={}\nport.serial=5554\n", self.id),
            )
            .unwrap();
            let mut state = self.state.lock().unwrap();
            state.spawn_count += 1;
            state.alive = !state.exit_immediately;
            state.serial_visible = !state.exit_immediately;
            Ok(Box::new(FakeChild {
                pid: self.pid,
                state: Arc::clone(&self.state),
            }))
        }

        fn process_exists(&self, pid: u32) -> bool {
            pid == self.pid && self.state.lock().unwrap().alive
        }

        fn signal(&self, pid: u32, signal: platform::ProcessSignal) -> Result<(), Error> {
            assert_eq!(pid, self.pid);
            let mut state = self.state.lock().unwrap();
            state.signals.push(signal);
            if signal == platform::ProcessSignal::Kill || !state.require_kill {
                state.alive = false;
                state.serial_visible = false;
            }
            Ok(())
        }
    }

    fn output(stdout: impl Into<String>) -> process::Output {
        process::Output {
            status: Some(0),
            stdout: stdout.into(),
            stderr: String::new(),
        }
    }

    fn temp_root(suffix: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "{}runtime_{}_{}_{suffix}",
            model::test_avd_prefix(),
            std::process::id(),
            NEXT_ROOT.fetch_add(1, Ordering::Relaxed)
        ))
    }

    fn test_id(suffix: &str) -> AvdId {
        AvdId::new(format!("{}{suffix}", model::test_avd_prefix())).unwrap()
    }

    fn snapshot(root: &Path) -> EnvironmentSnapshot {
        let tool_names = ToolNames {
            android: "android".into(),
            adb: "adb".into(),
            emulator: "emulator".into(),
            sdkmanager: "sdkmanager".into(),
            avdmanager: "avdmanager".into(),
        };
        EnvironmentSnapshot {
            host: Host {
                platform: Platform::MacOs,
                architecture: CpuArchitecture::Arm64,
                android_abi: Field::present("arm64-v8a".into()),
                supported: true,
            },
            paths: PlatformPaths {
                sdk_root: root.join("sdk"),
                user_root: root.join("user"),
                avd_root: root.join("user/avd"),
                runtime_root: root.join("runtime"),
                data_root: root.join("data"),
            },
            sdk_root_source: ValueSource::CallerOverride,
            environment: Vec::new(),
            tool_names,
            tools: ["android", "adb", "emulator"]
                .into_iter()
                .map(|name| ToolStatus {
                    name: name.into(),
                    path: Some(PathBuf::from(name)),
                    version: Field::Unavailable,
                    state: ToolState::Available,
                    source: Some(ToolSource::SearchPath),
                    package: None,
                    reasons: Vec::new(),
                    diagnostic: None,
                })
                .collect(),
            legacy_tools: Vec::new(),
            installed_packages: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    fn configured_avd(snapshot: &EnvironmentSnapshot, id: &AvdId) {
        let directory = snapshot.paths.avd_root.join(format!("{id}.avd"));
        fs::create_dir_all(&directory).unwrap();
        fs::write(
            snapshot.paths.avd_root.join(format!("{id}.ini")),
            format!("path={}\ntarget=android-36\n", directory.display()),
        )
        .unwrap();
        fs::write(directory.join("config.ini"), "target=android-36\n").unwrap();
    }

    fn backend(root: &Path, id: &AvdId, state: FakeState) -> Arc<FakeBackend> {
        Arc::new(FakeBackend {
            root: root.join("runtime"),
            id: id.clone(),
            pid: 4242,
            state: Arc::new(Mutex::new(state)),
        })
    }

    fn fast_timeouts() -> LifecycleTimeouts {
        LifecycleTimeouts {
            discovery: Duration::from_millis(20),
            adb_connect: Duration::from_millis(20),
            boot: Duration::from_millis(20),
            stop: Duration::from_millis(1),
            signal: Duration::from_millis(1),
            poll: Duration::from_millis(1),
        }
    }

    #[tokio::test]
    async fn discovery_prefers_a_live_discovery_file() {
        let root = temp_root("discover");
        let id = test_id("discover");
        let snapshot = snapshot(&root);
        fs::create_dir_all(&snapshot.paths.runtime_root).unwrap();
        fs::write(
            snapshot.paths.runtime_root.join("pid_4242.ini"),
            format!("avd.id={id}\nport.serial=5554\n"),
        )
        .unwrap();
        let backend = backend(
            &root,
            &id,
            FakeState {
                alive: true,
                serial_visible: true,
                ..FakeState::default()
            },
        );
        let instances = discover(backend.as_ref(), &snapshot, &CancellationToken::new())
            .await
            .unwrap();

        assert_eq!(instances.len(), 1);
        assert_eq!(instances[0].public.id, id);
        assert_eq!(instances[0].public.pid.as_value(), Some(&4242));
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn discovery_falls_back_to_the_boot_property() {
        let root = temp_root("property-fallback");
        let id = test_id("property_fallback");
        let snapshot = snapshot(&root);
        let backend = backend(
            &root,
            &id,
            FakeState {
                serial_visible: true,
                ..FakeState::default()
            },
        );
        let instances = discover(backend.as_ref(), &snapshot, &CancellationToken::new())
            .await
            .unwrap();

        assert_eq!(instances.len(), 1);
        assert_eq!(instances[0].public.id, id);
        assert!(instances[0].public.pid.as_value().is_none());
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn starts_detached_and_waits_for_both_boot_conditions() {
        let root = temp_root("start");
        let id = test_id("start");
        let snapshot = snapshot(&root);
        configured_avd(&snapshot, &id);
        let backend = backend(
            &root,
            &id,
            FakeState {
                ready: true,
                ..FakeState::default()
            },
        );
        let (events, mut receiver) = mpsc::unbounded_channel();
        let result = start_with_backend(
            id.clone(),
            snapshot,
            events,
            CancellationToken::new(),
            backend.clone(),
            fast_timeouts(),
        )
        .await
        .unwrap();

        assert!(matches!(result, OperationResult::DeviceStarted { .. }));
        assert_eq!(backend.state.lock().unwrap().spawn_count, 1);
        let collected = std::iter::from_fn(|| receiver.try_recv().ok()).collect::<Vec<_>>();
        assert!(collected.iter().any(|event| {
            matches!(event, Event::StepFinished { step, .. } if step == "wait_boot")
        }));
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn early_process_exit_maps_to_launch_failed_with_logs() {
        let root = temp_root("launch-failed");
        let id = test_id("launch_failed");
        let snapshot = snapshot(&root);
        configured_avd(&snapshot, &id);
        let backend = backend(
            &root,
            &id,
            FakeState {
                exit_immediately: true,
                ..FakeState::default()
            },
        );
        let (events, _receiver) = mpsc::unbounded_channel();
        let error = start_with_backend(
            id,
            snapshot,
            events,
            CancellationToken::new(),
            backend,
            fast_timeouts(),
        )
        .await
        .unwrap_err();

        assert_eq!(error.code, ErrorCode::LaunchFailed);
        assert_eq!(
            error
                .diagnostic
                .as_ref()
                .and_then(|diagnostic| diagnostic.exit_status),
            Some(1)
        );
        assert!(error
            .diagnostic
            .as_ref()
            .and_then(|diagnostic| diagnostic.stderr.as_deref())
            .unwrap()
            .contains("emulator stderr"));
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn cancelling_start_kills_the_detached_process() {
        let root = temp_root("cancel-start");
        let id = test_id("cancel_start");
        let snapshot = snapshot(&root);
        configured_avd(&snapshot, &id);
        let backend = backend(&root, &id, FakeState::default());
        let cancellation = CancellationToken::new();
        let cancellation_for_task = cancellation.clone();
        let backend_for_task = backend.clone();
        let (events, _receiver) = mpsc::unbounded_channel();
        let task = tokio::spawn(async move {
            start_with_backend(
                id,
                snapshot,
                events,
                cancellation_for_task,
                backend_for_task,
                fast_timeouts(),
            )
            .await
        });
        loop {
            if backend.state.lock().unwrap().spawn_count == 1 {
                break;
            }
            tokio::task::yield_now().await;
        }
        cancellation.cancel();
        let error = task.await.unwrap().unwrap_err();

        assert_eq!(error.code, ErrorCode::Cancelled);
        assert_eq!(
            backend.state.lock().unwrap().signals,
            [platform::ProcessSignal::Kill]
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn stop_escalates_to_kill_and_warns() {
        let root = temp_root("stop");
        let id = test_id("stop");
        let snapshot = snapshot(&root);
        fs::create_dir_all(&snapshot.paths.runtime_root).unwrap();
        fs::write(
            snapshot.paths.runtime_root.join("pid_4242.ini"),
            format!("avd.id={id}\nport.serial=5554\n"),
        )
        .unwrap();
        let backend = backend(
            &root,
            &id,
            FakeState {
                alive: true,
                serial_visible: true,
                require_kill: true,
                ..FakeState::default()
            },
        );
        let (events, mut receiver) = mpsc::unbounded_channel();
        let result = stop_with_backend(
            id.clone(),
            snapshot,
            StopStrategy {
                use_adb: true,
                metrics: AndroidCliMetrics::Inherit,
            },
            events,
            CancellationToken::new(),
            backend.clone(),
            fast_timeouts(),
        )
        .await
        .unwrap();

        assert_eq!(result, OperationResult::DeviceStopped { id });
        assert_eq!(
            backend.state.lock().unwrap().signals,
            [
                platform::ProcessSignal::Terminate,
                platform::ProcessSignal::Kill
            ]
        );
        assert!(std::iter::from_fn(|| receiver.try_recv().ok())
            .any(|event| matches!(event, Event::Warning { .. })));
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn stop_uses_android_cli_when_adb_is_not_selected() {
        let root = temp_root("stop-fallback");
        let id = test_id("stop_fallback");
        let snapshot = snapshot(&root);
        fs::create_dir_all(&snapshot.paths.runtime_root).unwrap();
        fs::write(
            snapshot.paths.runtime_root.join("pid_4242.ini"),
            format!("avd.id={id}\nport.serial=5554\n"),
        )
        .unwrap();
        let backend = backend(
            &root,
            &id,
            FakeState {
                alive: true,
                ..FakeState::default()
            },
        );
        let (events, _receiver) = mpsc::unbounded_channel();
        let result = stop_with_backend(
            id.clone(),
            snapshot,
            StopStrategy {
                use_adb: false,
                metrics: AndroidCliMetrics::Inherit,
            },
            events,
            CancellationToken::new(),
            backend.clone(),
            fast_timeouts(),
        )
        .await
        .unwrap();

        assert_eq!(result, OperationResult::DeviceStopped { id });
        assert!(backend.state.lock().unwrap().signals.is_empty());
        fs::remove_dir_all(root).unwrap();
    }
}
