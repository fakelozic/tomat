//! One tomat at a time. Opening it again knocks on the running one's door instead.

use std::fs::{File, TryLockError};
use std::io::{ErrorKind, Read, Write};
use std::os::fd::{AsFd, BorrowedFd};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};

use crate::paths::Paths;
use crate::private_file;

/// How long to keep trying when another tomat is starting or leaving.
const PATIENCE: Duration = Duration::from_secs(5);
/// How long a knock waits for its answer.
const ANSWER: Duration = Duration::from_secs(2);
/// How long the app waits for a caller to finish speaking.
const SPEAK: Duration = Duration::from_millis(200);
/// Most callers handled in one pass, so a flood can't stop the clock.
const CALLERS: usize = 4;
const HELLO: &str = "show ";
const DONE: &[u8] = b"ok\n";

/// The running app's side.
pub(super) struct Door {
    listener: UnixListener,
    path: PathBuf,
    // Held until exit. The kernel lets go of it, even after a crash.
    _lock: File,
}

/// Someone asked for the window.
pub(super) struct Knock {
    /// What lets the new window come to the front.
    pub token: Option<String>,
}

/// What opening tomat turned out to be.
pub(super) enum Opened {
    /// This is the one tomat.
    Here(Door),
    /// Another is running and is showing its window.
    Elsewhere,
}

impl Door {
    pub(super) fn open(paths: &Paths, token: Option<&str>) -> Result<Opened> {
        let deadline = Instant::now() + PATIENCE;
        let path = paths.door();
        loop {
            if knock(&path, token).is_ok() {
                return Ok(Opened::Elsewhere);
            }
            let lock = private_file::for_lock(&paths.lock()).context("can't open the lock")?;
            match lock.try_lock() {
                Ok(()) => {
                    // Whoever holds the lock owns the door, so this is a leftover.
                    match std::fs::remove_file(&path) {
                        Err(error) if error.kind() != ErrorKind::NotFound => return Err(error.into()),
                        _ => {}
                    }
                    let listener = UnixListener::bind(&path).context("can't open the door")?;
                    listener.set_nonblocking(true)?;
                    return Ok(Opened::Here(Self { listener, path, _lock: lock }));
                }
                Err(TryLockError::WouldBlock) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(100));
                }
                Err(TryLockError::WouldBlock) => bail!("another tomat is running but doesn't answer"),
                Err(TryLockError::Error(error)) => return Err(error).context("can't lock"),
            }
        }
    }

    pub(super) fn fd(&self) -> BorrowedFd<'_> {
        self.listener.as_fd()
    }

    /// Lets everyone waiting in, and returns the newest one that asked for the window.
    pub(super) fn answer(&self) -> Option<Knock> {
        let mut asked = None;
        for _ in 0..CALLERS {
            let Ok((caller, _)) = self.listener.accept() else { break };
            if let Some(knock) = hear(caller) {
                asked = Some(knock);
            }
        }
        asked
    }
}

impl Drop for Door {
    fn drop(&mut self) {
        // Fails only if it's already gone.
        let _ = std::fs::remove_file(&self.path);
    }
}

fn hear(mut caller: UnixStream) -> Option<Knock> {
    caller.set_nonblocking(false).ok()?;
    caller.set_read_timeout(Some(SPEAK)).ok()?;
    caller.set_write_timeout(Some(SPEAK)).ok()?;
    let mut bytes = [0; 512];
    let mut length = 0;
    let deadline = Instant::now() + SPEAK;
    while !bytes[..length].contains(&b'\n') {
        if length == bytes.len() || Instant::now() >= deadline {
            return None;
        }
        match caller.read(&mut bytes[length..]) {
            Ok(0) | Err(_) => return None,
            Ok(read) => length += read,
        }
    }
    let token = std::str::from_utf8(&bytes[..length]).ok()?.trim_end().strip_prefix(HELLO.trim_end())?;
    // A caller that left before its answer still asked.
    let _ = caller.write_all(DONE);
    Some(Knock { token: super::clean_token(token.trim()) })
}

fn knock(path: &std::path::Path, token: Option<&str>) -> Result<()> {
    let mut door = UnixStream::connect(path)?;
    door.set_read_timeout(Some(ANSWER))?;
    door.set_write_timeout(Some(ANSWER))?;
    door.write_all(format!("{HELLO}{}\n", token.unwrap_or_default()).as_bytes())?;
    let mut answer = [0; 3];
    door.read_exact(&mut answer)?;
    if answer == DONE { Ok(()) } else { bail!("a strange answer") }
}
