use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use super::{
    EntryKind, Exit, Finished, Generated, GenerationError, GenerationErrorKind, Host, Invocation,
    LoggedRunError, MsvcCompiler, command, sync_copy,
};

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
    command("rustc", args)
}

fn child_test(name: &str, variable: &str, value: &Path) -> io::Result<std::process::Child> {
    Command::new(std::env::current_exe()?)
        .args(["--exact", name, "--nocapture"])
        .env(variable, value)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
}

#[test]
fn system_reads_variables_kinds_and_the_host_os() -> io::Result<()> {
    let system = System::default();
    let dir = scratch("kinds")?;
    let file = dir.join("file.txt");
    system.write(&file, "text")?;
    assert_eq!(system.read(&file)?, "text");
    let log = dir.join("build.log");
    fs::write(&log, b"cl: \xfc\xe9 error\n")?;
    assert_eq!(system.read(&log)?, "cl: \u{FFFD}\u{FFFD} error\n");
    assert_eq!(system.kind(&dir)?, EntryKind::Directory);
    assert_eq!(system.kind(&file)?, EntryKind::File);
    assert_eq!(
        system
            .kind(&dir.join("missing"))
            .map_err(|error| error.kind()),
        Err(io::ErrorKind::NotFound)
    );
    assert_eq!(system.host_os(), std::env::consts::OS);
    assert_eq!(system.var("PATH"), std::env::var_os("PATH"));
    assert_eq!(
        system.var("SLANG_COMPILER_SYS_UNSET_VARIABLE"),
        None::<OsString>
    );
    assert_eq!(system.process_id(), std::process::id());
    fs::remove_dir_all(&dir)
}

#[test]
fn system_lists_every_file_under_nested_directories_and_the_names_in_a_directory() -> io::Result<()>
{
    let system = System::default();
    let dir = scratch("files")?;
    for file in ["top.txt", "a/one.txt", "a/b/c/deep.txt"] {
        system.write(&dir.join(file), file)?;
    }
    fs::create_dir_all(dir.join("empty"))?;
    let mut files = system.files(&dir)?;
    files.sort();
    assert_eq!(
        files,
        [
            Path::new("a").join("b").join("c").join("deep.txt"),
            Path::new("a").join("one.txt"),
            PathBuf::from("top.txt"),
        ]
    );
    let mut names = system.list(&dir)?;
    names.sort();
    assert_eq!(names, ["a", "empty", "top.txt"].map(OsString::from));
    assert_eq!(
        system
            .files(&dir.join("missing"))
            .map_err(|error| error.kind()),
        Err(io::ErrorKind::NotFound)
    );
    assert_eq!(
        system
            .list(&dir.join("missing"))
            .map_err(|error| error.kind()),
        Err(io::ErrorKind::NotFound)
    );
    fs::remove_dir_all(&dir)
}

#[test]
fn system_writes_renames_and_removes_files_and_directories_and_creates_parents() -> io::Result<()> {
    let system = System::default();
    let dir = scratch("write")?;
    let file = dir.join("x").join("y").join("bindings.rs");
    system.write(&file, "first")?;
    system.write(&file, "second")?;
    assert_eq!(fs::read_to_string(&file)?, "second");
    system.remove_file(&file)?;
    assert_eq!(
        system.remove_file(&file).map_err(|error| error.kind()),
        Err(io::ErrorKind::NotFound)
    );

    let temporary = dir.join("entry.tmp-1");
    system.write(&temporary.join("lib").join("libcore.a"), "archive")?;
    let entry = dir.join("entry");
    system.rename(&temporary, &entry)?;
    assert_eq!(
        fs::read_to_string(entry.join("lib").join("libcore.a"))?,
        "archive"
    );
    assert!(!fs::exists(&temporary)?);
    system.remove_dir_all(&entry)?;
    assert!(!fs::exists(&entry)?);
    assert_eq!(
        system.remove_dir_all(&entry).map_err(|error| error.kind()),
        Err(io::ErrorKind::NotFound)
    );
    fs::remove_dir_all(&dir)
}

