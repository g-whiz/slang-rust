use std::ffi::{OsStr, OsString};
use std::fs::TryLockError;
use std::io;
use std::path::{Path, PathBuf};

use super::{
    BuildError, CACHEDIR_TAG, Finished, GIT_REPOSITORY_VARIABLES, Host, Invocation,
    RECORDED_BUILD_TAG, RECORDED_COMMIT, REQUIRED_SUBMODULES, base_dir, clone_entry_name,
    configure, git,
};

include!("../../../system.rs");

fn scratch(name: &str) -> io::Result<PathBuf> {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("slang-src-system")
        .join(name);
    match fs::remove_dir_all(&dir) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error),
        _ => {}
    }
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn stamp_time() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_hours(262_968)
}

#[test]
fn system_creates_writes_lists_renames_and_removes_directories() -> io::Result<()> {
    let dir = scratch("files")?;
    let nested = dir.join("a").join("b");
    let system = System::default();
    assert!(!system.exists(&nested)?);
    system.create_dir_all(&nested)?;
    assert!(system.exists(&nested)?);
    system.write(&nested.join("build-tag"), "tag\n")?;
    assert_eq!(fs::read_to_string(nested.join("build-tag"))?, "tag\n");
    assert_eq!(system.list(&dir)?, [OsString::from("a")]);
    let renamed = dir.join("c");
    system.rename(&dir.join("a"), &renamed)?;
    assert!(!system.exists(&dir.join("a"))?);
    assert!(system.exists(&renamed.join("b").join("build-tag"))?);
    system.remove_dir_all(&renamed)?;
    assert!(!system.exists(&renamed)?);
    assert_eq!(
        system
            .list(&dir.join("missing"))
            .map_err(|error| error.kind()),
        Err(io::ErrorKind::NotFound)
    );
    assert_eq!(system.process_id(), std::process::id());
    assert_eq!(system.var("PATH"), std::env::var_os("PATH"));
    fs::remove_dir_all(&dir)
}

#[test]
fn system_writes_a_stamp_dated_2000_01_01() -> io::Result<()> {
    let dir = scratch("stamp")?;
    let stamp = dir.join("stamp");
    System::default().write_stamp(&stamp)?;
    let metadata = fs::metadata(&stamp)?;
    assert_eq!(metadata.len(), 0);
    assert_eq!(metadata.modified()?, stamp_time());
    fs::remove_dir_all(&dir)
}

#[test]
fn system_holds_the_lock_until_it_is_dropped() -> io::Result<()> {
    let dir = scratch("lock")?;
    let lock = dir.join("entry.lock");
    let system = System::default();
    system.lock(&lock)?;
    let other = File::open(&lock)?;
    assert!(matches!(other.try_lock(), Err(TryLockError::WouldBlock)));
    drop(system);
    assert!(other.try_lock().is_ok());
    other.unlock()?;
    assert_eq!(
        System::default()
            .lock(&dir.join("missing").join("entry.lock"))
            .map_err(|error| error.kind()),
        Err(io::ErrorKind::NotFound)
    );
    fs::remove_dir_all(&dir)
}

#[test]
fn system_runs_git_without_repository_variables_and_with_closed_input() -> io::Result<()> {
    let dir = scratch("git")?;
    let invocation = git(dir.clone(), &["--version"]);
    let command = system_command(&invocation);
    assert_eq!(command.get_program(), "git");
    assert_eq!(command.get_current_dir(), Some(dir.as_path()));
    let envs: Vec<(&OsStr, Option<&OsStr>)> = command.get_envs().collect();
    for name in GIT_REPOSITORY_VARIABLES {
        assert!(envs.contains(&(OsStr::new(name), None)), "{name}");
    }
    assert!(envs.contains(&(OsStr::new("GIT_OPTIONAL_LOCKS"), Some(OsStr::new("0")))));
    let finished = System::default().run(&invocation)?;
    assert_eq!(finished.code, Some(0), "{finished:?}");
    assert!(finished.stdout.starts_with("git version "), "{finished:?}");
    let outside = git(dir.join("missing"), &["--version"]);
    assert!(System::default().run(&outside).is_err());
    fs::remove_dir_all(&dir)
}

#[test]
fn repository_variables_cover_those_that_the_installed_git_lists() -> io::Result<()> {
    let dir = scratch("local-env-vars")?;
    let finished = System::default().run(&git(dir.clone(), &["rev-parse", "--local-env-vars"]))?;
    assert_eq!(finished.code, Some(0), "{finished:?}");
    for name in finished.stdout.lines() {
        assert!(GIT_REPOSITORY_VARIABLES.contains(&name), "{name}");
    }
    fs::remove_dir_all(&dir)
}

