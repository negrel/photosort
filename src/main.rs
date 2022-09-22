use std::{env, path::PathBuf, process::exit};

use replicator::HardLinkReplicator;
use template::Template;
use watcher::Watcher;

mod replicator;
mod template;
mod watcher;

pub fn main() {
    env_logger::init();

    let args: Vec<String> = env::args().collect();
    match args.len() {
        1 => panic!("missing template argument"),
        2 => panic!("missing source directory argument"),
        _ => {}
    }

    let tpl = Template::parse_str(&args[1]).expect("an error occurred while parsing template");

    let (_, src) = args.split_at(2);
    let src = src.iter().map(|str| PathBuf::from(&str)).collect();

    let replicator = HardLinkReplicator::default();
    let watcher = Watcher::new(src, tpl, &replicator, true);

    match watcher.start() {
        Ok(_) => {}
        Err(err) => {
            log::error!("{:?}", err);
            exit(1);
        }
    }
}
