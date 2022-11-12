use std::path::PathBuf;

use clap::{arg, builder::PathBufValueParser, Args, FromArgMatches, Parser, Subcommand};
use regex::Regex;

use crate::{ReplicatorKind, Template, TemplateParser};

/// A pictures/files organizer.
#[derive(Parser, Debug)]
#[command(author, version, about)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
#[command(author = None, version, about)]
pub enum Command {
    /// Sort all files once.
    Sort(CliArgs),

    /// Watch & sort files as their added.
    Watch(WatchCmd),
}

#[derive(Args, Debug)]
pub struct CliArgs {
    /// Overwrite destination file if it already exists
    #[arg(short, long, default_value = "false", group = "CliArgs")]
    pub overwrite: bool,

    /// Ignore source files that match this regular expression.
    #[arg(short, long, group = "CliArgs")]
    pub ignore_regex: Option<Regex>,

    /// How files are replicated in preference order.
    #[arg(short, long, default_values = ["hardlink", "softlink", "copy"], group = "CliArgs")]
    pub replicators: Vec<ReplicatorKind>,

    /// Destination file template.
    #[arg(value_parser = TemplateParser::default(), group = "CliArgs")]
    pub template: Template,

    /// Sources files/directories to replicates.
    #[arg(value_parser = PathBufValueParser::default(), group = "CliArgs")]
    pub sources: Vec<PathBuf>,
}

#[derive(Args, Debug)]
pub struct ConfigArgs {
    /// Sets config file path.
    #[arg(
        short = 'c',
        long = "config",
        conflicts_with = "overwrite",
        conflicts_with = "ignore_regex",
        conflicts_with = "replicators",
        conflicts_with = "template",
        conflicts_with = "sources",
        required = false
    )]
    pub path: PathBuf,
}

// User should specify either CliArgs or ConfigArgs
#[derive(Debug)]
pub enum CliOrConfigArgs {
    Cli(CliArgs),
    Config(ConfigArgs),
}

impl FromArgMatches for CliOrConfigArgs {
    fn from_arg_matches(matches: &clap::ArgMatches) -> Result<Self, clap::Error> {
        if matches.get_one::<PathBuf>("path").is_some() {
            ConfigArgs::from_arg_matches(matches).map(CliOrConfigArgs::Config)
        } else {
            CliArgs::from_arg_matches(matches).map(CliOrConfigArgs::Cli)
        }
    }

    fn update_from_arg_matches(&mut self, matches: &clap::ArgMatches) -> Result<(), clap::Error> {
        match self {
            Self::Cli(cli) => cli.update_from_arg_matches(matches),
            Self::Config(cfg) => cfg.update_from_arg_matches(matches),
        }
    }
}

impl Args for CliOrConfigArgs {
    fn augment_args(cmd: clap::Command) -> clap::Command {
        let cmd = CliArgs::augment_args(cmd);
        ConfigArgs::augment_args(cmd)
    }

    fn augment_args_for_update(cmd: clap::Command) -> clap::Command {
        let cmd = CliArgs::augment_args_for_update(cmd);
        ConfigArgs::augment_args_for_update(cmd)
    }
}

#[derive(Args, Debug)]
pub struct WatchCmd {
    #[command(flatten)]
    pub common: CliOrConfigArgs,

    /// Fork a daemon process.
    #[arg(short, long)]
    pub daemon: bool,
}
