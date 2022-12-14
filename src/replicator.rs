use std::fmt;
use std::fmt::Debug;
use std::fmt::Display;
use std::fs;
use std::io;
use std::path::Path;
use std::str::FromStr;

use clap::builder::PossibleValue;
use serde::de::Error;
use serde::de::Visitor;
use serde::{Deserialize, Serialize};
use symlink::symlink_file;
use thiserror::Error;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ReplicatorKind {
    #[serde(skip)]
    None,
    Copy,
    HardLink,
    SoftLink,
}

impl clap::ValueEnum for ReplicatorKind {
    fn value_variants<'a>() -> &'a [Self] {
        &[Self::Copy, Self::HardLink, Self::SoftLink]
    }
    fn to_possible_value<'a>(&self) -> ::std::option::Option<PossibleValue> {
        match self {
            Self::Copy => Some(PossibleValue::new("copy")),
            Self::HardLink => Some(PossibleValue::new("hardlink")),
            Self::SoftLink => Some(PossibleValue::new("softlink")),
            _ => None,
        }
    }
}

impl Display for ReplicatorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let str = match self {
            ReplicatorKind::None => "none",
            ReplicatorKind::Copy => "copy",
            ReplicatorKind::HardLink => "hardlink",
            ReplicatorKind::SoftLink => "softlink",
        };

        f.write_str(str)
    }
}

#[derive(Error, Debug)]
#[error("failed to parse string: {0}")]
pub struct ParseError(String);

impl FromStr for ReplicatorKind {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "copy" => Ok(ReplicatorKind::Copy),
            "hardlink" => Ok(ReplicatorKind::HardLink),
            "softlink" => Ok(ReplicatorKind::SoftLink),
            "none" => Ok(ReplicatorKind::None),
            _ => Err(ParseError(format!("unknown replicator kind: {}", s))),
        }
    }
}

/// Replicator represents a way to replicate a file.
pub trait Replicator: Send + Sync {
    fn replicate(&self, src: &Path, dst: &Path) -> io::Result<()>;
    fn kind(&self) -> ReplicatorKind;
}

impl<'a> Display for dyn Replicator + 'a {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(format!("{}Replicator", &self.kind()).as_str())
    }
}

impl<'a> Debug for dyn Replicator + 'a {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(format!("{}Replicator", &self.kind()).as_str())
    }
}

impl From<ReplicatorKind> for Box<dyn Replicator> {
    fn from(kind: ReplicatorKind) -> Self {
        match kind {
            ReplicatorKind::None => Box::new(NoneReplicator::default()),
            ReplicatorKind::Copy => Box::new(CopyReplicator::default()),
            ReplicatorKind::HardLink => Box::new(HardLinkReplicator::default()),
            ReplicatorKind::SoftLink => Box::new(SoftLinkReplicator::default()),
        }
    }
}

impl FromIterator<ReplicatorKind> for Box<dyn Replicator> {
    fn from_iter<T: IntoIterator<Item = ReplicatorKind>>(iter: T) -> Self {
        Box::<dyn Replicator>::from_iter(iter.into_iter().map(Box::<dyn Replicator>::from))
    }
}

impl FromIterator<Box<dyn Replicator>> for Box<dyn Replicator> {
    fn from_iter<T: IntoIterator<Item = Box<dyn Replicator>>>(iter: T) -> Self {
        let mut iter = iter.into_iter();
        let first = if let Some(next) = iter.next() {
            next
        } else {
            return Box::new(NoneReplicator::default());
        };

        Box::new(ReplicatorWithFallback::new(first, Box::from_iter(iter)))
    }
}

impl FromStr for Box<dyn Replicator> {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let kind = match ReplicatorKind::from_str(s) {
            Ok(kind) => kind,
            Err(err) => return Err(err),
        };

        Ok(Box::from(kind))
    }
}

impl<'de> Deserialize<'de> for Box<dyn Replicator> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct ReplicatorVisitor;
        impl<'de> Visitor<'de> for ReplicatorVisitor {
            type Value = Box<dyn Replicator>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a replicator or an array of replicator")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                match Box::<dyn Replicator>::from_str(v) {
                    Ok(replicator) => Ok(replicator),
                    Err(err) => Err(E::custom(err)),
                }
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let mut next: Box<dyn Replicator> = seq.next_element()?.ok_or_else(|| {
                    A::Error::custom("no values in seq, expecting at least one replicator")
                })?;

                while let Some(repl) = seq.next_element()? {
                    next = Box::new(ReplicatorWithFallback::new(repl, next));
                }

                Ok(next)
            }
        }

        deserializer.deserialize_any(ReplicatorVisitor)
    }
}

