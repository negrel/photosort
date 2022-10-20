use std::fs;
use std::io;
use std::path::Path;
use std::process::exit;
use std::thread;
use std::time::Duration;

use args::CliArgs;
use args::CliOrConfigArgs;
use args::Command;
use args::WatchCmd;
use clap::Parser;
use daemonize::Daemonize;
use env_logger::Env;
use notify::{
    event::AccessKind, event::AccessMode, event::CreateKind, Event, EventKind, RecursiveMode,
    Watcher,
};

use photosort::replicator::{Replicator, ReplicatorKind};
use photosort::sort;
use photosort::sort::SortError;
use photosort::sort::Sorter;
use photosort::template::Template;

mod args;
mod config;
mod value_parser;

use args::Cli;
use value_parser::TemplateParser;

type ExitCode = i32;

pub fn main() {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let cli = Cli::parse();

    let exit_code = match cli.command {
        Command::Sort(args) => sort_cmd(args),
        Command::Watch(args) => watch_cmd(args),
    };

    exit(exit_code);
}

fn sort_cmd(args: CliArgs) -> ExitCode {
    let replicator = Box::<dyn Replicator>::from_iter(args.replicators);
    let sorter = Sorter::new(sort::Config::new(args.template, replicator, args.overwrite));

    let mut exit_code = 0;

    for src_path in args.sources {
        if src_path.is_dir() {
            exit_code += sort_dir(&sorter, &src_path);
        } else {
            exit_code += log_result(sorter.sort_file(&src_path), &src_path);
        }
    }

    exit_code
}

fn sort_dir(sorter: &Sorter, src_path: &Path) -> ExitCode {
    // create iterator
    let dir_iter: Vec<io::Result<fs::DirEntry>> = match fs::read_dir(src_path) {
        Ok(read_dir) => read_dir.collect(),
        Err(err) => {
            log::error!("failed to walk directory {:?}: {}", src_path, err);
            return 1;
        }
    };

    let mut exit_code = 0;

    // iterate over files in src_path
    for dir_entry in dir_iter.into_iter().rev() {
        match dir_entry {
            Ok(entry) => {
                let path = entry.path();

                if path.is_dir() {
                    exit_code += sort_dir(sorter, &path);
                } else {
                    exit_code += log_result(sorter.sort_file(&path), &path);
                }
            }
            Err(err) => {
                exit_code += 1;
                log::error!("failed to walk directory {:?}: {}", src_path, err);
            }
        }
    }

    exit_code
}

fn watch_cmd(watch_args: WatchCmd) -> ExitCode {
    if watch_args.daemon {
        log::debug!("starting daemon process");
        match Daemonize::new()
            .exit_action(|| log::info!("daemon process successfully started"))
            .start()
        {
            Ok(_) => {}
            Err(err) => {
                log::error!("an error occurred while daemonzing the process: {}", err);
                return 1;
            }
        }
        log::info!("daemon process started");
    }
    match watch_args.common {
        CliOrConfigArgs::Cli(args) => {
            log::debug!("setting up config...");
            let cfg = config::Watch::from(args);
            log::debug!("config successfully setted up");

            watch(cfg);
        }
        CliOrConfigArgs::Config(args) => {
            log::debug!("reading config file...");
            let result = match fs::read_to_string(&args.path) {
                Ok(cfg_str) => toml::from_str(&cfg_str),
                Err(err) => {
                    log::error!("failed to read config file {:?}: {}", args.path, err);
                    return 1;
                }
            };
            log::debug!("config file successfully read");
            log::debug!("deserializing config file...");
            let cfg = match result {
                Ok(cfg) => cfg,
                Err(err) => {
                    log::error!("failed to deserialize config file: {}", err);
                    return 1;
                }
            };
            log::debug!("config file successfully deserialized");

            watch(cfg);
        }
    };

    0
}

pub fn watch(cfg: config::Watch) {
    log::debug!("watching with config: {:?}", cfg);

    let sorter = Sorter::new(cfg.sorter);

    log::debug!("creating watcher suitable for this platform");
    let result = notify::recommended_watcher(move |result| match result {
        Ok(event) => handle_watch_event(event, &sorter),
        Err(err) => log::error!("unexpected watch error occurred: {}", err),
    });
    let mut watcher = match result {
        Ok(w) => w,
        Err(err) => {
            log::error!("failed to create fs watcher: {}", err);
            return;
        }
    };
    log::debug!("watcher successfully created");

    log::debug!("adding sources to watcher watch list");
    for src in cfg.sources {
        log::debug!("adding {:?} to watch list", src);
        match watcher.watch(&src, RecursiveMode::Recursive) {
            Ok(_) => {}
            Err(err) => {
                log::error!("failed to add source {:?} to watch list: {}", src, err);
                return;
            }
        }
    }
    log::debug!("sources successfully added to watcher watch list");

    loop {
        thread::sleep(Duration::from_secs(60));
    }
}

fn handle_watch_event(event: Event, sorter: &Sorter) {
    match event.kind {
        EventKind::Access(AccessKind::Close(AccessMode::Write))
        | EventKind::Create(CreateKind::File) => {
            log::debug!("handling event: {:?}", event);
            if event.paths.is_empty() {
                panic!("event paths is empty: ${:?}", event);
            }

            let src_path = &event.paths[0];
            log_result(sorter.sort_file(src_path), src_path);
        }
        _ => {
            log::debug!("ignoring event {:?}", event);
            return;
        }
    }

    log::debug!("event handled: {:?}", event);
}

fn log_result(result: sort::Result, src_path: &Path) -> ExitCode {
    log::debug!("{:?}: {:?}", src_path, result);

    match result {
        Ok(sort_result) => {
            match sort_result {
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
                        "{:?} -x- {:?}, skipped because: {}",
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
                        "file sorted: {:?} --> {:?} (overwrite: {:?})",
                        src_path,
                        replicate_path,
                        overwrite
                    )
                }
            };
            0
        }
        Err(err) => {
            match err {
                SortError::TemplateError(err) => {
                    log::error!("{:?} -x- ???: {}", src_path, err);
                }
                SortError::TemplateContextError(err) => {
                    log::error!("{:?} -x- ???: {}", src_path, err);
                }
                SortError::ReplicateError(err, replicate_path)
                | SortError::OverwriteError(err, replicate_path) => {
                    log::error!("{:?} -x- {:?}: {}", src_path, replicate_path, err);
                }
            };
            1
        }
    }
}
