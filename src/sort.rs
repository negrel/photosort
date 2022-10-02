use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::replicator::Replicator;
use crate::template::{Context, Template, TemplateValue};

#[derive(Debug)]
pub struct Config {
    template: Template,
    replicator: Box<dyn Replicator>,
    overwrite: bool,
}

impl Config {
    pub fn new(template: Template, replicator: Box<dyn Replicator>, overwrite: bool) -> Self {
        Self {
            template,
            replicator,
            overwrite,
        }
    }
}

pub struct Sorter {
    cfg: Config,
}

impl Sorter {
    pub fn new(cfg: Config) -> Self {
        Self { cfg }
    }

    pub fn sort_dir(&self, src_path: &PathBuf) {
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
                        self.sort_dir(&path);
                    } else {
                        self.sort_file(&path);
                    }
                }
                Err(err) => log::error!("failed to walk directory {:?}: {}", src_path, err),
            }
        }

        log::debug!("directory {:?} sorted", src_path);
    }

    pub fn sort_file(&self, src_path: &Path) {
        // prepare template rendering context
        let mut ctx: HashMap<String, Box<dyn TemplateValue>> = HashMap::default();
        Self::prepare_template_ctx(&mut ctx, &src_path);

        // render destination path template
        let replicate_path = match self.cfg.template.render(&ctx) {
            Ok(p) => p,
            Err(err) => {
                log::error!("failed to render template: {:?}", err);
                return;
            }
        };

        match self.replicate_file(src_path, &replicate_path) {
            Ok(_) => {}
            Err(err) => log::error!(
                "an error occurred while replicating file {:?} to {:?}: {:?}",
                src_path,
                replicate_path,
                err
            ),
        }
    }

    fn replicate_file(&self, src_path: &Path, replicate_path: &PathBuf) -> io::Result<()> {
        if replicate_path.exists() {
            if self.cfg.overwrite {
                log::info!(
                    "removing {:?} file/directory to replicate {:?}",
                    replicate_path,
                    src_path
                );
                if replicate_path.is_dir() {
                    fs::remove_dir_all(replicate_path)?
                } else {
                    fs::remove_file(replicate_path)?
                }
            } else {
                log::warn!(
                    "replicating file {:?} to {:?} will overwrite the latter, skipping it",
                    src_path,
                    replicate_path
                );
                return Ok(());
            }
        }

        // Ensure parent directory exist
        if let Some(parent) = replicate_path.parent() {
            fs::create_dir_all(parent)?;
        }

        self.cfg.replicator.replicate(src_path, replicate_path)?;
        log::info!("file {:?} replicated to {:?}", src_path, replicate_path);
        Ok(())
    }

    fn prepare_template_ctx(ctx: &mut dyn Context, path: &Path) {
        // filepath
        ctx.insert("file.path".to_owned(), Box::new(path.to_owned()));

        // filename
        match path.file_name() {
            Some(fname) => ctx.insert("file.name".to_owned(), Box::new(fname.to_owned())),
            None => {}
        };

        match path.file_stem() {
            Some(fstem) => ctx.insert("file.stem".to_owned(), Box::new(fstem.to_owned())),
            None => {}
        }

        // file extension
        match path.extension() {
            Some(fext) => ctx.insert("file.extension".to_owned(), Box::new(fext.to_owned())),
            None => {}
        }
    }
}
