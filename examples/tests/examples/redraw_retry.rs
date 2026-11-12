use std::time::{Duration, Instant};

#[path = "../../src/bin/windowed-renderer/retry.rs"]
mod retry;

use retry::{REDRAW_RETRY, Retry};

#[test]
fn without_a_retry_nothing_is_due_and_the_watchdog_sets_the_wake() {
    let now = Instant::now();
    let mut retry = Retry::default();
    assert!(!retry.take_due(now + REDRAW_RETRY * 10));
    assert_eq!(retry.wake(None), None);
    let deadline = now + Duration::from_secs(10);
    assert_eq!(retry.wake(Some(deadline)), Some(deadline));
}

#[test]
fn a_retry_is_due_after_its_delay_and_then_cleared() {
    let now = Instant::now();
    let mut retry = Retry::default();
    retry.schedule(now);
    assert!(!retry.take_due(now + Duration::from_millis(99)));
    assert_eq!(retry.wake(None), Some(now + REDRAW_RETRY));
    assert!(retry.take_due(now + REDRAW_RETRY));
    assert!(!retry.take_due(now + REDRAW_RETRY * 2));
    assert_eq!(retry.wake(None), None);
}

#[test]
fn the_wake_is_the_earlier_of_the_retry_and_the_watchdog() {
    let now = Instant::now();
    let mut retry = Retry::default();
    retry.schedule(now);
    let later = now + Duration::from_secs(10);
    assert_eq!(retry.wake(Some(later)), Some(now + REDRAW_RETRY));
    let earlier = now + REDRAW_RETRY / 2;
    assert_eq!(retry.wake(Some(earlier)), Some(earlier));
}
