//! Stops one repeated event from filling the day's log and pushing out the line that explains a crash.
//! A display that keeps failing retries, so this matters. Pure.

/// Lines of one event allowed in one window.
const BURST: u32 = 30;
/// The window, in ms (one minute).
const WINDOW_MS: u64 = 60_000;
/// Event names tracked. There are fewer than this in the code.
const EVENTS: usize = 96;

#[derive(Debug)]
struct Seen {
    event: String,
    since: u64,
    count: u32,
}

/// Counts recent lines by event.
#[derive(Debug)]
pub(super) struct Limit {
    seen: Vec<Seen>,
}

impl Limit {
    /// The empty limiter.
    pub(super) const fn new() -> Self {
        Self { seen: Vec::new() }
    }

    /// `None` to drop this line. `Some(n)` to write it, after saying that `n` like it were dropped before it.
    pub(super) fn check(&mut self, event: &str, now_ms: u64) -> Option<u32> {
        let Some(seen) = self.seen.iter_mut().find(|seen| seen.event == event) else {
            if self.seen.len() < EVENTS {
                self.seen.push(Seen { event: event.to_owned(), since: now_ms, count: 1 });
            }
            return Some(0);
        };
        if now_ms.saturating_sub(seen.since) >= WINDOW_MS || now_ms < seen.since {
            let dropped = seen.count.saturating_sub(BURST);
            (seen.since, seen.count) = (now_ms, 1);
            return Some(dropped);
        }
        seen.count = seen.count.saturating_add(1);
        (seen.count <= BURST).then_some(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_flood_of_one_event_leaves_room_for_the_others() {
        let mut limit = Limit::new();
        let written = (0..10_000).filter(|&at| limit.check("glass.failed", at).is_some()).count();
        assert_eq!(written, 30);
        assert_eq!(limit.check("window.crashed", 10_000), Some(0));
        assert_eq!(limit.check("glass.failed", 60_000), Some(10_000 - BURST), "says how many it dropped");
        assert_eq!(limit.check("glass.failed", 60_001), Some(0));
    }
}
