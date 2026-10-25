//! tomat: a plain pomodoro timer for the terminal.
//!
//! Entry point and wiring: reads the flags, sets up an inline terminal, runs
//! the event loop and carries out the effects the app asks for. Decisions
//! live in `pomodoro`, drawing in `tui`, and outside effects in `system`.

mod cli;
mod pomodoro;
mod system;
mod tui;

use std::io::{self, Stdout, Write};
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime};

use anyhow::{Context, Result};
use chrono::Local;
use ratatui::backend::CrosstermBackend;
use ratatui::crossterm::cursor::{MoveTo, Show};
use ratatui::crossterm::event::{self, Event, KeyEventKind};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{self, Clear, ClearType};
use ratatui::{Terminal, TerminalOptions, Viewport};

use crate::cli::{Command, Config};
use crate::pomodoro::{App, Effect, Plan};
use crate::system::{History, Notifier};

/// The terminal tomat draws on: an inline block under the prompt.
type Inline = Terminal<CrosstermBackend<Stdout>>;

/// Longest wait between wakes when nothing on screen is counting down. Even
/// then the loop wakes now and again, to notice a stop signal and to collect
/// finished `notify-send` processes; a wake that changes nothing redraws
/// nothing.
const IDLE_WAKE: Duration = Duration::from_secs(1);

fn main() -> ExitCode {
    let config = match cli::parse(std::env::args_os().skip(1)) {
        Ok(Command::Run(config)) => config,
        // Write errors are ignored: `tomat --help | head` must not panic.
        Ok(Command::Help) => {
            let _ = io::stdout().lock().write_all(cli::HELP.as_bytes());
            return ExitCode::SUCCESS;
        }
        Ok(Command::Version) => {
            let _ = writeln!(io::stdout().lock(), "tomat {}", env!("CARGO_PKG_VERSION"));
            return ExitCode::SUCCESS;
        }
        Err(e) => {
            let _ = writeln!(io::stderr().lock(), "tomat: {e}\nTry 'tomat --help'.");
            return ExitCode::from(2);
        }
    };
    match run(&config) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            let _ = writeln!(io::stderr().lock(), "tomat: {e:#}");
            ExitCode::FAILURE
        }
    }
}

/// Builds the app from `config`, runs it in an inline terminal, and leaves
/// the prompt clean afterwards.
fn run(config: &Config) -> Result<()> {
    let history = History::default_location();
    let mut plans = Plan::presets();
    // A custom plan that matches a preset starts that preset rather than
    // appearing twice in the menu.
    let start = config.custom.map(|custom| {
        plans
            .iter()
            .position(|&plan| plan == custom)
            .unwrap_or_else(|| {
                plans.insert(0, custom);
                0
            })
    });
    let (today, notice) = match history
        .as_ref()
        .map(|h| h.count_on(Local::now().date_naive()))
    {
        Some(Ok(count)) => (count, None),
        Some(Err(e)) => (0, Some(format!("can't read history: {e:#}"))),
        None => (
            0,
            Some("no data directory; sessions won't be saved".to_owned()),
        ),
    };
    let mut app = App::new(plans, config.auto_start, today);
    if let Some(notice) = notice {
        app.set_notice(notice);
    }
    let mut notifier = Notifier::new(config.notify);
    let stop = stop_on_signals()?;

    let mut terminal = open_terminal().context("no interactive terminal to draw in")?;
    // Only now, so time spent setting up the terminal isn't counted.
    if let Some(index) = start {
        app.start(index, SystemTime::now());
    }
    let result = event_loop(
        &mut terminal,
        &mut app,
        history.as_ref(),
        &mut notifier,
        &stop,
    );
    let closed = close_terminal(&mut terminal);
    result.and(closed.map_err(Into::into))
}

