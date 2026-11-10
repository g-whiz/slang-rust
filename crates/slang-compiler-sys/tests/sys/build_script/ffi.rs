use std::ffi::OsString;
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use super::{EntryKind, Exit, Finished, Host, Invocation, LoggedRunError, invocation, sync_copy};

include!("../../../system.rs");

fn scratch(name: &str) -> io::Result<PathBuf> {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("build-script-system")
        .join(name);
    match fs::remove_dir_all(&dir) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error),
        _ => {}
    }
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn rustc(args: &[&str]) -> Invocation {
    Invocation {
        program: "rustc",
        args: args.iter().map(|arg| (*arg).to_owned()).collect(),
        ..invocation()
    }
}

#[test]
fn system_reads_variables_kinds_and_the_host_os() -> io::Result<()> {
    let dir = scratch("kinds")?;
    let file = dir.join("file.txt");
    System.write(&file, "text")?;
    assert_eq!(System.read(&file)?, "text");
    assert_eq!(System.kind(&dir)?, EntryKind::Directory);
    assert_eq!(System.kind(&file)?, EntryKind::File);
    assert_eq!(
        System
            .kind(&dir.join("missing"))
            .map_err(|error| error.kind()),
        Err(io::ErrorKind::NotFound)
    );
    assert_eq!(System.host_os(), std::env::consts::OS);
    assert_eq!(System.var("PATH"), std::env::var_os("PATH"));
    assert_eq!(
        System.var("SLANG_COMPILER_SYS_UNSET_VARIABLE"),
        None::<OsString>
    );
    fs::remove_dir_all(&dir)
}

#[test]
fn system_lists_every_file_under_nested_directories() -> io::Result<()> {
    let dir = scratch("files")?;
    for file in ["top.txt", "a/one.txt", "a/b/c/deep.txt"] {
        System.write(&dir.join(file), file)?;
    }
    fs::create_dir_all(dir.join("empty"))?;
    let mut files = System.files(&dir)?;
    files.sort();
    assert_eq!(
        files,
        [
            Path::new("a").join("b").join("c").join("deep.txt"),
            Path::new("a").join("one.txt"),
            PathBuf::from("top.txt"),
        ]
    );
    assert_eq!(
        System
            .files(&dir.join("missing"))
            .map_err(|error| error.kind()),
        Err(io::ErrorKind::NotFound)
    );
    fs::remove_dir_all(&dir)
}

#[test]
fn system_writes_and_removes_files_and_creates_their_parents() -> io::Result<()> {
    let dir = scratch("write")?;
    let file = dir.join("x").join("y").join("stamp");
    System.write(&file, "first")?;
    System.write(&file, "second")?;
    assert_eq!(fs::read_to_string(&file)?, "second");
    System.remove_file(&file)?;
    assert_eq!(
        System.remove_file(&file).map_err(|error| error.kind()),
        Err(io::ErrorKind::NotFound)
    );
    fs::remove_dir_all(&dir)
}

#[test]
fn system_sync_keeps_an_unchanged_file_and_rewrites_a_changed_one() -> io::Result<()> {
    let dir = scratch("sync")?;
    let (from, to) = (
        dir.join("source.h"),
        dir.join("copy").join("nested").join("source.h"),
    );
    fs::write(&from, "one")?;
    System.sync_file(&from, &to)?;
    assert_eq!(fs::read_to_string(&to)?, "one");

    let old = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000_000);
    File::options().write(true).open(&to)?.set_modified(old)?;
    System.sync_file(&from, &to)?;
    assert_eq!(fs::metadata(&to)?.modified()?, old);

    fs::write(&from, "two")?;
    System.sync_file(&from, &to)?;
    assert_eq!(fs::read_to_string(&to)?, "two");
    assert_ne!(fs::metadata(&to)?.modified()?, old);
    fs::remove_dir_all(&dir)
}

#[test]
fn system_sync_copy_copies_tracked_files_and_removes_stale_ones() -> io::Result<()> {
    let dir = scratch("sync-copy")?;
    let (source, copy) = (dir.join("source"), dir.join("copy"));
    let tracked = [
        PathBuf::from("CMakeLists.txt"),
        Path::new("src").join("a.cpp"),
    ];
    for file in &tracked {
        System.write(&source.join(file), "tracked")?;
    }
    System.write(&copy.join("src").join("generated.h"), "stale")?;
    System.write(&copy.join("stale.txt"), "stale")?;
    assert_eq!(
        sync_copy(&System, &source, &copy, &tracked).map_err(|error| format!("{error:?}")),
        Ok(())
    );
    let mut files = System.files(&copy)?;
    files.sort();
    assert_eq!(files, tracked);
    fs::remove_dir_all(&dir)
}

#[test]
fn system_runs_a_program_and_captures_its_output_and_exit_status() -> io::Result<()> {
    let Finished {
        exit,
        stdout,
        stderr,
    } = System.run(&rustc(&["--version"]))?;
    assert_eq!(exit, Exit { code: Some(0) });
    assert!(stdout.starts_with("rustc "), "{stdout:?}");
    assert_eq!(stderr, "");

    let failed = System.run(&rustc(&["--no-such-option"]))?;
    assert!(!failed.exit.success(), "{failed:?}");
    assert!(failed.stderr.contains("no-such-option"), "{failed:?}");

    let dir = scratch("run")?;
    let missing = Invocation {
        dir: Some(dir.join("missing")),
        ..rustc(&["--version"])
    };
    assert!(System.run(&missing).is_err());
    fs::remove_dir_all(&dir)
}

#[test]
fn system_appends_the_output_of_a_logged_program_to_its_log() -> io::Result<()> {
    let dir = scratch("log")?;
    let log = dir.join("build.log");
    fs::write(&log, "earlier\n")?;
    let exit = System.run_to_log(&rustc(&["--version"]), &log);
    assert_eq!(exit.map_err(logged_kind), Ok(Exit { code: Some(0) }));
    let failed = System.run_to_log(&rustc(&["--no-such-option"]), &log);
    assert!(failed.is_ok_and(|exit| !exit.success()));
    let text = fs::read_to_string(&log)?;
    assert!(text.starts_with("earlier\nrustc "), "{text:?}");
    assert!(text.contains("no-such-option"), "{text:?}");
    fs::remove_dir_all(&dir)
}

fn logged_kind(error: LoggedRunError) -> (&'static str, io::ErrorKind) {
    match error {
        LoggedRunError::Log(error) => ("log", error.kind()),
        LoggedRunError::Start(error) => ("start", error.kind()),
    }
}

#[test]
fn system_tells_a_log_that_cannot_be_opened_from_a_program_that_cannot_start() -> io::Result<()> {
    let dir = scratch("log-errors")?;
    let parent = dir.join("not-a-directory");
    fs::write(&parent, "")?;
    let unopenable = System.run_to_log(&rustc(&["--version"]), &parent.join("build.log"));
    assert!(
        matches!(unopenable, Err(LoggedRunError::Log(_))),
        "{:?}",
        unopenable.map_err(logged_kind)
    );

    let log = dir.join("build.log");
    let missing = Invocation {
        dir: Some(dir.join("missing")),
        ..rustc(&["--version"])
    };
    let unstartable = System.run_to_log(&missing, &log);
    assert_eq!(
        unstartable.map_err(logged_kind),
        Err(("start", io::ErrorKind::NotFound))
    );
    fs::remove_dir_all(&dir)
}
