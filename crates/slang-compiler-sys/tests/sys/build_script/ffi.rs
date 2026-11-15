use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};

use super::{EntryKind, Generated, GenerationError, GenerationErrorKind, Host};

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

#[test]
fn system_reads_variables_kinds_and_the_host_os() -> io::Result<()> {
    let dir = scratch("kinds")?;
    let file = dir.join("file.txt");
    System.write(&file, "text")?;
    assert_eq!(System.read(&file)?, "text");
    let other = dir.join("other.txt");
    fs::write(&other, b"tag: \xfc\xe9\n")?;
    assert_eq!(System.read(&other)?, "tag: \u{FFFD}\u{FFFD}\n");
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
fn system_writes_files_and_creates_their_parents() -> io::Result<()> {
    let dir = scratch("write")?;
    let file = dir.join("x").join("y").join("bindings.rs");
    System.write(&file, "first")?;
    System.write(&file, "second")?;
    assert_eq!(fs::read_to_string(&file)?, "second");
    fs::remove_dir_all(&dir)
}

#[test]
fn system_generate_maps_each_failure_of_the_generator_and_keeps_its_causes() -> io::Result<()> {
    let dir = scratch("generate")?;
    let failure = |target: &str| {
        System
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

const CHILD: &str = "SLANG_COMPILER_SYS_TEST_WITHOUT_LIBCLANG";

#[test]
fn system_generate_reports_a_libclang_that_cannot_be_loaded() -> io::Result<()> {
    let name = "build_script::ffi::system_generate_reports_a_libclang_that_cannot_be_loaded";
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("build-script-system")
        .join("no-libclang");
    if std::env::var_os(CHILD).is_some() {
        let error = System
            .generate(&dir, "x86_64-unknown-linux-gnu")
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
    let output = std::process::Command::new(std::env::current_exe()?)
        .args(["--exact", name, "--nocapture"])
        .env(CHILD, "1")
        .env("LIBCLANG_PATH", dir.join("no-such-directory"))
        .output()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "{output:?}");
    assert!(stdout.contains("1 passed"), "{stdout}");
    fs::remove_dir_all(&dir)
}
