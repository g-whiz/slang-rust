use std::cell::RefCell;
use std::collections::BTreeSet;

include!("../../recorded.rs");
include!("../../source.rs");

mod ffi;

type Responder = Box<dyn Fn(&Invocation) -> io::Result<Finished>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Op {
    Exists,
    Create,
    Lock,
    List,
    Remove,
    Write,
    Stamp,
    Rename,
}

struct FakeHost {
    vars: Vec<(&'static str, OsString)>,
    present: RefCell<BTreeSet<PathBuf>>,
    listing: Vec<OsString>,
    respond: Responder,
    checked_out: Vec<PathBuf>,
    failures: Vec<(Op, PathBuf, io::ErrorKind)>,
    read_vars: RefCell<Vec<String>>,
    events: RefCell<Vec<String>>,
    runs: RefCell<Vec<Invocation>>,
}

impl FakeHost {
    fn new() -> Self {
        Self {
            vars: vec![
                ("OUT_DIR", out().into_os_string()),
                ("TARGET", "x86_64-unknown-linux-gnu".into()),
            ],
            present: RefCell::new(BTreeSet::from([target().join(CACHEDIR_TAG)])),
            listing: Vec::new(),
            respond: Box::new(|invocation| Ok(answer(invocation))),
            checked_out: REQUIRED_SUBMODULES
                .iter()
                .map(|submodule| temporary_tree().join(submodule).join(".git"))
                .collect(),
            failures: Vec::new(),
            read_vars: RefCell::new(Vec::new()),
            events: RefCell::new(Vec::new()),
            runs: RefCell::new(Vec::new()),
        }
    }

    fn with_var(mut self, name: &'static str, value: impl Into<OsString>) -> Self {
        self.vars.retain(|(key, _)| *key != name);
        self.vars.push((name, value.into()));
        self
    }

    fn without_var(mut self, name: &str) -> Self {
        self.vars.retain(|(key, _)| *key != name);
        self
    }

    fn with_present(self, path: PathBuf) -> Self {
        self.present.borrow_mut().insert(path);
        self
    }

    fn with_entry(self) -> Self {
        self.with_present(entry())
            .with_present(entry().join("tree"))
    }

    fn responding(
        mut self,
        respond: impl Fn(&Invocation) -> io::Result<Finished> + 'static,
    ) -> Self {
        self.respond = Box::new(respond);
        self
    }

    fn failing(mut self, op: Op, path: PathBuf, kind: io::ErrorKind) -> Self {
        self.failures.push((op, path, kind));
        self
    }

    fn attempt(&self, op: Op, path: &Path) -> io::Result<()> {
        if let Some((_, _, kind)) = self
            .failures
            .iter()
            .find(|(failing, failing_path, _)| *failing == op && failing_path == path)
        {
            return Err(io::Error::from(*kind));
        }
        if op != Op::Exists {
            self.events
                .borrow_mut()
                .push(format!("{op:?} {}", path.display()));
        }
        Ok(())
    }

    fn events(&self) -> Vec<String> {
        self.events.borrow().clone()
    }

    fn runs(&self) -> Vec<Invocation> {
        self.runs.borrow().clone()
    }

    fn ran(&self, subcommand: &str) -> bool {
        self.runs()
            .iter()
            .any(|run| run.args.iter().any(|arg| arg == subcommand))
    }
}

impl Host for FakeHost {
    fn var(&self, name: &str) -> Option<OsString> {
        self.read_vars.borrow_mut().push(name.to_owned());
        self.vars
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| value.clone())
    }

    fn exists(&self, path: &Path) -> io::Result<bool> {
        self.attempt(Op::Exists, path)?;
        Ok(self.present.borrow().contains(path))
    }

    fn create_dir_all(&self, path: &Path) -> io::Result<()> {
        self.attempt(Op::Create, path)?;
        self.present.borrow_mut().insert(path.to_path_buf());
        Ok(())
    }

    fn lock(&self, path: &Path) -> io::Result<()> {
        self.attempt(Op::Lock, path)
    }

    fn list(&self, dir: &Path) -> io::Result<Vec<OsString>> {
        self.attempt(Op::List, dir)?;
        Ok(self.listing.clone())
    }

    fn remove_dir_all(&self, path: &Path) -> io::Result<()> {
        self.attempt(Op::Remove, path)
    }

    fn write(&self, path: &Path, text: &str) -> io::Result<()> {
        self.attempt(Op::Write, path)?;
        self.events.borrow_mut().push(format!("text {text:?}"));
        Ok(())
    }

    fn write_stamp(&self, path: &Path) -> io::Result<()> {
        self.attempt(Op::Stamp, path)
    }

    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        self.attempt(Op::Rename, from)?;
        self.events
            .borrow_mut()
            .push(format!("to {}", to.display()));
        Ok(())
    }

    fn process_id(&self) -> u32 {
        4242
    }

    fn run(&self, invocation: &Invocation) -> io::Result<Finished> {
        self.runs.borrow_mut().push(invocation.clone());
        self.events.borrow_mut().push(format!("Run {invocation}"));
        let finished = (self.respond)(invocation)?;
        if finished.code == Some(0) && invocation.args.iter().any(|arg| arg == "submodule") {
            self.present
                .borrow_mut()
                .extend(self.checked_out.iter().cloned());
        }
        Ok(finished)
    }
}

