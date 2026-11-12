//! The file of a scene that the windowed examples load again when it
//! changes, which the scene of each includes with a `#[path]` attribute.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// A file with the modification time of its last load.
pub(crate) struct Watched {
    path: PathBuf,
    modified: Option<SystemTime>,
}

impl Watched {
    /// Records the modification time of the file at `path`, before the
    /// first load.
    pub(crate) fn new(path: &Path) -> Self {
        Self {
            path: path.to_owned(),
            modified: modified(path),
        }
    }

    /// Returns the path of the file.
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    /// Returns whether the modification time of the file differs from the
    /// recorded one, and records the new time before the caller loads the
    /// file, so that a load that fails is not tried again until the time
    /// changes once more. A file whose modification time cannot be read
    /// counts as unchanged, as it can be while an editor replaces it.
    pub(crate) fn changed(&mut self) -> bool {
        let modified = modified(&self.path);
        if modified.is_none() || modified == self.modified {
            return false;
        }
        self.modified = modified;
        true
    }
}

fn modified(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .ok()
}