#[test]
fn system_sets_the_modification_time_of_a_file() -> io::Result<()> {
    let system = System::default();
    let dir = scratch("modified")?;
    let stamp = dir.join("stamp");
    system.write(&stamp, "")?;
    let old = SystemTime::UNIX_EPOCH + Duration::from_hours(262_968);
    system.set_modified(&stamp, old)?;
    assert_eq!(fs::metadata(&stamp)?.modified()?, old);
    assert_eq!(
        system
            .set_modified(&dir.join("missing"), old)
            .map_err(|error| error.kind()),
        Err(io::ErrorKind::NotFound)
    );
    fs::remove_dir_all(&dir)
}

#[test]
fn system_sync_keeps_an_unchanged_file_and_rewrites_a_changed_one() -> io::Result<()> {
    let system = System::default();
    let dir = scratch("sync")?;
    let (from, to) = (
        dir.join("source.h"),
        dir.join("copy").join("nested").join("source.h"),
    );
    fs::write(&from, "one")?;
    system.sync_file(&from, &to)?;
    assert_eq!(fs::read_to_string(&to)?, "one");

    let old = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000_000);
    File::options().write(true).open(&to)?.set_modified(old)?;
    system.sync_file(&from, &to)?;
    assert_eq!(fs::metadata(&to)?.modified()?, old);

    fs::write(&from, "two")?;
    system.sync_file(&from, &to)?;
    assert_eq!(fs::read_to_string(&to)?, "two");
    assert_ne!(fs::metadata(&to)?.modified()?, old);
    fs::remove_dir_all(&dir)
}

#[test]
fn system_sync_copy_copies_tracked_files_and_removes_stale_ones() -> io::Result<()> {
    let system = System::default();
    let dir = scratch("sync-copy")?;
    let (source, copy) = (dir.join("source"), dir.join("copy"));
    let tracked = [
        PathBuf::from("CMakeLists.txt"),
        Path::new("src").join("a.cpp"),
    ];
    for file in &tracked {
        system.write(&source.join(file), "tracked")?;
    }
    system.write(&copy.join("src").join("generated.h"), "stale")?;
    system.write(&copy.join("stale.txt"), "stale")?;
    assert_eq!(
        sync_copy(&system, &source, &copy, &tracked).map_err(|error| format!("{error:?}")),
        Ok(())
    );
    let mut files = system.files(&copy)?;
    files.sort();
    assert_eq!(files, tracked);
    fs::remove_dir_all(&dir)
}

const LOCK_CHILD: &str = "SLANG_COMPILER_SYS_TEST_LOCK";

