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
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: Command,
}

#[derive(Subcommand, Debug)]
#[command(author = None, version, about)]
pub(crate) enum Command {
    /// Sort all files once.
    Sort(SortCmd),

    /// Watch & sort files as their added/removed.
    Watch(WatchCmd),
}

#[derive(Debug)]
pub(crate) enum CommonArgs {
    Config(ConfigArgs),
    Cli(CliArgs),
}

impl FromArgMatches for CommonArgs {
    fn update_from_arg_matches(&mut self, matches: &clap::ArgMatches) -> Result<(), clap::Error> {
        if matches.get_one::<PathBuf>("config").is_some() {
            ConfigArgs::from_arg_matches(matches).map(|_| ())
        } else {
            CliArgs::from_arg_matches(matches).map(|_| ())
        }
    }

    fn from_arg_matches(matches: &clap::ArgMatches) -> Result<Self, clap::Error> {
        if matches.get_one::<PathBuf>("config").is_some() {
            ConfigArgs::from_arg_matches(matches).map(|args| CommonArgs::Config(args))
        } else {
            CliArgs::from_arg_matches(matches).map(|args| CommonArgs::Cli(args))
        }
    }
}

impl CommonArgs {
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

impl Args for CommonArgs {
    fn augment_args(cmd: clap::Command) -> clap::Command {
        Self::define_args(cmd)
    }

    fn augment_args_for_update(cmd: clap::Command) -> clap::Command {
        Self::define_args(cmd)
    }
}

#[derive(Parser, Debug)]
#[command()]
pub(crate) struct ConfigArgs {
    #[arg(short, long)]
    config: PathBuf,
}

#[derive(Parser, Debug)]
#[command()]
pub(crate) struct CliArgs {
    #[arg(short, long, default_value = "false")]
    pub overwrite: bool,

    #[arg(short, long, default_values = ["hardlink", "softlink", "copy"])]
    pub replicators: Vec<ReplicatorKind>,

    #[arg(short, long, value_parser = TemplateParser::default())]
    pub template: Template,

    #[arg()]
    pub sources: Vec<PathBuf>
}

#[derive(Parser, Debug)]
#[command(author, version, about)]
pub(crate) struct SortCmd {
    #[command(flatten)]
    pub common: CommonArgs,
}

#[derive(Parser, Debug)]
#[command(author, version, about)]
pub(crate) struct WatchCmd {
    #[command(flatten)]
    pub common: CommonArgs,

    /// Fork a daemon process.
    #[arg(short, long)]
    pub daemon: bool,
}
