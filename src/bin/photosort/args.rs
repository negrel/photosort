use std::path::PathBuf;

use clap::{
    arg,
    builder::{EnumValueParser, PathBufValueParser},
    Arg, ArgAction, ArgGroup, Args, FromArgMatches, Parser, Subcommand,
};

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

#[derive(Debug)]
pub enum CliOrConfigArgs {
    Config(ConfigArgs),
    Cli(CliArgs),
}

impl FromArgMatches for CliOrConfigArgs {
    fn update_from_arg_matches(&mut self, matches: &clap::ArgMatches) -> Result<(), clap::Error> {
        if matches.get_one::<PathBuf>("config").is_some() {
            ConfigArgs::from_arg_matches(matches).map(|_| ())
        } else {
            CliArgs::from_arg_matches(matches).map(|_| ())
        }
    }

    fn from_arg_matches(matches: &clap::ArgMatches) -> Result<Self, clap::Error> {
        if matches.get_one::<PathBuf>("config").is_some() {
            ConfigArgs::from_arg_matches(matches).map(CliOrConfigArgs::Config)
        } else {
            CliArgs::from_arg_matches(matches).map(CliOrConfigArgs::Cli)
        }
    }
}

impl CliOrConfigArgs {
    fn define_args(cmd: clap::Command) -> clap::Command {
        cmd.arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .value_parser(PathBufValueParser::default())
                .exclusive(true)
                .help("uses config file at the given path"),
        )
        .group(ArgGroup::new("config_grp"))
        .arg(
            Arg::new("overwrite")
                .short('o')
                .long("overwrite")
                .action(ArgAction::SetTrue)
                .help("enable destination file overwriting"),
        )
        .arg(
            Arg::new("replicators")
                .short('r')
                .long("replicator")
                .action(ArgAction::Append)
                .default_values(["hardlink", "softlink", "copy"])
                .value_parser(EnumValueParser::<ReplicatorKind>::new())
                .help("adds this replicator kind to the list"),
        )
        .arg(
            Arg::new("template")
                .value_parser(TemplateParser::default())
                .required(true)
                .help("sets path template for replicated file"),
        )
        .arg(
            Arg::new("sources")
                .num_args(1..)
                .action(ArgAction::Append)
                .value_parser(PathBufValueParser::default())
                .required(true)
                .help("add this replicator kind to the list"),
        )
        .group(
            ArgGroup::new("cli")
                .args(["replicators", "template", "sources"])
                .multiple(true),
        )
    }
}

impl Args for CliOrConfigArgs {
    fn augment_args(cmd: clap::Command) -> clap::Command {
        Self::define_args(cmd)
    }

    fn augment_args_for_update(cmd: clap::Command) -> clap::Command {
        Self::define_args(cmd)
    }
}

#[derive(Parser, Debug)]
#[command()]
pub struct ConfigArgs {
    #[arg(short, long)]
    pub config: PathBuf,
}

#[derive(Parser, Debug)]
#[command()]
pub struct CliArgs {
    #[arg(short, long, default_value = "false")]
    pub overwrite: bool,

    #[arg(short, long, default_values = ["hardlink", "softlink", "copy"])]
    pub replicators: Vec<ReplicatorKind>,

    #[arg(short, long, value_parser = TemplateParser::default())]
    pub template: Template,

    #[arg(value_parser = PathBufValueParser::default())]
    pub sources: Vec<PathBuf>,
}

#[derive(Parser, Debug)]
#[command(author, version, about)]
pub struct WatchCmd {
    #[command(flatten)]
    pub common: CliOrConfigArgs,

    /// Fork a daemon process.
    #[arg(short, long)]
    pub daemon: bool,
}