#[test]
fn base_dir_finds_the_tags_on_the_file_system() -> io::Result<()> {
    let dir = scratch("lookup")?;
    fs::write(dir.join(CACHEDIR_TAG), "")?;
    let triple = "aarch64-unknown-linux-gnu";
    let target = dir.join("target");
    let unit_out = target
        .join(triple)
        .join("debug")
        .join("build")
        .join("x-0123")
        .join("out");
    fs::create_dir_all(&unit_out)?;
    let system = System::default();
    assert_eq!(base_dir(&system, &unit_out, Some(OsStr::new(triple))), dir);
    fs::write(target.join(triple).join(CACHEDIR_TAG), "")?;
    assert_eq!(
        base_dir(&system, &unit_out, Some(OsStr::new(triple))),
        target.join(triple)
    );
    fs::write(target.join(CACHEDIR_TAG), "")?;
    assert_eq!(
        base_dir(&system, &unit_out, Some(OsStr::new(triple))),
        target
    );
    fs::write(unit_out.join(CACHEDIR_TAG), "")?;
    assert_eq!(
        base_dir(&system, &unit_out, Some(OsStr::new(triple))),
        unit_out
    );
    fs::remove_dir_all(&dir)
}

struct Offline {
    system: System,
    out: PathBuf,
}

impl Host for Offline {
    fn var(&self, name: &str) -> Option<OsString> {
        match name {
            "OUT_DIR" => Some(self.out.clone().into_os_string()),
            "CARGO_NET_OFFLINE" => Some("true".into()),
            _ => None,
        }
    }

    fn exists(&self, path: &Path) -> io::Result<bool> {
        self.system.exists(path)
    }

    fn create_dir_all(&self, path: &Path) -> io::Result<()> {
        self.system.create_dir_all(path)
    }

    fn lock(&self, path: &Path) -> io::Result<()> {
        self.system.lock(path)
    }

    fn list(&self, dir: &Path) -> io::Result<Vec<OsString>> {
        self.system.list(dir)
    }

    fn remove_dir_all(&self, path: &Path) -> io::Result<()> {
        self.system.remove_dir_all(path)
    }

    fn write(&self, path: &Path, text: &str) -> io::Result<()> {
        self.system.write(path, text)
    }

    fn write_stamp(&self, path: &Path) -> io::Result<()> {
        self.system.write_stamp(path)
    }

    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        self.system.rename(from, to)
    }

    fn process_id(&self) -> u32 {
        self.system.process_id()
    }

    fn run(&self, invocation: &Invocation) -> io::Result<Finished> {
        self.system.run(invocation)
    }
}

#[test]
fn configure_reuses_the_clone_entry_that_the_build_script_of_this_build_wrote() -> io::Result<()> {
    let shared = PathBuf::from(env!("SLANG_SRC_SHARED_DIR"));
    let base = shared.parent().unwrap_or(&shared).to_path_buf();
    let host = Offline {
        system: System::default(),
        out: base.clone(),
    };
    assert_eq!(base_dir(&host, &base, None), base);
    let mut directives = Vec::new();
    if let Err(BuildError(message)) = configure(&host, &mut directives) {
        return Err(io::Error::other(message));
    }
    let entry = shared.join("src").join(clone_entry_name());
    let tree = entry.join("tree");
    assert_eq!(
        directives,
        [
            format!("cargo::rerun-if-changed={}", entry.join("stamp").display()),
            format!("cargo::metadata=root={}", tree.display()),
            format!("cargo::metadata=commit={RECORDED_COMMIT}"),
            format!("cargo::metadata=build_tag={RECORDED_BUILD_TAG}"),
            format!("cargo::metadata=shared_dir={}", shared.display()),
            format!("cargo::rustc-env=SLANG_SRC_SHARED_DIR={}", shared.display()),
        ]
    );
    assert!(tree.join(".git").is_dir());
    for submodule in REQUIRED_SUBMODULES {
        assert!(
            fs::exists(tree.join(submodule).join(".git"))?,
            "{submodule}"
        );
    }
    assert_eq!(
        fs::read_to_string(entry.join("build-tag"))?,
        format!("{RECORDED_BUILD_TAG}\n")
    );
    assert_eq!(fs::metadata(entry.join("stamp"))?.modified()?, stamp_time());
    let temporaries: Vec<OsString> = host
        .list(&shared.join("src"))?
        .into_iter()
        .filter(|name| name.to_string_lossy().contains(".tmp-"))
        .collect();
    assert_eq!(temporaries, Vec::<OsString>::new());
    Ok(())
}

#[test]
fn configure_rejects_a_clone_entry_whose_work_tree_was_removed() -> io::Result<()> {
    let base = scratch("entry-without-tree")?;
    fs::write(base.join(CACHEDIR_TAG), "")?;
    let entry = base.join("slang-src").join("src").join(clone_entry_name());
    fs::create_dir_all(&entry)?;
    let host = Offline {
        system: System::default(),
        out: base
            .join("debug")
            .join("build")
            .join("slang-src-0123")
            .join("out"),
    };
    let mut directives = Vec::new();
    let Err(BuildError(message)) = configure(&host, &mut directives) else {
        return Err(io::Error::other(
            "the build accepted an entry without its work tree",
        ));
    };
    assert_eq!(
        message,
        format!(
            "git cannot read the clone entry {} of Slang, because its work tree {} does not exist. Another build may be reading the entry, so the build leaves it in place. Remove the entry, or run cargo clean, when no build runs",
            entry.display(),
            entry.join("tree").display()
        )
    );
    assert_eq!(directives, Vec::<String>::new());
    fs::remove_dir_all(&base)
}
