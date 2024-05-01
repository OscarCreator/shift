use clap::Parser;
use cli::{Cli, Commands};
use shift_lib::Config;
use std::{io::Write, path::Path};

use parse::to_date;

mod cli;
mod parse;

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
        Commands::Start(args) => {
            let start_time = args.at.as_ref().map(|t| {
                to_date(t).ok().unwrap_or_else(|| {
                    eprintln!("Could not parse --at time '{}'", t);
                    std::process::exit(1);
                })
            });
            let config = shift_lib::Config {
                uid: Some(args.name.clone()),
                start_time,
                ..Default::default()
            };
            shift.start(&config).unwrap_or_else(|err| {
                eprintln!("{err}");
                std::process::exit(1);
            });
        }
        Commands::Stop(args) => {
            let config = shift_lib::Config {
                uid: args.name.clone(),
                all: args.all,
                ..Default::default()
            };
            shift.stop(&config).unwrap_or_else(|err| {
                match err {
                    shift_lib::StopError::MultipleTasks(tasks) => {
                        for task in tasks {
                            eprintln!("{}", task);
                        }
                        eprintln!("Multiple tasks started. Need to specify a unique task or uuid")
                    }
                    shift_lib::StopError::UpdateError(task) => {
                        eprintln!("Could not update ongoing task with name: {} ", task.name)
                    }
                    shift_lib::StopError::SqlError(err) => {
                        eprintln!("SQL error: {}", err)
                    }
                    shift_lib::StopError::NoTasks => {
                        eprintln!("No tasks to stop")
                    }
                }
                std::process::exit(1);
            });
        }
        Commands::Log(args) => {
            let from_time = args.from.as_ref().map(|t| {
                to_date(t).ok().unwrap_or_else(|| {
                    eprintln!("Could not parse --from time '{}'", t);
                    std::process::exit(1);
                })
            });
            let to_time = args.to.as_ref().map(|t| {
                to_date(t).ok().unwrap_or_else(|| {
                    eprintln!("Could not parse --to time '{}'", t);
                    std::process::exit(1);
                })
            });

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
        Commands::Switch(args) => {
            shift
                .stop(&Config {
                    ..Default::default()
                })
                .unwrap_or_else(|err| {
                    eprintln!("{err}");
                    std::process::exit(1);
                });
            shift
                .start(&Config {
                    uid: Some(args.uid.clone()),
                    ..Default::default()
                })
                .unwrap_or_else(|err| {
                    eprintln!("{err}");
                    std::process::exit(1);
                });
        }
        Commands::Remove { uid: _ } => todo!(),
        Commands::Pause(args) => shift
            .pause(&Config {
                uid: args.uid.clone(),
                ..Default::default()
            })
            .unwrap_or_else(|err| {
                eprintln!("{err}");
                std::process::exit(1);
            }),
        Commands::Resume(args) => shift
            .resume(&Config {
                uid: args.uid.clone(),
                ..Default::default()
            })
            .unwrap_or_else(|err| {
                eprintln!("{err}");
                std::process::exit(1);
            }),
    }
}
