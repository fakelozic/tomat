//! The log files on disk: one a day, 7 days kept, capped, private to the user.
//! Each line opens, appends and closes, so both processes share the file and a deleted file comes back.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

use super::record;
use crate::private_file;
use rustix::fs::OFlags;

/// Most that one day may take, so a crash loop can't fill the disk. 7 days is 14 MB at worst.
pub(super) const DAY_LIMIT: u64 = 2 * 1024 * 1024;
/// The last of each day is kept for warnings, errors and shutdowns, so chatter can't crowd out a crash.
const RESERVED: u64 = 512 * 1024;

/// One log folder.
#[derive(Debug)]
pub(super) struct Files {
    folder: PathBuf,
}

impl Files {
    /// Creates the folder if needed.
    pub(super) fn open(folder: &Path) -> io::Result<Self> {
        fs::DirBuilder::new().recursive(true).mode(0o700).create(folder)?;
        Ok(Self { folder: folder.to_owned() })
    }

    /// Appends to that day's file. `Ok(false)` means the day is full.
    pub(super) fn append(&self, day: i64, line: &str, vital: bool) -> io::Result<bool> {
        let mut file = OpenOptions::new()
            .append(true)
            .create(true)
            .mode(0o600)
            // Never write through a link to some other file. Nonblocking, so a pipe can't hang tomat.
            .custom_flags(private_file::flags(OFlags::NOFOLLOW | OFlags::NONBLOCK))
            .open(self.folder.join(record::file_name(day)))?;
        let meta = file.metadata()?;
        if !meta.is_file() {
            return Err(io::Error::other("the log path is not a file"));
        }
        let limit = if vital { DAY_LIMIT } else { DAY_LIMIT - RESERVED };
        if meta.len() + line.len() as u64 > limit {
            return Ok(false);
        }
        // One write in append mode, so the two processes' lines can't interleave.
        file.write_all(line.as_bytes())?;
        Ok(true)
    }

    /// Removes our own files older than the kept days, and nothing else. Returns how many.
    pub(super) fn prune(&self, today: i64) -> io::Result<usize> {
        let (mut removed, mut failed) = (0, None);
        for entry in fs::read_dir(&self.folder)? {
            let Ok(entry) = entry else { continue };
            let ours = entry.file_name().to_str().is_some_and(|name| record::is_expired(name, today));
            // `file_type` doesn't follow links, so only real files are ever removed.
            if ours && entry.file_type().is_ok_and(|kind| kind.is_file()) {
                match fs::remove_file(entry.path()) {
                    Ok(()) => removed += 1,
                    // One stubborn file must not keep the others.
                    Err(error) => failed = Some(error),
                }
            }
        }
        failed.map_or(Ok(removed), Err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let folder = std::env::temp_dir().join(format!("tomat-log-test-{}-{name}", std::process::id()));
        fs::create_dir_all(&folder).expect("test setup");
        folder
    }

    #[test]
    fn prunes_only_our_old_files() {
        let folder = scratch("prune");
        let files = Files::open(&folder).expect("test setup");
        let today = 20_779;
        for back in 0..10 {
            assert!(files.append(today - back, "{}\n", false).expect("test setup"));
        }
        fs::write(folder.join("keep-me.log"), "x").expect("test setup");
        fs::create_dir(folder.join(record::file_name(today - 40))).expect("test setup");
        assert_eq!(files.prune(today).expect("test setup"), 3);
        assert_eq!(fs::read_dir(&folder).expect("test setup").count(), 7 + 2);
        fs::remove_dir_all(folder).expect("test setup");
    }

    #[test]
    fn a_day_cannot_grow_past_the_limit() {
        let folder = scratch("limit");
        let files = Files::open(&folder).expect("test setup");
        let line = "x".repeat(1024 * 1024);
        assert!(files.append(1, &line, false).expect("test setup"));
        assert!(!files.append(1, &line, false).expect("test setup"), "chatter stops short of the reserve");
        assert!(files.append(1, &line, true).expect("test setup"), "a crash still fits");
        assert!(!files.append(1, "one more", true).expect("test setup"));
        fs::remove_dir_all(folder).expect("test setup");
    }

    #[test]
    fn never_writes_through_a_link() {
        let folder = scratch("link");
        let target = folder.join("target");
        fs::write(&target, "untouched").expect("test setup");
        std::os::unix::fs::symlink(&target, folder.join(record::file_name(3))).expect("test setup");
        assert!(Files::open(&folder).expect("test setup").append(3, "bad", true).is_err());
        assert_eq!(fs::read_to_string(&target).expect("test setup"), "untouched");
        fs::remove_dir_all(folder).expect("test setup");
    }
}