#[test]
fn system_holds_a_lock_that_a_second_lock_on_the_same_file_waits_for() -> io::Result<()> {
    if let Some(lock) = std::env::var_os(LOCK_CHILD) {
        let lock = PathBuf::from(lock);
        fs::write(lock.with_extension("started"), "")?;
        System::default().lock(&lock)?;
        assert!(
            fs::exists(lock.with_extension("released"))?,
            "the second lock was taken while the first one was held"
        );
        return Ok(());
    }
    let name =
        "build_script::ffi::system_holds_a_lock_that_a_second_lock_on_the_same_file_waits_for";
    let dir = scratch("lock")?;
    let lock = dir.join("entries").join("key.lock");
    let holder = System::default();
    holder.lock(&lock)?;
    assert!(fs::exists(&lock)?);
    let child = child_test(name, LOCK_CHILD, &lock)?;
    let start = Instant::now();
    while !fs::exists(lock.with_extension("started"))? {
        assert!(
            start.elapsed() < Duration::from_secs(120),
            "the child did not start"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    std::thread::sleep(Duration::from_millis(200));
    fs::write(lock.with_extension("released"), "")?;
    drop(holder);
    let output = child.wait_with_output()?;
    assert!(output.status.success(), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("1 passed"),
        "{output:?}"
    );
    fs::remove_dir_all(&dir)
}

#[test]
fn system_finds_the_microsoft_compiler_only_for_a_vendored_build_of_its_toolchain() {
    let system = System::default();
    for target in ["x86_64-pc-windows-msvc", "aarch64-pc-windows-msvc"] {
        let expected = if cfg!(feature = "vendored") {
            find_msvc_tools::find_tool(target, "cl.exe").map(|tool| MsvcCompiler {
                path: tool.path().to_path_buf(),
                env: tool.env().into_iter().cloned().collect(),
            })
        } else {
            None
        };
        assert_eq!(system.msvc_compiler(target), expected, "{target}");
    }
    for target in ["x86_64-unknown-linux-gnu", "x86_64-pc-windows-gnu"] {
        assert_eq!(system.msvc_compiler(target), None, "{target}");
    }
    if cfg!(all(windows, feature = "vendored")) {
        let compiler = system.msvc_compiler("x86_64-pc-windows-msvc");
        assert!(
            compiler
                .as_ref()
                .is_some_and(|compiler| compiler.path.ends_with("cl.exe")
                    && compiler
                        .env
                        .iter()
                        .any(|(name, _)| name.eq_ignore_ascii_case("PATH"))),
            "{compiler:?}"
        );
    }
}

#[test]
fn system_runs_a_program_and_captures_its_output_and_exit_status() -> io::Result<()> {
    let system = System::default();
    let Finished {
        exit,
        stdout,
        stderr,
    } = system.run(&rustc(&["--version"]))?;
    assert_eq!(exit, Exit { code: Some(0) });
    assert!(stdout.starts_with("rustc "), "{stdout:?}");
    assert_eq!(stderr, "");

    let failed = system.run(&rustc(&["--no-such-option"]))?;
    assert!(!failed.exit.success(), "{failed:?}");
    assert!(failed.stderr.contains("no-such-option"), "{failed:?}");

    let dir = scratch("run")?;
    let missing = Invocation {
        dir: Some(dir.join("missing")),
        ..rustc(&["--version"])
    };
    assert!(system.run(&missing).is_err());
    fs::remove_dir_all(&dir)
}

#[test]
fn system_runs_a_program_with_the_variables_that_it_sets_and_removes() -> io::Result<()> {
    let system = System::default();
    let removed = "CARGO_MANIFEST_DIR";
    assert!(
        std::env::var_os(removed).is_some(),
        "the test process lacks {removed}"
    );
    let dir = scratch("env")?;
    let (set, absent) = (dir.join("set.rs"), dir.join("absent.rs"));
    fs::write(
        &set,
        "const _: () = assert!(matches!(env!(\"SLANG_COMPILER_SYS_SET\").as_bytes(), b\"set\"));\n",
    )?;
    fs::write(
        &absent,
        format!("const _: () = assert!(option_env!(\"{removed}\").is_none());\n"),
    )?;
    let check = |source: &Path| {
        let out = dir.display().to_string();
        let source = source.display().to_string();
        rustc(&[
            "--edition",
            "2024",
            "--crate-type",
            "lib",
            "--emit",
            "metadata",
            "--out-dir",
            &out,
            &source,
        ])
    };
    for (invocation, compiles) in [
        (check(&set), false),
        (
            Invocation {
                env: vec![(
                    OsString::from("SLANG_COMPILER_SYS_SET"),
                    OsString::from("set"),
                )],
                ..check(&set)
            },
            true,
        ),
        (check(&absent), false),
        (
            Invocation {
                env_remove: vec![removed],
                ..check(&absent)
            },
            true,
        ),
    ] {
        let finished = system.run(&invocation)?;
        assert_eq!(
            finished.exit.success(),
            compiles,
            "{invocation:?}: {finished:?}"
        );
    }
    fs::remove_dir_all(&dir)
}

fn logged_kind(error: LoggedRunError) -> (&'static str, io::ErrorKind) {
    match error {
        LoggedRunError::Log(error) => ("log", error.kind()),
        LoggedRunError::Start(error) => ("start", error.kind()),
    }
}

#[test]
fn system_appends_the_output_of_a_logged_program_to_its_log() -> io::Result<()> {
    let system = System::default();
    let dir = scratch("log")?;
    let log = dir.join("build.log");
    fs::write(&log, "earlier\n")?;
    let exit = system.run_to_log(&rustc(&["--version"]), &log);
    assert_eq!(exit.map_err(logged_kind), Ok(Exit { code: Some(0) }));
    let failed = system.run_to_log(&rustc(&["--no-such-option"]), &log);
    assert!(failed.is_ok_and(|exit| !exit.success()));
    let text = fs::read_to_string(&log)?;
    assert!(text.starts_with("earlier\nrustc "), "{text:?}");
    assert!(text.contains("no-such-option"), "{text:?}");
    fs::remove_dir_all(&dir)
}

#[test]
fn system_tells_a_log_that_cannot_be_opened_from_a_program_that_cannot_start() -> io::Result<()> {
    let system = System::default();
    let dir = scratch("log-errors")?;
    let parent = dir.join("not-a-directory");
    fs::write(&parent, "")?;
    let unopenable = system.run_to_log(&rustc(&["--version"]), &parent.join("build.log"));
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
    let unstartable = system.run_to_log(&missing, &log);
    assert!(
        matches!(unstartable, Err(LoggedRunError::Start(_))),
        "{:?}",
        unstartable.map_err(logged_kind)
    );
    fs::remove_dir_all(&dir)
}

#[test]
fn system_generate_maps_each_failure_of_the_generator_and_keeps_its_causes() -> io::Result<()> {
    let system = System::default();
    let dir = scratch("generate")?;
    let failure = |target: &str| {
        system
            .generate(&dir, target)
            .expect_err("the scratch headers do not generate")
    };
    let missing = failure("x86_64-unknown-linux-gnu");
    assert_eq!(missing.kind, GenerationErrorKind::Other, "{missing:?}");
    assert!(
        missing.message.starts_with(&format!(
            "cannot read the main header {}, caused by: ",
            dir.join("slang.h").display()
        )),
        "{missing:?}"
    );
    fs::write(dir.join("slang.h"), "struct Unknown { int value; };\n")?;
    let headers = failure("x86_64-unknown-linux-gnu");
    assert_eq!(headers.kind, GenerationErrorKind::Headers, "{headers:?}");
    assert!(
        headers
            .message
            .contains("decide no declaration or macro of the headers"),
        "{headers:?}"
    );
    let target = failure("notanarch-unknown-unknown");
    assert_eq!(target.kind, GenerationErrorKind::Target, "{target:?}");
    assert!(
        target.message.starts_with(
            "libclang cannot parse slang.h for target notanarch-unknown-unknown, which it does when it does not accept the target triple, caused by: "
        ),
        "{target:?}"
    );
    fs::remove_dir_all(&dir)
}

const LIBCLANG_CHILD: &str = "SLANG_COMPILER_SYS_TEST_WITHOUT_LIBCLANG";

#[test]
fn system_generate_reports_a_libclang_that_cannot_be_loaded() -> io::Result<()> {
    let name = "build_script::ffi::system_generate_reports_a_libclang_that_cannot_be_loaded";
    if let Some(dir) = std::env::var_os(LIBCLANG_CHILD) {
        let error = System::default()
            .generate(Path::new(&dir), "x86_64-unknown-linux-gnu")
            .expect_err("libclang cannot be loaded");
        assert_eq!(error.kind, GenerationErrorKind::Libclang, "{error:?}");
        assert!(
            error.message.starts_with("cannot load libclang: "),
            "{error:?}"
        );
        return Ok(());
    }
    let dir = scratch("no-libclang")?;
    fs::write(dir.join("slang.h"), "")?;
    let output = Command::new(std::env::current_exe()?)
        .args(["--exact", name, "--nocapture"])
        .env(LIBCLANG_CHILD, &dir)
        .env("LIBCLANG_PATH", dir.join("no-such-directory"))
        .output()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "{output:?}");
    assert!(stdout.contains("1 passed"), "{stdout}");
    fs::remove_dir_all(&dir)
}
