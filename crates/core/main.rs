use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(author, version)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Status,
    Start { name: String },
    Stop(StopArgs),
    Log(LogArgs),
}

#[derive(Args)]
struct StopArgs {
    #[arg(short, long)]
    uuid: Option<String>,
}

#[derive(Args)]
struct LogArgs {
    #[arg(short, long)]
    quantity: Option<String>,
}

fn main() {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Status => {
            let config = shift_lib::Config {
                json: false,
                uuid: None,
            };
            shift_lib::status(&config).unwrap();
        }
        Commands::Start { name } => {
            shift_lib::start(name).unwrap();
        }
        Commands::Stop(args) => {
            let config = shift_lib::Config {
                json: false,
                // TODO?
                uuid: args.uuid.clone(),
            };
            shift_lib::stop(&config).unwrap();
        }
        Commands::Log(_args) => {
            shift_lib::log().unwrap();
        }
    }
}
