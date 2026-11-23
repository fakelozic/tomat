//! What says a phase ended: a short chime and a popup.
//!
//! Both are the system's own programs, run as their own processes, so tomat links no audio or D-Bus
//! library and never waits for them. One that fails is never logged: the line's time would tell
//! when a phase ended.

use std::ffi::OsString;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use crate::log;

/// KDE's own short "done" sound, then the freedesktop one.
const SOUNDS: [&str; 2] =
    ["/usr/share/sounds/ocean/stereo/completion-success.oga", "/usr/share/sounds/freedesktop/stereo/complete.oga"];
/// `PipeWire`'s player, then `PulseAudio`'s, which `PipeWire` also serves.
const PLAYERS: [&str; 2] = ["pw-play", "paplay"];
/// Sends a desktop notification.
const POPUP: &str = "notify-send";
/// With this many still running, something is stuck: stay quiet.
const MOST_RUNNING: usize = 4;
/// How long the popup stays, for tomat only. Plasma uses an app's own time over its global one.
const POPUP_MS: &str = "--expire-time=2000";
/// Library paths another program (a terminal inside an `AppImage`, say) may have left behind.
/// The system's programs would load the wrong libraries with them.
const FOREIGN: [&str; 2] = ["LD_LIBRARY_PATH", "LD_PRELOAD"];

/// Which end it was.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Ending {
    /// The focus ended; the break has this much left.
    Focus { rest: Duration },
    /// The break ended: the timer stopped.
    Round,
}

/// The programs, found once at start, and the ones still running.
pub(super) struct Cue {
    chime: Option<(PathBuf, PathBuf)>,
    popup: Option<PathBuf>,
    running: Vec<Child>,
}

impl Cue {
    /// Finds a player, a sound and `notify-send`. Whatever is missing is said once, at start.
    pub(super) fn find() -> Self {
        let sound = SOUNDS.iter().map(PathBuf::from).find(|sound| sound.is_file());
        let player = PLAYERS.iter().find_map(|name| on_path(name));
        let chime = match (player, sound) {
            (Some(player), Some(sound)) => Some((player, sound)),
            (None, _) => quiet("chime.missing", "no pw-play or paplay"),
            (_, None) => quiet("chime.missing", "no completion sound in /usr/share/sounds"),
        };
        let popup = on_path(POPUP).or_else(|| quiet("popup.missing", "no notify-send"));
        Self { chime, popup, running: Vec::new() }
    }

    /// Chimes and pops up, without waiting.
    pub(super) fn ring(&mut self, ending: Ending) {
        self.reap();
        if let Some((player, sound)) = self.chime.clone() {
            self.run(&player, &[sound.into_os_string()]);
        }
        if let Some(popup) = self.popup.clone() {
            self.run(&popup, &popup_args(ending));
        }
    }

    fn run(&mut self, program: &Path, args: &[OsString]) {
        if self.running.len() >= MOST_RUNNING {
            return;
        }
        let mut command = Command::new(program);
        command.args(args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).process_group(0);
        for name in FOREIGN {
            command.env_remove(name);
        }
        if let Ok(child) = command.spawn() {
            self.running.push(child);
        }
    }

    /// Lets finished ones go, so none is left a zombie.
    pub(super) fn reap(&mut self) {
        self.running.retain_mut(|child| matches!(child.try_wait(), Ok(None)));
    }
}

/// `notify-send`'s arguments. Transient, so Plasma keeps no history of rounds; gone after 2 seconds.
fn popup_args(ending: Ending) -> Vec<OsString> {
    let (title, body) = match ending {
        Ending::Focus { rest } => ("Focus done", format!("{} min break", rest.as_secs().div_ceil(60).max(1))),
        Ending::Round => ("Break over", "tomat stopped".to_owned()),
    };
    [
        "--app-name=tomat",
        "--icon=tomat",
        "--transient",
        "--hint=string:desktop-entry:tomat",
        POPUP_MS,
        "--",
        title,
        &body,
    ]
    .into_iter()
    .map(OsString::from)
    .collect()
}

fn quiet<T>(event: &str, why: &str) -> Option<T> {
    log::warn(event, why);
    None
}

fn on_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).map(|dir| dir.join(name)).find(|path| runnable(path))
}

fn runnable(path: &Path) -> bool {
    path.metadata().is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use super::*;

    fn cue(chime: &str, popup: &str) -> Cue {
        Cue { chime: Some((chime.into(), "sound".into())), popup: Some(popup.into()), running: Vec::new() }
    }

    #[test]
    fn it_runs_both_and_lets_go() {
        let mut cue = cue("/usr/bin/true", "/usr/bin/true");
        cue.ring(Ending::Round);
        assert_eq!(cue.running.len(), 2);
        let deadline = Instant::now() + Duration::from_secs(5);
        while !cue.running.is_empty() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
            cue.reap();
        }
        assert!(cue.running.is_empty(), "finished ones are reaped");
    }

    #[test]
    fn stuck_ones_are_not_joined_by_more() {
        let mut cue = cue("/usr/bin/sleep", "/usr/bin/sleep");
        cue.chime = Some(("/usr/bin/sleep".into(), "30".into()));
        cue.popup = None;
        for _ in 0..9 {
            cue.ring(Ending::Round);
        }
        assert_eq!(cue.running.len(), MOST_RUNNING);
        for child in &mut cue.running {
            child.kill().expect("kills");
            child.wait().expect("waits");
        }
    }

    #[test]
    fn without_programs_it_is_quiet() {
        let mut cue = Cue { chime: None, popup: None, running: Vec::new() };
        cue.ring(Ending::Focus { rest: Duration::from_secs(300) });
        assert!(cue.running.is_empty());
    }

    #[test]
    fn the_popup_says_what_ended_and_leaves_no_history() {
        let text =
            |ending| popup_args(ending).into_iter().map(|arg| arg.into_string().expect("utf-8")).collect::<Vec<_>>();
        let focus = text(Ending::Focus { rest: Duration::from_millis(299_400) });
        assert_eq!(focus[focus.len() - 2..], ["Focus done", "5 min break"]);
        assert!(focus.contains(&"--transient".to_owned()));
        assert!(focus.contains(&"--expire-time=2000".to_owned()), "two seconds on screen");
        let short = text(Ending::Focus { rest: Duration::from_secs(10) });
        assert_eq!(short.last().map(String::as_str), Some("1 min break"));
        let round = text(Ending::Round);
        assert_eq!(round[round.len() - 2..], ["Break over", "tomat stopped"]);
    }
}
