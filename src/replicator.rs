use std::fmt;
use std::fs;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};
use symlink::symlink_file;

#[derive(Serialize, Deserialize, Debug)]
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

#[derive(Serialize, Deserialize)]
pub struct ReplicatorConfig {
    kind: ReplicatorKind,
    fallback: ReplicatorKind,
}

pub trait Replicator {
    fn replicate(&self, src: &Path, dst: &Path) -> Result<(), io::Error>;
    fn kind(&self) -> ReplicatorKind;
}

pub struct ReplicatorChain<'a> {
    chain: Vec<&'a dyn Replicator>,
}

impl<'a> ReplicatorChain<'a> {}

impl<'a> Replicator for ReplicatorChain<'a> {
    fn replicate(&self, src: &Path, dst: &Path) -> Result<(), io::Error> {
        for i in 0..self.chain.len() {
            let replicator = &self.chain[i];

            match replicator.replicate(src, dst) {
                Ok(_) => return Ok(()),
                Err(err) => {
                    log::warn!(
                        "using fallback replicator ({}) because an error occurred while replicating {} to {}: {}",
                        replicator.kind(),
                        src.display(),
                        dst.display(),
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

#[derive(Default)]
pub struct NoneReplicator {}

const NONE_REPLICATE_ERR_MSG: &str = "none replicator reached: replicate failed";

impl NoneReplicator {
    pub fn replicate_error(&self) -> io::Error {
        io::Error::new::<&str>(io::ErrorKind::Unsupported, NONE_REPLICATE_ERR_MSG)
    }
}

impl Replicator for NoneReplicator {
    fn replicate(&self, _src: &Path, _dst: &Path) -> Result<(), io::Error> {
        log::error!("{}", NONE_REPLICATE_ERR_MSG);
        Err(self.replicate_error())
    }

    fn kind(&self) -> ReplicatorKind {
        ReplicatorKind::None
    }
}

#[derive(Default)]
pub struct SoftLinkReplicator {}

impl Replicator for SoftLinkReplicator {
    fn replicate(&self, src: &Path, dst: &Path) -> Result<(), io::Error> {
        symlink_file(&src, &dst)
    }

    fn kind(&self) -> ReplicatorKind {
        ReplicatorKind::SoftLink
    }
}

#[derive(Default)]
pub struct HardLinkReplicator {}

impl Replicator for HardLinkReplicator {
    fn replicate(&self, src: &Path, dst: &Path) -> Result<(), io::Error> {
        fs::hard_link(src, dst)
    }

    fn kind(&self) -> ReplicatorKind {
        ReplicatorKind::HardLink
    }
}

#[derive(Default)]
pub struct CopyReplicator {}

impl Replicator for CopyReplicator {
    fn replicate(&self, src: &Path, dst: &Path) -> Result<(), io::Error> {
        match fs::copy(src, dst) {
            Ok(_) => Ok(()),
            Err(err) => Err(err),
        }
    }

    fn kind(&self) -> ReplicatorKind {
        ReplicatorKind::Copy
    }
}

#[cfg(test)]
mod tests {
    use std::env::temp_dir;
    use std::fs;
    use std::io::{Read, Write};
    use std::path::{Path, PathBuf};

    #[cfg(unix)]
    use std::os::unix::fs::MetadataExt;

    use super::{
        CopyReplicator, HardLinkReplicator, NoneReplicator, Replicator, SoftLinkReplicator,
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
}