fn ok(stdout: &str) -> Finished {
    Finished {
        code: Some(0),
        stdout: stdout.to_owned(),
        stderr: String::new(),
    }
}

fn failed(code: Option<i32>, stderr: &str) -> Finished {
    Finished {
        code,
        stdout: String::new(),
        stderr: stderr.to_owned(),
    }
}

fn gitlinks(submodules: &[&str]) -> String {
    submodules
        .iter()
        .map(|submodule| {
            format!("160000 commit 0123456789abcdef0123456789abcdef01234567\t{submodule}\0")
        })
        .collect::<Vec<String>>()
        .concat()
}

fn answer(invocation: &Invocation) -> Finished {
    match invocation
        .args
        .iter()
        .find(|arg| !arg.starts_with('-') && !arg.contains('='))
        .map(String::as_str)
    {
        Some("rev-parse") => ok(&format!("{RECORDED_COMMIT}\n")),
        Some("ls-tree") => ok(&format!(
            "100644 blob 0123456789abcdef0123456789abcdef01234567\tCMakeLists.txt\0{}",
            gitlinks(&REQUIRED_SUBMODULES)
        )),
        _ => ok(""),
    }
}

fn root() -> PathBuf {
    std::env::temp_dir().join("project")
}

fn target() -> PathBuf {
    root().join("target")
}

fn out() -> PathBuf {
    target()
        .join("debug")
        .join("build")
        .join("slang-src-0123456789abcdef")
        .join("out")
}

fn shared() -> PathBuf {
    target().join("slang-src")
}

fn sources() -> PathBuf {
    shared().join("src")
}

fn entry() -> PathBuf {
    sources().join(clone_entry_name())
}

fn temporary() -> PathBuf {
    sources().join(format!("{}.tmp-4242", clone_entry_name()))
}

fn temporary_tree() -> PathBuf {
    temporary().join("tree")
}

fn run_configure(host: &FakeHost) -> (Result<(), BuildError>, Vec<String>) {
    let mut directives = Vec::new();
    let result = configure(host, &mut directives);
    (result, directives)
}

fn failure(host: &FakeHost) -> String {
    let (result, directives) = run_configure(host);
    assert_eq!(directives, Vec::<String>::new());
    match result {
        Err(BuildError(message)) => message,
        Ok(()) => "the build succeeded".to_owned(),
    }
}

fn exported(shared: &Path) -> Vec<String> {
    let entry = shared.join("src").join(clone_entry_name());
    vec![
        format!("cargo::rerun-if-changed={}", entry.join("stamp").display()),
        format!("cargo::metadata=root={}", entry.join("tree").display()),
        format!("cargo::metadata=commit={RECORDED_COMMIT}"),
        format!("cargo::metadata=build_tag={RECORDED_BUILD_TAG}"),
        format!("cargo::metadata=shared_dir={}", shared.display()),
        format!("cargo::rustc-env=SLANG_SRC_SHARED_DIR={}", shared.display()),
    ]
}

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|arg| (*arg).to_owned()).collect()
}

fn network_args(list: &[&str]) -> Vec<String> {
    args(&[NETWORK_SETTINGS.as_slice(), list].concat())
}

#[test]
fn configure_clones_the_recorded_commit_into_a_new_clone_entry() {
    let host = FakeHost::new();
    let (result, directives) = run_configure(&host);
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(directives, exported(&shared()));
    let tree = temporary_tree();
    let mut update = vec!["submodule", "update", "--init", "--depth", "1", "--"];
    update.extend(REQUIRED_SUBMODULES);
    let mut ls_tree = vec!["ls-tree", "-z", "HEAD", "--"];
    ls_tree.extend(REQUIRED_SUBMODULES);
    let commands: Vec<Vec<String>> = host.runs().into_iter().map(|run| run.args).collect();
    assert_eq!(
        commands,
        [
            args(&["init"]),
            args(&[
                "remote",
                "add",
                "origin",
                "https://github.com/shader-slang/slang.git"
            ]),
            network_args(&["fetch", "--depth", "1", "origin", RECORDED_COMMIT]),
            args(&["checkout", "--detach", "FETCH_HEAD"]),
            args(&ls_tree),
            network_args(&update),
        ]
        .into_iter()
        .chain(std::iter::repeat_n(
            args(&["ls-files", "-z", "--deleted"]),
            REQUIRED_SUBMODULES.len() + 1
        ))
        .collect::<Vec<_>>()
    );
    let dirs: Vec<PathBuf> = host.runs().into_iter().map(|run| run.dir).collect();
    let (clone_dirs, listed_dirs) = dirs.split_at(6);
    assert!(clone_dirs.iter().all(|dir| *dir == tree));
    assert_eq!(
        listed_dirs,
        std::iter::once(tree.clone())
            .chain(
                REQUIRED_SUBMODULES
                    .iter()
                    .map(|submodule| tree.join(submodule))
            )
            .collect::<Vec<_>>()
    );
    assert_eq!(
        host.events(),
        [
            format!("Create {}", sources().display()),
            format!(
                "Lock {}",
                sources()
                    .join(format!("{}.lock", clone_entry_name()))
                    .display()
            ),
            format!("List {}", sources().display()),
            format!("Create {}", tree.display()),
            format!("Run {}", host.runs()[0]),
            format!("Run {}", host.runs()[1]),
            format!("Run {}", host.runs()[2]),
            format!("Run {}", host.runs()[3]),
            format!("Run {}", host.runs()[4]),
            format!("Run {}", host.runs()[5]),
        ]
        .into_iter()
        .chain(host.runs()[6..].iter().map(|run| format!("Run {run}")))
        .chain([
            format!("Write {}", temporary().join("build-tag").display()),
            format!("text \"{RECORDED_BUILD_TAG}\\n\""),
            format!("Stamp {}", temporary().join("stamp").display()),
            format!("Rename {}", temporary().display()),
            format!("to {}", entry().display()),
        ])
        .collect::<Vec<_>>()
    );
}

