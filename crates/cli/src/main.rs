use clap::{Parser, Subcommand};
use kit::{Kit, KitConfig};

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
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let kit = Kit::new(KitConfig::default())?;

    match cli.command.unwrap_or(Command::Environment) {
        Command::Environment => {
            let report = kit.environment();
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!(
                    "host: {:?} {:?}",
                    report.snapshot.platform.host.platform,
                    report.snapshot.platform.host.architecture
                );
                println!("sdk: {}", report.snapshot.platform.paths.sdk_root.display());
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
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&kit.capabilities())?);
            } else {
                for capability in kit.capabilities() {
                    println!("{}: {:?}", capability.operation, capability.state);
                }
            }
        }
        Command::Refresh => {
            kit.refresh()?;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&kit.environment())?);
            } else {
                println!("environment refreshed");
            }
        }
    }
    Ok(())
}
