use clap::{Parser, Subcommand, ValueEnum};
use crate::rules::Profile;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// Path to file or directory
    pub path: Option<String>,

    /// Walk directories recursively
    #[arg(short, long)]
    pub recursive: bool,

    /// Show what would change, write nothing
    #[arg(short = 'd', long)]
    pub dry_run: bool,

    /// Print unified diff to stdout
    #[arg(long)]
    pub diff: bool,

    /// Confirm each match individually
    #[arg(short, long)]
    pub interactive: bool,

    /// Rule set: conservative, prose, code, aggressive
    #[arg(short, long, value_enum, default_value_t = ProfileArg::Prose)]
    pub profile: ProfileArg,

    /// Enable only this rule (repeatable)
    #[arg(long)]
    pub fix: Vec<String>,

    /// Disable this rule (repeatable)
    #[arg(long)]
    pub skip: Vec<String>,

    /// Write to this path instead
    #[arg(short, long)]
    pub output: Option<String>,

    /// Print score summary after processing
    #[arg(long)]
    pub report: bool,

    /// Exit code 1 if any file exceeds N hits/1k chars
    #[arg(long)]
    pub threshold: Option<f32>,

    /// Skip matches below this confidence
    #[arg(long)]
    pub min_confidence: Option<f32>,

    /// Show reasoning for each match
    #[arg(long)]
    pub explain: bool,

    /// Output format: human, json
    #[arg(long, value_enum, default_value_t = FormatArg::Human)]
    pub format: FormatArg,

    /// Load config from path
    #[arg(long)]
    pub config: Option<String>,

    /// Force language: py, js, rs, md, txt, ...
    #[arg(long)]
    pub lang: Option<String>,

    /// Scan git-staged files only (pre-commit mode)
    #[arg(long)]
    pub staged: bool,

    /// Write a .desloprc.toml for the current project
    #[arg(long)]
    pub init: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Validate all .deslop-rules files
    CheckRules,
    /// Install a pre-commit hook to block commits with slop
    InstallHook,
}

#[derive(ValueEnum, Clone, Debug, PartialEq)]
pub enum ProfileArg {
    Conservative,
    Prose,
    Code,
    Aggressive,
}

impl Into<Profile> for ProfileArg {
    fn into(self) -> Profile {
        match self {
            ProfileArg::Conservative => Profile::Conservative,
            ProfileArg::Prose => Profile::Prose,
            ProfileArg::Code => Profile::Code,
            ProfileArg::Aggressive => Profile::Aggressive,
        }
    }
}

#[derive(ValueEnum, Clone, Debug, PartialEq)]
pub enum FormatArg {
    Human,
    Json,
}
