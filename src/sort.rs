use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::result;

use thiserror::Error;

use crate::replicator::Replicator;
use crate::template;
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

#[derive(Debug)]
pub struct Sorter {
    cfg: Config,
}

impl Sorter {
    pub fn new(cfg: Config) -> Self {
        Self { cfg }
    }

    pub fn sort_file(&self, src_path: &Path) -> Result {
        // prepare template rendering context
        let mut ctx: HashMap<String, Box<dyn TemplateValue>> = HashMap::default();
        Self::prepare_template_ctx(&mut ctx, src_path);

        // render destination path template
        let replicate_path = match self.cfg.template.render(&ctx) {
            Ok(path) => path,
            Err(err) => return Err(SortError::TemplateError(err)),
        };

        self.replicate_file(src_path, replicate_path)
    }

    fn replicate_file(&self, src_path: &Path, replicate_path: PathBuf) -> Result {
        // TODO canonicalize src and replicate path
        if replicate_path == src_path {
            return Ok(SortResult::Skipped {
                replicate_path,
                reason: SkippedReason::SameFile,
            });
        }

        let mut overwrite = false;
        if replicate_path.exists() {
            if self.cfg.overwrite {
                overwrite = true;
                if replicate_path.is_dir() {
                    if let Err(err) = fs::remove_dir_all(&replicate_path) {
                        return Err(SortError::OverwriteError(err, replicate_path));
                    };
                } else if let Err(err) = fs::remove_file(&replicate_path) {
                    return Err(SortError::OverwriteError(err, replicate_path));
                }
            } else {
                return Ok(SortResult::Skipped {
                    replicate_path,
                    reason: SkippedReason::Overwrite,
                });
            }
        }

        // Ensure parent directory exist
        if let Some(parent) = replicate_path.parent() {
            if let Err(err) = fs::create_dir_all(parent) {
                return Err(SortError::ReplicateError(err, replicate_path));
            };
        }

        if let Err(err) = self.cfg.replicator.replicate(src_path, &replicate_path) {
            return Err(SortError::ReplicateError(err, replicate_path));
        }

        Ok(SortResult::Replicated {
            replicate_path,
            overwrite,
        })
    }

    fn prepare_template_ctx(ctx: &mut dyn Context, path: &Path) {
        // filepath
        ctx.insert("file.path".to_owned(), Box::new(path.to_owned()));

        // filename
        if let Some(fname) = path.file_name() {
            ctx.insert("file.name".to_owned(), Box::new(fname.to_owned()));
        };

        if let Some(fstem) = path.file_stem() {
            ctx.insert("file.stem".to_owned(), Box::new(fstem.to_owned()));
        }

        // file extension
        if let Some(fext) = path.extension() {
            ctx.insert("file.extension".to_owned(), Box::new(fext.to_owned()));
        }
    }
}

pub type Result = result::Result<SortResult, SortError>;

#[derive(Debug)]
pub enum SortResult {
    /// File wasn't replicated because overwrite is disabled or source path
    /// is same as replicate path.
    Skipped {
        replicate_path: PathBuf,
        reason: SkippedReason,
    },

    /// File was replicated.
    Replicated {
        replicate_path: PathBuf,
        /// A file was overwritten to replicate this file
        overwrite: bool,
    },
}

