use std::ffi::OsString;
use std::path::Path;
use std::time::{Duration, Instant};

#[path = "../../src/shared/frames.rs"]
mod frames;

use frames::{Frames, OPTIONS, UNATTENDED_STALL, parse_args};

fn args(args: &[&str]) -> Vec<OsString> {
    args.iter().map(OsString::from).collect()
}

#[test]
fn parse_args_reads_the_file_and_the_frame_count() {
    let parsed = parse_args(args(&["--frames", "3", "shader.slang"]))
        .expect("the arguments are valid")
        .expect("the arguments ask for no help");
    assert_eq!(parsed.file, Path::new("shader.slang"));
    assert_eq!(parsed.frames, Some(3));

    let parsed = parse_args(args(&["shader.slang"]))
        .expect("the arguments are valid")
        .expect("the arguments ask for no help");
    assert_eq!(parsed.frames, None);
}

#[test]
fn parse_args_returns_none_for_help() {
    for flag in ["--help", "-h"] {
        assert!(
            parse_args(args(&["shader.slang", flag]))
                .expect("help is valid")
                .is_none()
        );
    }
}

#[test]
fn parse_args_rejects_zero_frames() {
    let error = parse_args(args(&["--frames", "0", "shader.slang"]))
        .map(|_| ())
        .expect_err("zero frames are rejected");
    assert_eq!(error.to_string(), "--frames needs a number above zero");
}

#[test]
fn the_options_describe_both_flags() {
    assert!(OPTIONS.contains("--frames N"), "{OPTIONS}");
    assert!(OPTIONS.contains("-h, --help"), "{OPTIONS}");
}

#[test]
fn frames_without_a_limit_run_on_and_are_not_watched() {
    let start = Instant::now();
    let mut frames = Frames::new(None, start);
    for _ in 0..3 {
        frames.drawn(start);
    }
    assert!(frames.running());
    let late = start + UNATTENDED_STALL * 3;
    assert!(frames.watch(late).expect("no limit, no stall").is_none());
    assert_eq!(frames.summary(), None);
}

#[test]
fn frames_stop_running_when_the_limit_is_drawn() {
    let start = Instant::now();
    let mut frames = Frames::new(Some(2), start);
    assert!(frames.running());
    frames.drawn(start);
    assert!(frames.running());
    frames.drawn(start);
    assert!(!frames.running());
    assert_eq!(frames.summary().as_deref(), Some("drew 2 frames"));
}

#[test]
fn the_watchdog_fails_a_run_that_draws_no_frame_for_ten_seconds() {
    assert_eq!(UNATTENDED_STALL, Duration::from_secs(10));
    let start = Instant::now();
    let mut frames = Frames::new(Some(5), start);
    let deadline = start + UNATTENDED_STALL;
    let just_before = start + Duration::from_millis(9_999);
    assert_eq!(
        frames.watch(just_before).expect("the bound is not reached"),
        Some(deadline)
    );
    frames.drawn(start + Duration::from_secs(1));
    assert_eq!(
        frames.watch(deadline).expect("a frame restarted the bound"),
        Some(deadline + Duration::from_secs(1))
    );
    let error = frames
        .watch(deadline + Duration::from_secs(1))
        .expect_err("the bound is reached");
    assert_eq!(
        error.to_string(),
        "drew no frame for 10 seconds after 1 frames with --frames set"
    );
}

#[test]
fn starting_restarts_the_watchdog() {
    let created = Instant::now();
    let mut frames = Frames::new(Some(5), created);
    let ready = created + Duration::from_secs(30);
    frames.start(ready);
    assert_eq!(
        frames
            .watch(ready)
            .expect("the bound restarts when the example is ready"),
        Some(ready + UNATTENDED_STALL)
    );
}
