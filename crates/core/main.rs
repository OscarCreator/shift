use chrono::Local;
use clap::Parser;
use cli::{Cli, Commands};
use shift_lib::{
    commands::{
        event,
        events::{self, events},
        pause::{pause, resume},
        start::{start, StartOpts},
        status::status,
        stop::{self, stop, StopOpts},
        undo::{self, undo},
    },
    Config, TaskEvent,
};
use std::{env::var, fs, io::Write, path::Path};

use parse::to_date;

mod cli;
mod parse;

fn main() {
    let cli = Cli::parse();

    let config_home = var("XDG_CONFIG_HOME")
        .or_else(|_| var("HOME").map(|home| format!("{}/.local/share/st", home)))
        .unwrap_or_else(|_| {
            eprintln!("XDG_CONFIG_HOME or HOME environment variable not found");
            std::process::exit(1);
        });
    fs::create_dir_all(&config_home).unwrap_or_else(|err| {
        eprintln!("Could not create {config_home} directories, Error: {err}");
        std::process::exit(1);
    });
    let db_path = Path::new(&config_home).join("events.db");
    let shift = shift_lib::ShiftDb::new(db_path);

    match &cli.command {
        Commands::Status => {
            let config = shift_lib::Config {
                uid: None,
                ..Default::default()
            };
            // TODO add json support
            status(&shift, &config).unwrap_or_else(|err| {
                eprintln!("{err}");
                std::process::exit(1);
            });
        }
        Commands::Start(args) => {
            let start_time = args.at.as_ref().map(|t| {
                to_date(t).ok().unwrap_or_else(|| {
                    eprintln!("Could not parse --at time '{t}'");
                    std::process::exit(1);
                })
            });
            let opts = shift_lib::commands::start::StartOpts {
                uid: Some(args.name.clone()),
                start_time,
            };
            start(&shift, &opts).unwrap_or_else(|err| {
                eprintln!("{err}");
                std::process::exit(1);
            });
        }
        Commands::Stop(args) => {
            let config = shift_lib::commands::stop::StopOpts {
                uid: args.name.clone(),
                all: args.all,
                ..Default::default()
            };
            stop(&shift, &config).unwrap_or_else(|err| {
                match err {
                    stop::Error::MultipleSessions(tasks) => {
                        for task in tasks {
                            eprintln!("{task}");
                        }
                        eprintln!("Multiple tasks started. Need to specify a unique task or uuid");
                    }
                    stop::Error::NoTasks => {
                        eprintln!("No tasks to stop");
                    }
                }
                std::process::exit(1);
            });
        }
        Commands::Log(args) => {
            let from_time = args.from.as_ref().map(|t| {
                to_date(t).ok().unwrap_or_else(|| {
                    eprintln!("Could not parse --from time '{t}'");
                    std::process::exit(1);
                })
            });
            let to_time = args.to.as_ref().map(|t| {
                to_date(t).ok().unwrap_or_else(|| {
                    eprintln!("Could not parse --to time '{t}'");
                    std::process::exit(1);
                })
            });

            let tasks = events(
                &shift,
                &events::Opts {
                    from: from_time,
                    to: to_time,
                    tasks: args.task.clone(),
                    count: if args.all { None } else { Some(args.count) },
                },
            )
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
        // TODO do no be able to switch to same as ongoing
        Commands::Switch(args) => {
            let time = Local::now();
            stop(
                &shift,
                &StopOpts {
                    stop_time: Some(time),
                    ..Default::default()
                },
            )
            .unwrap_or_else(|err| {
                eprintln!("{err}");
                std::process::exit(1);
            });

            start(
                &shift,
                &StartOpts {
                    uid: Some(args.uid.clone()),
                    start_time: Some(time),
                },
            )
            .unwrap_or_else(|err| {
                eprintln!("{err}");
                std::process::exit(1);
            });
        }
        Commands::Remove { uid: _ } => todo!(),
        Commands::Pause(args) => pause(
            &shift,
            &Config {
                uid: args.uid.clone(),
                all: args.all,
                ..Default::default()
            },
        )
        .unwrap_or_else(|err| {
            eprintln!("{err}");
            std::process::exit(1);
        }),
        Commands::Resume(args) => resume(
            &shift,
            &Config {
                uid: args.uid.clone(),
                all: args.all,
                ..Default::default()
            },
        )
        .unwrap_or_else(|err| {
            eprintln!("{err}");
            std::process::exit(1);
        }),
        Commands::Undo => {
            undo(&shift, &undo::Opts::default()).unwrap_or_else(|err| {
                eprintln!("{err}");
                std::process::exit(1);
            });
        }
        Commands::Edit(args) => {
            // get event, default latest otherwise by uid
            let event = event::event(
                &shift,
                &event::Opts {
                    uid: args.uid.to_owned(),
                },
            )
            .unwrap_or_else(|err| {
                eprintln!("{err}");
                std::process::exit(1);
            });

            let res = edit::edit(
                serde_json::to_string_pretty(&event)
                    .expect("Default impl of serialize should not fail"),
            )
            .unwrap_or_else(|err| {
                eprintln!("{err}");
                std::process::exit(1);
            });

            let updated_event: TaskEvent = serde_json::from_str(&res).unwrap();
            // TODO validate so it does not break anything
            event::update(&shift, event, updated_event).unwrap_or_else(|err| {
                eprintln!("{err}");
                std::process::exit(1);
            })
        }
    }
}
