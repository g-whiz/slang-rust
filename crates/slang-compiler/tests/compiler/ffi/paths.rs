//! The test that relative paths that Slang receives, including relative
//! search paths, resolve against the working directory of the process.
//!
//! The working directory belongs to the whole process, so each case runs
//! this test binary again as a child process with its own working directory.

use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;

use slang_compiler::{GlobalSession, Session, Target, TargetFormat};

use super::support::scratch_dir;

/// The environment variable that selects the input of a child process.
const CHILD: &str = "SLANG_COMPILER_PATHS_CHILD";

/// The module that the search path holds.
const HELPER: &str = "public int helperValue() { return 7; }\n";

/// A file that the source of the second case includes.
const INCLUDED: &str = "int includedValue() { return 3; }\n";

/// A source that includes [`INCLUDED`] by a path relative to the source.
const INCLUDING: &str = "#include \"included.slang\"\nint useIt() { return includedValue(); }\n";

/// Runs the child input `input` in the working directory `dir` and returns
/// its standard output.
fn run_child(input: &str, dir: &Path) -> String {
    let output = Command::new(env::current_exe().expect("the test binary has a path"))
        .args([
            "--exact",
            "ffi::paths::child",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(CHILD, input)
        .current_dir(dir)
        .output()
        .expect("the child process starts");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    assert!(
        output.status.success(),
        "{stdout}{}",
        String::from_utf8_lossy(&output.stderr)
    );
    stdout
}

/// The body of the child processes of the test below, which does nothing
/// unless the variable `CHILD` names an input.
#[test]
fn child() {
    let Ok(input) = env::var(CHILD) else {
        return;
    };
    let global = GlobalSession::new().expect("Slang creates a global session");
    let session = Session::builder(&global)
        .target(Target::new(TargetFormat::Spirv))
        .search_path("mods")
        .build()
        .expect("Slang creates the session");
    let result = match input.as_str() {
        "search_path" => session.load_module("helper").map(|_| ()),
        "module_path" => session.load_module("mods/helper.slang").map(|_| ()),
        "include" => session
            .load_module_from_source("including", "src/including.slang", INCLUDING)
            .map(|_| ()),
        other => panic!("unknown input {other}"),
    };
    match result {
        Ok(()) => println!("loaded"),
        Err(error) => println!("failed with {:?}", error.kind()),
    }
}

#[test]
fn relative_paths_resolve_against_the_working_directory() {
    let dir = scratch_dir("relative_paths");
    fs::create_dir_all(dir.join("mods")).expect("the directory is created");
    fs::create_dir_all(dir.join("src")).expect("the directory is created");
    fs::create_dir_all(dir.join("elsewhere")).expect("the directory is created");
    fs::write(dir.join("mods").join("helper.slang"), HELPER).expect("the file is written");
    fs::write(dir.join("src").join("included.slang"), INCLUDED).expect("the file is written");
    for input in ["search_path", "module_path", "include"] {
        assert!(
            run_child(input, &dir).contains("loaded"),
            "{input} in the directory that holds the files"
        );
        let elsewhere = run_child(input, &dir.join("elsewhere"));
        assert!(
            elsewhere.contains("failed with Compile"),
            "{input} elsewhere: {elsewhere}"
        );
    }
}
