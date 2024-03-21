use clap::{Args, Parser, Subcommand};
use shift_lib::Config;
use std::{io::Write, path::Path};

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
    Start { uid: String },
    Stop(StopArgs),
    Log(LogArgs),
    Switch { uid: String },
    Remove { uid: String },
    Pause { uid: String },
    Resume { uid: String },
}

#[derive(Args)]
struct StopArgs {
    /// Name or uuid of task
    #[arg(short, long)]
    uid: Option<String>,
}

#[derive(Args)]
struct LogArgs {
    /// Search from time
    #[arg(short, long)]
    from: Option<String>,

    /// Search to time
    #[arg(long)]
    to: Option<String>,

    /// Task names
    #[arg(short, long)]
    task: Vec<String>,

    #[arg(
        short,
        long,
        default_value_t = 10,
        help = "Show quantity of tasks",
        long_help = "Max value of tasks displayed. Most recent tasks will be chosen \
                     first and from the time of --from and forward in time if \
                     it's specified."
    )]
    count: usize,

    /// Output as json
    #[arg(short, long)]
    json: bool,
}

fn main() {
    let cli = Cli::parse();

    let shift = shift_lib::Shift::new(Path::new(env!("CARGO_MANIFEST_DIR")).join("tasks.db"));

    match &cli.command {
        Commands::Status => {
            let config = shift_lib::Config {
                uid: None,
                ..Default::default()
            };
            shift.status(&config).unwrap_or_else(|err| {
                eprintln!("{err}");
                std::process::exit(1);
            });
        }
        Commands::Start { uid: name } => {
            shift.start(name).unwrap_or_else(|err| {
                eprintln!("{err}");
                std::process::exit(1);
            });
        }
        Commands::Stop(args) => {
            let config = shift_lib::Config {
                // TODO?
                uid: args.uid.clone(),
                ..Default::default()
            };
            shift.stop(&config).unwrap_or_else(|err| {
                eprintln!("{err}");
                std::process::exit(1);
            });
        }
        Commands::Log(args) => {
            // TODO parse dates

            let tasks = shift
                .log(&Config {
                    from: None,
                    to: None,
                    tasks: args.task.clone(),
                    count: args.count,
                    ..Default::default()
                })
                .unwrap_or_else(|err| {
                    eprintln!("{err}");
                    std::process::exit(1);
                });

            if args.json {
                let stdout = std::io::stdout();
                let mut handle = stdout.lock();
                handle
                    .write(
                        serde_json::to_string(&tasks)
                            .expect("could not deserialize tasks")
                            .as_bytes(),
                    )
                    .expect("could not write to stdout");
            } else {
                for task in tasks {
                    println!("{task}");
                }
            }
        }
        Commands::Switch { uid: _ } => todo!(),
        Commands::Remove { uid: _ } => todo!(),
        Commands::Pause { uid: _ } => todo!(),
        Commands::Resume { uid: _ } => todo!(),
    }
}
