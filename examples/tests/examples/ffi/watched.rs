use std::fs;

use crate::common::{rewrite, scratch_copy};
use crate::watched::Watched;

#[test]
fn watched_reports_each_change_of_the_modification_time_once() {
    let path = scratch_copy("watched", "scale.slang", "changes");
    let mut watched = Watched::new(&path);
    assert_eq!(watched.path(), path);
    assert!(!watched.changed());
    rewrite(&path, "// changed\n");
    assert!(watched.changed());
    assert!(!watched.changed());
}

#[test]
fn watched_counts_a_file_that_it_cannot_read_as_unchanged() {
    let path = scratch_copy("watched", "scale.slang", "replaced");
    let mut watched = Watched::new(&path);
    fs::remove_file(&path).expect("the file is removed");
    assert!(!watched.changed());
    assert!(!watched.changed());
    fs::write(&path, "// replaced\n").expect("the file is written again");
    rewrite(&path, "// replaced\n");
    assert!(watched.changed());
    assert!(!watched.changed());
}
