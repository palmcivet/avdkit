use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    str::FromStr,
    time::Duration,
};

use model::{
    Diagnostic, EnvironmentDiagnostic, EnvironmentDiagnosticCode, EnvironmentSnapshot,
    EnvironmentValue, Error, ErrorCode, Field, Package, PackageId, PackageKind, Reason, ReasonCode,
    Revision, ToolSource, ToolState, ToolStatus, ValueSource,
};
use process::{CommandSpec, Runner};

const RELEVANT_ENVIRONMENT: &[&str] = &[
    "PATH",
    "HOME",
    "XDG_DATA_HOME",
    "ANDROID_SDK_ROOT",
    "ANDROID_HOME",
    "ANDROID_USER_HOME",
    "ANDROID_AVD_HOME",
    "SHELL",
];

#[derive(Debug)]
struct SdkSelection {
    path: PathBuf,
    source: ValueSource,
    diagnostics: Vec<EnvironmentDiagnostic>,
}

#[derive(Debug, Default)]
struct SdkScan {
    tools: Vec<ToolStatus>,
    legacy_tools: Vec<ToolStatus>,
    packages: Vec<Package>,
    diagnostics: Vec<EnvironmentDiagnostic>,
}

pub async fn probe(sdk_root: Option<PathBuf>) -> Result<EnvironmentSnapshot, Error> {
    let process_environment = platform::process_environment();
    let runner = Runner {
        default_timeout: Duration::from_secs(30),
    };
    let (login_environment, mut diagnostics) =
        read_login_shell_environment(&runner, &process_environment).await;
    let (merged_environment, values, merge_diagnostics) =
        merge_environment(&process_environment, &login_environment);
    diagnostics.extend(merge_diagnostics);

    let description_environment = merged_environment.clone();
    let mut description = tokio::task::spawn_blocking(move || {
        platform::describe_with_environment(&description_environment)
    })
    .await
    .map_err(|error| {
        Error::new(
            ErrorCode::Internal,
            format!("platform environment probe failed: {error}"),
        )
    })?;
    let android_path = locate_in_path(
        &description.tools.android,
        merged_environment.get("PATH").map(String::as_str),
    );
    let mut android_status = missing_tool(description.tools.android.clone());
    let mut android_sdk_root = None;

    if let Some(path) = android_path {
        let version_output = drivers::execute(
            &runner,
            drivers::Invocation::android_discovery(path.clone(), ["--version".into()])
                .with_variables(merged_environment.clone()),
        )
        .await;
        match version_output {
            Ok(output) => match drivers::android::parse_version(&output) {
                Ok(version) => {
                    android_status = ToolStatus {
                        name: description.tools.android.clone(),
                        path: Some(path.clone()),
                        version: Field::present(version),
                        state: ToolState::Available,
                        source: Some(ToolSource::SearchPath),
                        package: None,
                        reasons: Vec::new(),
                        diagnostic: None,
                    };
                    match drivers::execute(
                        &runner,
                        drivers::Invocation::android_discovery(path, ["info".into(), "sdk".into()])
                            .with_variables(merged_environment.clone()),
                    )
                    .await
                    {
                        Ok(output) => match drivers::android::parse_sdk_root(&output) {
                            Ok(path) => android_sdk_root = Some(path),
                            Err(error) => diagnostics.push(error_diagnostic(
                                "Android CLI could not report its SDK root",
                                error,
                            )),
                        },
                        Err(error) => diagnostics
                            .push(error_diagnostic("Android CLI SDK root probe failed", error)),
                    }
                }
                Err(error) => android_status = unavailable_android_tool(path, error),
            },
            Err(error) => android_status = unavailable_android_tool(path, error),
        }
    }

    let selection = select_sdk_root(
        sdk_root,
        &merged_environment,
        &values,
        android_sdk_root,
        description.paths.sdk_root.clone(),
    );
    description.paths.sdk_root = selection.path;
    diagnostics.extend(selection.diagnostics);

    let sdk_root = description.paths.sdk_root.clone();
    let tool_names = description.tools.clone();
    let scan = tokio::task::spawn_blocking(move || scan_sdk(&sdk_root, &tool_names))
        .await
        .map_err(|error| {
            Error::new(
                ErrorCode::Internal,
                format!("SDK filesystem probe failed: {error}"),
            )
        })?;
    diagnostics.extend(scan.diagnostics);

    let mut tools = vec![android_status];
    tools.extend(scan.tools);

    Ok(EnvironmentSnapshot {
        host: description.host,
        paths: description.paths,
        sdk_root_source: selection.source,
        environment: values,
        tool_names: description.tools,
        tools,
        legacy_tools: scan.legacy_tools,
        installed_packages: scan.packages,
        diagnostics,
    })
}