#[derive(Debug)]
pub struct ReplicatorWithFallback {
    inner: Box<dyn Replicator>,
    fallback: Box<dyn Replicator>,
}

impl ReplicatorWithFallback {
    pub fn new(inner: Box<dyn Replicator>, fallback: Box<dyn Replicator>) -> Self {
        Self { inner, fallback }
    }
}

impl Replicator for ReplicatorWithFallback {
    fn replicate(&self, src: &Path, dst: &Path) -> io::Result<()> {
        match self.inner.replicate(src, dst) {
            Ok(_) => Ok(()),
            Err(err) => {
                if let Err(fallback_err) = self.fallback.replicate(src, dst) {
                    Err(io::Error::new(
                        io::ErrorKind::Other,
                        ReplicatorFallbackError(self.kind().to_string(), err, fallback_err),
                    ))
                } else {
                    Ok(())
                }
            }
        }
    }

    fn kind(&self) -> ReplicatorKind {
        self.inner.kind()
    }
}

impl Display for ReplicatorWithFallback {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.inner, f)?;
        f.write_str(" -> ")?;
        Display::fmt(&self.fallback, f)?;
        Ok(())
    }
}

#[derive(Debug, Error)]
#[error("{0} replicator: {1}, {2}")]
struct ReplicatorFallbackError(String, io::Error, io::Error);

#[derive(Debug, Default)]
pub struct NoneReplicator {}

const NONE_REPLICATE_ERR_MSG: &str = "none replicator reached: replicate failed";

impl NoneReplicator {
    pub fn replicate_error() -> io::Error {
        io::Error::new::<&str>(io::ErrorKind::Unsupported, NONE_REPLICATE_ERR_MSG)
    }
}

impl Replicator for NoneReplicator {
    fn replicate(&self, _src: &Path, _dst: &Path) -> io::Result<()> {
        Err(Self::replicate_error())
    }

    fn kind(&self) -> ReplicatorKind {
        ReplicatorKind::None
    }
}

#[derive(Debug, Default)]
pub struct SoftLinkReplicator {}

impl Replicator for SoftLinkReplicator {
    fn replicate(&self, src: &Path, dst: &Path) -> io::Result<()> {
        symlink_file(&src, &dst)
    }

    fn kind(&self) -> ReplicatorKind {
        ReplicatorKind::SoftLink
    }
}

#[derive(Debug, Default)]
pub struct HardLinkReplicator {}

impl Replicator for HardLinkReplicator {
    fn replicate(&self, src: &Path, dst: &Path) -> io::Result<()> {
        fs::hard_link(src, dst)
    }

    fn kind(&self) -> ReplicatorKind {
        ReplicatorKind::HardLink
    }
}

#[derive(Debug, Default)]
pub struct CopyReplicator {}

impl Replicator for CopyReplicator {
    fn replicate(&self, src: &Path, dst: &Path) -> io::Result<()> {
        match fs::copy(src, dst) {
            Ok(_) => Ok(()),
            Err(err) => Err(err),
        }
    }

    fn kind(&self) -> ReplicatorKind {
        ReplicatorKind::Copy
    }
}

#[derive(Default)]
struct MockReplicator<F>
where
    F: Fn(&Path, &Path) -> io::Result<()>,
{
    pub replicate_fn: F,
}

impl<F: Fn(&Path, &Path) -> io::Result<()> + Send + Sync> Replicator for MockReplicator<F> {
    fn replicate(&self, src: &Path, dst: &Path) -> io::Result<()> {
        (self.replicate_fn)(src, dst)
    }

    fn kind(&self) -> ReplicatorKind {
        ReplicatorKind::None
    }
}

impl<F: Fn(&Path, &Path) -> io::Result<()>> Display for MockReplicator<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("mock")
    }
}

#[cfg(test)]
mod tests {
    use std::env::temp_dir;
    use std::io::{Read, Write};
    use std::path::{Path, PathBuf};
    use std::{fs, io};

    #[cfg(unix)]
    use std::os::unix::fs::MetadataExt;

    use crate::replicator::NONE_REPLICATE_ERR_MSG;

    use super::{
        CopyReplicator, HardLinkReplicator, MockReplicator, NoneReplicator, Replicator,
        SoftLinkReplicator,
    };
    use uuid::Uuid;

