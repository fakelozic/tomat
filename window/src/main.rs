//! tomat's window. It decides nothing: it shows what the app says and tells it what was pressed.
//! The app gives it a socket as stdin, and it leaves when that closes.

mod fields;
mod ui;

use std::cell::Cell;
use std::io::{BufRead, BufReader, IsTerminal, Read, Write};
use std::os::fd::AsFd;
use std::os::unix::net::UnixStream;
use std::process::ExitCode;
use std::rc::Rc;
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use rustix::event::{PollFd, PollFlags, poll};
use slint::{ComponentHandle, SharedString};
use tomat::link::{Ask, MOST_LINE, View};
use tomat::plan::Slots;
use tomat::theme::{Look, Watch};
use tomat::timer::Phase;
use tomat::{log, paths::Paths, theme};

use ui::{AppWindow, Format, Theme};

/// How long the first frame may wait to know what to show.
const FIRST_VIEW: Duration = Duration::from_secs(1);
/// How long a press may wait for a busy app.
const SEND: Duration = Duration::from_secs(1);

thread_local! {
    /// What the app last said the slots were.
    static SAVED: Cell<Option<Slots>> = const { Cell::new(None) };
}

fn main() -> ExitCode {
    let result = Paths::find().and_then(|paths| {
        log::start(log::Process::Window, &paths.logs(), &paths.home);
        run(&paths)
    });
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            log::error("window.failed", &format!("{error:#}"));
            ExitCode::FAILURE
        }
    }
}

