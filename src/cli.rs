//! Command-line flags.
//!
//! Hand-rolled on top of `lexopt`, a tiny parser with no dependencies of its
//! own. tomat has four flags, so a full framework would cost more binary
//! (and memory) than it saves in code. See `docs/decisions.md`.

use std::ffi::{OsStr, OsString};
use std::time::Duration;

use crate::pomodoro::Plan;

/// The `--help` text.
pub const HELP: &str = "\
tomat: a plain pomodoro timer for the terminal

Usage: tomat [OPTIONS]

With no options, opens the menu (15/5, 30/5 or 60/10 minutes).
Giving -w or -b starts that plan straight away; a length you leave
out defaults to 30 min focus / 5 min break.

Options:
  -w, --work <MIN>    Focus length in minutes (decimals allowed: 0.5)
  -b, --break <MIN>   Break length in minutes
      --auto          Start each next phase without waiting for Enter
      --no-notify     No desktop notifications (the bell still rings)
  -h, --help          Show this help
  -V, --version       Show the version
";

/// Longest phase accepted, in seconds: a day.
const MAX_SECONDS: f64 = 24.0 * 60.0 * 60.0;

/// What the command line asks for.
#[derive(Debug, PartialEq)]
pub enum Command {
    /// Run the timer with these settings.
    Run(Config),
    /// Print [`HELP`] and exit.
    Help,
    /// Print the version and exit.
    Version,
}

/// Settings for a run.
#[derive(Debug, PartialEq)]
pub struct Config {
    /// A plan to start at once, from `-w`/`-b`; `None` opens the menu.
    pub custom: Option<Plan>,
    /// Start each next phase without waiting for Enter.
    pub auto_start: bool,
    /// Send desktop notifications.
    pub notify: bool,
}

/// Parses `args` (without the program name).
///
/// # Errors
/// Fails on an unknown flag, a missing value, or a length that is not a
/// number of minutes between 0 and 24 hours.
pub fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Command, lexopt::Error> {
    use lexopt::prelude::*;

    let mut parser = lexopt::Parser::from_args(args);
    let (mut work, mut rest) = (None, None);
    let mut config = Config {
        custom: None,
        auto_start: false,
        notify: true,
    };
    while let Some(arg) = parser.next()? {
        match arg {
            Short('w') | Long("work") => work = Some(minutes("--work", &parser.value()?)?),
            Short('b') | Long("break") => rest = Some(minutes("--break", &parser.value()?)?),
            Long("auto") => config.auto_start = true,
            Long("no-notify") => config.notify = false,
            Short('h') | Long("help") => return Ok(Command::Help),
            Short('V') | Long("version") => return Ok(Command::Version),
            _ => return Err(arg.unexpected()),
        }
    }
    if work.is_some() || rest.is_some() {
        let fallback = Plan::from_minutes(30, 5);
        config.custom = Some(Plan {
            focus: work.unwrap_or(fallback.focus),
            rest: rest.unwrap_or(fallback.rest),
        });
    }
    Ok(Command::Run(config))
}

/// Parses the value of `flag` as a length in minutes, e.g. `25` or `0.5`,
/// rounded to whole seconds so the clock, captions and history all agree.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "checked to be a whole number from 1 to MAX_SECONDS first"
)]
fn minutes(flag: &str, value: &OsStr) -> Result<Duration, lexopt::Error> {
    let text = value.to_string_lossy();
    let minutes: f64 = text
        .parse()
        .map_err(|_| format!("{flag}: `{text}` is not a number of minutes"))?;
    let seconds = (minutes * 60.0).round();
    // Written so NaN fails the check too.
    if !(1.0..=MAX_SECONDS).contains(&seconds) {
        return Err(format!("{flag}: must be from 1 second to 24 hours (1440 minutes)").into());
    }
    Ok(Duration::from_secs(seconds as u64))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(args: &[&str]) -> Result<Command, String> {
        parse(args.iter().map(OsString::from)).map_err(|e| e.to_string())
    }

    fn config(args: &[&str]) -> Config {
        match run(args) {
            Ok(Command::Run(config)) => config,
            other => panic!("expected a run, got {other:?}"),
        }
    }

    #[test]
    fn no_flags_opens_the_menu_with_notifications() {
        let c = config(&[]);
        assert_eq!(c.custom, None);
        assert!(c.notify && !c.auto_start);
    }

    #[test]
    fn work_and_break_make_a_custom_plan() {
        assert_eq!(
            config(&["-w", "45", "--break=10"]).custom,
            Some(Plan::from_minutes(45, 10))
        );
        assert_eq!(config(&["-w45"]).custom, Some(Plan::from_minutes(45, 5)));
        assert_eq!(
            config(&["-b", "0.5"]).custom.map(|p| p.rest),
            Some(Duration::from_secs(30))
        );
    }

    #[test]
    fn lengths_round_to_whole_seconds() {
        let plan = config(&["-w", "0.33", "-b", "1.005"])
            .custom
            .expect("custom plan");
        assert_eq!(plan.focus, Duration::from_secs(20));
        assert_eq!(plan.rest, Duration::from_secs(60));
    }

    #[test]
    fn switches_turn_on_auto_start_and_off_notifications() {
        let c = config(&["--auto", "--no-notify"]);
        assert!(c.auto_start && !c.notify);
    }

    #[test]
    fn help_and_version_win_over_other_flags() {
        assert_eq!(run(&["-w", "5", "--help"]), Ok(Command::Help));
        assert_eq!(run(&["-V"]), Ok(Command::Version));
    }

    #[test]
    fn bad_input_is_rejected_with_a_clear_message() {
        assert!(run(&["-w", "0"]).is_err_and(|e| e.contains("--work: must be from 1 second")));
        assert!(run(&["-w", "1e-12"]).is_err(), "rounds to zero seconds");
        assert!(run(&["-b", "NaN"]).is_err());
        assert!(run(&["-w", "soon"]).is_err_and(|e| e.contains("not a number of minutes")));
        assert!(run(&["-w", "2000"]).is_err());
        assert!(run(&["--rounds", "4"]).is_err());
        assert!(run(&["-w"]).is_err());
    }
}