#[derive(Error, Debug)]
pub enum SortError {
    #[error("failed to render file path template: {0}")]
    TemplateError(#[source] template::RenderError),

    #[error("failed to replicate file to {1:?}: {0}")]
    ReplicateError(#[source] io::Error, PathBuf),

    #[error("failed to overwrite destination file {1:?}: {0}")]
    OverwriteError(#[source] io::Error, PathBuf),
}

#[derive(Error, Debug, PartialEq, Eq)]
pub enum SkippedReason {
    #[error("can't overwrite replicate file")]
    Overwrite,

    #[error("source and replicate paths are the same")]
    SameFile,
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::path::{Path, PathBuf};
    use std::str::FromStr;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};
    use std::{env, fs, io};

    use uuid::Uuid;

    use crate::replicator::CopyReplicator;
    use crate::sort::{SkippedReason, SortResult};
    use crate::{
        replicator::{NoneReplicator, SoftLinkReplicator},
        template::{self, Template},
    };

    use super::{SortError, Sorter};

    #[test]
    fn template_error() {
        let sorter = Sorter::new(super::Config {
            template: Template::from_str(":inexistent.variable:").unwrap(),
            replicator: Box::new(NoneReplicator::default()),
            overwrite: false,
        });

        let result = sorter.sort_file(&PathBuf::from("/dev/null"));

        assert!(result.is_err());
        let err = result.unwrap_err();
        let err = match err {
            SortError::TemplateError(err) => err,
            _ => panic!("{} is not of type TemplateError", err),
        };

        assert_eq!(
            err,
            template::RenderError::UndefinedVariable("inexistent.variable".to_owned()),
        );
    }

    #[test]
    fn replicate_error() {
        let sorter = Sorter::new(super::Config {
            template: Template::from_str(":file.path:2").unwrap(),
            replicator: Box::new(NoneReplicator::default()),
            overwrite: false,
        });

        let result = sorter.sort_file(&PathBuf::from("/dev/null"));

        assert!(result.is_err());
        let err = result.unwrap_err();
        let (err, dest_path) = match err {
            SortError::ReplicateError(err, dest_path) => (err, dest_path),
            _ => panic!("expected error of type ReplicateError, got \"{}\"", err),
        };

        assert_eq!(dest_path, PathBuf::from("/dev/null2"));
        assert_eq!(err.kind(), NoneReplicator::replicate_error().kind());
    }

    #[cfg(unix)]
    #[test]
    fn overwrite_error() {
        let src_path = PathBuf::from("/proc/self/stat");

        let sorter = Sorter::new(super::Config {
            template: Template::from_str(":file.path:us").unwrap(),
            replicator: Box::new(SoftLinkReplicator::default()),
            overwrite: true,
        });

        let result = sorter.sort_file(&src_path);

        assert!(result.is_err());
        let err = result.unwrap_err();
        let (err, dest_path) = match err {
            SortError::OverwriteError(err, dest_path) => (err, dest_path),
            _ => panic!("expected error of type OverwriteError, got \"{}\"", err),
        };

        assert_eq!(dest_path, PathBuf::from("/proc/self/status"));
        assert_eq!(err.kind(), io::ErrorKind::PermissionDenied);
    }

    #[test]
    fn skipped_source_and_destination_are_same() {
        let src_path = PathBuf::from(env::args().next().unwrap());
        let sorter = Sorter::new(super::Config {
            template: Template::from_str(src_path.to_str().unwrap()).unwrap(),
            replicator: Box::new(SoftLinkReplicator::default()),
            overwrite: true,
        });

        let result = sorter.sort_file(&src_path);

        assert!(result.is_ok());
        let result = result.unwrap();
        let (replicate_path, skip_reason) = match result {
            crate::sort::SortResult::Skipped {
                replicate_path,
                reason,
            } => (replicate_path, reason),
            _ => panic!("expected sort result of type Skipped, got \"{:?}\"", result),
        };

        assert_eq!(replicate_path, src_path);
        assert_eq!(skip_reason, SkippedReason::SameFile);
    }

    #[test]
    fn skipped_overwrite_disabled() {
        let src_path = PathBuf::from(env::args().next().unwrap());
        let sorter = Sorter::new(super::Config {
            template: Template::from_str(src_path.to_str().unwrap()).unwrap(),
            replicator: Box::new(SoftLinkReplicator::default()),
            overwrite: true,
        });

        let result = sorter.sort_file(&src_path);

        assert!(result.is_ok());
        let result = result.unwrap();
        let (replicate_path, skip_reason) = match result {
            crate::sort::SortResult::Skipped {
                replicate_path,
                reason,
            } => (replicate_path, reason),
            _ => panic!("expected sort result of type Skipped, got \"{:?}\"", result),
        };

        assert_eq!(replicate_path, src_path);
        assert_eq!(skip_reason, SkippedReason::SameFile);
    }

    fn setup() -> PathBuf {
        let tmpdir = env::temp_dir();

        let src = tmpdir.join(format!("{}.txt", Uuid::new_v4()));
        let mut src_file = fs::File::create(&src).unwrap();
        writeln!(&mut src_file, "{}", Uuid::new_v4()).unwrap();

        src
    }

    fn teardown(src: &Path, dst: &Path) {
        let _ = fs::remove_file(src);
        let _ = fs::remove_file(dst);
    }

    fn file_content_eq(src: &Path, dst: &Path) -> bool {
        let mut src_file = fs::File::open(src).unwrap();
        let mut src_content = String::new();
        let mut dst_content = String::new();

        src_file.read_to_string(&mut src_content).unwrap();

        match fs::File::open(dst) {
            Ok(mut dst_file) => {
                dst_file.read_to_string(&mut dst_content).unwrap();
            }
            Err(_) => return false,
        }

        src_content == dst_content
    }

    #[test]
    fn replicated() {
        let src = setup();
        let mut expected_dst = src.to_str().unwrap().to_string();
        expected_dst.push_str("-copy");

        let sorter = Sorter::new(super::Config {
            template: Template::from_str(":file.path:-copy").unwrap(),
            replicator: Box::new(CopyReplicator::default()),
            overwrite: false,
        });

        let result = sorter.sort_file(&src);
        assert!(result.is_ok());

        let result = result.unwrap();
        let (replicate_path, overwrite) = match result {
            SortResult::Replicated {
                replicate_path,
                overwrite,
            } => (replicate_path, overwrite),
            _ => panic!(
                "expected sort result of type Replicated, got \"{:?}\"",
                result
            ),
        };

        assert!(!overwrite);
        assert_eq!(replicate_path.to_str().unwrap(), expected_dst);
        assert!(file_content_eq(&src, &replicate_path));

        teardown(&src, &replicate_path);
    }

    #[test]
    fn replicated_with_overwrite() {
        let src = setup();

        let mut expected_dst = src.to_str().unwrap().to_string();
        expected_dst.push_str("-copy");
        let _ = fs::File::create(&expected_dst).unwrap();

        let sorter = Sorter::new(super::Config {
            template: Template::from_str(":file.path:-copy").unwrap(),
            replicator: Box::new(CopyReplicator::default()),
            overwrite: true,
        });

        let result = sorter.sort_file(&src);
        assert!(result.is_ok());

        let result = result.unwrap();
        let (replicate_path, overwrite) = match result {
            SortResult::Replicated {
                replicate_path,
                overwrite,
            } => (replicate_path, overwrite),
            _ => panic!(
                "expected sort result of type Replicated, got \"{:?}\"",
                result
            ),
        };

        assert!(overwrite);
        assert_eq!(replicate_path.to_str().unwrap(), expected_dst);
        assert!(file_content_eq(&src, &replicate_path));

        teardown(&src, &replicate_path);
    }
}
