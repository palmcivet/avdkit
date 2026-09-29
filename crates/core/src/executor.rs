use std::{
    collections::HashSet,
    fs::{self, OpenOptions},
    future::Future,
    hash::{DefaultHasher, Hash, Hasher},
    io::{self, Write},
    path::{Path, PathBuf},
    pin::Pin,
    sync::{Arc, Mutex, OnceLock},
};

use model::{
    CompensationResult, CreateDeviceDraft, EnvironmentSnapshot, Error, ErrorCode, Event, LogStream,
    OperationResult, PackageKind, ToolState, PRODUCT_NAME,
};
use process::{CancellationToken, Runner};
use tokio::sync::mpsc;

static ACTIVE_WRITES: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

trait Backend: Send + Sync {
    fn execute<'a>(
        &'a self,
        invocation: drivers::Invocation,
        cancellation: &'a CancellationToken,
    ) -> Pin<Box<dyn Future<Output = Result<process::Output, Error>> + Send + 'a>>;
}

struct RealBackend {
    runner: Runner,
}

impl Backend for RealBackend {
    fn execute<'a>(
        &'a self,
        invocation: drivers::Invocation,
        cancellation: &'a CancellationToken,
    ) -> Pin<Box<dyn Future<Output = Result<process::Output, Error>> + Send + 'a>> {
        Box::pin(drivers::execute_cancellable(
            &self.runner,
            invocation,
            cancellation,
        ))
    }
}

pub(crate) async fn execute_create(
    operation_id: String,
    draft: CreateDeviceDraft,
    snapshot: EnvironmentSnapshot,
    metrics: model::AndroidCliMetrics,
    events: mpsc::UnboundedSender<Event>,
    cancellation: CancellationToken,
) -> Result<OperationResult, Error> {
    execute_create_with_backend(
        operation_id,
        draft,
        snapshot,
        metrics,
        events,
        cancellation,
        Arc::new(RealBackend {
            runner: Runner::default(),
        }),
    )
    .await
}

