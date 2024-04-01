use chrono::{
    offset::LocalResult, DateTime, Local, MappedLocalTime, NaiveDateTime, NaiveTime, TimeZone,
    Timelike, Utc,
};
use clap::{Args, Parser, Subcommand};
use shift_lib::Config;
use std::{io::Write, path::Path, str::FromStr, time::SystemTime};

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

    /// Show all tasks
    #[arg(short, long)]
    all: bool,
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
            let from_time = match &args.from {
                Some(t) => Some(to_date(&t).ok().unwrap_or_else(|| {
                    eprintln!("Could not parse --from time '{}'", t);
                    std::process::exit(1);
                })),
                None => None,
            };
            let to_time = match &args.to {
                Some(t) => Some(to_date(&t).ok().unwrap_or_else(|| {
                    eprintln!("Could not parse --to time '{}'", t);
                    std::process::exit(1);
                })),
                None => None,
            };

            let tasks = shift
                .tasks(&Config {
                    from: from_time,
                    to: to_time,
                    tasks: args.task.clone(),
                    count: args.count,
                    all: args.all,
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
                    .write_all(
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

fn to_date(s: &String) -> anyhow::Result<DateTime<Local>> {
    let time_formats = vec!["%H:%M", "%H:%M:%S"];
    for f in time_formats {
        if let Ok(nt) = NaiveTime::parse_from_str(s, f) {
            if let LocalResult::Single(d) = Local::now().with_time(nt) {
                return Ok(d);
            }
        }
    }
    let date_formats = vec!["%Y-%m-%d %H:%M", "%Y-%m-%d %H:%M:%S"];
    for f in date_formats {
        if let Ok(dt) = NaiveDateTime::parse_from_str(s, f) {
            if let LocalResult::Single(d) = Local.from_local_datetime(&dt) {
                return Ok(d);
            }
        }
    }

    anyhow::bail!("could not parse time")
}
