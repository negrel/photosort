use std::fs;
use std::io;
use std::path::PathBuf;

use clap::builder::{BoolValueParser, EnumValueParser, PathBufValueParser};
use clap::{crate_name, App, Arg, ArgMatches, Command};
use env_logger::Env;
use replicator::Replicator;
use template::Template;
use watcher::{sort_file, Watcher};

use crate::replicator::ReplicatorKind;
use crate::value_parser::TemplateParser;

mod replicator;
mod template;
mod value_parser;
mod watcher;

pub fn main() {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let template_arg = Arg::new("template")
        .help("template string used to sort file")
        .required(true)
        .number_of_values(1)
        .multiple_values(false)
        .value_parser(TemplateParser::new());

    let source_args = Arg::new("source")
        .help("source(s) directory/file to sort")
        .required(true)
        .multiple_values(true)
        .value_parser(PathBufValueParser::new());

    let replicator_args = Arg::new("replicator")
        .help("the kind of replicator to use (copy, hardlink, softlink)")
        .long_help("the kind of replicator to use, if more than one replicator is specified others will be used as fallback")
        .short('r')
        .value_parser(EnumValueParser::<ReplicatorKind>::new())
        .default_values(&["hardlink", "softlink", "copy"])
        .multiple_occurrences(true);

    let overwrite_arg = Arg::new("overwrite")
        .help("overwrite replicated file if it already exist")
        .short('o')
        .action(clap::ArgAction::SetTrue)
        .value_parser(BoolValueParser::new());

    let app = App::new(crate_name!())
        .about("A pictures/file organizer")
        .version("0.1.0")
        .author("Alexandre Negrel <negrel.dev@protonmail.com>")
        .subcommand_required(true)
        .subcommand(
            Command::new("sort")
                .about("Sort all files once")
                .arg(overwrite_arg.clone())
                .arg(template_arg.clone())
                .arg(source_args.clone())
                .arg(replicator_args.clone()),
        )
        .subcommand(
            Command::new("daemon")
                .about("Daemon that watch & sort files as their added/removed")
                .arg(
                    Arg::new("config")
                        .help("path to daemon configuration file [UNIMPLEMENTED]")
                        .short('c')
                        .exclusive(true)
                        .number_of_values(1)
                        .value_parser(PathBufValueParser::new()),
                )
                .arg(overwrite_arg.clone())
                .arg(template_arg.clone())
                .arg(source_args.clone())
                .arg(replicator_args.clone()),
        );

    let matches = app.get_matches();

    match matches.subcommand() {
        Some(("daemon", args)) => daemon_cmd(args),
        Some(("sort", args)) => sort_cmd(args),
        None => unreachable!(),
        _ => panic!("unexpected input, please report a bug"),
    }
}

fn sort_cmd(args: &ArgMatches) {
    let sources: Vec<PathBuf> = args
        .get_many::<PathBuf>("source")
        .unwrap()
        .into_iter()
        .map(|pbuf| pbuf.to_owned())
        .collect();

    let replicator = replicator_from_args(args);
    let template = args.get_one::<Template>("template").unwrap();
    let overwrite = args.get_one::<bool>("overwrite").unwrap();

    for src in sources {
        if src.is_dir() {
            sort_dir(&src, template, replicator.as_ref(), *overwrite)
        } else {
            sort_file(&src, template, replicator.as_ref(), *overwrite);
        }
    }
}

fn daemon_cmd(args: &ArgMatches) {
    if let Some(_config_file) = args.get_one::<PathBuf>("config") {
        unimplemented!("config file")
    }

    let sources: Vec<PathBuf> = args
        .get_many::<PathBuf>("source")
        .unwrap()
        .into_iter()
        .map(|pbuf| pbuf.to_owned())
        .collect();

    let replicator = replicator_from_args(args);
    let template = args.get_one::<Template>("template").unwrap();
    let overwrite = args.get_one::<bool>("overwrite").unwrap();

    match Watcher::new(sources, template.to_owned(), replicator, *overwrite).start() {
        Ok(_) => {}
        Err(err) => log::error!("an error occurred while running in daemon mode: {}", err),
    }
}

fn replicator_from_args(args: &ArgMatches) -> Box<dyn Replicator> {
    Box::from_iter(
        args.get_many::<ReplicatorKind>("replicator")
            .unwrap()
            .into_iter()
            .map(|kind| kind.to_owned()),
    )
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