async fn execute_create_with_backend(
    operation_id: String,
    draft: CreateDeviceDraft,
    snapshot: EnvironmentSnapshot,
    metrics: model::AndroidCliMetrics,
    events: mpsc::UnboundedSender<Event>,
    cancellation: CancellationToken,
    backend: Arc<dyn Backend>,
) -> Result<OperationResult, Error> {
    let keys = lock_keys(&draft);
    let _process_lock = ProcessWriteLock::acquire(keys.clone())?;
    preflight(backend.as_ref(), &draft, &snapshot, &events, &cancellation).await?;

    check_cancelled(&cancellation)?;
    let directory_locks = blocking({
        let root = snapshot.paths.avd_root.clone();
        move || DirectoryLocks::acquire(&root, &keys)
    })
    .await?;
    blocking({
        let root = snapshot.paths.avd_root.clone();
        let user_root = snapshot.paths.user_root.clone();
        let source = draft.profile.clone();
        let target = draft.id.clone();
        move || check_name_conflicts(&root, &user_root, &source, &target)
    })
    .await?;

    let android = tool_path(&snapshot, &snapshot.tool_names.android)?;
    let environment = drivers::ToolEnvironment::from(&snapshot.paths);
    let invocation = drivers::Invocation::android(
        android,
        &snapshot.paths.sdk_root,
        metrics,
        [
            "emulator".into(),
            "create".into(),
            draft.profile.as_str().into(),
        ],
    )
    .with_environment(&environment);

    let mut created = false;
    let transaction = Arc::new(Mutex::new(None::<avdfs::CreateTransaction>));
    let result = async {
        start_step(&events, "create", "create an AVD from the selected profile");
        let output = match backend.execute(invocation, &cancellation).await {
            Ok(output) => output,
            Err(error) => {
                created = matches!(error.code, ErrorCode::Cancelled | ErrorCode::Timeout);
                return Err(failed(error, "create"));
            }
        };
        emit_output(&events, &output);
        drivers::android::require_success(&output, "Android CLI failed to create an AVD")
            .map_err(|error| failed(error, "create"))?;
        created = true;
        finish_step(&events, "create", "temporary profile created");

        check_cancelled(&cancellation).map_err(|error| failed(error, "rename"))?;
        start_step(&events, "rename", "move the AVD to its target identifier");
        let backup_root = snapshot.paths.data_root.join("backups").join(&operation_id);
        let created_transaction = blocking({
            let root = snapshot.paths.avd_root.clone();
            let user_root = snapshot.paths.user_root.clone();
            let source = model::AvdId::new(draft.profile.as_str()).map_err(Error::from)?;
            let target = draft.id.clone();
            move || avdfs::CreateTransaction::begin(root, user_root, backup_root, source, target)
        })
        .await
        .map_err(|error| failed(error, "rename"))?;
        *transaction
            .lock()
            .map_err(|_| Error::new(ErrorCode::Internal, "transaction lock is poisoned"))? =
            Some(created_transaction);
        transaction_action(&transaction, |transaction| transaction.rename())
            .await
            .map_err(|error| failed(error, "rename"))?;
        finish_step(&events, "rename", "AVD identifier and paths rewritten");

        check_cancelled(&cancellation).map_err(|error| failed(error, "image"))?;
        start_step(&events, "image", "configure the selected system image");
        let image = draft.image.clone();
        transaction_action(&transaction, move |transaction| {
            transaction.set_image(&image)
        })
        .await
        .map_err(|error| failed(error, "image"))?;
        finish_step(&events, "image", "system image and target rewritten");

        check_cancelled(&cancellation).map_err(|error| failed(error, "display_name"))?;
        start_step(&events, "display_name", "set the AVD display name");
        let display_name = draft
            .display_name
            .clone()
            .unwrap_or_else(|| draft.id.as_str().to_owned());
        transaction_action(&transaction, move |transaction| {
            transaction.set_display_name(&display_name)
        })
        .await
        .map_err(|error| failed(error, "display_name"))?;

        verify_created_device(&snapshot, &draft)
            .await
            .map_err(|error| failed(error, "display_name"))?;

        transaction_action(&transaction, avdfs::CreateTransaction::commit)
            .await
            .map_err(|error| failed(error, "display_name"))?;
        finish_step(
            &events,
            "display_name",
            "display name rewritten and AVD verified",
        );
        Ok(OperationResult::PlanCompleted {
            plan_id: format!("create:{}", draft.id),
        })
    }
    .await;

    let final_result = match result {
        Ok(value) => Ok(value),
        Err(mut error) => {
            if created || transaction_has_value(&transaction) {
                compensate(
                    &events,
                    &transaction,
                    &snapshot.paths.avd_root,
                    &draft,
                    &mut error,
                )
                .await;
            }
            Err(error)
        }
    };
    drop(directory_locks);
    final_result
}

async fn preflight(
    backend: &dyn Backend,
    draft: &CreateDeviceDraft,
    snapshot: &EnvironmentSnapshot,
    events: &mpsc::UnboundedSender<Event>,
    cancellation: &CancellationToken,
) -> Result<(), Error> {
    validate_image(&draft.image)?;
    tool_path(snapshot, &snapshot.tool_names.android)?;
    tool_path(snapshot, &snapshot.tool_names.emulator)?;
    let adb = tool_path(snapshot, &snapshot.tool_names.adb)?;

    require_package(snapshot, PackageKind::Emulator, "emulator")?;
    require_package(snapshot, PackageKind::PlatformTools, "platform-tools")?;
    if !snapshot
        .installed_packages
        .iter()
        .any(|package| package.installed && package.id == draft.image)
        || !snapshot
            .paths
            .sdk_root
            .join(draft.image.render_slash())
            .is_dir()
    {
        return Err(Error::new(
            ErrorCode::PackageNotFound,
            format!(
                "system image {} is not installed",
                draft.image.render_semicolon()
            ),
        ));
    }

    blocking({
        let root = snapshot.paths.avd_root.clone();
        let user_root = snapshot.paths.user_root.clone();
        let source = draft.profile.clone();
        let target = draft.id.clone();
        move || check_name_conflicts(&root, &user_root, &source, &target)
    })
    .await?;

    check_cancelled(cancellation)?;
    let environment = drivers::ToolEnvironment::from(&snapshot.paths);
    let output = backend
        .execute(
            drivers::Invocation::adb(adb.clone(), ["devices".into(), "-l".into()])
                .with_environment(&environment),
            cancellation,
        )
        .await?;
    emit_output(events, &output);
    for device in drivers::adb::parse_devices(&output)?
        .into_iter()
        .filter(|device| device.serial.starts_with("emulator-"))
    {
        if device.state != "device" {
            return Err(Error::new(
                ErrorCode::PreconditionFailed,
                format!(
                    "cannot determine the AVD name for {} in state {}",
                    device.serial, device.state
                ),
            ));
        }
        let output = backend
            .execute(
                drivers::Invocation::adb(
                    adb.clone(),
                    [
                        "-s".into(),
                        device.serial,
                        "shell".into(),
                        "getprop".into(),
                        "ro.boot.qemu.avd_name".into(),
                    ],
                )
                .with_environment(&environment),
                cancellation,
            )
            .await?;
        emit_output(events, &output);
        if output.status != Some(0) {
            return Err(Error::new(
                ErrorCode::PreconditionFailed,
                "could not determine a running emulator's AVD identifier",
            ));
        }
        if output.stdout.trim() == draft.id.as_str() {
            return Err(Error::new(
                ErrorCode::DeviceRunning,
                format!("Android virtual device {} is running", draft.id),
            ));
        }
    }
    Ok(())
}

