//! Telling the user a phase ended: the terminal bell, plus a desktop
//! notification through the system's `notify-send` (from libnotify).
//!
//! Running `notify-send` for each popup, instead of linking a D-Bus client,
//! keeps tomat small: a linked client stayed resident with two threads and
//! about 1 MiB after the first popup. The child exits in milliseconds, and
//! [`Notifier::reap`] collects it so no zombie is left behind.

use std::io::Write;
use std::process::{Child, Command, Stdio};

/// Sends phase-done notifications and keeps track of the `notify-send`
/// processes it started until they exit.
#[derive(Debug)]
pub struct Notifier {
    desktop: bool,
    running: Vec<Child>,
}

impl Notifier {
    /// A notifier that rings the bell, and also shows desktop
    /// notifications if `desktop` is set.
    #[must_use]
    pub fn new(desktop: bool) -> Self {
        Self {
            desktop,
            running: Vec::new(),
        }
    }

    /// Rings the terminal bell and, if enabled, shows a desktop notification.
    ///
    /// Failures are ignored on purpose: `notify-send` or a notification
    /// daemon may be missing (SSH, a bare TTY), and the screen already
    /// shows that the phase changed.
    pub fn phase_done(&mut self, title: &str, body: &str) {
        let mut out = std::io::stdout();
        // BEL is outside the screen buffer, so writing it between frames is safe.
        let _ = out.write_all(b"\x07").and_then(|()| out.flush());

        if !self.desktop {
            return;
        }
        let child = Command::new("notify-send")
            .args([
                "--app-name=tomat",
                "--icon=alarm-symbolic",
                "--",
                title,
                body,
            ])
            // Detached from the terminal, so it can neither read our keys
            // nor print over the screen.
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
        if let Ok(child) = child {
            self.running.push(child);
        }
    }

    /// Collects `notify-send` processes that have exited. Cheap; call it
    /// once per frame. A process whose status can't be read this time is
    /// kept and tried again, since dropping it unwaited would leave a zombie.
    pub fn reap(&mut self) {
        self.running
            .retain_mut(|child| !matches!(child.try_wait(), Ok(Some(_))));
    }
}
