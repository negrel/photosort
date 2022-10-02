use std::path::PathBuf;
use std::sync::mpsc::channel;

use notify::event::RemoveKind;
use notify::{event::AccessKind, event::AccessMode, Event, EventKind};
use notify::{RecursiveMode, Watcher as NotifyWatcher};
use thiserror::Error;

use crate::sort::Sorter;

pub struct Watcher {
    sources: Vec<PathBuf>,
    sorter: Sorter,
}

impl Watcher {
    pub fn new(sources: Vec<PathBuf>, sorter: Sorter) -> Self {
        Watcher { sources, sorter }
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

        self.sorter.sort_file(src_path);
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