fn validate_image(image: &model::PackageId) -> Result<(), Error> {
    if image.kind != PackageKind::SystemImage {
        return Err(Error::new(
            ErrorCode::InvalidInput,
            "device image must be a system image",
        ));
    }
    for (value, name) in [
        (image.api.as_deref(), "API"),
        (image.tag.as_deref(), "tag"),
        (image.abi.as_deref(), "ABI"),
    ] {
        if !matches!(value, Some(value) if !value.is_empty()) {
            return Err(Error::new(
                ErrorCode::InvalidInput,
                format!("device image {name} is missing"),
            ));
        }
    }
    Ok(())
}

async fn verify_created_device(
    snapshot: &EnvironmentSnapshot,
    draft: &CreateDeviceDraft,
) -> Result<(), Error> {
    let metadata = blocking({
        let root = snapshot.paths.avd_root.clone();
        let user_root = snapshot.paths.user_root.clone();
        let id = draft.id.clone();
        move || avdfs::AvdStore::new(root, user_root).get(&id)
    })
    .await?;
    let display_name = draft
        .display_name
        .as_deref()
        .unwrap_or_else(|| draft.id.as_str());
    let target = draft
        .image
        .api
        .as_deref()
        .map(|api| {
            if api.starts_with("android-") {
                api.to_owned()
            } else {
                format!("android-{api}")
            }
        })
        .unwrap_or_default();
    let expected_directory = snapshot.paths.avd_root.join(format!("{}.avd", draft.id));
    if metadata.directory != expected_directory
        || metadata.display_name.as_deref() != Some(display_name)
        || metadata.profile.as_ref() != Some(&draft.profile)
        || metadata.image.as_ref() != Some(&draft.image)
        || metadata.target.as_deref() != Some(target.as_str())
    {
        return Err(Error::new(
            ErrorCode::Internal,
            format!(
                "created Android virtual device {} failed post-execution verification",
                draft.id
            ),
        ));
    }
    Ok(())
}

async fn compensate(
    events: &mpsc::UnboundedSender<Event>,
    transaction: &Arc<Mutex<Option<avdfs::CreateTransaction>>>,
    avd_root: &Path,
    draft: &CreateDeviceDraft,
    error: &mut Error,
) {
    let description = "remove the newly created AVD and restore file backups";
    let _ = events.send(Event::CompensationStarted {
        step: "create".into(),
        message: description.into(),
    });
    let outcome = if transaction_has_value(transaction) {
        transaction_action(transaction, avdfs::CreateTransaction::compensate).await
    } else {
        blocking({
            let root = avd_root.to_path_buf();
            let source = model::AvdId::new(draft.profile.as_str()).map_err(Error::from);
            move || source.and_then(|id| avdfs::remove_created_artifacts(&root, &id))
        })
        .await
    };
    let (succeeded, message) = match outcome {
        Ok(()) => (true, "created AVD removed and backups cleaned".into()),
        Err(compensation_error) => (false, compensation_error.message),
    };
    let _ = events.send(Event::CompensationFinished {
        step: "create".into(),
        succeeded,
        message: message.clone(),
    });
    error.compensations.push(CompensationResult {
        step: "create".into(),
        succeeded,
        message,
    });
}

