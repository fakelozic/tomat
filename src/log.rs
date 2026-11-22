//! The one logger: JSON lines in `~/.local/state/tomat/logs`, 7 days kept (docs/operations.md).
//!
//! What goes wrong, plus the window opening and closing. **Never a timer's start, finish or
//! length:** that would be tracking. It never panics and never stops the app.

mod files;
mod limit;
mod record;

use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use files::Files;
use record::{Level, Writer};

struct Log {
    writer: Writer,
    files: Option<Files>,
}

static LOG: OnceLock<Log> = OnceLock::new();
static LIMIT: Mutex<limit::Limit> = Mutex::new(limit::Limit::new());
static PRUNED_ON: AtomicI64 = AtomicI64::new(-1);

/// Which program is logging.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Process {
    /// The timer and the glass.
    App,
    /// The window.
    Window,
}

/// Starts logging to `folder` and records any panic from now on. Call once, first thing.
pub fn start(process: Process, folder: &Path, home: &Path) {
    let files = Files::open(folder);
    let problem = files.as_ref().err().map(ToString::to_string);
    let writer = Writer {
        process: if process == Process::App { "app" } else { "window" },
        pid: std::process::id(),
        home: home.to_string_lossy().into_owned(),
    };
    if LOG.set(Log { writer, files: files.ok() }).is_err() {
        return;
    }
    std::panic::set_hook(Box::new(|info| {
        let message = info.payload_as_str().unwrap_or("(no message)");
        let place = info.location().map_or_else(|| "an unknown place".to_owned(), ToString::to_string);
        error("process.panic", &format!("at={place} message={message}"));
    }));
    if let Some(problem) = problem {
        warn("log.unavailable", &problem);
    }
}

/// An ordinary milestone. Never anything about a timer.
pub fn note(event: &str, detail: &str) {
    write(Level::Info, event, detail);
}

/// Odd, but handled.
pub fn warn(event: &str, detail: &str) {
    write(Level::Warn, event, detail);
}

/// Something broke.
pub fn error(event: &str, detail: &str) {
    write(Level::Error, event, detail);
}

fn write(level: Level, event: &str, detail: &str) {
    let Some(log) = LOG.get() else { return };
    let saved = log.files.as_ref().is_some_and(|files| save(&log.writer, files, level, event, detail));
    if !saved && level != Level::Info {
        // Nowhere left to report a failure to write the log itself.
        let _ = writeln!(std::io::stderr(), "tomat: {event} {detail}");
    }
}

fn save(writer: &Writer, files: &Files, level: Level, event: &str, detail: &str) -> bool {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| u64::try_from(since.as_millis()).unwrap_or(u64::MAX));
    let day = (millis / 86_400_000).cast_signed();
    if PRUNED_ON.swap(day, Ordering::Relaxed) != day
        && let Err(problem) = files.prune(day)
    {
        let line = writer.line(millis, Level::Warn, "log.prune_failed", &problem.to_string());
        // The line below still gets its own try.
        let _ = files.append(day, &line, true);
    }
    // try_lock: a panic inside here would log, and must not wait for itself.
    let dropped = match LIMIT.try_lock() {
        Ok(mut limit) if event != "process.panic" => match limit.check(event, millis) {
            Some(dropped) => dropped,
            None => return true,
        },
        _ => 0,
    };
    if dropped > 0 {
        let what = format!("event={event} lines={dropped}");
        let line = writer.line(millis, Level::Warn, "log.suppressed", &what);
        // Only a count of lines: losing it loses nothing.
        let _ = files.append(day, &line, false);
    }
    let vital = level != Level::Info;
    files.append(day, &writer.line(millis, level, event, detail), vital).is_ok()
}
