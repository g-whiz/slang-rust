use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use super::{EntryKind, Exit, Finished, Host, Invocation, LoggedRunError, MsvcCompiler, command};

include!("../../../system.rs");

fn scratch(name: &str) -> io::Result<PathBuf> {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("glslang-build-script-system")
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
    assert_eq!(system.var("SLANG_GLSLANG_SYS_UNSET_VARIABLE"), None);
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
fn system_runs_a_program_with_its_environment_and_reports_its_output() -> io::Result<()> {
    let system = System::default();
    let invocation = Invocation {
        env: vec![(
            OsString::from("SLANG_GLSLANG_SYS_PROBE"),
            OsString::from("set"),
        )],
        ..command("cmake", &["-E", "environment"])
    };
    let finished = system.run(&invocation)?;
    assert_eq!(finished.exit, Exit { code: Some(0) });
    assert!(
        finished.stdout.contains("SLANG_GLSLANG_SYS_PROBE=set"),
        "{}",
        finished.stdout
    );
    let failed = system.run(&command("cmake", &["-E", "false"]))?;
    assert!(!failed.exit.success());
    assert_eq!(
        system
            .run(&command("slang-glslang-sys-missing-program", &[]))
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
    let missing = system.run_to_log(&command("slang-glslang-sys-missing-program", &[]), &log);
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
fn system_finds_the_microsoft_compiler_as_find_msvc_tools_does() {
    let target = "x86_64-pc-windows-msvc";
    let expected = find_msvc_tools::find_tool(target, "cl.exe").map(|tool| MsvcCompiler {
        path: tool.path().to_path_buf(),
        env: tool.env().into_iter().cloned().collect(),
    });
    assert_eq!(System::default().msvc_compiler(target), expected);
}
