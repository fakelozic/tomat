//! Notices when KDE's colour file changes, without polling.

use std::mem::MaybeUninit;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::path::Path;

use rustix::fs::inotify;

/// KDE replaces the file by renaming over it, so the folder is watched, not the file.
pub(super) struct Watch {
    fd: Option<OwnedFd>,
}

impl Watch {
    /// Watches the config folder. Without inotify, the theme just stays as it was at start.
    pub(super) fn new(config: &Path) -> Self {
        let flags = inotify::CreateFlags::NONBLOCK | inotify::CreateFlags::CLOEXEC;
        let Ok(fd) = inotify::init(flags) else {
            return Self { fd: None };
        };
        let events = inotify::WatchFlags::MOVED_TO
            | inotify::WatchFlags::CLOSE_WRITE
            | inotify::WatchFlags::DELETE;
        let mut watching = false;
        for folder in [config.to_owned(), config.join("kdedefaults")] {
            watching |= inotify::add_watch(&fd, &folder, events).is_ok();
        }
        Self {
            fd: watching.then_some(fd),
        }
    }

    pub(super) fn fd(&self) -> Option<BorrowedFd<'_>> {
        self.fd.as_ref().map(AsFd::as_fd)
    }

    /// Takes every waiting event. True if any of them was about the colour file.
    pub(super) fn changed(&self) -> bool {
        let Some(fd) = self.fd() else { return false };
        let mut buffer = [MaybeUninit::uninit(); 4096];
        let mut events = inotify::Reader::new(fd, &mut buffer);
        let mut changed = false;
        while let Ok(event) = events.next() {
            changed |= event.events().contains(inotify::ReadFlags::QUEUE_OVERFLOW)
                || event
                    .file_name()
                    .is_some_and(|name| name.to_bytes() == b"kdeglobals");
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sees_a_rename_over_the_file_and_ignores_others() {
        let folder = std::env::temp_dir().join(format!("tomat-watch-{}", std::process::id()));
        std::fs::create_dir_all(&folder).expect("temp folder");
        let watch = Watch::new(&folder);
        std::fs::write(folder.join("otherrc"), "x").expect("write");
        let other = watch.changed();
        std::fs::write(folder.join("new"), "x").expect("write");
        std::fs::rename(folder.join("new"), folder.join("kdeglobals")).expect("rename");
        let ours = watch.changed();
        std::fs::remove_dir_all(&folder).expect("clean up");
        assert!(!other);
        assert!(ours);
    }
}
