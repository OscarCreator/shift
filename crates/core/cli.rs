use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(author, version)]
#[command(propagate_version = true)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: Commands,
}

#[derive(Subcommand)]
pub(crate) enum Commands {
    /// Show current status
    Status,
    /// Start a task
    Start(StartArgs),
    /// Stop a task
    Stop(StopArgs),
    /// Log tasks
    Log(LogArgs),
    /// Switch to another task
    Switch(SwitchArgs),
    /// TODO
    Remove { uid: String },
    /// Pause a ongoing task
    Pause(PauseArgs),
    /// Resume a paused task
    Resume(ResumeArgs),
    /// Undo latest command
    Undo,
}

#[derive(Args)]
pub(crate) struct StartArgs {
    /// Name of task
    pub(crate) name: String,

    /// Start time instead of task
    #[arg(short, long)]
    pub(crate) at: Option<String>,
}

#[derive(Args)]
pub(crate) struct StopArgs {
    /// Name or uuid of task
    pub(crate) name: Option<String>,

    /// Stop all started tasks
    #[arg(short, long)]
    pub(crate) all: bool,
}

#[derive(Args)]
pub(crate) struct LogArgs {
    /// Search from time
    #[arg(short, long)]
    pub(crate) from: Option<String>,

    /// Search to time
    #[arg(long)]
    pub(crate) to: Option<String>,

    /// Task names
    #[arg(short, long)]
    pub(crate) task: Vec<String>,

    #[arg(
        short,
        long,
        default_value_t = 10,
        help = "Show quantity of tasks",
        long_help = "Max value of tasks displayed. Most recent tasks will be chosen \
                     first and from the time of --from and forward in time if \
                     it's specified."
    )]
    pub(crate) count: usize,

    /// Output as json
    #[arg(short, long)]
    pub(crate) json: bool,

    /// Show all tasks
    #[arg(short, long)]
    pub(crate) all: bool,
}

#[derive(Args)]
pub(crate) struct SwitchArgs {
    pub(crate) uid: String,
}

#[derive(Args)]
pub(crate) struct PauseArgs {
    pub(crate) uid: Option<String>,
    pub(crate) all: bool,
}

#[derive(Args)]
pub(crate) struct ResumeArgs {
    pub(crate) uid: Option<String>,
    pub(crate) all: bool,
}