#[test]
fn configure_runs_git_with_the_network_settings_only_where_it_fetches() {
    let host = FakeHost::new();
    let (result, _) = run_configure(&host);
    assert!(result.is_ok(), "{result:?}");
    for run in host.runs() {
        let fetches = run
            .args
            .iter()
            .any(|arg| arg == "fetch" || arg == "submodule");
        assert_eq!(run.network, fetches, "{run}");
        let mut removed = GIT_REPOSITORY_VARIABLES.to_vec();
        let mut set = vec![("GIT_OPTIONAL_LOCKS", "0")];
        if fetches {
            assert_eq!(run.args[..8], NETWORK_SETTINGS, "{run}");
            removed.push("GIT_ALLOW_PROTOCOL");
            set.push(("GIT_TERMINAL_PROMPT", "0"));
        } else {
            assert!(!run.args.iter().any(|arg| arg == "-c"), "{run}");
        }
        assert_eq!(run.env_remove, removed, "{run}");
        assert_eq!(run.env, set, "{run}");
    }
    assert!(GIT_REPOSITORY_VARIABLES.contains(&"GIT_DIR"));
    assert!(GIT_REPOSITORY_VARIABLES.contains(&"GIT_INDEX_FILE"));
}

#[test]
fn configure_uses_an_existing_clone_entry_without_the_network() {
    let host = FakeHost::new()
        .with_entry()
        .with_var("CARGO_NET_OFFLINE", "true");
    let (result, directives) = run_configure(&host);
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(directives, exported(&shared()));
    let runs = host.runs();
    assert_eq!(runs.len(), 1);
    assert_eq!(
        runs[0].args,
        args(&["rev-parse", "--verify", "HEAD^{commit}"])
    );
    assert_eq!(runs[0].dir, entry().join("tree"));
    assert!(!runs[0].network);
    assert_eq!(
        host.events(),
        [
            format!("Create {}", sources().display()),
            format!(
                "Lock {}",
                sources()
                    .join(format!("{}.lock", clone_entry_name()))
                    .display()
            ),
            format!("Run {}", runs[0]),
        ]
    );
}

#[test]
fn configure_reads_only_out_dir_target_and_cargo_net_offline() {
    let host = FakeHost::new();
    let (result, _) = run_configure(&host);
    assert!(result.is_ok(), "{result:?}");
    let read: BTreeSet<String> = host.read_vars.borrow().iter().cloned().collect();
    assert_eq!(
        read,
        BTreeSet::from([
            "CARGO_NET_OFFLINE".to_owned(),
            "OUT_DIR".to_owned(),
            "TARGET".to_owned()
        ])
    );
}

#[test]
fn configure_refuses_to_clone_when_cargo_net_offline_is_true() {
    let host = FakeHost::new().with_var("CARGO_NET_OFFLINE", "true");
    let message = failure(&host);
    assert_eq!(
        message,
        format!(
            "the vendored build needs to clone Slang at the commit {RECORDED_COMMIT}, which no earlier build in this target directory cloned, but CARGO_NET_OFFLINE is true, so the build cannot reach the network. {OFFLINE_ADVICE}"
        )
    );
    assert!(message.contains("needs network access for its first build in a target directory"));
    assert!(message.contains(
        "without the Cargo feature `vendored` links the Slang of an installed Vulkan SDK"
    ));
    assert_eq!(host.runs(), Vec::<Invocation>::new());
}

#[test]
fn configure_clones_when_cargo_net_offline_is_not_true() {
    for value in ["false", "1", "TRUE", ""] {
        let host = FakeHost::new().with_var("CARGO_NET_OFFLINE", value);
        let (result, _) = run_configure(&host);
        assert!(result.is_ok(), "{value}: {result:?}");
        assert!(host.ran("fetch"), "{value}");
    }
}

#[test]
fn configure_removes_only_the_numbered_temporary_directories_of_the_commit() {
    let mut host = FakeHost::new();
    host.listing = [
        format!("{}.tmp-12", clone_entry_name()),
        format!("{}.tmp-4242", clone_entry_name()),
        format!("{}.tmp-", clone_entry_name()),
        format!("{}.tmp-x1", clone_entry_name()),
        format!("{}.lock", clone_entry_name()),
        format!("{RECORDED_COMMIT}.tmp-7"),
        RECORDED_COMMIT.to_owned(),
        "0123456789abcdef0123456789abcdef01234567.tmp-7".to_owned(),
    ]
    .into_iter()
    .map(OsString::from)
    .collect();
    let (result, _) = run_configure(&host);
    assert!(result.is_ok(), "{result:?}");
    let removed: Vec<String> = host
        .events()
        .into_iter()
        .filter(|event| event.starts_with("Remove "))
        .collect();
    assert_eq!(
        removed,
        [
            format!(
                "Remove {}",
                sources()
                    .join(format!("{}.tmp-12", clone_entry_name()))
                    .display()
            ),
            format!("Remove {}", temporary().display()),
        ]
    );
    let events = host.events();
    let last_removal = events
        .iter()
        .rposition(|event| event.starts_with("Remove "))
        .unwrap_or(usize::MAX);
    let creation = events
        .iter()
        .position(|event| *event == format!("Create {}", temporary_tree().display()))
        .unwrap_or(0);
    assert!(last_removal < creation, "{events:?}");
}

