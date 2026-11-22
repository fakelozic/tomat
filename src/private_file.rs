//! Small files of our own, opened so that a pipe, link or huge file left in their place can't hang
//! startup, redirect a write or eat memory.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

use rustix::fs::OFlags;

pub(crate) fn flags(flags: OFlags) -> i32 {
    flags.bits().cast_signed()
}

/// Reads a regular file of at most `most` bytes, without ever waiting on a pipe.
pub(crate) fn read(path: &Path, most: u64) -> io::Result<Vec<u8>> {
    let file = OpenOptions::new().read(true).custom_flags(flags(OFlags::NONBLOCK | OFlags::NOCTTY)).open(path)?;
    if !file.metadata()?.is_file() {
        return Err(io::Error::other("it is not a file"));
    }
    let mut bytes = Vec::new();
    file.take(most + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > most {
        return Err(io::Error::other("it is far too large"));
    }
    Ok(bytes)
}

/// Replaces `path` so that a power cut leaves all of the old or all of the new.
/// Writes only a file it has just created, so it can't write through a link.
pub(crate) fn replace(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let (Some(folder), Some(name)) = (path.parent(), path.file_name()) else {
        return Err(io::Error::other("it has no folder"));
    };
    let mut temp_name = name.to_owned();
    temp_name.push(".tmp");
    let temp = folder.join(temp_name);
    match fs::remove_file(&temp) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e),
        _ => {}
    }
    let mut file = OpenOptions::new().write(true).create_new(true).mode(0o600).open(&temp)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    fs::rename(&temp, path)?;
    // The rename is only durable once the folder is synced.
    File::open(folder)?.sync_all()
}

/// Opens or creates a file to lock.
pub(crate) fn for_lock(path: &Path) -> io::Result<File> {
    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(flags(OFlags::NOFOLLOW | OFlags::NONBLOCK))
        .open(path)?;
    if file.metadata()?.is_file() { Ok(file) } else { Err(io::Error::other("it is not a file")) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_pipes_and_huge_files_are_harmless() {
        let folder = std::env::temp_dir().join(format!("tomat-private-file-{}", std::process::id()));
        fs::create_dir_all(&folder).expect("test setup");
        let (victim, path) = (folder.join("victim"), folder.join("state"));
        fs::write(&victim, "untouched").expect("test setup");
        std::os::unix::fs::symlink(&victim, folder.join("state.tmp")).expect("test setup");
        replace(&path, b"new").expect("test setup");
        assert_eq!(read(&path, 10).expect("test setup"), b"new");
        assert_eq!(fs::read_to_string(&victim).expect("test setup"), "untouched");
        assert!(read(&victim, 4).is_err(), "over the limit");
        let pipe = folder.join("pipe");
        assert!(std::process::Command::new("/usr/bin/mkfifo").arg(&pipe).status().expect("test setup").success());
        assert!(read(&pipe, 10).is_err() && for_lock(&pipe).is_err());
        fs::remove_dir_all(folder).expect("test setup");
    }
}
