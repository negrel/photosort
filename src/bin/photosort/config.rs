use std::path::PathBuf;

use serde::Deserialize;

use photosort::sort;

use crate::args::CliArgs;

#[derive(Debug, Deserialize)]
pub struct Watch {
    pub sources: Vec<PathBuf>,

    #[serde(flatten)]
    pub sorter: sort::Config,
}

impl From<CliArgs> for Watch {
    fn from(args: CliArgs) -> Self {
        let sorter = sort::Config::new(
            args.template,
            Box::from_iter(args.replicators),
            args.overwrite,
        );

        Self {
            sources: args.sources,
            sorter,
        }
    }
}