#[test]
fn configure_rejects_a_clone_entry_at_another_commit() {
    let other = "0123456789abcdef0123456789abcdef01234567";
    let host = FakeHost::new()
        .with_entry()
        .responding(move |_| Ok(ok(&format!("{other}\n"))));
    let message = failure(&host);
    assert_eq!(
        message,
        format!(
            "the clone entry {} of Slang holds the commit {other}, not the recorded commit {RECORDED_COMMIT}. Another build may be reading the entry, so the build leaves it in place. Remove the entry, or run cargo clean, when no build runs",
            entry().display()
        )
    );
    assert!(!host.ran("fetch"));
}

#[test]
fn configure_rejects_a_clone_entry_that_git_cannot_read() {
    let host = FakeHost::new()
        .with_entry()
        .responding(|_| Ok(failed(Some(128), "fatal: not a git repository\n")));
    let message = failure(&host);
    assert_eq!(
        message,
        format!(
            "git cannot read the clone entry {} of Slang: `git rev-parse --verify HEAD^{{commit}}` ended with exit status 128: fatal: not a git repository. Another build may be reading the entry, so the build leaves it in place. Remove the entry, or run cargo clean, when no build runs",
            entry().display()
        )
    );
    assert!(
        host.events()
            .iter()
            .all(|event| !event.starts_with("Remove "))
    );
}

#[test]
fn configure_reports_git_that_cannot_start() {
    let host = FakeHost::new().responding(|_| Err(io::Error::from(io::ErrorKind::NotFound)));
    let message = failure(&host);
    assert_eq!(
        message,
        format!(
            "cannot start `git init`: {}. A vendored build needs git on PATH",
            io::Error::from(io::ErrorKind::NotFound)
        )
    );
    let host = FakeHost::new()
        .with_entry()
        .responding(|_| Err(io::Error::from(io::ErrorKind::PermissionDenied)));
    let message = failure(&host);
    assert_eq!(
        message,
        format!(
            "cannot start `git rev-parse --verify HEAD^{{commit}}`: {}",
            io::Error::from(io::ErrorKind::PermissionDenied)
        )
    );
}

#[test]
fn configure_rejects_a_clone_entry_without_its_work_tree() {
    let host = FakeHost::new().with_present(entry());
    let message = failure(&host);
    assert_eq!(
        message,
        format!(
            "git cannot read the clone entry {} of Slang, because its work tree {} does not exist. Another build may be reading the entry, so the build leaves it in place. Remove the entry, or run cargo clean, when no build runs",
            entry().display(),
            entry().join("tree").display()
        )
    );
    assert_eq!(host.runs(), Vec::<Invocation>::new());
    let host = FakeHost::new().with_present(entry()).failing(
        Op::Exists,
        entry().join("tree"),
        io::ErrorKind::PermissionDenied,
    );
    assert_eq!(
        failure(&host),
        format!(
            "cannot inspect {}: {}",
            entry().join("tree").display(),
            io::Error::from(io::ErrorKind::PermissionDenied)
        )
    );
}

#[test]
fn configure_reports_a_failed_fetch_with_the_offline_advice() {
    let host = FakeHost::new().responding(|invocation| {
        if invocation.args.iter().any(|arg| arg == "fetch") {
            Ok(failed(Some(128), "fatal: unable to access 'https://github.com/shader-slang/slang.git/': Could not resolve host: github.com\n"))
        } else {
            Ok(answer(invocation))
        }
    });
    let message = failure(&host);
    assert_eq!(
        message,
        format!(
            "`git -c protocol.allow=never -c protocol.https.allow=always -c http.lowSpeedLimit=1000 -c http.lowSpeedTime=60 fetch --depth 1 origin {RECORDED_COMMIT}` ended with exit status 128 while cloning Slang at the commit {RECORDED_COMMIT}: fatal: unable to access 'https://github.com/shader-slang/slang.git/': Could not resolve host: github.com. {OFFLINE_ADVICE}"
        )
    );
    assert!(!host.ran("checkout"));
    assert!(
        host.events()
            .iter()
            .all(|event| !event.starts_with("Rename "))
    );
}

#[test]
fn configure_reports_a_failed_submodule_update_with_the_offline_advice() {
    let host = FakeHost::new().responding(|invocation| {
        if invocation.args.iter().any(|arg| arg == "submodule") {
            Ok(failed(None, "error: RPC failed\n"))
        } else {
            Ok(answer(invocation))
        }
    });
    let message = failure(&host);
    assert!(message.starts_with("`git -c protocol.allow=never -c protocol.https.allow=always -c http.lowSpeedLimit=1000 -c http.lowSpeedTime=60 submodule update --init --depth 1 -- external/cmark "), "{message}");
    assert!(
        message.ends_with(&format!(
            "external/slang-rhi` ended with no exit status, because a signal ended it while cloning Slang at the commit {RECORDED_COMMIT}: error: RPC failed. {OFFLINE_ADVICE}"
        )),
        "{message}"
    );
}