fn run(paths: &Paths) -> Result<()> {
    let socket = socket_on_stdin()?;
    socket.set_write_timeout(Some(SEND))?;
    let toolkit = |error: &dyn std::fmt::Display| anyhow!("the window toolkit failed: {error}");
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name("software".into())
        .select()
        .map_err(|e| toolkit(&e))?;
    slint::set_xdg_app_id("tomat").map_err(|e| toolkit(&e))?;
    let app = AppWindow::new().map_err(|e| toolkit(&e))?;

    let format = app.global::<Format<'_>>();
    format.on_digits(|text| fields::digits(&text).into());
    format.on_valid(|text| fields::valid(&text));

    // Watch first, so a change made while reading is still seen.
    let watch = Watch::new(&paths.config);
    app.global::<Theme<'_>>().set_dark(theme::current(&paths.config) == Look::Smoked);

    socket.set_read_timeout(Some(FIRST_VIEW))?;
    let mut reader = BufReader::new(socket.try_clone()?);
    let first = read_view(&mut reader).context("the app didn't say what to show")?;
    socket.set_read_timeout(None)?;
    show(&app, first);
    wire(&app, &Rc::new(socket));

    let weak = app.as_weak();
    std::thread::Builder::new().name("link".into()).spawn(move || {
        while let Some(view) = read_view(&mut reader) {
            if weak.upgrade_in_event_loop(move |app| show(&app, view)).is_err() {
                return;
            }
        }
        // The app closed it, or is gone: a window with no timer behind it mustn't linger.
        if let Err(error) = slint::quit_event_loop() {
            log::warn("window.quit_failed", &error.to_string());
        }
    })?;

    let (weak, config) = (app.as_weak(), paths.config.clone());
    std::thread::Builder::new().name("theme".into()).spawn(move || {
        let Some(fd) = watch.fd() else { return };
        loop {
            let mut fds = [PollFd::from_borrowed_fd(fd, PollFlags::IN)];
            match poll(&mut fds, None) {
                Ok(_) => {}
                Err(rustix::io::Errno::INTR) => continue,
                Err(_) => return,
            }
            if watch.changed() {
                let dark = theme::current(&config) == Look::Smoked;
                let set = weak.upgrade_in_event_loop(move |app| app.global::<Theme<'_>>().set_dark(dark));
                if set.is_err() {
                    return;
                }
            }
        }
    })?;

    app.run().map_err(|e| toolkit(&e))
}

fn socket_on_stdin() -> Result<UnixStream> {
    const NOT_OURS: &str = "tomat-window is started by tomat: run tomat instead";
    let stdin = std::io::stdin();
    anyhow::ensure!(!stdin.is_terminal(), NOT_OURS);
    let socket = UnixStream::from(stdin.as_fd().try_clone_to_owned().context(NOT_OURS)?);
    socket.peer_addr().context(NOT_OURS)?;
    Ok(socket)
}

fn read_view(reader: &mut BufReader<UnixStream>) -> Option<View> {
    let mut line = String::new();
    let read = reader.by_ref().take(MOST_LINE as u64 + 1).read_line(&mut line).ok()?;
    if read == 0 || !line.ends_with('\n') {
        return None;
    }
    let view = View::decode(line.trim_end());
    if view.is_none() {
        log::warn("window.bad_message", "a message it can't read");
    }
    view
}

fn show(app: &AppWindow, view: View) {
    match view {
        View::Slots(slots) => {
            let first = SAVED.replace(Some(slots)).is_none();
            // The app echoes every edit. Writing that back would move the cursor mid-typing.
            if first || app.get_running() {
                fill(app, slots);
            }
            app.set_running(false);
        }
        View::Running { phase, paused, seconds, next } => {
            app.set_resting(phase == Phase::Break);
            app.set_paused(paused);
            app.set_clock(format!("{:02}:{:02}", seconds / 60, seconds % 60).into());
            let next = match next {
                Some(minutes) => format!("then {minutes} min break"),
                None => "then stop".to_owned(),
            };
            app.set_next(next.into());
            app.set_running(true);
        }
    }
}

fn fill(app: &AppWindow, slots: Slots) {
    let text = |minutes: u16| SharedString::from(minutes.to_string());
    let [a, b, c] = slots.0;
    app.set_focus_1(text(a.focus()));
    app.set_break_1(text(a.rest()));
    app.set_focus_2(text(b.focus()));
    app.set_break_2(text(b.rest()));
    app.set_focus_3(text(c.focus()));
    app.set_break_3(text(c.rest()));
}

fn typed(app: &AppWindow) -> [[String; 2]; 3] {
    [
        [app.get_focus_1().into(), app.get_break_1().into()],
        [app.get_focus_2().into(), app.get_break_2().into()],
        [app.get_focus_3().into(), app.get_break_3().into()],
    ]
}

fn send(socket: &UnixStream, ask: Ask) {
    if let Err(error) = (&mut &*socket).write_all(ask.encode().as_bytes()) {
        log::warn("window.send_failed", &error.to_string());
    }
}

fn wire(app: &AppWindow, socket: &Rc<UnixStream>) {
    let (weak, out) = (app.as_weak(), Rc::clone(socket));
    app.on_edited(move || {
        if let Some(app) = weak.upgrade() {
            let saved = SAVED.get().unwrap_or_default();
            send(&out, Ask::Save(fields::merge(saved, &typed(&app))));
        }
    });

    let (weak, out) = (app.as_weak(), Rc::clone(socket));
    app.on_start(move |index| {
        let Some(app) = weak.upgrade() else { return };
        let rows = typed(&app);
        let row = usize::try_from(index).ok().and_then(|index| rows.get(index));
        if let Some(slot) = row.and_then(|[focus, rest]| fields::slot(focus, rest)) {
            send(&out, Ask::Start(slot));
        }
    });

    let out = Rc::clone(socket);
    app.on_set_paused(move |paused| send(&out, if paused { Ask::Pause } else { Ask::Resume }));
    let out = Rc::clone(socket);
    app.on_stop(move || send(&out, Ask::Stop));

    let weak = app.as_weak();
    app.on_dismiss(move || {
        if let Some(app) = weak.upgrade()
            && let Err(error) = app.hide()
        {
            log::warn("window.hide_failed", &error.to_string());
        }
    });
}
