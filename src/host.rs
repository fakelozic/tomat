//! The app: owns the timer, the glass and the window, and is gone when none is left.
//!
//! One thread that sleeps in poll. The glass, the window, the door and the signals wake it.

mod door;
mod window;

use std::os::fd::{AsFd, BorrowedFd};
use std::os::unix::net::UnixStream;
use std::time::{Duration, Instant, SystemTime};

use anyhow::{Context, Result};
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use signal_hook::consts::{SIGHUP, SIGINT, SIGQUIT, SIGTERM};

use crate::glass::{Glass, Showing};
use crate::link::{Ask, View};
use crate::paths::Paths;
use crate::plan::Slots;
use crate::timer::Timer;
use crate::{log, private_file};
use door::{Door, Opened};
use window::{Closing, Leaving, Window};

/// How often to try again after the display took the glass away or refused it.
const RETRY: Duration = Duration::from_secs(2);
/// With no display for this long and no window, the session is over.
const NO_DISPLAY: Duration = Duration::from_secs(60);
/// Opening again sooner than this is the same click, so the window stays as it is.
const SAME_CLICK: Duration = Duration::from_millis(700);
/// How often to look at a window that is closing.
const REAP: Duration = Duration::from_millis(50);
/// With nothing to count down, it still looks around this often.
const IDLE: Duration = Duration::from_secs(60);
/// No honest saved file is larger.
const MOST_FILE: u64 = 256;

/// Opens tomat: shows the running one's window, or becomes the app until there's nothing left to do.
///
/// # Errors
/// When it can't create its folders, take its lock or watch for signals.
pub fn run(paths: &Paths, token: Option<&str>) -> Result<()> {
    paths.create().context("can't create tomat's folders")?;
    let token = token.and_then(clean_token);
    let door = match Door::open(paths, token.as_deref())? {
        Opened::Here(door) => door,
        Opened::Elsewhere => return Ok(()),
    };
    let (stop, wake) = UnixStream::pair().context("no socket")?;
    for signal in [SIGTERM, SIGINT, SIGHUP, SIGQUIT] {
        signal_hook::low_level::pipe::register(signal, wake.try_clone()?).context("can't watch for signals")?;
    }
    stop.set_nonblocking(true)?;
    let mut host = Host::new(paths, door, stop);
    host.open_window(token.as_deref());
    host.serve();
    Ok(())
}

/// An activation token, if it looks like one.
pub(super) fn clean_token(token: &str) -> Option<String> {
    let fine = (1..=256).contains(&token.len()) && token.bytes().all(|byte| byte.is_ascii_graphic());
    fine.then(|| token.to_owned())
}

struct Host<'a> {
    paths: &'a Paths,
    door: Door,
    stop: UnixStream,
    slots: Slots,
    timer: Option<Timer>,
    glass: Option<Glass>,
    /// Not before then, after the display failed.
    retry: Option<Instant>,
    failing_since: Option<Instant>,
    window: Option<Window>,
    leaving: Vec<Leaving>,
}

impl<'a> Host<'a> {
    fn new(paths: &'a Paths, door: Door, stop: UnixStream) -> Self {
        let slots = match private_file::read(&paths.slots(), MOST_FILE) {
            Ok(bytes) => Slots::parse(&String::from_utf8_lossy(&bytes)).unwrap_or_else(|| {
                log::warn("slots.reset", "the saved slots can't be read");
                Slots::default()
            }),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Slots::default(),
            Err(error) => {
                log::warn("slots.reset", &error.to_string());
                Slots::default()
            }
        };
        let timer = private_file::read(&paths.timer(), MOST_FILE)
            .ok()
            .and_then(|bytes| Timer::restore(&String::from_utf8_lossy(&bytes), SystemTime::now()));
        let host = Self {
            paths,
            door,
            stop,
            slots,
            timer,
            glass: None,
            retry: None,
            failing_since: None,
            window: None,
            leaving: Vec::new(),
        };
        host.save_timer();
        host
    }

    fn serve(&mut self) {
        loop {
            if let Some(timer) = self.timer.as_mut()
                && timer.tick(SystemTime::now())
            {
                self.save_timer();
            }
            self.update_glass();
            let view = self.view();
            if let Some(window) = self.window.as_mut() {
                window.show(view);
            }
            self.leaving.retain_mut(|leaving| !leaving.check());

            let gave_up = self.failing_since.is_some_and(|since| since.elapsed() >= NO_DISPLAY);
            if self.window.is_none() && (self.timer.is_none() || gave_up) {
                // Someone may have opened it this very moment.
                match self.door.answer() {
                    Some(knock) => {
                        self.failing_since = None;
                        self.open_window(knock.token.as_deref());
                        // The window may not start. Then the next pass leaves.
                        if self.window.is_some() {
                            continue;
                        }
                    }
                    None if gave_up => log::error("display.gone", "no display for a minute: closing"),
                    None => {}
                }
                break;
            }

            self.sleep();
            if self.told_to_stop() {
                break;
            }
            if let Some(knock) = self.door.answer() {
                self.open_window(knock.token.as_deref());
            }
            self.hear_window();
        }
        if let Some(timer) = self.timer.as_mut() {
            timer.tick(SystemTime::now());
            self.save_timer();
        }
        if let Some(window) = self.window.take() {
            self.leaving.push(window.close(Closing::Ours));
        }
        self.glass = None;
        // Dropping waits for each, up to its deadline.
        self.leaving.clear();
    }

