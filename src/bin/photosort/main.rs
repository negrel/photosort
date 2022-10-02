use std::fs;
use std::io;
use std::path::Path;

use args::Command;
use args::CommonArgs;
use args::SortCmd;
use args::WatchCmd;
use clap::Parser;
use env_logger::Env;

use photosort::replicator::{Replicator, ReplicatorKind};
use photosort::sort;
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
    let sorter = Sorter::new(sort::Config::new(args.template, replicator, args.overwrite));

    for src_path in args.sources {
        if src_path.is_dir() {
            sort_dir(&sorter, &src_path)
        } else {
            log_result(sorter.sort_file(&src_path), &src_path);
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
    let config = sort::Config::new(args.template, replicator, args.overwrite);

    match Watcher::new(args.sources, Sorter::new(config)).start() {
        Ok(_) => {}
        Err(err) => log::error!("an error occurred while running in daemon mode: {}", err),
    }
}

fn sort_dir(sorter: &Sorter, src_path: &Path) {
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
                    sort_dir(sorter, &path);
                } else {
                    log_result(sorter.sort_file(&path), &path);
                }
            }
            Err(err) => log::error!("failed to walk directory {:?}: {}", src_path, err),
        }
    }
}

fn log_result(result: sort::Result, src_path: &Path) {
    log::debug!("{:?}: {:?}", src_path, result);

    match result {
        Ok(sort_result) => match sort_result {
            sort::SortResult::Skipped {
                replicate_path,
                reason,
            } => {
                let level = match reason {
                    sort::SkippedReason::Overwrite => log::Level::Warn,
                    sort::SkippedReason::SameFile => log::Level::Info,
                };
                log::log!(
                    level,
                    "{:?} -> {:?}, skipped because: {}",
                    src_path,
                    replicate_path,
                    reason
                )
            }
            sort::SortResult::Replicated {
                replicate_path,
                overwrite,
            } => {
                log::info!(
                    "file sorted: {:?} -> {:?} (overwrite: {:?})",
                    src_path,
                    replicate_path,
                    overwrite
                )
            }
        },
        Err(err) => {
            log::error!(
                "an error occurred while sorting file {:?}: {}",
                src_path,
                err
            );
        }
    }
}
