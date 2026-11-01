//! Command line: five subcommands and the common flags (RFC "CLI i konfiguracja").

use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "git-bonsai",
    version,
    about = "Grows a pixel bonsai from a GitHub account's public activity"
)]
pub struct Cli {
    #[command(subcommand)]
    pub cmd: Cmd,
    #[command(flatten)]
    pub common: Common,
}

/// Flags every subcommand accepts.
#[derive(Args)]
pub struct Common {
    /// Time zone of the day to grow (IANA name, e.g. Europe/Warsaw); default UTC.
    #[arg(long, global = true)]
    pub tz: Option<String>,
    /// Static SVG without the growth animation.
    #[arg(long, global = true)]
    pub no_animate: bool,
    /// Also write `bonsai.gif` (grow, plant).
    #[arg(long, global = true)]
    pub gif: bool,
    /// Living branches at most (overrides bonsai.toml).
    #[arg(long, global = true)]
    pub max_branches: Option<usize>,
    /// Comma-separated repo name globs without a branch (overrides bonsai.toml).
    #[arg(long, global = true, value_delimiter = ',')]
    pub exclude: Option<Vec<String>>,
    /// Log progress to stderr (never the token).
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    pub verbose: u8,
    /// Config file; default `bonsai.toml` next to the state file.
    #[arg(long, global = true)]
    pub config: Option<PathBuf>,
    /// GitHub token; default GITHUB_TOKEN, GH_TOKEN, then `gh auth token`.
    #[arg(long, global = true)]
    pub token: Option<String>,
    /// The day to fetch and grow (YYYY-MM-DD); default yesterday in --tz.
    #[arg(long, global = true)]
    pub date: Option<String>,
}

#[derive(Debug, Subcommand)]
pub enum Cmd {
    /// Fetch, grow the missing days, write the state and the images.
    Grow {
        #[arg(long)]
        login: String,
        #[arg(long, default_value = "bonsai/state.json")]
        state: PathBuf,
        #[arg(long, default_value = "bonsai")]
        out: PathBuf,
        /// Themes to write as `bonsai-<theme>.svg`.
        #[arg(long, value_enum, value_delimiter = ',', default_value = "dark,light")]
        themes: Vec<ThemeArg>,
        /// Use a saved snapshot instead of fetching.
        #[arg(long)]
        from_snapshot: Option<PathBuf>,
    },
    /// Render a state to one SVG, offline.
    Render {
        #[arg(long)]
        state: PathBuf,
        #[arg(long, value_enum, default_value = "dark")]
        theme: ThemeArg,
        #[arg(long)]
        out: PathBuf,
        /// Show the tree in this phase instead of the state's.
        #[arg(long, value_enum)]
        phase: Option<PhaseArg>,
    },
    /// Save the day's data to a file (a test fixture).
    Snapshot {
        #[arg(long)]
        login: String,
        #[arg(long)]
        out: PathBuf,
    },
    /// New state with the account's history simulated day by day.
    Plant {
        /// Required unless --from-snapshot is given.
        #[arg(long)]
        login: Option<String>,
        #[arg(long)]
        from_snapshot: Option<PathBuf>,
        #[arg(long, default_value = "bonsai/state.json")]
        state: PathBuf,
        #[arg(long, default_value = "bonsai")]
        out: PathBuf,
        #[arg(long, value_enum, value_delimiter = ',', default_value = "dark,light")]
        themes: Vec<ThemeArg>,
        /// Replace an existing state file.
        #[arg(long)]
        force: bool,
    },
    /// Print the tree's grid as an ASCII map.
    Debug {
        #[arg(long, default_value = "bonsai/state.json")]
        state: PathBuf,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ThemeArg {
    Dark,
    Light,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum PhaseArg {
    Sapling,
    Growing,
    Bloom,
    Dormant,
}
