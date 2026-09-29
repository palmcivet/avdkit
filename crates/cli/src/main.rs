#![allow(clippy::result_large_err)]

use std::process::ExitCode;

use clap::{Parser, Subcommand};
use kit::{AvdId, Envelope, Error, Field, Kit, KitConfig};

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
}

#[derive(Debug, Subcommand)]
enum DevicesCommand {
    List,
    Get { id: String },
    Profiles,
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    let json = cli.json;
    match run(cli).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            if json {
                eprintln!("{}", render_json(&Envelope::new(error)));
            } else {
                eprintln!("{error}");
            }
            ExitCode::from(1)
        }
    }
}

async fn run(cli: Cli) -> Result<(), Error> {
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
    }
    Ok(())
}

fn render_json<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_string_pretty(value).expect("public response models must serialize")
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
}
