use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::channel;
use std::{fs, io};

use notify::event::RemoveKind;
use notify::{event::AccessKind, event::AccessMode, Event, EventKind};
use notify::{RecursiveMode, Watcher as NotifyWatcher};
use thiserror::Error;

use crate::replicator::Replicator;
use crate::template::{Context, Template, TemplateValue};

pub struct Watcher {
    sources: Vec<PathBuf>,
    template: Template,
    replicator: Box<dyn Replicator>,
    overwrite: bool,
}

impl Watcher {
    pub fn new(
        sources: Vec<PathBuf>,
        template: Template,
        replicator: Box<dyn Replicator>,
        overwrite: bool,
    ) -> Self {
        Watcher {
            sources,
            template,
            replicator,
            overwrite,
        }
    }

    pub fn start(self) -> Result<(), WatcherError> {
        let (tx, rx) = channel();
        let mut watcher = notify::recommended_watcher(tx).unwrap();

        log::info!("start watching events");
        for src in &self.sources {
            log::debug!("watching source directory {:?}", src);
            watcher
                .watch(src, RecursiveMode::Recursive)
                .map_err(|err| WatcherError::WatchError(src.to_path_buf(), err))?
        }

        for res in rx {
            match res {
                Ok(event) => self.handle_event(event),
                Err(err) => eprintln!("new watch error: {}", err),
            }
        }

        Ok(())
    }

    fn handle_event(&self, event: Event) {
        match event.kind {
            EventKind::Access(AccessKind::Close(AccessMode::Write)) => {
                log::debug!("handling event: {:?}", event);
                self.handle_file_change(&event)
            }
            EventKind::Remove(RemoveKind::File) => self.handle_file_remove(&event),
            _ => {
                log::debug!("ignoring event {:?}", event);
                return;
            }
        }

        log::debug!("event handled: {:?}", event);
    }

    fn handle_file_change(&self, event: &Event) {
        if event.paths.is_empty() {
            panic!("event paths is empty: ${:?}", event);
        }

        let src_path = &event.paths[0];

        sort_file(
            src_path,
            &self.template,
            self.replicator.as_ref(),
            self.overwrite,
        )
    }

    fn handle_file_remove(&self, _event: &Event) {}
}
#[derive(Error)]
pub enum WatcherError {
    #[error("failed to watch source directory {0:?}")]
    WatchError(PathBuf, #[source] notify::Error),
}

impl std::fmt::Debug for WatcherError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

fn error_chain_fmt(
    e: &impl std::error::Error,
    f: &mut std::fmt::Formatter<'_>,
) -> std::fmt::Result {
    writeln!(f, "{}\n", e)?;
    let mut current = e.source();
    while let Some(cause) = current {
        writeln!(f, "Caused by:\n\t{}", cause)?;
        current = cause.source();
    }
    Ok(())
}
pub fn sort_file(
    src_path: &PathBuf,
    template: &Template,
    replicator: &dyn Replicator,
    overwrite: bool,
) {
    // prepare template rendering context
    let mut ctx: HashMap<String, Box<dyn TemplateValue>> = HashMap::default();
    prepare_template_ctx(&mut ctx, src_path);

    // render destination path template
    let replicate_path = match template.render(&ctx) {
        Ok(p) => p,
        Err(err) => {
            log::error!("failed to render template: {:?}", err);
            return;
        }
    };

    match replicate_file(replicator, src_path, &replicate_path, overwrite) {
        Ok(_) => {}
        Err(err) => log::error!(
            "an error occurred while replicating file {:?} to {:?}: {:?}",
            src_path,
            replicate_path,
            err
        ),
    }
}

fn replicate_file(
    replicator: &dyn Replicator,
    src_path: &PathBuf,
    replicate_path: &PathBuf,
    overwrite: bool,
) -> io::Result<()> {
    if replicate_path.exists() {
        if overwrite {
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

    log::debug!(
        "replicating ({:?}) {:?} to {:?}",
        replicator,
        src_path,
        replicate_path
    );

    // Ensure parent directory exist
    if let Some(parent) = replicate_path.parent() {
        fs::create_dir_all(parent)?;
    }

    replicator.replicate(src_path, replicate_path)?;
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

    match  path.file_stem() {
        Some(fstem) => ctx.insert("file.stem".to_owned(), Box::new(fstem.to_owned())),
        None => {}
    }

    // file extension
    match path.extension() {
        Some(fext) => ctx.insert("file.extension".to_owned(), Box::new(fext.to_owned())),
        None => {}
    }
}