async fn read_login_shell_environment(
    runner: &Runner,
    process_environment: &BTreeMap<String, String>,
) -> (BTreeMap<String, String>, Vec<EnvironmentDiagnostic>) {
    let Some(shell) = platform::login_shell(process_environment) else {
        return (BTreeMap::new(), Vec::new());
    };
    let result = runner
        .run(CommandSpec {
            program: shell.program,
            args: shell.args,
            ..CommandSpec::default()
        })
        .await;
    match result {
        Ok(output) if output.status == Some(0) => (parse_environment(&output.stdout), Vec::new()),
        Ok(output) => (
            BTreeMap::new(),
            vec![EnvironmentDiagnostic {
                code: EnvironmentDiagnosticCode::ProbeFailed,
                message: "login shell environment probe failed".into(),
                values: vec![EnvironmentValue {
                    name: "exit_status".into(),
                    value: output
                        .status
                        .map_or_else(|| "signal".into(), |status| status.to_string()),
                    source: ValueSource::LoginShell,
                }],
            }],
        ),
        Err(error) => (
            BTreeMap::new(),
            vec![error_diagnostic(
                "login shell environment probe failed",
                error,
            )],
        ),
    }
}

fn parse_environment(output: &str) -> BTreeMap<String, String> {
    output
        .lines()
        .filter_map(|line| line.split_once('='))
        .map(|(name, value)| (name.to_owned(), value.to_owned()))
        .collect()
}

fn merge_environment(
    process: &BTreeMap<String, String>,
    login: &BTreeMap<String, String>,
) -> (
    BTreeMap<String, String>,
    Vec<EnvironmentValue>,
    Vec<EnvironmentDiagnostic>,
) {
    let mut merged = login.clone();
    merged.extend(process.clone());
    if process.contains_key("PATH") || login.contains_key("PATH") {
        let mut paths = Vec::new();
        let mut seen = BTreeSet::new();
        for value in [process.get("PATH"), login.get("PATH")]
            .into_iter()
            .flatten()
        {
            for path in std::env::split_paths(value) {
                if seen.insert(path.clone()) {
                    paths.push(path);
                }
            }
        }
        if let Ok(value) = std::env::join_paths(paths) {
            merged.insert("PATH".into(), value.to_string_lossy().into_owned());
        }
    }
    let mut values = Vec::new();
    let mut diagnostics = Vec::new();
    for name in RELEVANT_ENVIRONMENT {
        if let Some(value) = process.get(*name) {
            values.push(EnvironmentValue {
                name: (*name).into(),
                value: value.clone(),
                source: ValueSource::ProcessEnvironment,
            });
        }
        if let Some(value) = login.get(*name) {
            values.push(EnvironmentValue {
                name: (*name).into(),
                value: value.clone(),
                source: ValueSource::LoginShell,
            });
        }
        if name.starts_with("ANDROID_")
            && process.get(*name).is_some()
            && login.get(*name).is_some()
            && process.get(*name) != login.get(*name)
        {
            diagnostics.push(EnvironmentDiagnostic {
                code: EnvironmentDiagnosticCode::SourceConflict,
                message: format!("{name} differs between the process and login shell"),
                values: vec![
                    EnvironmentValue {
                        name: (*name).into(),
                        value: process[*name].clone(),
                        source: ValueSource::ProcessEnvironment,
                    },
                    EnvironmentValue {
                        name: (*name).into(),
                        value: login[*name].clone(),
                        source: ValueSource::LoginShell,
                    },
                ],
            });
        }
    }
    (merged, values, diagnostics)
}