#[test]
fn configure_reports_a_failed_local_git_command_with_its_directory() {
    let host = FakeHost::new().responding(|invocation| {
        if invocation.args.iter().any(|arg| arg == "checkout") {
            Ok(failed(
                Some(1),
                "error: unable to create file: No space left on device\n",
            ))
        } else {
            Ok(answer(invocation))
        }
    });
    let message = failure(&host);
    assert_eq!(
        message,
        format!(
            "`git checkout --detach FETCH_HEAD` in {} ended with exit status 1: error: unable to create file: No space left on device",
            temporary_tree().display()
        )
    );
    assert!(!message.contains(OFFLINE_ADVICE));
}

#[test]
fn configure_stops_when_a_required_submodule_has_no_gitlink() {
    let present: Vec<&str> = REQUIRED_SUBMODULES
        .into_iter()
        .filter(|submodule| *submodule != "external/glslang")
        .collect();
    let listing = gitlinks(&present)
        + "100644 blob 0123456789abcdef0123456789abcdef01234567\texternal/glslang\0";
    let host = FakeHost::new().responding(move |invocation| {
        if invocation.args.iter().any(|arg| arg == "ls-tree") {
            Ok(ok(&listing))
        } else {
            Ok(answer(invocation))
        }
    });
    let message = failure(&host);
    assert_eq!(
        message,
        format!(
            "the Slang commit {RECORDED_COMMIT} lacks the submodule external/glslang, which the native builds of Slang need. A commit patched in through the [patch] table of Cargo may lack it, so use the recorded commit of slang-src or a commit that has every submodule that it requires"
        )
    );
    assert!(!host.ran("submodule"));
}

#[test]
fn configure_stops_when_a_required_submodule_is_not_checked_out() {
    let mut host = FakeHost::new();
    let missing = temporary_tree().join("external/lua").join(".git");
    host.checked_out.retain(|path| *path != missing);
    let message = failure(&host);
    assert!(message.starts_with(&format!(
        "the Slang commit {RECORDED_COMMIT} lacks the submodule external/lua, which the native builds of Slang need."
    )), "{message}");
    assert!(
        host.events()
            .iter()
            .all(|event| !event.starts_with("Rename "))
    );
}

#[test]
fn configure_reports_every_file_operation_that_fails() {
    let denied = io::Error::from(io::ErrorKind::PermissionDenied);
    let lock = sources().join(format!("{}.lock", clone_entry_name()));
    let stale = sources().join(format!("{}.tmp-1", clone_entry_name()));
    let cases = [
        (
            Op::Create,
            sources(),
            format!("cannot create {}: {denied}", sources().display()),
        ),
        (
            Op::Lock,
            lock.clone(),
            format!("cannot lock {}: {denied}", lock.display()),
        ),
        (
            Op::Exists,
            entry(),
            format!("cannot inspect {}: {denied}", entry().display()),
        ),
        (
            Op::List,
            sources(),
            format!("cannot list {}: {denied}", sources().display()),
        ),
        (
            Op::Remove,
            stale.clone(),
            format!("cannot remove {}: {denied}", stale.display()),
        ),
        (
            Op::Create,
            temporary_tree(),
            format!("cannot create {}: {denied}", temporary_tree().display()),
        ),
        (
            Op::Exists,
            temporary_tree().join("external/vulkan").join(".git"),
            format!(
                "cannot inspect {}: {denied}",
                temporary_tree()
                    .join("external/vulkan")
                    .join(".git")
                    .display()
            ),
        ),
        (
            Op::Write,
            temporary().join("build-tag"),
            format!(
                "cannot write {}: {denied}",
                temporary().join("build-tag").display()
            ),
        ),
        (
            Op::Stamp,
            temporary().join("stamp"),
            format!(
                "cannot write {}: {denied}",
                temporary().join("stamp").display()
            ),
        ),
        (
            Op::Rename,
            temporary(),
            format!(
                "cannot rename {} to {}: {denied}",
                temporary().display(),
                entry().display()
            ),
        ),
    ];
    for (op, path, expected) in cases {
        let mut host = FakeHost::new().failing(op, path, io::ErrorKind::PermissionDenied);
        host.listing = vec![OsString::from(format!("{}.tmp-1", clone_entry_name()))];
        assert_eq!(failure(&host), expected);
    }
}

