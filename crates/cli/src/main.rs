#![allow(clippy::result_large_err)]

use std::{
    fs,
    path::{Path, PathBuf},
    process::ExitCode,
};

use clap::{Parser, Subcommand};
use kit::{
    AvdId, CreateDeviceDraft, Envelope, Error, ErrorCode, Event, Field, HardwareConfig, Kit,
    KitConfig, Operation, PackageId, PackageKind, Plan, ProfileId,
};

#[derive(Debug, Parser)]
#[command(name = "avdkit", version, about = "Android virtual device management")]
struct Cli {
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    Environment,
    Capabilities,
    Refresh,
    Devices {
        #[command(subcommand)]
        command: DevicesCommand,
    },
    Plan {
        #[command(subcommand)]
        command: PlanCommand,
    },
    Runtime {
        #[command(subcommand)]
        command: RuntimeCommand,
    },
}

#[derive(Debug, Subcommand)]
enum DevicesCommand {
    List,
    Get { id: String },
    Profiles,
}

#[derive(Debug, Subcommand)]
enum PlanCommand {
    Create {
        #[arg(long)]
        id: String,
        #[arg(long)]
        profile: String,
        #[arg(long)]
        image: String,
        #[arg(long)]
        display_name: Option<String>,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    Show {
        path: PathBuf,
    },
    Execute {
        path: PathBuf,
        #[arg(long)]
        approve: bool,
    },
}

#[derive(Debug, Subcommand)]
enum RuntimeCommand {
    Running,
    BootStatus { id: String },
    Start { id: String },
    Stop { id: String },
}

struct CliFailure {
    error: Error,
    rendered: bool,
}

impl From<Error> for CliFailure {
    fn from(error: Error) -> Self {
        Self {
            error,
            rendered: false,
        }
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    let json = cli.json;
    match run(cli).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(failure) => {
            if !failure.rendered {
                if json {
                    eprintln!("{}", render_json(&Envelope::new(&failure.error)));
                } else {
                    eprintln!("{}", failure.error);
                }
            }
            ExitCode::from(exit_code(failure.error.code))
        }
    }
}

async fn run(cli: Cli) -> Result<(), CliFailure> {
    let kit = Kit::new_async(KitConfig::default()).await?;
    match cli.command.unwrap_or(Command::Environment) {
        Command::Environment => {
            let report = kit.environment().await?;
            if cli.json {
                println!("{}", render_json(&Envelope::new(&report)));
            } else {
                println!(
                    "host: {:?} {:?}",
                    report.snapshot.host.platform, report.snapshot.host.architecture
                );
                println!("sdk: {}", report.snapshot.paths.sdk_root.display());
                for tool in report.snapshot.tools {
                    println!(
                        "{}: {:?} {}",
                        tool.name,
                        tool.state,
                        tool.path
                            .map_or_else(|| "missing".into(), |path| path.display().to_string())
                    );
                }
            }
        }
        Command::Capabilities => {
            let capabilities = kit.capabilities().await?;
            if cli.json {
                println!("{}", render_json(&Envelope::new(&capabilities)));
            } else {
                for capability in capabilities {
                    println!("{}: {:?}", capability.id.as_str(), capability.state);
                }
            }
        }
        Command::Refresh => {
            let report = kit.refresh().await?;
            if cli.json {
                println!("{}", render_json(&Envelope::new(&report)));
            } else {
                println!("environment refreshed");
            }
        }
        Command::Devices { command } => match command {
            DevicesCommand::List => {
                let devices = kit.list_devices().await?;
                if cli.json {
                    println!("{}", render_json(&Envelope::new(&devices)));
                } else {
                    for device in devices {
                        match device.display_name {
                            Field::Present { value } => println!("{}\t{}", device.id, value),
                            Field::Unavailable | Field::Inapplicable => println!("{}", device.id),
                        }
                    }
                }
            }
            DevicesCommand::Get { id } => {
                let device = kit
                    .get_device(&AvdId::new(id).map_err(Error::from)?)
                    .await?;
                if cli.json {
                    println!("{}", render_json(&Envelope::new(&device)));
                } else {
                    println!("id: {}", device.id);
                    if let Field::Present { value } = device.display_name {
                        println!("name: {value}");
                    }
                    if let Field::Present { value } = device.profile {
                        println!("profile: {value}");
                    }
                    if let Field::Present { value } = device.image {
                        println!("image: {}", value.render_slash());
                    }
                    if let Field::Present { value } = device.target {
                        println!("target: {value}");
                    }
                }
            }
            DevicesCommand::Profiles => {
                let profiles = kit.profiles().await?;
                if cli.json {
                    println!("{}", render_json(&Envelope::new(&profiles)));
                } else {
                    for profile in profiles {
                        println!("{}", profile.id);
                    }
                }
            }
        },
        Command::Plan { command } => match command {
            PlanCommand::Create {
                id,
                profile,
                image,
                display_name,
                output,
            } => {
                let plan = kit.plan_create(CreateDeviceDraft {
                    id: AvdId::new(id).map_err(Error::from)?,
                    profile: ProfileId::new(profile).map_err(Error::from)?,
                    image: parse_image(&image)?,
                    display_name,
                    hardware: HardwareConfig::default(),
                })?;
                if let Some(path) = output {
                    write_plan(&path, &plan)?;
                }
                render_plan(&plan, cli.json);
            }
            PlanCommand::Show { path } => {
                let plan = read_plan(&path)?;
                render_plan(&plan, cli.json);
            }
            PlanCommand::Execute { path, approve } => {
                if !approve {
                    return Err(Error::new(
                        ErrorCode::InvalidInput,
                        "plan execution requires --approve",
                    )
                    .into());
                }
                let plan = read_plan(&path)?;
                run_operation(kit.execute_plan(plan), cli.json).await?;
            }
        },
        Command::Runtime { command } => match command {
            RuntimeCommand::Running => {
                let instances = kit.running().await?;
                if cli.json {
                    println!("{}", render_json(&Envelope::new(&instances)));
                } else {
                    for instance in instances {
                        println!(
                            "{}\t{}",
                            instance.id,
                            instance
                                .serial
                                .as_value()
                                .map_or("unavailable", |serial| serial.as_str())
                        );
                    }
                }
            }
            RuntimeCommand::BootStatus { id } => {
                let status = kit
                    .boot_status(&AvdId::new(id).map_err(Error::from)?)
                    .await?;
                if cli.json {
                    println!("{}", render_json(&Envelope::new(status)));
                } else {
                    println!("{status:?}");
                }
            }
            RuntimeCommand::Start { id } => {
                let id = AvdId::new(id).map_err(Error::from)?;
                run_operation(kit.start(id, Default::default()), cli.json).await?;
            }
            RuntimeCommand::Stop { id } => {
                let id = AvdId::new(id).map_err(Error::from)?;
                run_operation(kit.stop(id), cli.json).await?;
            }
        },
    }
    Ok(())
}

async fn run_operation(operation: Operation, json: bool) -> Result<(), CliFailure> {
    let cancellation = tokio::signal::ctrl_c();
    tokio::pin!(cancellation);
    let mut cancellation_requested = false;
    loop {
        let event = if cancellation_requested {
            operation.next_event().await
        } else {
            tokio::select! {
                event = operation.next_event() => event,
                signal = &mut cancellation => {
                    operation.cancel();
                    cancellation_requested = true;
                    if let Err(error) = signal {
                        return Err(Error::new(
                            ErrorCode::Internal,
                            format!("install Ctrl-C handler: {error}"),
                        ).into());
                    }
                    continue;
                }
            }
        };
        let Some(event) = event else {
            break;
        };
        if json {
            println!("{}", render_json_line(&Envelope::new(event)));
        } else {
            render_event(&event);
        }
    }
    match operation.result().await {
        Ok(result) => {
            if json {
                println!("{}", render_json_line(&Envelope::new(result)));
            } else {
                println!("{result:?}");
            }
            Ok(())
        }
        Err(error) if json => {
            println!("{}", render_json_line(&Envelope::new(&error)));
            Err(CliFailure {
                error,
                rendered: true,
            })
        }
        Err(error) => Err(error.into()),
    }
}

fn render_event(event: &Event) {
    match event {
        Event::StepStarted { message, .. }
        | Event::StepFinished { message, .. }
        | Event::CompensationStarted { message, .. }
        | Event::CompensationFinished { message, .. }
        | Event::Warning { message } => println!("{message}"),
        Event::Progress { ratio } => match ratio {
            Some(ratio) => println!("{:.0}%", ratio * 100.0),
            None => println!("progress unavailable"),
        },
        Event::Log { line, .. } => println!("{line}"),
    }
}

fn parse_image(value: &str) -> Result<PackageId, Error> {
    let normalized = value.replace(';', "/");
    let segments = normalized
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    if segments.len() < 4 || segments[0] != "system-images" {
        return Err(Error::new(
            ErrorCode::InvalidInput,
            "image must be system-images;android-<api>;<tag>;<abi>",
        ));
    }
    Ok(PackageId {
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
    })
}

fn read_plan(path: &Path) -> Result<Plan, Error> {
    let contents = fs::read_to_string(path).map_err(|error| {
        Error::new(
            ErrorCode::InvalidInput,
            format!("read plan {}: {error}", path.display()),
        )
    })?;
    serde_json::from_str(&contents).map_err(|error| {
        Error::new(
            ErrorCode::InvalidInput,
            format!("parse plan {}: {error}", path.display()),
        )
    })
}

fn write_plan(path: &Path, plan: &Plan) -> Result<(), Error> {
    let contents = format!("{}\n", render_json(plan));
    fs::write(path, contents).map_err(|error| {
        Error::new(
            ErrorCode::Internal,
            format!("write plan {}: {error}", path.display()),
        )
    })
}

fn render_plan(plan: &Plan, json: bool) {
    if json {
        println!("{}", render_json(&Envelope::new(plan)));
    } else {
        println!("plan: {} ({:?})", plan.id, plan.kind);
        for step in &plan.steps {
            println!("  {}: {}", step.id, step.description);
        }
    }
}

fn exit_code(code: ErrorCode) -> u8 {
    match code {
        ErrorCode::InvalidInput => 2,
        ErrorCode::CapabilityUnavailable
        | ErrorCode::ToolNotFound
        | ErrorCode::PlatformNotSupported => 3,
        ErrorCode::PreconditionFailed
        | ErrorCode::DeviceRunning
        | ErrorCode::DeviceNotFound
        | ErrorCode::NameConflict
        | ErrorCode::PackageNotFound => 4,
        ErrorCode::Cancelled | ErrorCode::Timeout => 5,
        ErrorCode::LaunchFailed | ErrorCode::ToolOutputUnrecognized | ErrorCode::Internal => 1,
    }
}

fn render_json<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_string_pretty(value).expect("public response models must serialize")
}