fn select_sdk_root(
    caller: Option<PathBuf>,
    environment: &BTreeMap<String, String>,
    environment_values: &[EnvironmentValue],
    android_cli: Option<PathBuf>,
    platform_default: PathBuf,
) -> SdkSelection {
    let mut candidates = Vec::new();
    if let Some(path) = caller {
        candidates.push((
            path,
            ValueSource::CallerOverride,
            "caller sdk_root".to_owned(),
        ));
    }
    // `ANDROID_SDK_ROOT` is deprecated in favor of `ANDROID_HOME`.
    for name in ["ANDROID_HOME", "ANDROID_SDK_ROOT"] {
        if let Some(value) = environment.get(name) {
            candidates.push((
                PathBuf::from(value),
                value_source(environment_values, name),
                name.to_owned(),
            ));
        }
    }
    if let Some(path) = android_cli {
        candidates.push((path, ValueSource::AndroidCli, "android info sdk".into()));
    }
    candidates.push((
        platform_default,
        ValueSource::PlatformDefault,
        "platform default".into(),
    ));

    let (path, source, _) = candidates[0].clone();
    let conflicting = candidates
        .iter()
        .filter(|(candidate, _, _)| candidate != &path)
        .map(|(candidate, source, name)| EnvironmentValue {
            name: name.clone(),
            value: candidate.to_string_lossy().into_owned(),
            source: *source,
        })
        .collect::<Vec<_>>();
    let diagnostics = if conflicting.is_empty() {
        Vec::new()
    } else {
        let mut values = vec![EnvironmentValue {
            name: "selected sdk_root".into(),
            value: path.to_string_lossy().into_owned(),
            source,
        }];
        values.extend(conflicting);
        vec![EnvironmentDiagnostic {
            code: EnvironmentDiagnosticCode::SourceConflict,
            message: "SDK root sources disagree; the highest-priority value was selected".into(),
            values,
        }]
    };
    SdkSelection {
        path,
        source,
        diagnostics,
    }
}

fn value_source(values: &[EnvironmentValue], name: &str) -> ValueSource {
    values
        .iter()
        .find(|value| value.name == name)
        .map_or(ValueSource::ProcessEnvironment, |value| value.source)
}

fn locate_in_path(name: &str, path: Option<&str>) -> Option<PathBuf> {
    let candidate = PathBuf::from(name);
    if candidate.is_absolute() && candidate.is_file() {
        return Some(candidate);
    }
    path.into_iter()
        .flat_map(std::env::split_paths)
        .map(|directory| directory.join(name))
        .find(|candidate| candidate.is_file())
}

fn unavailable_android_tool(path: PathBuf, error: Error) -> ToolStatus {
    let reason = if error.code == ErrorCode::PreconditionFailed {
        Reason::new(ReasonCode::ToolNotReady, error.message.clone())
    } else {
        Reason::new(
            ReasonCode::ToolNotReady,
            "Android CLI version could not be determined",
        )
    };
    ToolStatus {
        name: path.file_name().map_or_else(
            || "android".into(),
            |name| name.to_string_lossy().into_owned(),
        ),
        path: Some(path),
        version: Field::Unavailable,
        state: ToolState::Unavailable,
        source: Some(ToolSource::SearchPath),
        package: None,
        reasons: vec![reason],
        diagnostic: error.diagnostic.map(|diagnostic| *diagnostic),
    }
}