#[test]
fn configure_rejects_an_out_dir_that_cannot_be_a_cargo_instruction() {
    let advice = "which is not an absolute path in UTF-8 without line breaks. Cargo sets OUT_DIR when it runs a build script, so run the build through Cargo";
    let unset = FakeHost::new().without_var("OUT_DIR");
    assert_eq!(failure(&unset), format!("OUT_DIR is not set, {advice}"));
    let relative = FakeHost::new().with_var("OUT_DIR", "target/out");
    assert_eq!(
        failure(&relative),
        format!("OUT_DIR is \"target/out\", {advice}")
    );
    let broken = out().join("a\nb\tc").into_os_string();
    let shown = broken.to_string_lossy().escape_debug().to_string();
    assert!(shown.contains("a\\nb\\tc"), "{shown}");
    let host = FakeHost::new().with_var("OUT_DIR", broken);
    assert_eq!(failure(&host), format!("OUT_DIR is \"{shown}\", {advice}"));
    let host = FakeHost::new().with_var("OUT_DIR", out().join("a\rb"));
    assert!(failure(&host).contains("a\\rb"));
    assert_eq!(host.events(), Vec::<String>::new());
}

#[cfg(unix)]
#[test]
fn configure_rejects_an_out_dir_that_is_not_utf8() {
    use std::os::unix::ffi::OsStringExt;
    let host = FakeHost::new().with_var("OUT_DIR", OsString::from_vec(b"/tmp/\xff/out".to_vec()));
    assert!(failure(&host).starts_with("OUT_DIR is \"/tmp/\u{FFFD}/out\", which is not"));
}

#[cfg(windows)]
#[test]
fn configure_rejects_an_out_dir_that_is_not_utf8() {
    use std::os::windows::ffi::OsStringExt;
    let wide: Vec<u16> = "C:\\a\\"
        .encode_utf16()
        .chain([0xD800])
        .chain("\\out".encode_utf16())
        .collect();
    let host = FakeHost::new().with_var("OUT_DIR", OsString::from_wide(&wide));
    assert!(failure(&host).starts_with("OUT_DIR is \"C:\\\\a\\\\\u{FFFD}\\\\out\", which is not"));
}

fn lookup(tags: &[PathBuf], out: &Path, target: Option<&str>) -> PathBuf {
    let host = FakeHost::new();
    host.present.borrow_mut().clear();
    for tag in tags {
        host.present.borrow_mut().insert(tag.join(CACHEDIR_TAG));
    }
    base_dir(&host, out, target.map(OsStr::new))
}

#[test]
fn base_dir_is_the_nearest_tagged_ancestor_of_out_dir() {
    assert_eq!(
        lookup(&[target()], &out(), Some("x86_64-unknown-linux-gnu")),
        target()
    );
    assert_eq!(lookup(&[root(), target()], &out(), None), target());
    let deeper = target()
        .join("debug")
        .join("build")
        .join("slang-src")
        .join("0123456789abcdef")
        .join("out");
    assert_eq!(lookup(&[target()], &deeper, None), target());
    let build_dir = root().join("build-dir");
    let in_build_dir = build_dir
        .join("debug")
        .join("build")
        .join("slang-src-0123")
        .join("out");
    assert_eq!(
        lookup(&[target(), build_dir.clone()], &in_build_dir, None),
        build_dir
    );
    assert_eq!(lookup(&[out()], &out(), None), out());
}

#[test]
fn base_dir_is_out_dir_when_no_ancestor_is_tagged() {
    assert_eq!(lookup(&[], &out(), Some("x86_64-unknown-linux-gnu")), out());
    let host = FakeHost::new().failing(
        Op::Exists,
        target().join(CACHEDIR_TAG),
        io::ErrorKind::PermissionDenied,
    );
    assert_eq!(base_dir(&host, &out(), None), out());
}

#[test]
fn base_dir_of_a_target_unit_is_the_tagged_parent_of_the_directory_of_its_target() {
    let triple = "aarch64-unknown-linux-gnu";
    let triple_dir = target().join(triple);
    let unit_out = triple_dir
        .join("debug")
        .join("build")
        .join("slang-src-0123")
        .join("out");
    assert_eq!(
        lookup(&[target(), triple_dir.clone()], &unit_out, Some(triple)),
        target()
    );
    assert_eq!(
        lookup(std::slice::from_ref(&triple_dir), &unit_out, Some(triple)),
        triple_dir
    );
    assert_eq!(
        lookup(
            &[target(), triple_dir.clone()],
            &unit_out,
            Some("x86_64-unknown-linux-gnu")
        ),
        triple_dir
    );
    assert_eq!(
        lookup(&[target(), triple_dir.clone()], &unit_out, None),
        triple_dir
    );
}

#[test]
fn configure_exports_the_shared_folder_that_the_lookup_finds() {
    let triple = "aarch64-unknown-linux-gnu";
    let unit_out = target()
        .join(triple)
        .join("release")
        .join("build")
        .join("slang-src-0123")
        .join("out");
    let host = FakeHost::new()
        .with_var("OUT_DIR", unit_out.clone())
        .with_var("TARGET", triple)
        .with_present(target().join(triple).join(CACHEDIR_TAG));
    let (result, directives) = run_configure(&host);
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(directives, exported(&shared()));

    let mut untagged = FakeHost::new();
    untagged.present.borrow_mut().clear();
    let tree = out()
        .join("slang-src")
        .join("src")
        .join(format!("{}.tmp-4242", clone_entry_name()))
        .join("tree");
    untagged.checked_out = REQUIRED_SUBMODULES
        .iter()
        .map(|submodule| tree.join(submodule).join(".git"))
        .collect();
    let (result, directives) = run_configure(&untagged);
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(directives, exported(&out().join("slang-src")));
}