fn check_name_conflicts(
    root: &Path,
    user_root: &Path,
    profile: &model::ProfileId,
    target: &model::AvdId,
) -> Result<(), Error> {
    let store = avdfs::AvdStore::new(root, user_root);
    let profile_id = model::AvdId::new(profile.as_str()).map_err(Error::from)?;
    for id in [&profile_id, target] {
        if store.contains_paths(id)? {
            return Err(Error::new(
                ErrorCode::NameConflict,
                format!("Android virtual device {id} already exists"),
            ));
        }
    }
    Ok(())
}

fn require_package(
    snapshot: &EnvironmentSnapshot,
    kind: PackageKind,
    name: &str,
) -> Result<(), Error> {
    if snapshot
        .installed_packages
        .iter()
        .any(|package| package.installed && package.id.kind == kind)
    {
        Ok(())
    } else {
        Err(Error::new(
            ErrorCode::PreconditionFailed,
            format!("required SDK package {name} is not installed"),
        ))
    }
}

fn tool_path(snapshot: &EnvironmentSnapshot, name: &str) -> Result<PathBuf, Error> {
    snapshot
        .tools
        .iter()
        .find(|tool| tool.name == name && tool.state == ToolState::Available)
        .and_then(|tool| tool.path.clone())
        .ok_or_else(|| Error::new(ErrorCode::ToolNotFound, format!("{name} was not found")))
}

fn start_step(events: &mpsc::UnboundedSender<Event>, step: &str, message: &str) {
    let _ = events.send(Event::StepStarted {
        step: step.into(),
        message: message.into(),
    });
}

fn finish_step(events: &mpsc::UnboundedSender<Event>, step: &str, message: &str) {
    let _ = events.send(Event::StepFinished {
        step: step.into(),
        message: message.into(),
    });
}

fn emit_output(events: &mpsc::UnboundedSender<Event>, output: &process::Output) {
    for line in output.stdout.lines() {
        let _ = events.send(Event::Log {
            stream: LogStream::Stdout,
            line: line.into(),
        });
    }
    for line in output.stderr.lines() {
        let _ = events.send(Event::Log {
            stream: LogStream::Stderr,
            line: line.into(),
        });
    }
}

fn failed(mut error: Error, step: &str) -> Error {
    error.failed_step = Some(step.into());
    error
}

fn check_cancelled(cancellation: &CancellationToken) -> Result<(), Error> {
    if cancellation.is_cancelled() {
        Err(Error::new(ErrorCode::Cancelled, "operation cancelled"))
    } else {
        Ok(())
    }
}

async fn transaction_action<F>(
    transaction: &Arc<Mutex<Option<avdfs::CreateTransaction>>>,
    action: F,
) -> Result<(), Error>
where
    F: FnOnce(&mut avdfs::CreateTransaction) -> Result<(), Error> + Send + 'static,
{
    blocking({
        let transaction = Arc::clone(transaction);
        move || {
            let mut transaction = transaction
                .lock()
                .map_err(|_| Error::new(ErrorCode::Internal, "transaction lock is poisoned"))?;
            let transaction = transaction.as_mut().ok_or_else(|| {
                Error::new(ErrorCode::Internal, "file transaction was not initialized")
            })?;
            action(transaction)
        }
    })
    .await
}

