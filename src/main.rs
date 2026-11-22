//! `tomat`: opens the app. From a terminal it lets go of the terminal, so closing it stops nothing.

use std::io::IsTerminal;
use std::os::unix::process::CommandExt;
use std::process::{Command, ExitCode, Stdio};

use anyhow::{Context, Result};
use tomat::{host, log, paths::Paths};

const HELP: &str = "tomat: three focus/break slots and a timer that stays on screen.

Run it, fill a slot and press play. Run it again to pause, stop or move the timer.

  -h, --help       this text
  -V, --version    the version
";
/// How tomat starts itself free of a terminal.
const DETACHED: &str = "--detached";

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let first = args.next();
    let result = match (first.as_deref(), args.next()) {
        (None, _) if on_a_terminal() => detach(),
        (None, _) => open(),
        (Some(DETACHED), None) => {
            // Fails only if it already leads a session, which is what's wanted.
            let _ = rustix::process::setsid();
            open()
        }
        (Some("-h" | "--help"), None) => {
            say(HELP);
            Ok(())
        }
        (Some("-V" | "--version"), None) => {
            say(&format!("tomat {}\n", env!("CARGO_PKG_VERSION")));
            Ok(())
        }
        _ => {
            complain("tomat takes no options. See tomat --help");
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            log::error("app.failed", &format!("{error:#}"));
            complain(&format!("tomat: {error:#}"));
            ExitCode::FAILURE
        }
    }
}

fn on_a_terminal() -> bool {
    std::io::stdin().is_terminal() || std::io::stdout().is_terminal() || std::io::stderr().is_terminal()
}

fn detach() -> Result<()> {
    Command::new("/proc/self/exe")
        .arg0("tomat")
        .arg(DETACHED)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("can't start in the background")?;
    Ok(())
}

fn open() -> Result<()> {
    let paths = Paths::find()?;
    log::start(log::Process::App, &paths.logs(), &paths.home);
    let token = std::env::var("XDG_ACTIVATION_TOKEN").ok();
    host::run(&paths, token.as_deref())
}

fn say(text: &str) {
    // Piped into `head`: a closed pipe is fine.
    let _ = std::io::Write::write_all(&mut std::io::stdout(), text.as_bytes());
}

fn complain(text: &str) {
    let _ = std::io::Write::write_all(&mut std::io::stderr(), format!("{text}\n").as_bytes());
}