#[test]
fn check_recorded_rejects_a_build_tag_of_another_form() {
    let form = "<major>.<minor>, optionally followed by .<patch>, optionally followed by -<count>-g<hash>, in which each number is 0 or a decimal number without a leading zero and <hash> is at least nine lowercase hexadecimal digits";
    for tag in ["v2026.17", "2026.17-25", "2026.17-25-gc7954eb", "2026\n.17"] {
        let Err(BuildError(message)) = check_recorded(RECORDED_COMMIT, tag, RECORDED_CLONE_ENTRY)
        else {
            panic!("{tag} was accepted");
        };
        assert_eq!(
            message,
            format!(
                "the recorded build tag is \"{}\", which does not have the form of a build tag: {form}",
                tag.escape_debug()
            )
        );
    }
}

#[test]
fn check_recorded_rejects_a_commit_that_is_not_a_full_lowercase_hash() {
    for commit in [
        "c7954ebd1",
        "C7954EBD1F1984AD2BFDE15F76248DB41DC34AA3",
        "../../../../escape/0123456789abcdef0123456",
        "",
    ] {
        let Err(BuildError(message)) =
            check_recorded(commit, RECORDED_BUILD_TAG, RECORDED_CLONE_ENTRY)
        else {
            panic!("{commit} was accepted");
        };
        assert_eq!(
            message,
            format!(
                "the recorded commit is \"{commit}\", which is not a full commit hash of 40 lowercase hexadecimal digits"
            )
        );
    }
    assert!(check_recorded(RECORDED_COMMIT, RECORDED_BUILD_TAG, RECORDED_CLONE_ENTRY).is_ok());
}

#[test]
fn the_recorded_clone_entry_is_the_name_that_the_build_computes() {
    assert_eq!(RECORDED_CLONE_ENTRY, clone_entry_name());
    let stale = format!("{RECORDED_COMMIT}-076690edef70bf9e");
    let Err(BuildError(message)) = check_recorded(RECORDED_COMMIT, RECORDED_BUILD_TAG, &stale)
    else {
        panic!("{stale} was accepted");
    };
    assert_eq!(
        message,
        format!(
            "the recorded clone entry is \"{stale}\", but the recorded commit and the required submodules name the clone entry \"{}\". Record that name as RECORDED_CLONE_ENTRY in recorded.rs, where the recipes read it",
            clone_entry_name()
        )
    );
}

#[test]
fn slang_version_writes_every_part_of_a_build_tag_as_a_number() {
    let commit = "abcdef0123456789abcdef0123456789abcdef01";
    let cases = [
        ("2026.17-25-gc7954ebd1", Some("2026.17.0.25.gc7954ebd1")),
        ("2026.17.1-3-g0123456789ab", Some("2026.17.1.3.g012345678")),
        ("2026.17", Some("2026.17.0.0.gabcdef012")),
        ("2026.17.2", Some("2026.17.2.0.gabcdef012")),
        ("0.0-0-g000000000", Some("0.0.0.0.g000000000")),
        ("2026", None),
        ("2026.17.1.2", None),
        ("2026.017", None),
        ("2026.17-025-gc7954ebd1", None),
        ("2026.17-x-gc7954ebd1", None),
        ("2026.17-25-gC7954EBD1", None),
        ("2026.17-25-gc7954ebd", None),
        ("2026.17-25-c7954ebd1", None),
        ("2026.-1", None),
        ("", None),
    ];
    for (tag, expected) in cases {
        assert_eq!(slang_version(tag, commit).as_deref(), expected, "{tag}");
    }
}

#[test]
fn recorded_constants_match_the_version_of_the_manifest() {
    let pre = env!("CARGO_PKG_VERSION_PRE");
    let Some(identifiers) = slang_version(RECORDED_BUILD_TAG, RECORDED_COMMIT)
        .map(|version| format!("slang.{version}"))
    else {
        panic!("{RECORDED_BUILD_TAG} is not a build tag that slang_version reads");
    };
    let release = pre
        .strip_suffix(&identifiers)
        .unwrap_or_else(|| panic!("the pre-release {pre} does not end with {identifiers}"));
    assert!(
        release.is_empty() || release.ends_with('.'),
        "the pre-release {pre} does not separate {identifiers} with a dot"
    );
    if let Some((_, hash)) = RECORDED_BUILD_TAG.rsplit_once("-g") {
        assert!(
            RECORDED_COMMIT.starts_with(hash),
            "{RECORDED_BUILD_TAG} does not abbreviate {RECORDED_COMMIT}"
        );
    }
}

#[test]
fn crate_documentation_names_the_recorded_commit_and_its_build_tag() {
    let documentation = include_str!("../../src/lib.rs");
    assert!(documentation.contains(&format!("`{RECORDED_COMMIT}`")));
    assert!(documentation.contains(&format!("`{RECORDED_BUILD_TAG}`")));
    assert!(documentation.contains(&format!("`{}`", env!("CARGO_PKG_VERSION"))));
}

#[test]
fn fnv1a_gives_the_published_values_of_the_64_bit_hash() {
    assert_eq!(fnv1a(b""), "cbf29ce484222325");
    assert_eq!(fnv1a(b"a"), "af63dc4c8601ec8c");
    assert_eq!(fnv1a(b"foobar"), "85944171f73967e8");
}