fn render_json_line<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_string(value).expect("public response models must serialize")
}

#[cfg(test)]
mod tests {
    use super::*;
    use kit::Reason;

    #[test]
    fn cli_error_json_matches_golden_contract() {
        let error = Error::capability_unavailable(
            "runtime_start is unavailable",
            vec![Reason::not_implemented()],
        );
        let actual = format!("{}\n", render_json(&Envelope::new(error)));
        assert_eq!(actual, include_str!("../tests/golden/error-response.json"));
    }

    #[test]
    fn stable_exit_codes_cover_every_error_class() {
        assert_eq!(exit_code(ErrorCode::Internal), 1);
        assert_eq!(exit_code(ErrorCode::InvalidInput), 2);
        assert_eq!(exit_code(ErrorCode::CapabilityUnavailable), 3);
        assert_eq!(exit_code(ErrorCode::NameConflict), 4);
        assert_eq!(exit_code(ErrorCode::Cancelled), 5);
    }

    #[test]
    fn parses_both_repository_and_path_image_ids() {
        let semicolon = parse_image("system-images;android-36;google_apis;arm64-v8a").unwrap();
        let slash = parse_image("system-images/android-36/google_apis/arm64-v8a").unwrap();
        assert_eq!(semicolon, slash);
    }
}
