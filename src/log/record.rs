//! One log line and the rules about dates and privacy. Pure.

/// Days of log files kept, today included.
pub(super) const KEEP_DAYS: i64 = 7;
/// Most of one message that is written.
const MESSAGE_LIMIT: usize = 8_192;

/// How bad it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Level {
    /// A normal milestone.
    Info,
    /// Odd, but handled.
    Warn,
    /// Something broke.
    Error,
}

impl Level {
    fn name(self) -> &'static str {
        match self {
            Level::Info => "info",
            Level::Warn => "warn",
            Level::Error => "error",
        }
    }
}

/// Who is writing.
#[derive(Debug, Clone)]
pub(super) struct Writer {
    /// `app` or `window`.
    pub process: &'static str,
    /// The process id.
    pub pid: u32,
    /// The home folder, written as `~` so logs can be shared without the user's name.
    pub home: String,
}

impl Writer {
    /// One JSON line, newline included. `millis` is milliseconds since 1970, UTC.
    pub(super) fn line(&self, millis: u64, level: Level, event: &str, detail: &str) -> String {
        let mut detail = if self.home.len() > 1 { detail.replace(&self.home, "~") } else { detail.to_owned() };
        if detail.len() > MESSAGE_LIMIT {
            let mut cut = MESSAGE_LIMIT;
            while !detail.is_char_boundary(cut) {
                cut -= 1;
            }
            detail.truncate(cut);
            detail.push_str(" [cut]");
        }
        format!(
            "{{\"time\":\"{}\",\"level\":\"{}\",\"event\":{},\"process\":\"{}\",\"pid\":{},\"detail\":{}}}\n",
            timestamp(millis),
            level.name(),
            quote(event),
            self.process,
            self.pid,
            quote(&detail),
        )
    }
}

/// A JSON string.
fn quote(text: &str) -> String {
    let mut out = String::from("\"");
    for letter in text.chars() {
        match letter {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\u{0}'..='\u{1f}' | '\u{7f}' => {
                out.push_str("\\u00");
                let code = u32::from(letter);
                out.extend([code >> 4, code & 15].iter().filter_map(|&half| char::from_digit(half, 16)));
            }
            _ => out.push(letter),
        }
    }
    out.push('"');
    out
}

/// `2026-01-02T03:04:05.123Z`.
pub(super) fn timestamp(millis: u64) -> String {
    let seconds = millis / 1000;
    let (year, month, day) = date((seconds / 86_400).cast_signed());
    let (hour, minute, second) = (seconds / 3600 % 24, seconds / 60 % 60, seconds % 60);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{:03}Z", millis % 1000)
}

/// The file for a day, counted from 1970-01-01.
pub(super) fn file_name(day: i64) -> String {
    let (year, month, date) = date(day);
    format!("tomat-{year:04}-{month:02}-{date:02}.log")
}

/// True only for one of our own files that is older than the last `KEEP_DAYS` days.
pub(super) fn is_expired(name: &str, today: i64) -> bool {
    day_of(name).is_some_and(|day| day <= today - KEEP_DAYS)
}

fn day_of(name: &str) -> Option<i64> {
    let text = name.strip_prefix("tomat-")?.strip_suffix(".log")?;
    let mut parts = text.split('-');
    let (year, month, day): (i64, i64, i64) =
        (parts.next()?.parse().ok()?, parts.next()?.parse().ok()?, parts.next()?.parse().ok()?);
    if !(1970..=9999).contains(&year) || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let days = days_since_1970(year, month, day);
    // Round trip, so impossible dates and odd spellings never count as ours.
    (file_name(days) == name).then_some(days)
}

/// Howard Hinnant's `civil_from_days`.
fn date(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let of_era = z.rem_euclid(146_097);
    let year_of_era = (of_era - of_era / 1460 + of_era / 36_524 - of_era / 146_096) / 365;
    let of_year = of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let m = (5 * of_year + 2) / 153;
    let month = if m < 10 { m + 3 } else { m - 9 };
    (year_of_era + era * 400 + i64::from(month <= 2), month, of_year - (153 * m + 2) / 5 + 1)
}

/// Howard Hinnant's `days_from_civil`.
fn days_since_1970(year: i64, month: i64, day: i64) -> i64 {
    let year = year - i64::from(month <= 2);
    let era = year.div_euclid(400);
    let year_of_era = year.rem_euclid(400);
    let of_year = (153 * if month > 2 { month - 3 } else { month + 9 } + 2) / 5 + day - 1;
    let of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + of_year;
    era * 146_097 + of_era - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times_are_utc_iso() {
        assert_eq!(timestamp(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(timestamp(951_782_400_000), "2000-02-29T00:00:00.000Z");
        assert_eq!(timestamp(1_795_331_572_123), "2026-11-22T07:12:52.123Z");
    }

    #[test]
    fn every_day_for_80_years_survives_the_round_trip() {
        for day in 0..30_000 {
            assert_eq!(day_of(&file_name(day)), Some(day));
        }
    }

    #[test]
    fn keeps_exactly_seven_days() {
        let today = days_since_1970(2026, 11, 22);
        let kept = (0..30).filter(|back| !is_expired(&file_name(today - back), today)).count();
        assert_eq!(kept, 7);
        assert!(!is_expired("tomat-2026-11-16.log", today) && is_expired("tomat-2026-11-15.log", today));
        assert!(!is_expired("tomat-2027-01-01.log", today), "a clock set backwards must not delete logs");
    }

    #[test]
    fn never_claims_files_that_are_not_ours() {
        let today = days_since_1970(2026, 11, 22);
        for name in [
            "state",
            "notes.log",
            "tomat-2020-02-30.log",
            "tomat-2020-1-01.log",
            "tomat-2020-01-01.log.bak",
            "../tomat-2020-01-01.log",
        ] {
            assert!(!is_expired(name, today), "{name}");
        }
    }

    #[test]
    fn lines_are_json_and_hide_the_home_folder() {
        let writer = Writer { process: "app", pid: 7, home: "/home/someone".into() };
        let line = writer.line(0, Level::Error, "app.failed", "/home/someone/\"a\"\\\n\u{1b}");
        let expected = concat!(
            r#"{"time":"1970-01-01T00:00:00.000Z","level":"error","event":"app.failed","#,
            r#""process":"app","pid":7,"detail":"~/\"a\"\\\n\u001b"}"#,
            "\n"
        );
        assert_eq!(line, expected);
        assert!(writer.line(0, Level::Info, "x", &"é".repeat(9000)).len() < 9000, "long messages are cut");
    }
}
