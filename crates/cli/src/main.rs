use clap::{Parser, Subcommand};
use kit::{Envelope, Kit, KitConfig};

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
    Profiles,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let kit = Kit::new(KitConfig::default())?;

    match cli.command.unwrap_or(Command::Environment) {
        Command::Environment => {
            let report = kit.environment().await?;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&Envelope::new(&report))?);
            } else {
                println!(
                    "host: {:?} {:?}",
                    report.snapshot.host.platform, report.snapshot.host.architecture
                );
                println!("sdk: {}", report.snapshot.paths.sdk_root.display());
                for tool in report.snapshot.tools {
                    println!(
                        "{}: {}",
                        tool.name,
                        tool.path
                            .map_or_else(|| "missing".into(), |path| path.display().to_string())
                    );
                }
            }
        }
        Command::Capabilities => {
            let capabilities = kit.capabilities().await?;
            if cli.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&Envelope::new(&capabilities))?
                );
            } else {
                for capability in capabilities {
                    println!("{}: {:?}", capability.id.as_str(), capability.state);
                }
            }
        }
        Command::Refresh => {
            let report = kit.refresh().await?;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&Envelope::new(&report))?);
            } else {
                println!("environment refreshed");
            }
        }
        Command::Devices {
            command: DevicesCommand::Profiles,
        } => {
            let profiles = kit.profiles().await?;
            if cli.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&Envelope::new(&profiles))?
                );
            } else {
                for profile in profiles {
                    println!("{}", profile.id);
                }
            }
        }
    }
    Ok(())
}