fn missing_tool(name: String) -> ToolStatus {
    ToolStatus {
        name,
        path: None,
        version: Field::Unavailable,
        state: ToolState::Missing,
        source: None,
        package: None,
        reasons: vec![Reason::new(
            ReasonCode::ToolNotFound,
            "the executable was not found",
        )],
        diagnostic: None,
    }
}

fn scan_sdk(sdk_root: &Path, names: &model::ToolNames) -> SdkScan {
    let mut scan = SdkScan::default();
    let emulator_package = read_package(
        &sdk_root.join("emulator/source.properties"),
        &mut scan.diagnostics,
    );
    let platform_tools_package = read_package(
        &sdk_root.join("platform-tools/source.properties"),
        &mut scan.diagnostics,
    );

    scan.tools.push(sdk_tool(
        names.emulator.clone(),
        sdk_root.join("emulator").join(&names.emulator),
        emulator_package.clone(),
        &mut scan.diagnostics,
    ));
    scan.tools.push(sdk_tool(
        names.adb.clone(),
        sdk_root.join("platform-tools").join(&names.adb),
        platform_tools_package.clone(),
        &mut scan.diagnostics,
    ));

    let command_line_packages = command_line_tool_packages(sdk_root, &mut scan.diagnostics);
    for (name, executable) in [
        (&names.sdkmanager, names.sdkmanager.as_str()),
        (&names.avdmanager, names.avdmanager.as_str()),
    ] {
        let candidate = command_line_packages
            .iter()
            .filter_map(|(root, package)| {
                let executable = root.join("bin").join(executable);
                executable.is_file().then(|| (executable, package.clone()))
            })
            .max_by(|left, right| package_revision(&left.1).cmp(&package_revision(&right.1)));
        scan.tools.push(candidate.map_or_else(
            || missing_tool(name.clone()),
            |(path, package)| sdk_tool(name.clone(), path, Some(package), &mut scan.diagnostics),
        ));
    }

    for name in [&names.sdkmanager, &names.avdmanager] {
        let path = sdk_root.join("tools/bin").join(name);
        if path.is_file() {
            scan.legacy_tools.push(ToolStatus {
                name: name.clone(),
                path: Some(path),
                version: Field::Unavailable,
                state: ToolState::ReportOnly,
                source: Some(ToolSource::LegacyToolsBin),
                package: None,
                reasons: vec![Reason::new(
                    ReasonCode::ToolNotReady,
                    "deprecated tools/bin executables are not used as implementations",
                )],
                diagnostic: None,
            });
        }
    }

    scan.packages.extend(emulator_package);
    scan.packages.extend(platform_tools_package);
    scan.packages.extend(
        command_line_packages
            .into_iter()
            .map(|(_, package)| package),
    );
    collect_packages(
        &sdk_root.join("system-images"),
        6,
        &mut scan.packages,
        &mut scan.diagnostics,
    );
    scan.packages.sort_by(|left, right| {
        left.id
            .render_semicolon()
            .cmp(&right.id.render_semicolon())
            .then_with(|| package_revision(left).cmp(&package_revision(right)))
    });
    let mut seen = BTreeSet::new();
    scan.packages.retain(|package| {
        seen.insert((
            package.id.render_semicolon(),
            package
                .revision
                .as_value()
                .map(|revision| format!("{revision:?}")),
        ))
    });
    scan
}

fn command_line_tool_packages(
    sdk_root: &Path,
    diagnostics: &mut Vec<EnvironmentDiagnostic>,
) -> Vec<(PathBuf, Package)> {
    let root = sdk_root.join("cmdline-tools");
    let Ok(entries) = fs::read_dir(&root) else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .filter_map(|path| {
            read_package(&path.join("source.properties"), diagnostics)
                .map(|package| (path, package))
        })
        .collect()
}

