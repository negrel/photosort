use std::{path::PathBuf, thread, time::Duration};

use notify::{
    event::{AccessKind, AccessMode, CreateKind},
    Event, EventKind, RecursiveMode, Watcher,
};
use photosort::sort::{SortError, SortResult, Sorter};
use regex::Regex;
use thiserror::Error;

use crate::config;

#[derive(Error, Debug)]
pub enum WatcherError {
    #[error("failed to create filesystem watcher: {0}")]
    CreatingWatcher(#[source] notify::Error),

    #[error("failed to add source {0:?} to watch list: {1}")]
    Watch(PathBuf, #[source] notify::Error),
}

pub trait SortResultHandler {
    fn handle_result(result: SortResult);
}

pub struct EventWatcher {}

impl EventWatcher {
    pub fn start<F>(cfg: config::Watch, result_handler: F) -> Result<(), WatcherError>
    where
        F: Fn(Result<EventHandlerResult, EventHandlerError>) + Send + 'static,
    {
        let filter = EventFilter::new(cfg.ignore_regex);
        let sorter = Sorter::new(cfg.sorter);
        let handler = EventHandler::new(filter, sorter);

        log::debug!("creating watcher suitable for this platform");
        let mut watcher = notify::recommended_watcher(move |event| {
            let result = handler.handle_event(event);
            result_handler(result);
        })
        .map_err(WatcherError::CreatingWatcher)?;
        log::debug!("watcher successfully created");

        log::debug!("adding sources to watcher watch list");
        for src in cfg.sources {
            log::debug!("adding {:?} to watch list", src);
            watcher
                .watch(&src, RecursiveMode::Recursive)
                .map_err(|err| WatcherError::Watch(src.to_owned(), err))?;
        }
        log::debug!("sources successfully added to watcher watch list");

        loop {
            thread::sleep(Duration::from_secs(60));
        }
    }
}

pub struct EventHandler {
    event_filter: EventFilter,
    sorter: Sorter,
}

pub enum EventHandlerResult {
    Ignored(Event),
    Sort(PathBuf, Result<SortResult, SortError>),
    Filtered(FilterReason),
}

#[derive(Debug, Error)]
pub enum EventHandlerError {
    #[error("failed to retrieve event: {0}")]
    RetrieveEvent(notify::Error),
}

impl EventHandler {
    pub fn new(event_filter: EventFilter, sorter: Sorter) -> Self {
        Self {
            event_filter,
            sorter,
        }
    }

    fn handle_event(
        &self,
        event: notify::Result<Event>,
    ) -> Result<EventHandlerResult, EventHandlerError> {
        let event = match event {
            Ok(e) => e,
            Err(err) => return Err(EventHandlerError::RetrieveEvent(err)),
        };

        match event.kind {
            EventKind::Access(AccessKind::Close(AccessMode::Write))
            | EventKind::Create(CreateKind::File) => {
                log::debug!("handling event: {:?}", event);
                if let Err(filter_reason) = self.event_filter.filter(&event) {
                    return Ok(EventHandlerResult::Filtered(filter_reason));
                }

                let src_path = &event.paths[0];
                let sort_result = self.sorter.sort_file(src_path);
                log::debug!("event handled: {:?}", event);
                Ok(EventHandlerResult::Sort(src_path.to_owned(), sort_result))
            }
            _ => {
                Ok(EventHandlerResult::Ignored(event))
            }
        }
    }
}

#[derive(Error, Debug)]
pub enum FilterReason {
    #[error("missing file path in event: {0:?}")]
    MissingEventPath(Event),
    #[error("{0:?} matched ignore regex")]
    MatchIgnoreRegex(PathBuf),
}

pub struct EventFilter {
    ignore_regex: Option<Regex>,
}

impl EventFilter {
    pub fn new(ignore_regex: Option<Regex>) -> Self {
        Self { ignore_regex }
    }

    pub fn filter(&self, event: &Event) -> Result<(), FilterReason> {
        let path = match event.paths.first() {
            Some(p) => p,
            None => return Err(FilterReason::MissingEventPath(event.clone())),
        };

        let path = match path.to_str() {
            Some(p) => p,
            None => return Ok(()),
        };

        if let Some(ignore_regex) = &self.ignore_regex {
            if ignore_regex.is_match(path) {
                return Err(FilterReason::MatchIgnoreRegex(event.paths[0].to_owned()));
            }
        }

        Ok(())
    }
}
