use std::fs;
use std::io;
use std::path::Path;
use std::process::exit;

use args::CliArgs;
use args::CliOrConfigArgs;
use args::Command;
use args::WatchCmd;
use clap::Parser;
use daemonize::Daemonize;
use env_logger::Env;

use photosort::replicator::{Replicator, ReplicatorKind};
use photosort::sort;
use photosort::sort::SortError;
use photosort::sort::Sorter;
use photosort::template::Template;

mod args;
mod config;
mod value_parser;
mod watch;

use args::Cli;
use value_parser::TemplateParser;
use watch::EventHandlerError;
use watch::EventHandlerResult;
use watch::EventWatcher;
use watch::FilterReason;

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
            let result = sorter.sort_file(&src_path);
            if result.is_err() {
                exit_code += 1;
            }
            log_sort_result(&result, &src_path);
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
                    exit_code += sort_file(sorter, &path);
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

fn sort_file(sorter: &Sorter, src_path: &Path) -> ExitCode {
    let abs_path = match fs::canonicalize(src_path) {
        Ok(path) => path,
        Err(err) => {
            log::error!("failed to canonicalize source path {:?}: {}", src_path, err);
            return 1;
        }
    };

    let result = sorter.sort_file(&abs_path);
    log_sort_result(&result, &abs_path);
    if result.is_err() {
        1
    } else {
        0
    }
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
    let cfg = match watch_args.common {
        CliOrConfigArgs::Cli(args) => {
            log::debug!("setting up config...");
            let cfg = config::Watch::from(args);
            log::debug!("config successfully setted up");

            cfg
        }
        CliOrConfigArgs::Config(args) => {
            log::debug!("reading config file...");
            let cfg_str = match fs::read_to_string(&args.path) {
                Ok(cfg_str) => cfg_str,
                Err(err) => {
                    log::error!("failed to read config file {:?}: {}", args.path, err);
                    return 1;
                }
            };
            log::debug!("config file successfully read");
            log::debug!("deserializing config file...");
            let cfg = match toml::from_str(&cfg_str) {
                Ok(cfg) => cfg,
                Err(err) => {
                    log::error!("failed to deserialize config file: {}", err);
                    return 1;
                }
            };
            log::debug!("config file successfully deserialized");

            cfg
        }
    };

    let result = EventWatcher::start(cfg, log_result);

    match result {
        Ok(_) => {}
        Err(err) => {
            log::error!("failed to start event watcher: {}", err);
            return 1;
        }
    }

    0
}

fn log_result(result: Result<EventHandlerResult, EventHandlerError>) {
    match result {
        Ok(res) => match res {
            EventHandlerResult::Filtered(reason) => log_filtered(reason),
            EventHandlerResult::Sort(src_path, result) => log_sort_result(&result, &src_path),
            EventHandlerResult::Ignored(event) => log::debug!("ignored event: {:?}", event),
        },
        Err(err) => match err {
            EventHandlerError::RetrieveEvent(err) => {
                log::error!("failed to retrieve fs event: {}", err)
            }
        },
    }
}

fn log_filtered(reason: FilterReason) {
    match reason {
        FilterReason::MissingEventPath(event) => {
            log::error!("missing file path in event: {:?}", event)
        }
        FilterReason::MatchIgnoreRegex(path) => log::info!("{:?} matched ignore regex", path),
    }
}

fn log_sort_result(result: &sort::Result, src_path: &Path) {
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
        }
    }
}