/// Puts the terminal in raw mode with a fixed block under the prompt, and
/// makes sure a panic later gives the terminal back.
///
/// # Errors
/// Fails without a usable terminal (no TTY, or one that won't report the
/// cursor position), leaving the terminal as it was.
fn open_terminal() -> io::Result<Inline> {
    terminal::enable_raw_mode()?;
    let options = TerminalOptions {
        viewport: Viewport::Inline(tui::HEIGHT),
    };
    match Terminal::with_options(CrosstermBackend::new(io::stdout()), options) {
        Ok(inline) => {
            restore_on_panic();
            Ok(inline)
        }
        Err(e) => {
            restore();
            Err(e)
        }
    }
}

/// Wipes the block and puts the cursor where tomat started, so the next
/// prompt appears in its place instead of after a stale timer.
fn close_terminal(inline: &mut Inline) -> io::Result<()> {
    let top = inline.get_frame().area().y;
    let cleared = execute!(
        inline.backend_mut(),
        MoveTo(0, top),
        Clear(ClearType::FromCursorDown)
    );
    restore();
    cleared
}

/// Raw mode off and the cursor back. Not ratatui's `restore`: that also
/// leaves the alternate screen, which tomat never entered, and some
/// terminals (tmux, xterm) then move the cursor to where an earlier
/// full-screen program left it.
fn restore() {
    let _ = terminal::disable_raw_mode();
    let _ = execute!(io::stdout(), Show);
}

/// Restores the terminal before the panic message prints. Release builds
/// abort on panic, which skips destructors, so this hook is the only
/// cleanup a panic gets.
fn restore_on_panic() {
    let report = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore();
        report(info);
    }));
}

/// A flag that turns on when tomat is asked to stop (`kill`, a closed
/// terminal, Ctrl-\), so the loop can exit cleanly and give the terminal
/// back. A second signal while stopping ends the process at once.
fn stop_on_signals() -> io::Result<Arc<AtomicBool>> {
    use signal_hook::consts::{SIGHUP, SIGINT, SIGQUIT, SIGTERM};
    use signal_hook::flag;

    let stop = Arc::new(AtomicBool::new(false));
    for signal in [SIGTERM, SIGHUP, SIGINT, SIGQUIT] {
        flag::register_conditional_shutdown(signal, 1, Arc::clone(&stop))?;
        flag::register(signal, Arc::clone(&stop))?;
    }
    Ok(stop)
}

/// Draws, sleeps until the next change on screen or a key press, advances
/// the clock and carries out effects, until the app or a signal says stop.
fn event_loop(
    inline: &mut Inline,
    app: &mut App,
    history: Option<&History>,
    notifier: &mut Notifier,
    stop: &AtomicBool,
) -> Result<()> {
    while !stop.load(Ordering::Relaxed) {
        inline.draw(|frame| tui::draw(frame, app, SystemTime::now()))?;

        let wait = app
            .next_change(SystemTime::now(), tui::ANIMATION_STEP)
            .map_or(IDLE_WAKE, |change| change.min(IDLE_WAKE));
        let key = if event::poll(wait)?
            && let Event::Key(key) = event::read()?
            // Some terminals also report key releases; act on presses only.
            && key.kind == KeyEventKind::Press
        {
            tui::action_for(key)
        } else {
            None
        };

        // The clock first: a phase that ran out before the key arrived is
        // finished (recorded, announced) before the key acts on the next.
        let now = SystemTime::now();
        let mut effects = app.tick(now);
        if let Some(action) = key {
            effects.extend(app.handle(action, now));
        }
        notifier.reap();

        let mut quit = false;
        for effect in effects {
            match effect {
                // Last, so a finish in the same pass is still recorded.
                Effect::Quit => quit = true,
                Effect::Notify { title, body } => notifier.phase_done(&title, &body),
                Effect::RecordFocus(length) => {
                    if let Some(history) = history
                        && let Err(e) = history.record(Local::now(), length)
                    {
                        app.set_notice(format!("session not saved: {e:#}"));
                    }
                }
            }
        }
        if quit {
            break;
        }
    }
    Ok(())
}
