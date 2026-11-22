//! The glass: a small frosted timer that floats above every window.
//!
//! Clicks pass through it. While it is movable it can be dragged, and it
//! remembers where it was left. It never takes keyboard focus. It follows KDE
//! between smoked (dark) and light, live.

mod buffers;
mod digits;
mod paint;
mod palette;
mod place;
mod protocols;
mod scheme;
mod shape;
mod surface;
mod watch;

use std::io::ErrorKind;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use wayland_client::backend::WaylandError;
use wayland_client::globals::registry_queue_init;
use wayland_client::{Connection, EventQueue};

use surface::State;

/// Which half of a round is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// Work time, in orange.
    Focus,
    /// Rest time, in green.
    Break,
}

/// What the glass shows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Showing {
    /// Focus or break.
    pub phase: Phase,
    /// A paused timer is yellow.
    pub paused: bool,
    /// The time left, shown rounded up to the second.
    pub left: Duration,
    /// How much of the phase is done, from 0 to 1.
    pub progress: f32,
}

/// The glass on screen. Dropping it takes it off the screen.
pub struct Glass {
    queue: EventQueue<State>,
    state: State,
    watch: watch::Watch,
    config: PathBuf,
}

impl Glass {
    /// Puts the glass on screen.
    ///
    /// # Errors
    /// When there is no Wayland session, or it has no layer shell.
    pub fn open(showing: Showing) -> Result<Self> {
        let connection = Connection::connect_to_env().context("no Wayland session")?;
        let (globals, mut queue) =
            registry_queue_init::<State>(&connection).context("can't read the Wayland globals")?;
        let config = dirs::config_dir().context("no config folder")?;
        // Watch first, so a change made while reading is still seen.
        let watch = watch::Watch::new(&config);
        let look = scheme::current(&config);
        let file = dirs::state_dir().map(|folder| folder.join("tomat").join("glass-position"));
        let mut state = State::new(&globals, &queue.handle(), showing, look, file)?;
        // Twice: monitors and the blur capability arrive, then each monitor's name and size.
        for _ in 0..2 {
            queue
                .roundtrip(&mut state)
                .context("the display didn't answer")?;
        }
        state.update()?;
        queue
            .roundtrip(&mut state)
            .context("the display refused the glass")?;
        state.update()?;
        Ok(Self {
            queue,
            state,
            watch,
            config,
        })
    }

    /// Shows a new time or phase. It only redraws when what you see changes.
    pub fn show(&mut self, showing: Showing) {
        self.state.showing = showing;
    }

    /// Movable, it can be dragged. Otherwise every click passes through it.
    pub fn set_movable(&mut self, movable: bool) {
        self.state.movable = movable;
    }

    /// Handles the display for up to `timeout`, or until something happens.
    ///
    /// # Errors
    /// When the Wayland connection is lost.
    pub fn wait(&mut self, timeout: Duration) -> Result<()> {
        self.queue.dispatch_pending(&mut self.state)?;
        self.state.update()?;
        // A full socket isn't an error: poll below waits for room.
        let blocked = match self.queue.flush() {
            Ok(()) => false,
            Err(WaylandError::Io(error)) if error.kind() == ErrorKind::WouldBlock => true,
            Err(error) => return Err(error).context("lost the display"),
        };
        let Some(guard) = self.queue.prepare_read() else {
            return Ok(());
        };

        let timeout = self
            .state
            .due(Instant::now())
            .map_or(timeout, |due| due.min(timeout));
        let timeout = Timespec {
            tv_sec: i64::try_from(timeout.as_secs()).unwrap_or(i64::MAX),
            tv_nsec: timeout.subsec_nanos().into(),
        };
        let display = guard.connection_fd();
        let wanted = if blocked {
            PollFlags::IN | PollFlags::OUT
        } else {
            PollFlags::IN
        };
        let mut fds = vec![PollFd::from_borrowed_fd(display, wanted)];
        if let Some(watched) = self.watch.fd() {
            fds.push(PollFd::from_borrowed_fd(watched, PollFlags::IN));
        }
        match poll(&mut fds, Some(&timeout)) {
            // A signal: the caller decides what it means.
            Ok(_) | Err(rustix::io::Errno::INTR) => {}
            Err(error) => return Err(error).context("can't wait for the display"),
        }
        let readable = fds[0]
            .revents()
            .intersects(PollFlags::IN | PollFlags::HUP | PollFlags::ERR);
        let theme_moved = fds
            .get(1)
            .is_some_and(|fd| fd.revents().contains(PollFlags::IN));
        drop(fds);

        if readable {
            match guard.read() {
                Ok(_) => {}
                Err(WaylandError::Io(error)) if error.kind() == ErrorKind::WouldBlock => {}
                Err(error) => return Err(error).context("lost the display"),
            }
        } else {
            drop(guard);
        }
        if theme_moved && self.watch.changed() {
            self.state.look = scheme::current(&self.config);
        }
        self.queue.dispatch_pending(&mut self.state)?;
        self.state.update()?;
        match self.queue.flush() {
            Ok(()) => Ok(()),
            Err(WaylandError::Io(error)) if error.kind() == ErrorKind::WouldBlock => Ok(()),
            Err(error) => Err(error).context("lost the display"),
        }
    }
}
