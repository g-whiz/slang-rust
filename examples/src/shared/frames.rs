//! The command line and the frame count of the windowed examples, which run
//! unattended with `--frames N`, and which both binaries include with a
//! `#[path]` attribute.

use std::error::Error;
use std::ffi::OsString;
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// How long an unattended run may draw no frame before it fails.
pub(crate) const UNATTENDED_STALL: Duration = Duration::from_secs(10);

/// The options of the usage text that both windowed examples share.
pub(crate) const OPTIONS: &str = "
Options:
      --frames N  Close the window after N frames and print one line, or
                  fail when no frame is drawn for 10 seconds
  -h, --help      Print this help
";

/// The arguments of a windowed example.
pub(crate) struct Args {
    /// The Slang file to draw with.
    pub(crate) file: PathBuf,
    /// The number of frames after which the example exits, if any.
    pub(crate) frames: Option<u64>,
}

/// Parses the arguments after the program name, and returns `None` for
/// `-h` or `--help`.
pub(crate) fn parse_args(
    args: impl IntoIterator<Item = OsString>,
) -> Result<Option<Args>, Box<dyn Error>> {
    let mut file = None;
    let mut frames = None;
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        if arg == "-h" || arg == "--help" {
            return Ok(None);
        } else if arg == "--frames" {
            let count = args.next().ok_or("--frames needs a number")?;
            let count = count.to_str().and_then(|count| count.parse::<u64>().ok());
            let count = count.ok_or("--frames needs a whole number")?;
            if count == 0 {
                return Err("--frames needs a number above zero".into());
            }
            frames = Some(count);
        } else if file.is_none() {
            file = Some(PathBuf::from(arg));
        } else {
            return Err("expected one Slang file, see --help".into());
        }
    }
    let file = file.ok_or("expected one Slang file, see --help")?;
    Ok(Some(Args { file, frames }))
}

/// The count of the frames drawn, with the limit of `--frames` and the time
/// of the last frame, which the watchdog of an unattended run checks.
pub(crate) struct Frames {
    limit: Option<u64>,
    drawn: u64,
    last_drawn: Instant,
}

impl Frames {
    /// Starts the count at `now`, with the limit of `--frames`, if any.
    pub(crate) fn new(limit: Option<u64>, now: Instant) -> Self {
        Self {
            limit,
            drawn: 0,
            last_drawn: now,
        }
    }

    /// Restarts the watchdog at `now`, when the example is ready to draw.
    pub(crate) fn start(&mut self, now: Instant) {
        self.last_drawn = now;
    }

    /// Counts a frame drawn at `now`.
    pub(crate) fn drawn(&mut self, now: Instant) {
        self.drawn += 1;
        self.last_drawn = now;
    }

    /// Returns whether the example draws further frames, which it does
    /// until it reaches the limit.
    pub(crate) fn running(&self) -> bool {
        self.limit.is_none_or(|limit| self.drawn < limit)
    }

    /// Checks the watchdog at `now`, and returns the time of the next check,
    /// or `None` without a limit.
    ///
    /// # Errors
    ///
    /// Returns an error when a run with a limit has drawn no frame for
    /// [`UNATTENDED_STALL`].
    pub(crate) fn watch(&self, now: Instant) -> Result<Option<Instant>, Box<dyn Error>> {
        if self.limit.is_none() {
            return Ok(None);
        }
        let deadline = self.last_drawn + UNATTENDED_STALL;
        if now < deadline {
            return Ok(Some(deadline));
        }
        Err(format!(
            "drew no frame for {} seconds after {} frames with --frames set",
            UNATTENDED_STALL.as_secs(),
            self.drawn
        )
        .into())
    }

    /// Returns the line that a run with a limit prints when it ends, or
    /// `None` without a limit.
    pub(crate) fn summary(&self) -> Option<String> {
        self.limit.map(|_| format!("drew {} frames", self.drawn))
    }
}