fn sdk_tool(
    name: String,
    path: PathBuf,
    package: Option<Package>,
    diagnostics: &mut Vec<EnvironmentDiagnostic>,
) -> ToolStatus {
    if !path.is_file() {
        return missing_tool(name);
    }
    let Some(package) = package else {
        diagnostics.push(EnvironmentDiagnostic {
            code: EnvironmentDiagnosticCode::ProbeFailed,
            message: format!("{} has no readable source.properties", path.display()),
            values: Vec::new(),
        });
        return ToolStatus {
            name,
            path: Some(path),
            version: Field::Unavailable,
            state: ToolState::Unavailable,
            source: Some(ToolSource::SdkPackage),
            package: None,
            reasons: vec![Reason::new(
                ReasonCode::ToolNotReady,
                "SDK package metadata is missing or invalid",
            )],
            diagnostic: None,
        };
    };
    let version = package.revision.clone();
    let state = if version.as_value().is_some() {
        ToolState::Available
    } else {
        ToolState::Unavailable
    };
    let reasons = if state == ToolState::Available {
        Vec::new()
    } else {
        vec![Reason::new(
            ReasonCode::ToolNotReady,
            "SDK package revision is missing or invalid",
        )]
    };
    ToolStatus {
        name,
        path: Some(path),
        version,
        state,
        source: Some(ToolSource::SdkPackage),
        package: Some(package),
        reasons,
        diagnostic: None,
    }
}

fn read_package(
    source_properties: &Path,
    diagnostics: &mut Vec<EnvironmentDiagnostic>,
) -> Option<Package> {
    let contents = match fs::read_to_string(source_properties) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return None,
        Err(error) => {
            diagnostics.push(EnvironmentDiagnostic {
                code: EnvironmentDiagnosticCode::ProbeFailed,
                message: format!("cannot read {}: {error}", source_properties.display()),
                values: Vec::new(),
            });
            return None;
        }
    };
    let properties = contents
        .lines()
        .filter_map(|line| line.split_once('='))
        .map(|(key, value)| (key.trim(), value.trim()))
        .collect::<BTreeMap<_, _>>();
    let id = properties
        .get("Pkg.Path")
        .and_then(|path| package_id(path))
        .or_else(|| infer_package_id(source_properties));
    let Some(id) = id else {
        diagnostics.push(EnvironmentDiagnostic {
            code: EnvironmentDiagnosticCode::ProbeFailed,
            message: format!(
                "{} has no usable package identifier",
                source_properties.display()
            ),
            values: Vec::new(),
        });
        return None;
    };
    let revision = properties
        .get("Pkg.Revision")
        .and_then(|value| Revision::from_str(value).ok())
        .map_or(Field::Unavailable, Field::present);
    Some(Package {
        id,
        revision,
        installed: true,
    })
}

fn infer_package_id(source_properties: &Path) -> Option<PackageId> {
    let components = source_properties
        .parent()?
        .components()
        .filter_map(|component| component.as_os_str().to_str())
        .collect::<Vec<_>>();
    for marker in [
        "system-images",
        "platforms",
        "build-tools",
        "cmdline-tools",
        "emulator",
        "platform-tools",
    ] {
        if let Some(index) = components.iter().position(|component| *component == marker) {
            return package_id(&components[index..].join(";"));
        }
    }
    None
}

fn package_id(value: &str) -> Option<PackageId> {
    let segments = value
        .split([';', '/'])
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    let first = *segments.first()?;
    let id = match first {
        "system-images" if segments.len() >= 4 => PackageId {
            kind: PackageKind::SystemImage,
            api: Some(
                segments[1]
                    .strip_prefix("android-")
                    .unwrap_or(segments[1])
                    .into(),
            ),
            tag: Some(segments[2].into()),
            abi: Some(segments[3].into()),
            qualifier: (segments.len() > 4).then(|| segments[4..].join("/")),
        },
        "platforms" => PackageId {
            kind: PackageKind::Platform,
            api: segments.get(1).map(|value| (*value).into()),
            tag: None,
            abi: None,
            qualifier: (segments.len() > 2).then(|| segments[2..].join("/")),
        },
        "build-tools" => PackageId {
            kind: PackageKind::BuildTools,
            api: segments.get(1).map(|value| (*value).into()),
            tag: None,
            abi: None,
            qualifier: (segments.len() > 2).then(|| segments[2..].join("/")),
        },
        "emulator" => simple_package(PackageKind::Emulator, &segments),
        "platform-tools" => simple_package(PackageKind::PlatformTools, &segments),
        "cmdline-tools" => simple_package(PackageKind::CommandLineTools, &segments),
        _ => PackageId {
            kind: PackageKind::Other,
            api: None,
            tag: None,
            abi: None,
            qualifier: Some(segments.join("/")),
        },
    };
    Some(id)
}