fn transaction_has_value(transaction: &Arc<Mutex<Option<avdfs::CreateTransaction>>>) -> bool {
    transaction
        .lock()
        .map(|transaction| transaction.is_some())
        .unwrap_or(true)
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

fn lock_keys(draft: &CreateDeviceDraft) -> Vec<String> {
    let mut keys = vec![
        format!("name:{}", draft.id),
        format!("name:{}", draft.profile),
    ];
    keys.sort();
    keys.dedup();
    keys
}

struct ProcessWriteLock {
    keys: Vec<String>,
}

impl ProcessWriteLock {
    fn acquire(keys: Vec<String>) -> Result<Self, Error> {
        let active = ACTIVE_WRITES.get_or_init(|| Mutex::new(HashSet::new()));
        let mut active = active
            .lock()
            .map_err(|_| Error::new(ErrorCode::Internal, "write lock is poisoned"))?;
        if let Some(key) = keys.iter().find(|key| active.contains(*key)) {
            return Err(Error::new(
                ErrorCode::PreconditionFailed,
                format!("another write plan holds {key}"),
            ));
        }
        active.extend(keys.iter().cloned());
        Ok(Self { keys })
    }
}

impl Drop for ProcessWriteLock {
    fn drop(&mut self) {
        if let Ok(mut active) = ACTIVE_WRITES
            .get_or_init(|| Mutex::new(HashSet::new()))
            .lock()
        {
            for key in &self.keys {
                active.remove(key);
            }
        }
    }
}

struct DirectoryLocks {
    paths: Vec<PathBuf>,
}

impl DirectoryLocks {
    fn acquire(root: &Path, keys: &[String]) -> Result<Self, Error> {
        fs::create_dir_all(root).map_err(|error| io_error("create AVD directory", root, error))?;
        let mut paths = Vec::new();
        for key in keys {
            let mut hasher = DefaultHasher::new();
            key.hash(&mut hasher);
            let path = root.join(format!(
                ".{PRODUCT_NAME}.write-lock-{:016x}",
                hasher.finish()
            ));
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(mut file) => {
                    if let Err(error) = writeln!(file, "{}", std::process::id()) {
                        for path in &paths {
                            let _ = fs::remove_file(path);
                        }
                        return Err(io_error("write AVD directory lock", &path, error));
                    }
                    paths.push(path);
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                    for path in &paths {
                        let _ = fs::remove_file(path);
                    }
                    return Err(Error::new(
                        ErrorCode::PreconditionFailed,
                        format!("another write plan holds directory lock {}", path.display()),
                    ));
                }
                Err(error) => {
                    for path in &paths {
                        let _ = fs::remove_file(path);
                    }
                    return Err(io_error("create AVD directory lock", &path, error));
                }
            }
        }
        Ok(Self { paths })
    }
}

impl Drop for DirectoryLocks {
    fn drop(&mut self) {
        for path in &self.paths {
            let _ = fs::remove_file(path);
        }
    }
}

