//! Step 1's preview: the glass with a pretend timer, to try on a real screen.
//!
//! It can be dragged for the first 30 seconds, then clicks pass
//! through it. It takes itself away after 10 minutes, or on Ctrl-C.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use anyhow::Result;
use lexopt::prelude::*;
use tomat::glass::{Glass, Phase, Showing};

/// A pretend round: a focus, its break, then a pause, so every colour gets seen.
fn pretend(elapsed: Duration) -> Showing {
    let at = elapsed.as_secs() % 50;
    let (phase, paused, start): (_, _, u16) = match at {
        0..20 => (Phase::Focus, false, 1421),
        20..40 => (Phase::Break, false, 252),
        _ => (Phase::Focus, true, 1421),
    };
    let left: u16 = if paused {
        start
    } else {
        start - u16::try_from(at % 20).unwrap_or_default()
    };
    Showing {
        phase,
        paused,
        left: Duration::from_secs(left.into()),
    }
}

fn main() -> Result<()> {
    let (mut movable_for, mut run_for) = (30_u64, 600_u64);
    let mut parser = lexopt::Parser::from_env();
    while let Some(arg) = parser.next()? {
        match arg {
            Long("movable-for") => movable_for = parser.value()?.parse()?,
            Long("run-for") => run_for = parser.value()?.parse()?,
            _ => return Err(arg.unexpected().into()),
        }
    }
    let stop = Arc::new(AtomicBool::new(false));
    for signal in [
        signal_hook::consts::SIGINT,
        signal_hook::consts::SIGTERM,
        signal_hook::consts::SIGHUP,
    ] {
        signal_hook::flag::register(signal, Arc::clone(&stop))?;
    }

    let started = Instant::now();
    let mut glass = Glass::open(pretend(Duration::ZERO))?;
    while !stop.load(Ordering::Relaxed) {
        let elapsed = started.elapsed();
        if elapsed.as_secs() >= run_for {
            break;
        }
        glass.set_movable(elapsed.as_secs() < movable_for);
        glass.show(pretend(elapsed));
        let next_second = 1000 - u64::from(elapsed.subsec_millis());
        glass.wait(Duration::from_millis(next_second))?;
    }
    Ok(())
}