fn simple_package(kind: PackageKind, segments: &[&str]) -> PackageId {
    PackageId {
        kind,
        api: None,
        tag: None,
        abi: None,
        qualifier: (segments.len() > 1).then(|| segments[1..].join("/")),
    }
}

fn collect_packages(
    root: &Path,
    depth: usize,
    packages: &mut Vec<Package>,
    diagnostics: &mut Vec<EnvironmentDiagnostic>,
) {
    if depth == 0 {
        return;
    }
    if let Some(package) = read_package(&root.join("source.properties"), diagnostics) {
        packages.push(package);
    }
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            collect_packages(&entry.path(), depth - 1, packages, diagnostics);
        }
    }
}

fn package_revision(package: &Package) -> Option<&Revision> {
    package.revision.as_value()
}

fn error_diagnostic(message: &str, error: Error) -> EnvironmentDiagnostic {
    let mut values = Vec::new();
    if let Some(Diagnostic {
        command,
        stdout,
        stderr,
        exit_status,
    }) = error.diagnostic.map(|diagnostic| *diagnostic)
    {
        if let Some(command) = command {
            values.push(EnvironmentValue {
                name: "command".into(),
                value: command,
                source: ValueSource::AndroidCli,
            });
        }
        if let Some(stdout) = stdout {
            values.push(EnvironmentValue {
                name: "stdout".into(),
                value: stdout,
                source: ValueSource::AndroidCli,
            });
        }
        if let Some(stderr) = stderr {
            values.push(EnvironmentValue {
                name: "stderr".into(),
                value: stderr,
                source: ValueSource::AndroidCli,
            });
        }
        if let Some(status) = exit_status {
            values.push(EnvironmentValue {
                name: "exit_status".into(),
                value: status.to_string(),
                source: ValueSource::AndroidCli,
            });
        }
    }
    EnvironmentDiagnostic {
        code: EnvironmentDiagnosticCode::ProbeFailed,
        message: format!("{message}: {}", error.message),
        values,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEMP_DIR: AtomicU64 = AtomicU64::new(1);

    fn temp_root() -> PathBuf {
        std::env::temp_dir().join(format!(
            "avdkit-env-discovery-{}-{}",
            std::process::id(),
            NEXT_TEMP_DIR.fetch_add(1, Ordering::Relaxed)
        ))
    }

    #[test]
    fn process_environment_overrides_login_shell_and_preserves_sources() {
        let process = BTreeMap::from([
            ("PATH".into(), "/process/bin".into()),
            ("ANDROID_HOME".into(), "/process/sdk".into()),
        ]);
        let login = BTreeMap::from([
            ("PATH".into(), "/login/bin".into()),
            ("ANDROID_HOME".into(), "/login/sdk".into()),
            ("ANDROID_AVD_HOME".into(), "/login/avd".into()),
        ]);
        let (merged, values, diagnostics) = merge_environment(&process, &login);
        let paths = std::env::split_paths(&merged["PATH"]).collect::<Vec<_>>();
        assert_eq!(paths[0], PathBuf::from("/process/bin"));
        assert!(paths.contains(&PathBuf::from("/login/bin")));
        assert_eq!(merged["ANDROID_AVD_HOME"], "/login/avd");
        assert!(values.iter().any(|value| {
            value.name == "ANDROID_AVD_HOME" && value.source == ValueSource::LoginShell
        }));
        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == EnvironmentDiagnosticCode::SourceConflict));
    }

    #[test]
    fn sdk_root_precedence_and_conflicts_are_explicit() {
        let environment = BTreeMap::from([
            ("ANDROID_HOME".into(), "/environment/sdk".into()),
            ("ANDROID_SDK_ROOT".into(), "/deprecated/sdk".into()),
        ]);
        let values = [
            EnvironmentValue {
                name: "ANDROID_HOME".into(),
                value: "/environment/sdk".into(),
                source: ValueSource::LoginShell,
            },
            EnvironmentValue {
                name: "ANDROID_SDK_ROOT".into(),
                value: "/deprecated/sdk".into(),
                source: ValueSource::ProcessEnvironment,
            },
        ];
        let selection = select_sdk_root(
            Some(PathBuf::from("/caller/sdk")),
            &environment,
            &values,
            Some(PathBuf::from("/android/sdk")),
            PathBuf::from("/default/sdk"),
        );
        assert_eq!(selection.path, PathBuf::from("/caller/sdk"));
        assert_eq!(selection.source, ValueSource::CallerOverride);
        assert_eq!(selection.diagnostics.len(), 1);
        assert_eq!(selection.diagnostics[0].values.len(), 5);

        let selection = select_sdk_root(
            None,
            &environment,
            &values,
            None,
            PathBuf::from("/default/sdk"),
        );
        assert_eq!(selection.path, PathBuf::from("/environment/sdk"));
        assert_eq!(selection.source, ValueSource::LoginShell);
    }

    #[test]
    fn scans_sdk_packages_and_reports_legacy_tools_without_routing_them() {
        let root = temp_root();
        fs::create_dir_all(root.join("emulator")).unwrap();
        fs::create_dir_all(root.join("platform-tools")).unwrap();
        fs::create_dir_all(root.join("cmdline-tools/12.0/bin")).unwrap();
        fs::create_dir_all(root.join("system-images/android-36/google_apis/arm64-v8a")).unwrap();
        fs::create_dir_all(root.join("tools/bin")).unwrap();
        for path in [
            root.join("emulator/emulator"),
            root.join("platform-tools/adb"),
            root.join("cmdline-tools/12.0/bin/sdkmanager"),
            root.join("cmdline-tools/12.0/bin/avdmanager"),
            root.join("tools/bin/sdkmanager"),
        ] {
            fs::write(path, "").unwrap();
        }
        fs::write(
            root.join("emulator/source.properties"),
            "Pkg.Path=emulator\nPkg.Revision=37.1.11\n",
        )
        .unwrap();
        fs::write(
            root.join("platform-tools/source.properties"),
            "Pkg.Revision=37.0.1\n",
        )
        .unwrap();
        fs::write(
            root.join("cmdline-tools/12.0/source.properties"),
            "Pkg.Path=cmdline-tools;12.0\nPkg.Revision=12.0\n",
        )
        .unwrap();
        fs::write(
            root.join("system-images/android-36/google_apis/arm64-v8a/source.properties"),
            "Pkg.Revision=1\nSystemImage.Abi=arm64-v8a\n",
        )
        .unwrap();

        let scan = scan_sdk(
            &root,
            &model::ToolNames {
                android: "android".into(),
                adb: "adb".into(),
                emulator: "emulator".into(),
                sdkmanager: "sdkmanager".into(),
                avdmanager: "avdmanager".into(),
            },
        );
        assert_eq!(
            scan.tools
                .iter()
                .filter(|tool| tool.state == ToolState::Available)
                .count(),
            4
        );
        assert_eq!(scan.legacy_tools.len(), 1);
        assert_eq!(scan.legacy_tools[0].state, ToolState::ReportOnly);
        assert!(scan
            .packages
            .iter()
            .any(|package| package.id.kind == PackageKind::SystemImage));

        fs::remove_dir_all(root).unwrap();
    }
}