fn io_error(action: &str, path: &Path, error: io::Error) -> Error {
    Error::new(
        ErrorCode::Internal,
        format!("{action} {}: {error}", path.display()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use model::{
        AndroidCliMetrics, CpuArchitecture, Field, HardwareConfig, Host, Package, PackageId,
        Platform, PlatformPaths, ProfileId, Revision, ToolNames, ToolSource, ToolStatus,
        ValueSource,
    };
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_ROOT: AtomicU64 = AtomicU64::new(1);

    struct FakeBackend {
        malformed_create: bool,
    }

    impl Backend for FakeBackend {
        fn execute<'a>(
            &'a self,
            invocation: drivers::Invocation,
            cancellation: &'a CancellationToken,
        ) -> Pin<Box<dyn Future<Output = Result<process::Output, Error>> + Send + 'a>> {
            Box::pin(async move {
                check_cancelled(cancellation)?;
                match invocation.tool {
                    drivers::Tool::Adb => Ok(process::Output {
                        status: Some(0),
                        stdout: "List of devices attached\n\n".into(),
                        stderr: String::new(),
                    }),
                    drivers::Tool::Android => {
                        let profile = invocation.args.last().unwrap();
                        let root = PathBuf::from(&invocation.environment["ANDROID_AVD_HOME"]);
                        let directory = root.join(format!("{profile}.avd"));
                        fs::create_dir_all(&directory).unwrap();
                        fs::write(
                            root.join(format!("{profile}.ini")),
                            format!(
                                "path={}\npath.rel=avd/{profile}.avd\ntarget=android-36\n",
                                directory.display()
                            ),
                        )
                        .unwrap();
                        if !self.malformed_create {
                            fs::write(
                                directory.join("config.ini"),
                                format!("AvdId=Profile\nhw.device.name={profile}\nunknown=a=b\n"),
                            )
                            .unwrap();
                        }
                        Ok(process::Output {
                            status: Some(0),
                            stdout: "created\n".into(),
                            stderr: String::new(),
                        })
                    }
                    _ => unreachable!(),
                }
            })
        }
    }

    struct CancellingBackend {
        create_started: Arc<tokio::sync::Notify>,
    }

    impl Backend for CancellingBackend {
        fn execute<'a>(
            &'a self,
            invocation: drivers::Invocation,
            cancellation: &'a CancellationToken,
        ) -> Pin<Box<dyn Future<Output = Result<process::Output, Error>> + Send + 'a>> {
            Box::pin(async move {
                if invocation.tool == drivers::Tool::Adb {
                    return Ok(process::Output {
                        status: Some(0),
                        stdout: "List of devices attached\n\n".into(),
                        stderr: String::new(),
                    });
                }
                let profile = invocation.args.last().unwrap();
                let root = PathBuf::from(&invocation.environment["ANDROID_AVD_HOME"]);
                let directory = root.join(format!("{profile}.avd"));
                fs::create_dir_all(&directory).unwrap();
                fs::write(
                    root.join(format!("{profile}.ini")),
                    format!("path={}\n", directory.display()),
                )
                .unwrap();
                fs::write(directory.join("config.ini"), "AvdId=Profile\n").unwrap();
                self.create_started.notify_one();
                cancellation.cancelled().await;
                Err(Error::new(ErrorCode::Cancelled, "command cancelled"))
            })
        }
    }

    fn temp_root(suffix: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "{}{}_{}_{suffix}",
            model::test_avd_prefix(),
            std::process::id(),
            NEXT_ROOT.fetch_add(1, Ordering::Relaxed)
        ))
    }

    fn draft(suffix: &str) -> CreateDeviceDraft {
        CreateDeviceDraft {
            id: model::AvdId::new(format!("{}{suffix}", model::test_avd_prefix())).unwrap(),
            profile: ProfileId::new(format!("{}profile_{suffix}", model::test_avd_prefix()))
                .unwrap(),
            image: PackageId {
                kind: PackageKind::SystemImage,
                api: Some("36".into()),
                tag: Some("google_apis".into()),
                abi: Some("arm64-v8a".into()),
                qualifier: None,
            },
            display_name: Some("Test Phone".into()),
            hardware: HardwareConfig::default(),
        }
    }

    fn snapshot(root: &Path, image: &PackageId) -> EnvironmentSnapshot {
        let sdk_root = root.join("sdk");
        fs::create_dir_all(sdk_root.join(image.render_slash())).unwrap();
        let tool_names = ToolNames {
            android: "android".into(),
            adb: "adb".into(),
            emulator: "emulator".into(),
            sdkmanager: "sdkmanager".into(),
            avdmanager: "avdmanager".into(),
        };
        let packages = [
            PackageId {
                kind: PackageKind::Emulator,
                api: None,
                tag: None,
                abi: None,
                qualifier: None,
            },
            PackageId {
                kind: PackageKind::PlatformTools,
                api: None,
                tag: None,
                abi: None,
                qualifier: None,
            },
            image.clone(),
        ]
        .into_iter()
        .map(|id| Package {
            id,
            revision: Field::present(Revision {
                components: vec![1],
                suffix: None,
            }),
            installed: true,
        })
        .collect();
        EnvironmentSnapshot {
            host: Host {
                platform: Platform::MacOs,
                architecture: CpuArchitecture::Arm64,
                android_abi: Field::present("arm64-v8a".into()),
                supported: true,
            },
            paths: PlatformPaths {
                sdk_root,
                user_root: root.join("user"),
                avd_root: root.join("user/avd"),
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
            installed_packages: packages,
            diagnostics: Vec::new(),
        }
    }

    #[tokio::test]
    async fn executes_create_steps_and_emits_events() {
        let root = temp_root("success");
        let draft = draft("success");
        let snapshot = snapshot(&root, &draft.image);
        let (events, mut receiver) = mpsc::unbounded_channel();
        let result = execute_create_with_backend(
            "operation-success".into(),
            draft.clone(),
            snapshot.clone(),
            AndroidCliMetrics::Inherit,
            events,
            CancellationToken::new(),
            Arc::new(FakeBackend {
                malformed_create: false,
            }),
        )
        .await
        .unwrap();

        assert_eq!(
            result,
            OperationResult::PlanCompleted {
                plan_id: format!("create:{}", draft.id)
            }
        );
        let config = fs::read_to_string(
            snapshot
                .paths
                .avd_root
                .join(format!("{}.avd/config.ini", draft.id)),
        )
        .unwrap();
        assert!(config.contains(&format!("AvdId={}", draft.id)));
        assert!(config.contains("unknown=a=b"));
        assert!(config.contains("image.sysdir.1=system-images/android-36/google_apis/arm64-v8a/"));
        assert!(config.contains("avd.ini.displayname=Test Phone"));
        let collected = std::iter::from_fn(|| receiver.try_recv().ok()).collect::<Vec<_>>();
        assert_eq!(
            collected
                .iter()
                .filter(|event| matches!(event, Event::StepFinished { .. }))
                .count(),
            4
        );
        assert!(!snapshot
            .paths
            .data_root
            .join("backups/operation-success")
            .exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn compensates_a_created_avd_when_file_preparation_fails() {
        let root = temp_root("compensation");
        let draft = draft("compensation");
        let snapshot = snapshot(&root, &draft.image);
        let (events, mut receiver) = mpsc::unbounded_channel();
        let error = execute_create_with_backend(
            "operation-compensation".into(),
            draft.clone(),
            snapshot.clone(),
            AndroidCliMetrics::Inherit,
            events,
            CancellationToken::new(),
            Arc::new(FakeBackend {
                malformed_create: true,
            }),
        )
        .await
        .unwrap_err();

        assert_eq!(error.failed_step.as_deref(), Some("rename"));
        assert_eq!(error.compensations.len(), 1);
        assert!(error.compensations[0].succeeded);
        let profile = model::AvdId::new(draft.profile.as_str()).unwrap();
        assert!(
            !avdfs::AvdStore::new(&snapshot.paths.avd_root, &snapshot.paths.user_root)
                .contains_paths(&profile)
                .unwrap()
        );
        assert!(
            std::iter::from_fn(|| receiver.try_recv().ok()).any(|event| {
                matches!(
                    event,
                    Event::CompensationFinished {
                        succeeded: true,
                        ..
                    }
                )
            })
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn preflight_failure_does_not_create_an_avd() {
        let root = temp_root("preflight");
        let draft = draft("preflight");
        let mut snapshot = snapshot(&root, &draft.image);
        snapshot
            .installed_packages
            .retain(|package| package.id != draft.image);
        let (events, _receiver) = mpsc::unbounded_channel();
        let error = execute_create_with_backend(
            "operation-preflight".into(),
            draft,
            snapshot.clone(),
            AndroidCliMetrics::Inherit,
            events,
            CancellationToken::new(),
            Arc::new(FakeBackend {
                malformed_create: false,
            }),
        )
        .await
        .unwrap_err();

        assert_eq!(error.code, ErrorCode::PackageNotFound);
        assert!(!snapshot.paths.avd_root.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn cancellation_during_create_removes_partial_artifacts() {
        let root = temp_root("cancel");
        let draft = draft("cancel");
        let snapshot = snapshot(&root, &draft.image);
        let cancellation = CancellationToken::new();
        let cancellation_for_task = cancellation.clone();
        let create_started = Arc::new(tokio::sync::Notify::new());
        let backend = Arc::new(CancellingBackend {
            create_started: Arc::clone(&create_started),
        });
        let (events, _receiver) = mpsc::unbounded_channel();
        let draft_for_task = draft.clone();
        let snapshot_for_task = snapshot.clone();
        let task = tokio::spawn(async move {
            execute_create_with_backend(
                "operation-cancel".into(),
                draft_for_task,
                snapshot_for_task,
                AndroidCliMetrics::Inherit,
                events,
                cancellation_for_task,
                backend,
            )
            .await
        });
        create_started.notified().await;
        cancellation.cancel();
        let error = task.await.unwrap().unwrap_err();

        assert_eq!(error.code, ErrorCode::Cancelled);
        assert_eq!(error.failed_step.as_deref(), Some("create"));
        assert!(error.compensations[0].succeeded);
        let profile = model::AvdId::new(draft.profile.as_str()).unwrap();
        assert!(
            !avdfs::AvdStore::new(&snapshot.paths.avd_root, &snapshot.paths.user_root)
                .contains_paths(&profile)
                .unwrap()
        );
        fs::remove_dir_all(root).unwrap();
    }
}
