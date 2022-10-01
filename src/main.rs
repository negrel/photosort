use std::fs;
use std::io;
use std::path::PathBuf;

use args::Command;
use args::CommonArgs;
use args::SortCmd;
use args::WatchCmd;
use clap::Parser;
use env_logger::Env;
use replicator::Replicator;
use template::Template;
use watcher::{sort_file, Watcher};

use crate::args::Cli;
use crate::replicator::ReplicatorKind;
use crate::value_parser::TemplateParser;

mod args;
mod replicator;
mod template;
mod value_parser;
mod watcher;

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

    for src in args.sources {
        if src.is_dir() {
            sort_dir(&src, &args.template, replicator.as_ref(), args.overwrite)
        } else {
            sort_file(&src, &args.template, replicator.as_ref(), args.overwrite);
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

    match Watcher::new(args.sources, args.template, replicator, args.overwrite).start() {
        Ok(_) => {}
        Err(err) => log::error!("an error occurred while running in daemon mode: {}", err),
    }
}

fn sort_dir(src_path: &PathBuf, template: &Template, replicator: &dyn Replicator, overwrite: bool) {
    log::debug!("sorting directory {:?}", src_path);

    // create iterator
    let dir_iter: Vec<io::Result<fs::DirEntry>> = match fs::read_dir(src_path) {
        Ok(read_dir) => read_dir.collect(),
        Err(err) => {
            log::error!("failed to walk directory {:?}: {}", src_path, err);
            return;
        }
    };

    // iterate over files in src_path
    for dir_entry in dir_iter.into_iter().rev() {
        match dir_entry {
            Ok(entry) => {
                let path = entry.path();

                if path.is_dir() {
                    sort_dir(&path, template, replicator, overwrite);
                } else {
                    sort_file(&path, template, replicator, overwrite);
                }
            }
            Err(err) => log::error!("failed to walk directory {:?}: {}", src_path, err),
        }
    }

    log::debug!("directory {:?} sorted", src_path);
}