    fn setup() -> (PathBuf, PathBuf) {
        let tmpdir = temp_dir();

        let src = tmpdir.join(format!("{}.txt", Uuid::new_v4()));
        let dst = tmpdir.join(format!("{}.txt", Uuid::new_v4()));

        let mut src_file = fs::File::create(&src).unwrap();
        writeln!(&mut src_file, "{}", Uuid::new_v4()).unwrap();

        (src, dst)
    }

    fn teardown(src: &Path, dst: &Path) {
        fs::remove_file(src).unwrap();
        fs::remove_file(dst).unwrap_or_default();
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

    fn file_content_is(f: &Path, expected_content: &str) -> bool {
        let mut file = fs::File::open(f).unwrap();
        let mut actual_content = String::new();

        file.read_to_string(&mut actual_content).unwrap();

        actual_content == expected_content
    }

    #[test]
    fn none_replicator_error() {
        let (src, dst) = setup();
        let replicator = &NoneReplicator::default();
        let result = replicator.replicate(&src, &dst);

        assert!(src.exists());
        assert!(!dst.exists());

        teardown(&src, &dst);

        assert!(result.is_err());
        assert_eq!(
            result.err().unwrap().kind(),
            NoneReplicator::replicate_error().kind()
        );
    }

    #[test]
    fn copy_replicate() {
        let (src, dst) = setup();
        let replicator = &CopyReplicator::default();
        let result = replicator.replicate(&src, &dst);

        assert!(src.exists());
        assert!(dst.exists());

        let metadata = fs::symlink_metadata(dst.clone()).unwrap();
        let file_type = metadata.file_type();

        assert!(file_type.is_file());
        assert!(file_content_eq(&src, &dst));

        teardown(&src, &dst);

        assert!(result.is_ok());
    }

    #[test]
    fn softlink_replicate() {
        let (src, dst) = setup();
        let replicator = &SoftLinkReplicator::default();
        let result = replicator.replicate(&src, &dst);

        assert!(src.exists());
        assert!(dst.exists());

        let metadata = fs::symlink_metadata(dst.clone()).unwrap();
        let file_type = metadata.file_type();

        assert!(file_type.is_symlink());
        assert!(file_content_eq(&src, &dst));

        teardown(&src, &dst);

        assert!(result.is_ok());
    }

    #[test]
    fn hardlink_replicate() {
        let (src, dst) = setup();
        let replicator = &HardLinkReplicator::default();
        let result = replicator.replicate(&src, &dst);

        assert!(src.exists());
        assert!(dst.exists());

        let dst_metadata = fs::symlink_metadata(dst.clone()).unwrap();
        let src_metadata = fs::symlink_metadata(src.clone()).unwrap();

        assert_eq!(dst_metadata.ino(), src_metadata.ino());
        assert!(file_content_eq(&src, &dst));

        teardown(&src, &dst);

        assert!(result.is_ok());
    }

    #[test]
    fn replicator_with_fallback() {
        let (src, dst) = setup();
        let replicator = Box::<dyn Replicator>::from_iter(vec![
            Box::new(MockReplicator {
                replicate_fn: |_src, dst| {
                    if !dst.exists() {
                        fs::write(dst, "foo")
                    } else {
                        Err(io::Error::new::<&str>(
                            io::ErrorKind::Unsupported,
                            "replictor1 error",
                        ))
                    }
                },
            }) as Box<dyn Replicator>,
            Box::new(MockReplicator {
                replicate_fn: |_src, dst| {
                    if !file_content_is(dst, "bar") {
                        fs::write(dst, "bar")
                    } else {
                        Err(io::Error::new::<&str>(
                            io::ErrorKind::Unsupported,
                            "replictor2 error",
                        ))
                    }
                },
            }),
        ]);

        // first replicator should be called
        let result = replicator.replicate(&src, &dst);
        assert!(src.exists());
        assert!(dst.exists());
        assert!(file_content_is(&dst, "foo"));
        assert!(result.is_ok());

        // replicate again, this time 2nd replicator should be called
        let result = replicator.replicate(&src, &dst);
        assert!(src.exists());
        assert!(dst.exists());
        assert!(file_content_is(&dst, "bar"));
        assert!(result.is_ok());

        // replicate again, this time an error should be returned
        let result = replicator.replicate(&src, &dst);
        assert!(src.exists());
        assert!(dst.exists());
        assert!(file_content_is(&dst, "bar"));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string() == NONE_REPLICATE_ERR_MSG);

        teardown(&src, &dst);
    }
}
