//! The window process, as the app sees it. Nothing here waits for the window.

use std::io::{ErrorKind, Read, Write};
use std::net::Shutdown;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};

use crate::link::{Ask, MOST_LINE, View};
use crate::log;

const PROGRAM: &str = "tomat-window";
/// How long a closed window may take to leave before it is killed.
const LEAVE: Duration = Duration::from_secs(1);
/// Most bytes taken in one pass.
const MOUTHFUL: usize = 4096;

/// What the window left to read.
pub(super) struct News {
    pub asks: Vec<Ask>,
    pub gone: bool,
}

/// One open window.
pub(super) struct Window {
    child: Child,
    socket: UnixStream,
    unread: Vec<u8>,
    shown: Option<View>,
    pub since: Instant,
}

impl Window {
    pub(super) fn start(token: Option<&str>, view: View) -> Result<Self> {
        let (socket, theirs) = UnixStream::pair().context("no socket")?;
        let mut command = Command::new(program()?);
        // A token works once, so never pass on the one tomat itself was started with.
        command.env_remove("XDG_ACTIVATION_TOKEN").env_remove("DESKTOP_STARTUP_ID");
        if let Some(token) = token {
            command.env("XDG_ACTIVATION_TOKEN", token);
        }
        let child = command
            .stdin(Stdio::from(OwnedFd::from(theirs)))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0)
            .spawn()
            .context("can't start the window")?;
        socket.set_nonblocking(true)?;
        let mut window = Self { child, socket, unread: Vec::new(), shown: None, since: Instant::now() };
        window.show(view);
        Ok(window)
    }

    pub(super) fn fd(&self) -> BorrowedFd<'_> {
        self.socket.as_fd()
    }

    /// Tells it what to show, if that changed. A full socket is retried at the next change.
    pub(super) fn show(&mut self, view: View) {
        if self.shown == Some(view) {
            return;
        }
        let line = view.encode();
        match self.socket.write(line.as_bytes()) {
            Ok(written) if written == line.len() => self.shown = Some(view),
            // Half a line: the window can't follow any more, and closing says so.
            Ok(_) => drop(self.socket.shutdown(Shutdown::Both)),
            // Full, or gone: `read` finds out which.
            Err(_) => {}
        }
    }

    pub(super) fn read(&mut self) -> News {
        let mut bytes = [0; MOUTHFUL];
        let mut gone = false;
        match self.socket.read(&mut bytes) {
            Ok(0) => gone = true,
            Ok(read) => self.unread.extend_from_slice(&bytes[..read]),
            Err(error) if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::Interrupted) => {}
            Err(_) => gone = true,
        }
        let mut asks = Vec::new();
        while let Some(end) = self.unread.iter().position(|&byte| byte == b'\n') {
            match std::str::from_utf8(&self.unread[..end]).ok().and_then(Ask::decode) {
                Some(ask) => asks.push(ask),
                None => log::warn("window.bad_message", "ignored a message it can't read"),
            }
            self.unread.drain(..=end);
        }
        if self.unread.len() > MOST_LINE {
            log::warn("window.bad_message", "a line that never ends");
            gone = true;
        }
        News { asks, gone }
    }

    /// Closes it.
    pub(super) fn close(self, why: Closing) -> Leaving {
        // Closing the socket is what tells it to exit. It fails only if it's already closed.
        let _ = self.socket.shutdown(Shutdown::Both);
        Leaving { child: self.child, until: Instant::now() + LEAVE, why, done: false }
    }
}

/// Who closed the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Closing {
    /// The app: it is leaving, starting a timer or replacing the window.
    Ours,
    /// The window: its close button, or a crash.
    Theirs,
}

/// A window on its way out.
pub(super) struct Leaving {
    child: Child,
    until: Instant,
    why: Closing,
    done: bool,
}

impl Leaving {
    /// True once it's gone.
    pub(super) fn check(&mut self) -> bool {
        if self.done {
            return true;
        }
        self.done = match self.child.try_wait() {
            Ok(Some(status)) => {
                if status.success() || self.why == Closing::Ours {
                    log::note("window.closed", "");
                } else {
                    log::error("window.crashed", &status.to_string());
                }
                true
            }
            Ok(None) if Instant::now() >= self.until => {
                log::warn("window.killed", "it didn't close by itself");
                // Both fail only if it has exited since.
                let _ = self.child.kill();
                let _ = self.child.wait();
                true
            }
            Ok(None) => false,
            Err(error) => {
                log::warn("window.lost", &error.to_string());
                true
            }
        };
        self.done
    }
}

impl Drop for Leaving {
    fn drop(&mut self) {
        while !self.check() {
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

/// `tomat-window`, which is installed beside this program.
fn program() -> Result<PathBuf> {
    let here = std::env::current_exe().context("can't find this program")?;
    let beside = here.with_file_name(PROGRAM);
    let usable = std::fs::metadata(&beside).is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0);
    anyhow::ensure!(usable, "{PROGRAM} is missing beside tomat ({})", beside.display());
    Ok(beside)
}
