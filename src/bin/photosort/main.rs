use args::Command;
use args::CommonArgs;
use args::SortCmd;
use args::WatchCmd;
use clap::Parser;
use env_logger::Env;
use photosort::replicator::{Replicator, ReplicatorKind};
use photosort::sort::Config;
use photosort::sort::Sorter;
use photosort::template::Template;
use photosort::watcher::Watcher;

mod args;
mod value_parser;

use args::Cli;
use value_parser::TemplateParser;

pub fn main() {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let cli = Cli::parse();

    match cli.command {
        Command::Sort(args) => sort_cmd(args),
        Command::Watch(args) => watch_cmd(args),
    }
}

fn sort_cmd(args: SortCmd) {
    let args = match args.common {
        CommonArgs::Cli(args) => args,
        CommonArgs::Config(_args) => unimplemented!("config file is not supported for the moment"),
    };

    let replicator = Box::<dyn Replicator>::from_iter(args.replicators);
    let sorter = Sorter::new(Config::new(args.template, replicator, args.overwrite));

    for src in args.sources {
        if src.is_dir() {
            sorter.sort_dir(&src)
        } else {
            sorter.sort_file(&src);
        }
    }
}

fn watch_cmd(watch_args: WatchCmd) {
    let args = match watch_args.common {
        CommonArgs::Cli(args) => args,
        CommonArgs::Config(_args) => unimplemented!("config file is not supported for the moment"),
    };

    if watch_args.daemon {
        unimplemented!("daemon mode is not supported for the moment")
    }

    let replicator = Box::<dyn Replicator>::from_iter(args.replicators);
    let config = Config::new(args.template, replicator, args.overwrite);

    match Watcher::new(args.sources, Sorter::new(config)).start() {
        Ok(_) => {}
        Err(err) => log::error!("an error occurred while running in daemon mode: {}", err),
    }
}