#[test]
fn clone_entry_name_is_a_commit_prefix_and_the_hash_of_the_required_submodules() {
    let paths = REQUIRED_SUBMODULES.join("\0") + "\0";
    let name = clone_entry_name();
    let commit = &RECORDED_COMMIT[..23];
    assert_eq!(name, format!("{commit}-{}", fnv1a(paths.as_bytes())));
    let Some(hash) = name.strip_prefix(&format!("{commit}-")) else {
        panic!("{name} does not start with the first 23 digits of the recorded commit");
    };
    assert_eq!(hash.len(), 16);
    assert!(
        hash.bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
    );
    let without_rhi = REQUIRED_SUBMODULES
        .iter()
        .filter(|path| **path != "external/slang-rhi")
        .fold(String::new(), |text, path| text + path + "\0");
    assert_ne!(hash, fnv1a(without_rhi.as_bytes()));
}

#[test]
fn clone_entry_name_is_at_most_as_long_as_a_full_commit_hash() {
    let name = clone_entry_name();
    assert!(
        name.len() <= 40,
        "the clone entry {name} has {} characters, but every path of the Slang tree in the clone grows with the name, and with a name longer than 40 characters the longest paths of the tree exceed the limit of 260 characters of a Windows path in the target directory of a CI runner",
        name.len()
    );
    assert!(
        name.bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f' | b'-')),
        "{name} holds a character other than a lowercase hexadecimal digit or a hyphen"
    );
}

#[test]
fn configure_neither_reads_nor_removes_a_clone_entry_of_an_earlier_layout() {
    for name in [
        RECORDED_COMMIT.to_owned(),
        format!("{RECORDED_COMMIT}-{}", &clone_entry_name()[24..]),
    ] {
        configure_ignores_the_entry(&sources().join(name));
    }
}

fn configure_ignores_the_entry(old: &Path) {
    let old = old.to_path_buf();
    let host = FakeHost::new()
        .with_present(old.clone())
        .with_present(old.join("tree"));
    let (result, directives) = run_configure(&host);
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(directives, exported(&shared()));
    assert!(host.ran("fetch"));
    let old = old.display().to_string();
    for event in host.events() {
        let path = event.split_once(' ').map_or("", |(_, path)| path);
        assert!(
            path != old && !path.starts_with(&format!("{old}/")),
            "{event}"
        );
    }
    assert!(host.runs().iter().all(|run| !run.dir.starts_with(&old)));
}

#[test]
fn crate_documentation_names_every_required_submodule() {
    let documentation = include_str!("../../src/lib.rs");
    for submodule in REQUIRED_SUBMODULES {
        assert!(
            documentation.contains(&format!("`{submodule}`")),
            "{submodule}"
        );
    }
}

#[test]
fn configure_rejects_a_clone_whose_work_tree_lacks_tracked_files() {
    let long = "docs/generated/tests/design/cross-cutting/diagnostics-catalog/30311-type-cannot-conform-to-both-value-and-pointer-diff-interfaces.slang";
    let host = FakeHost::new().responding(move |invocation| {
        Ok(match invocation.args.first().map(String::as_str) {
            Some("ls-files") if invocation.dir == temporary_tree() => {
                ok(&format!("docs/short.md\0{long}\0"))
            }
            Some("ls-files") if invocation.dir == temporary_tree().join("external/lua") => {
                ok("lua.c\0")
            }
            _ => answer(invocation),
        })
    });
    let path = entry().join("tree").join(long);
    assert_eq!(
        failure(&host),
        format!(
            "the clone of the Slang commit {RECORDED_COMMIT} lacks 3 of its tracked files, which git did not create in the work tree {}, such as {long}, whose path in the clone entry {} has {} characters. Git leaves out a file that it cannot create, most often because its path exceeds the limit of 260 characters of a Windows path, which the compilers and CMake that read the tree observe as well. The clone entry sits in the shared folder {} of the Cargo target directory, so set CARGO_TARGET_DIR to a directory with a shorter path and build again",
            temporary_tree().display(),
            path.display(),
            path.display().to_string().chars().count(),
            shared().display()
        )
    );
    assert!(
        !host
            .events()
            .iter()
            .any(|event| event.starts_with("Rename"))
    );
}

#[test]
fn configure_names_a_missing_file_of_a_submodule_with_the_path_of_the_submodule() {
    let host = FakeHost::new().responding(|invocation| {
        Ok(match invocation.args.first().map(String::as_str) {
            Some("ls-files") if invocation.dir == temporary_tree().join("external/spirv-tools") => {
                ok("source/opt/pass.cpp\0")
            }
            _ => answer(invocation),
        })
    });
    let message = failure(&host);
    assert!(
        message.contains(" lacks 1 of its tracked files, ")
            && message.contains(", such as external/spirv-tools/source/opt/pass.cpp, "),
        "{message}"
    );
}

#[test]
fn configure_stops_when_git_cannot_list_the_missing_files() {
    let host = FakeHost::new().responding(|invocation| {
        Ok(match invocation.args.first().map(String::as_str) {
            Some("ls-files") => failed(Some(128), "fatal: not a git repository\n"),
            _ => answer(invocation),
        })
    });
    assert_eq!(
        failure(&host),
        format!(
            "`git ls-files -z --deleted` in {} ended with exit status 128: fatal: not a git repository",
            temporary_tree().display()
        )
    );
}
