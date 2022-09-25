use std::ffi::OsStr;
use std::fmt;
use std::fs;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};
use symlink::symlink_file;

#[derive(Serialize, Deserialize, Debug, clap::ValueEnum, Clone, Copy, PartialEq, Eq)]
#[clap(rename_all = "lowercase")]
pub enum ReplicatorKind {
    #[serde(skip)]
    None,

    #[serde(rename = "copy")]
    Copy,

    #[serde(rename = "hardlink")]
    HardLink,

    #[serde(rename = "softlink")]
    SoftLink,

    #[serde(skip)]
    Chain,
}

impl fmt::Display for ReplicatorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let str = match self {
            ReplicatorKind::None => "none",
            ReplicatorKind::Copy => "copy",
            ReplicatorKind::HardLink => "hardlink",
            ReplicatorKind::SoftLink => "softlink",
            ReplicatorKind::Chain => "chain",
        };

        f.write_str(str)
    }
}

impl From<&OsStr> for ReplicatorKind {
    fn from(str: &OsStr) -> Self {
        match str.to_str().to_owned().unwrap() {
            "copy" => ReplicatorKind::Copy,
            "hardlink" => ReplicatorKind::HardLink,
            "softlink" => ReplicatorKind::SoftLink,
            "chain" => ReplicatorKind::Chain,
            "none" | &_ => ReplicatorKind::None,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct ReplicatorConfig {
    kind: ReplicatorKind,
    fallback: ReplicatorKind,
}

pub trait Replicator: fmt::Debug {
    fn replicate(&self, src: &Path, dst: &Path) -> io::Result<()>;
    fn kind(&self) -> ReplicatorKind;
}

impl From<ReplicatorKind> for Box<dyn Replicator> {
    fn from(kind: ReplicatorKind) -> Self {
        match kind {
            ReplicatorKind::None => Box::new(NoneReplicator::default()),
            ReplicatorKind::Copy => Box::new(CopyReplicator::default()),
            ReplicatorKind::HardLink => Box::new(HardLinkReplicator::default()),
            ReplicatorKind::SoftLink => Box::new(SoftLinkReplicator::default()),
            ReplicatorKind::Chain => Box::new(ReplicatorChain::default()),
        }
    }
}

impl From<Vec<&ReplicatorKind>> for Box<dyn Replicator> {
    fn from(vec: Vec<&ReplicatorKind>) -> Self {
        if vec.len() == 1 {
            return Box::from(vec[0].to_owned());
        }

        let chain: Vec<Box<dyn Replicator>> = vec
            .iter()
            .map(|kind| Box::<dyn Replicator>::from(kind.to_owned().to_owned()))
            .collect();

        Box::new(ReplicatorChain::new(chain))
    }
}

#[derive(Debug)]
pub struct ReplicatorChain {
    chain: Vec<Box<dyn Replicator>>,
}

impl ReplicatorChain {
    pub fn new(mut chain: Vec<Box<dyn Replicator>>) -> Self {
        if let Some(last) = chain.last() {
            if last.kind() != ReplicatorKind::None {
                chain.push(Box::new(NoneReplicator {}))
            }
        } else {
            chain.push(Box::new(NoneReplicator {}))
        }

        Self { chain }
    }
}

impl Replicator for ReplicatorChain {
    fn replicate(&self, src: &Path, dst: &Path) -> io::Result<()> {
        for i in 0..self.chain.len() {
            let replicator = &self.chain[i];

            match replicator.replicate(src, dst) {
                Ok(_) => return Ok(()),
                Err(err) => {
                    log::warn!(
                        "replicator error ({} {:?} -> {:?}): {}",
                        replicator.kind(),
                        src,
                        dst,
                        err
                    );
                    // Last replicator failed, return the error
                    if i == self.chain.len() - 1 {
                        return Err(err);
                    }
                }
            }
        }

        panic!("invalid replicator chain state: doesn't contains a NoneReplicator")
    }

    fn kind(&self) -> ReplicatorKind {
        ReplicatorKind::Chain
    }
}

impl Default for ReplicatorChain {
    fn default() -> Self {
        Self::new(vec![
            Box::new(HardLinkReplicator {}),
            Box::new(SoftLinkReplicator {}),
            Box::new(CopyReplicator {}),
            Box::new(NoneReplicator {}),
        ])
    }
}

#[derive(Debug, Default)]
pub struct NoneReplicator {}

const NONE_REPLICATE_ERR_MSG: &str = "none replicator reached: replicate failed";

impl NoneReplicator {
    pub fn replicate_error(&self) -> io::Error {
        io::Error::new::<&str>(io::ErrorKind::Unsupported, NONE_REPLICATE_ERR_MSG)
    }
}

impl Replicator for NoneReplicator {
    fn replicate(&self, _src: &Path, _dst: &Path) -> io::Result<()> {
        log::error!("{}", NONE_REPLICATE_ERR_MSG);
        Err(self.replicate_error())
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

impl<F: Fn(&Path, &Path) -> io::Result<()>> Replicator for MockReplicator<F> {
    fn replicate(&self, src: &Path, dst: &Path) -> io::Result<()> {
        (self.replicate_fn)(src, dst)
    }

    fn kind(&self) -> ReplicatorKind {
        ReplicatorKind::None
    }
}

impl<F: Fn(&Path, &Path) -> io::Result<()>> fmt::Debug for MockReplicator<F> {
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
        ReplicatorChain, SoftLinkReplicator,
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

        println!("{:?}", actual_content);

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
            replicator.replicate_error().kind()
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
    fn chain_replicator() {
        let (src, dst) = setup();
        let replicator = ReplicatorChain::new(vec![
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
            }),
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
