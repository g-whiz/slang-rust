//! The retry of a frame for which the surface of the example
//! `windowed-renderer` gave no texture, which the tests of the examples also
//! compile.

use std::time::{Duration, Instant};

/// How long the example waits before it tries a frame again.
pub(crate) const REDRAW_RETRY: Duration = Duration::from_millis(100);

/// The time at which the example tries a frame again, if a retry is
/// scheduled.
#[derive(Default)]
pub(crate) struct Retry {
    at: Option<Instant>,
}

impl Retry {
    /// Schedules a retry [`REDRAW_RETRY`] after `now`.
    pub(crate) fn schedule(&mut self, now: Instant) {
        self.at = Some(now + REDRAW_RETRY);
    }

    /// Returns whether a scheduled retry is due at `now`, and clears it
    /// when it is.
    pub(crate) fn take_due(&mut self, now: Instant) -> bool {
        let due = self.at.is_some_and(|at| at <= now);
        if due {
            self.at = None;
        }
        due
    }

    /// Returns the time at which the event loop wakes, which is the earlier
    /// of the scheduled retry and the `deadline` of the watchdog, or `None`
    /// when neither is set.
    pub(crate) fn wake(&self, deadline: Option<Instant>) -> Option<Instant> {
        self.at.into_iter().chain(deadline).min()
    }
}