    fn view(&self) -> View {
        self.timer.as_ref().map_or(View::Slots(self.slots), |timer| {
            let left = timer.left();
            let seconds = left.as_secs() + u64::from(left.subsec_nanos() > 0);
            View::Running {
                phase: timer.phase(),
                paused: timer.paused(),
                seconds: u32::try_from(seconds).unwrap_or(u32::MAX),
                next: timer.next_minutes(),
            }
        })
    }

    fn update_glass(&mut self) {
        let Some(timer) = self.timer.as_ref() else {
            self.glass = None;
            (self.retry, self.failing_since) = (None, None);
            return;
        };
        let showing = Showing { phase: timer.phase(), paused: timer.paused(), left: timer.left() };
        if self.glass.is_none() && self.retry.is_none_or(|retry| Instant::now() >= retry) {
            match Glass::open(self.paths, showing) {
                Ok(glass) => {
                    self.glass = Some(glass);
                    (self.retry, self.failing_since) = (None, None);
                }
                Err(error) => self.glass_failed(&error),
            }
        }
        if let Some(glass) = self.glass.as_mut() {
            glass.show(showing);
            // Open tomat to move it. Otherwise every click passes through.
            glass.set_movable(self.window.is_some());
        }
    }

    fn glass_failed(&mut self, error: &anyhow::Error) {
        log::warn("glass.failed", &format!("{error:#}"));
        self.glass = None;
        self.retry = Some(Instant::now() + RETRY);
        self.failing_since.get_or_insert_with(Instant::now);
    }

    fn timeout(&self) -> Duration {
        let mut wait = IDLE;
        if let Some(timer) = &self.timer {
            if let Some(next) = timer.next_change() {
                // A hair late, so the new second is already showing.
                wait = wait.min(next + Duration::from_millis(1));
            }
            if let Some(retry) = self.retry {
                wait = wait.min(retry.saturating_duration_since(Instant::now()));
            }
        }
        if self.leaving.is_empty() { wait } else { wait.min(REAP) }
    }

    fn sleep(&mut self) {
        let timeout = self.timeout();
        let mut fds: Vec<BorrowedFd<'_>> = vec![self.stop.as_fd(), self.door.fd()];
        if let Some(window) = &self.window {
            fds.push(window.fd());
        }
        let result = match self.glass.as_mut() {
            Some(glass) => glass.wait(timeout, &fds),
            None => wait(timeout, &fds),
        };
        if let Err(error) = result {
            if self.glass.is_some() {
                self.glass_failed(&error);
            } else {
                log::error("app.wait_failed", &format!("{error:#}"));
                std::thread::sleep(REAP);
            }
        }
    }

    fn told_to_stop(&self) -> bool {
        let mut bytes = [0; 8];
        std::io::Read::read(&mut &self.stop, &mut bytes).is_ok_and(|read| read > 0)
    }

    fn open_window(&mut self, token: Option<&str>) {
        if self.window.as_ref().is_some_and(|window| window.since.elapsed() < SAME_CLICK) {
            return;
        }
        // Wayland can't raise another program's window, so a new one takes its place.
        match Window::start(token, self.view()) {
            Ok(window) => {
                log::note("window.opened", "");
                if let Some(old) = self.window.replace(window) {
                    self.leaving.push(old.close(Closing::Ours));
                }
            }
            Err(error) => log::error("window.start_failed", &format!("{error:#}")),
        }
    }

    fn hear_window(&mut self) {
        let Some(window) = self.window.as_mut() else { return };
        let news = window.read();
        let mut close = news.gone.then_some(Closing::Theirs);
        for ask in news.asks {
            let now = SystemTime::now();
            match ask {
                Ask::Save(slots) => self.save_slots(slots),
                Ask::Start(slot) if self.timer.is_none() => {
                    self.timer = Some(Timer::start(slot, now));
                    self.save_timer();
                    close = Some(Closing::Ours);
                }
                Ask::Start(_) => {}
                Ask::Pause | Ask::Resume => {
                    if let Some(timer) = self.timer.as_mut() {
                        timer.set_paused(ask == Ask::Pause, now);
                        self.save_timer();
                    }
                }
                Ask::Stop => {
                    self.timer = None;
                    self.save_timer();
                }
            }
        }
        if let Some(why) = close
            && let Some(window) = self.window.take()
        {
            self.leaving.push(window.close(why));
        }
    }

    fn save_slots(&mut self, slots: Slots) {
        if slots == self.slots {
            return;
        }
        self.slots = slots;
        if let Err(error) = private_file::replace(&self.paths.slots(), format!("{slots}\n").as_bytes()) {
            log::warn("slots.save_failed", &error.to_string());
        }
    }

    /// Only the running timer is kept, and only until Stop.
    fn save_timer(&self) {
        let path = self.paths.timer();
        let result = match &self.timer {
            Some(timer) => private_file::replace(&path, format!("{}\n", timer.save()).as_bytes()),
            None => match std::fs::remove_file(&path) {
                Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error),
                _ => Ok(()),
            },
        };
        if let Err(error) = result {
            log::warn("timer.save_failed", &error.to_string());
        }
    }
}

/// Sleeps until one of `fds` has news, or `timeout`.
fn wait(timeout: Duration, fds: &[BorrowedFd<'_>]) -> Result<()> {
    let mut fds: Vec<PollFd<'_>> = fds.iter().map(|&fd| PollFd::from_borrowed_fd(fd, PollFlags::IN)).collect();
    let timeout = Timespec {
        tv_sec: i64::try_from(timeout.as_secs()).unwrap_or(i64::MAX),
        tv_nsec: timeout.subsec_nanos().into(),
    };
    match poll(&mut fds, Some(&timeout)) {
        Ok(_) | Err(rustix::io::Errno::INTR) => Ok(()),
        Err(error) => Err(error).context("can't wait"),
    }
}
