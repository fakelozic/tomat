//! The history: one CSV line per finished focus session.
//!
//! File: `$XDG_DATA_HOME/tomat/log.csv`, usually `~/.local/share/tomat/log.csv`
//! (see `docs/data.md`). Each line is `<RFC 3339 local timestamp>,<focus
//! length in seconds>`. The file is only ever appended to; lines that fail
//! to parse are skipped, never fatal.

use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, ErrorKind, Write};
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result};
use chrono::{DateTime, Local, NaiveDate, SecondsFormat};

/// The append-only file of finished focus sessions.
#[derive(Debug, Clone)]
pub struct History {
    path: PathBuf,
}

impl History {
    /// The history at its standard place in the user's data directory, or
    /// `None` if the platform has no data directory.
    #[must_use]
    pub fn default_location() -> Option<Self> {
        dirs::data_dir().map(|dir| Self::at(dir.join("tomat").join("log.csv")))
    }

    /// A history stored at `path`.
    #[must_use]
    pub fn at(path: PathBuf) -> Self {
        Self { path }
    }

    /// Records one finished focus session of length `focus` ending at
    /// `when`, creating the file and its folder if needed.
    ///
    /// # Errors
    /// Fails if the folder cannot be created or the file cannot be written.
    pub fn record(&self, when: DateTime<Local>, focus: Duration) -> Result<()> {
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .with_context(|| format!("opening {}", self.path.display()))?;
        // One write call per line, so two tomats appending at once can't
        // interleave halves of lines.
        let line = format!(
            "{},{}\n",
            when.to_rfc3339_opts(SecondsFormat::Secs, false),
            focus.as_secs()
        );
        file.write_all(line.as_bytes())
            .with_context(|| format!("writing {}", self.path.display()))
    }

    /// Counts the sessions recorded on `day`. A missing file counts as zero.
    ///
    /// # Errors
    /// Fails only if the file exists but cannot be read.
    pub fn count_on(&self, day: NaiveDate) -> Result<usize> {
        match File::open(&self.path) {
            // Line by line, so memory stays flat however long the history gets.
            Ok(file) => count_on(BufReader::new(file), day)
                .with_context(|| format!("reading {}", self.path.display())),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(0),
            Err(e) => Err(e).with_context(|| format!("opening {}", self.path.display())),
        }
    }
}

/// Counts the lines of `history` whose timestamp falls on `day`.
///
/// Lines are read as bytes, so one that isn't valid UTF-8 (a stray binary
/// write, an editor saving another encoding) is skipped like any other
/// unparsable line instead of failing the whole count.
fn count_on(history: impl BufRead, day: NaiveDate) -> std::io::Result<usize> {
    let mut count = 0;
    for line in history.split(b'\n') {
        let line = line?;
        let Ok(line) = std::str::from_utf8(&line) else {
            continue;
        };
        let stamp = line.split(',').next().unwrap_or_default().trim();
        // The stamp carries the offset it was written with; its own local
        // date is the day the user experienced.
        if DateTime::parse_from_rfc3339(stamp).is_ok_and(|stamp| stamp.date_naive() == day) {
            count += 1;
        }
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(s: &str) -> NaiveDate {
        s.parse().expect("valid test date")
    }

    fn count(text: &str, on: &str) -> usize {
        count_on(text.as_bytes(), day(on)).expect("reading from memory")
    }

    #[test]
    fn counts_only_lines_from_the_given_day() {
        let text = "\
2026-10-24T23:50:00+02:00,1500
2026-10-25T09:00:00+02:00,1500
2026-10-25T10:00:00+02:00,3000
";
        assert_eq!(count(text, "2026-10-25"), 2);
        assert_eq!(count(text, "2026-10-24"), 1);
    }

    #[test]
    fn skips_broken_and_empty_lines() {
        let text = "garbage\n\n,1500\n2026-10-25T09:00:00+02:00,1500\n2026-10-25";
        assert_eq!(count(text, "2026-10-25"), 1);
    }

    #[test]
    fn a_line_that_is_not_utf8_is_skipped_not_fatal() {
        let mut bytes = b"2026-10-25T09:00:00+02:00,1500\n".to_vec();
        bytes.extend_from_slice(b"\xff\xfe broken\n2026-10-25T10:00:00+02:00,1500\n");
        assert_eq!(
            count_on(bytes.as_slice(), day("2026-10-25")).expect("read"),
            2
        );
    }

    /// A temporary folder that is removed when dropped, even if a test fails.
    struct TempDir(std::path::PathBuf);

    impl TempDir {
        fn new() -> Self {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos());
            let name = format!("tomat-test-{}-{nanos}", std::process::id());
            Self(std::env::temp_dir().join(name))
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn record_then_count_round_trips_and_a_missing_file_is_zero() {
        let dir = TempDir::new();
        let history = History::at(dir.0.join("nested").join("log.csv"));
        let now = Local::now();
        let today = now.date_naive();
        assert_eq!(history.count_on(today).expect("missing file is fine"), 0);

        history
            .record(now, Duration::from_secs(1500))
            .expect("record");
        history
            .record(now, Duration::from_secs(1500))
            .expect("record");
        assert_eq!(history.count_on(today).expect("read"), 2);
    }
}
