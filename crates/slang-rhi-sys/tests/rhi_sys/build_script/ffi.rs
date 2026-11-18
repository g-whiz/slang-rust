use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use super::{
    EntryKind, Exit, Finished, Generated, GenerationError, GenerationErrorKind, Host, Invocation,
    LoggedRunError, command,
};

include!("../../../system.rs");

fn scratch(name: &str) -> io::Result<PathBuf> {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("rhi-build-script-system")
        .join(name);
    match fs::remove_dir_all(&dir) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error),
        _ => {}
    }
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

#[test]
fn system_reads_variables_files_and_kinds() -> io::Result<()> {
    let system = System::default();
    let dir = scratch("read")?;
    let file = dir.join("file.txt");
    fs::write(&file, "text")?;
    assert_eq!(
        system.var("CARGO_MANIFEST_DIR"),
        Some(OsString::from(env!("CARGO_MANIFEST_DIR")))
    );
    assert_eq!(system.var("SLANG_RHI_SYS_UNSET_VARIABLE"), None);
    assert_eq!(system.read(&file)?, "text");
    assert_eq!(system.kind(&file)?, EntryKind::File);
    assert_eq!(system.kind(&dir)?, EntryKind::Directory);
    assert_eq!(
        system
            .kind(&dir.join("missing"))
            .map_err(|error| error.kind()),
        Err(io::ErrorKind::NotFound)
    );
    Ok(())
}

#[test]
fn system_runs_a_program_with_closed_input_and_reports_its_output() -> io::Result<()> {
    let system = System::default();
    let finished: Finished = system.run(&command("cmake", &["-E", "echo", "printed"]))?;
    assert_eq!(finished.exit, Exit { code: Some(0) });
    assert_eq!(finished.stdout, "printed\n");
    let reader = system.run(&command("cmake", &["-E", "cat", "/dev/stdin"]))?;
    assert!(reader.exit.success(), "{}", reader.stderr);
    assert_eq!(reader.stdout, "");
    let failed = system.run(&command("cmake", &["-E", "false"]))?;
    assert!(!failed.exit.success());
    assert_eq!(
        system
            .run(&command("slang-rhi-sys-missing-program", &[]))
            .map_err(|error| error.kind()),
        Err(io::ErrorKind::NotFound)
    );
    Ok(())
}

#[test]
fn system_appends_both_output_streams_of_a_run_to_the_log() -> io::Result<()> {
    let system = System::default();
    let dir = scratch("log")?;
    let log = dir.join("build.log");
    fs::write(&log, "before\n")?;
    let exit = system
        .run_to_log(&command("cmake", &["-E", "echo", "printed"]), &log)
        .map_err(|error| match error {
            LoggedRunError::Log(error) | LoggedRunError::Start(error) => error,
        })?;
    assert!(exit.success());
    assert_eq!(fs::read_to_string(&log)?, "before\nprinted\n");
    let missing = system.run_to_log(&command("slang-rhi-sys-missing-program", &[]), &log);
    assert!(matches!(missing, Err(LoggedRunError::Start(_))));
    let unopenable = system.run_to_log(
        &command("cmake", &["--version"]),
        &dir.join("no").join("log"),
    );
    assert!(matches!(unopenable, Err(LoggedRunError::Log(_))));
    Ok(())
}

#[test]
fn system_lists_copies_writes_stamps_renames_and_removes() -> io::Result<()> {
    let system = System::default();
    let dir = scratch("files")?;
    let from = dir.join("from.a");
    fs::write(&from, "archive")?;
    let temporary = dir.join("entry.tmp-1");
    system.copy_file(&from, &temporary.join("lib").join("from.a"))?;
    system.write(&temporary.join("stamp"), "")?;
    let time = SystemTime::UNIX_EPOCH + Duration::from_hours(262_968);
    system.set_modified(&temporary.join("stamp"), time)?;
    assert_eq!(fs::metadata(temporary.join("stamp"))?.modified()?, time);
    let entry = dir.join("entry");
    system.rename(&temporary, &entry)?;
    assert_eq!(
        fs::read_to_string(entry.join("lib").join("from.a"))?,
        "archive"
    );
    let mut names = system.list(&dir)?;
    names.sort();
    assert_eq!(names, [OsString::from("entry"), OsString::from("from.a")]);
    system.remove_file(&from)?;
    system.remove_dir_all(&entry)?;
    assert_eq!(system.list(&dir)?, Vec::<OsString>::new());
    Ok(())
}

#[test]
fn system_holds_a_lock_until_it_is_dropped() -> io::Result<()> {
    let dir = scratch("lock")?;
    let path = dir.join("entries").join("key.lock");
    let system = System::default();
    system.lock(&path)?;
    let other = File::options().write(true).open(&path)?;
    assert!(matches!(
        other.try_lock(),
        Err(fs::TryLockError::WouldBlock)
    ));
    drop(system);
    other
        .try_lock()
        .map_err(|error| io::Error::other(error.to_string()))?;
    Ok(())
}

#[test]
fn system_generate_maps_a_failure_of_the_generator_and_keeps_its_causes() -> io::Result<()> {
    let system = System::default();
    let dir = scratch("generate")?;
    let missing = system
        .generate(&dir, &dir, &dir, "x86_64-unknown-linux-gnu")
        .expect_err("the scratch directory holds no headers");
    assert_eq!(missing.kind, GenerationErrorKind::Other, "{missing:?}");
    assert!(
        missing.message.starts_with(&format!(
            "cannot read the main header {}, caused by: ",
            dir.join("slang.h").display()
        )),
        "{missing:?}"
    );
    fs::remove_dir_all(&dir)
}

const LIBCLANG_CHILD: &str = "SLANG_RHI_SYS_TEST_WITHOUT_LIBCLANG";

#[test]
fn system_generate_reports_a_libclang_that_cannot_be_loaded() -> io::Result<()> {
    let name = "build_script::ffi::system_generate_reports_a_libclang_that_cannot_be_loaded";
    if let Some(dir) = std::env::var_os(LIBCLANG_CHILD) {
        let dir = Path::new(&dir);
        let error = System::default()
            .generate(dir, dir, dir, "x86_64-unknown-linux-gnu")
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
    fs::write(dir.join("slang-rhi.h"), "")?;
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
