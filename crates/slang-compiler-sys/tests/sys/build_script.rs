use std::cell::RefCell;

use slang_compiler_sys::{SLANG_BUILD_TAG, SLANG_COMMIT, VULKAN_SDK_VERSION};

include!("../../route.rs");

mod ffi;

type Responder = Box<dyn Fn(&Invocation) -> io::Result<Finished>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FileOp {
    List,
    OpenLog,
    Sync,
    Remove,
    Write,
}

struct FakeHost {
    vars: Vec<(&'static str, OsString)>,
    files: Vec<(PathBuf, String)>,
    entries: Vec<(PathBuf, Result<EntryKind, io::ErrorKind>)>,
    host_os: &'static str,
    respond: Responder,
    copied: Option<Vec<PathBuf>>,
    failures: Vec<(FileOp, PathBuf, io::ErrorKind)>,
    read_vars: RefCell<Vec<String>>,
    runs: RefCell<Vec<(Invocation, Option<PathBuf>)>>,
    synced: RefCell<Vec<(PathBuf, PathBuf)>>,
    removed: RefCell<Vec<PathBuf>>,
    written: RefCell<Vec<(PathBuf, String)>>,
    events: RefCell<Vec<String>>,
}

impl FakeHost {
    fn attempt(&self, op: FileOp, path: &Path) -> io::Result<()> {
        if let Some((_, _, kind)) = self
            .failures
            .iter()
            .find(|(failing, failing_path, _)| *failing == op && failing_path == path)
        {
            return Err(io::Error::from(*kind));
        }
        self.events
            .borrow_mut()
            .push(format!("{op:?} {}", path.display()));
        Ok(())
    }

    fn record_run(&self, invocation: &Invocation, log: Option<&Path>) {
        self.runs
            .borrow_mut()
            .push((invocation.clone(), log.map(Path::to_path_buf)));
        self.events.borrow_mut().push(format!("Run {invocation}"));
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

    fn read(&self, path: &Path) -> io::Result<String> {
        self.files
            .iter()
            .find(|(file, _)| file == path)
            .map(|(_, text)| text.clone())
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
    }

    fn kind(&self, path: &Path) -> io::Result<EntryKind> {
        self.entries
            .iter()
            .find(|(entry, _)| entry == path)
            .map_or(Err(io::ErrorKind::NotFound), |(_, kind)| *kind)
            .map_err(io::Error::from)
    }

    fn host_os(&self) -> &str {
        self.host_os
    }

    fn run(&self, invocation: &Invocation) -> io::Result<Finished> {
        self.record_run(invocation, None);
        (self.respond)(invocation)
    }

    fn run_to_log(&self, invocation: &Invocation, log: &Path) -> Result<Exit, LoggedRunError> {
        self.attempt(FileOp::OpenLog, log)
            .map_err(LoggedRunError::Log)?;
        self.record_run(invocation, Some(log));
        (self.respond)(invocation)
            .map(|finished| finished.exit)
            .map_err(LoggedRunError::Start)
    }

    fn files(&self, root: &Path) -> io::Result<Vec<PathBuf>> {
        self.attempt(FileOp::List, root)?;
        match &self.copied {
            Some(files) if root == copy_dir() => Ok(files.clone()),
            _ => Err(io::Error::from(io::ErrorKind::NotFound)),
        }
    }

    fn sync_file(&self, from: &Path, to: &Path) -> io::Result<()> {
        self.attempt(FileOp::Sync, to)?;
        self.synced
            .borrow_mut()
            .push((from.to_path_buf(), to.to_path_buf()));
        Ok(())
    }

    fn remove_file(&self, path: &Path) -> io::Result<()> {
        self.attempt(FileOp::Remove, path)?;
        self.removed.borrow_mut().push(path.to_path_buf());
        Ok(())
    }

    fn write(&self, path: &Path, text: &str) -> io::Result<()> {
        self.attempt(FileOp::Write, path)?;
        self.written
            .borrow_mut()
            .push((path.to_path_buf(), text.to_owned()));
        Ok(())
    }
}

fn sdk_root() -> PathBuf {
    std::env::temp_dir().join("vulkan-sdk")
}

fn tag_header() -> PathBuf {
    sdk_root()
        .join("include")
        .join("slang")
        .join("slang-tag-version.h")
}

fn header_text(tag: &str) -> String {
    format!("#pragma once\n#define {TAG_MACRO} \"{tag}\"\n#define SLANG_VERSION_NUMERIC \"0\"\n")
}

fn lib_dir() -> PathBuf {
    sdk_root().join("lib")
}

fn empty_host() -> FakeHost {
    FakeHost {
        vars: Vec::new(),
        files: Vec::new(),
        entries: Vec::new(),
        host_os: "linux",
        respond: Box::new(|_| Err(io::Error::from(io::ErrorKind::Unsupported))),
        copied: None,
        failures: Vec::new(),
        read_vars: RefCell::default(),
        runs: RefCell::default(),
        synced: RefCell::default(),
        removed: RefCell::default(),
        written: RefCell::default(),
        events: RefCell::default(),
    }
}

fn sdk_host(header: &str) -> FakeHost {
    FakeHost {
        vars: vec![
            (VULKAN_SDK, sdk_root().into_os_string()),
            (TARGET_OS, OsString::from("linux")),
        ],
        files: vec![(tag_header(), header.to_owned())],
        entries: vec![
            (lib_dir(), Ok(EntryKind::Directory)),
            (lib_dir().join("libslang-compiler.so"), Ok(EntryKind::File)),
        ],
        ..empty_host()
    }
}

fn set_target_os(host: &mut FakeHost, os: &str) {
    host.vars.retain(|(name, _)| *name != TARGET_OS);
    host.vars.push((TARGET_OS, OsString::from(os)));
}

fn set_entry(host: &mut FakeHost, path: PathBuf, kind: Result<EntryKind, io::ErrorKind>) {
    host.entries.retain(|(entry, _)| *entry != path);
    host.entries.push((path, kind));
}

fn pinned_sdk_host() -> FakeHost {
    sdk_host(&header_text(SLANG_BUILD_TAG))
}

fn run(host: &FakeHost) -> (Vec<String>, Result<(), String>) {
    let mut directives = Vec::new();
    let result = configure(host, &mut directives).map_err(|error| format!("{error:?}"));
    (directives, result)
}

fn assert_contains_all(message: &str, parts: &[&str]) {
    for part in parts {
        assert!(message.contains(part), "{message:?} lacks {part:?}");
    }
}

fn assert_reruns_on_the_sdk(directives: &[String]) {
    for directive in [
        "cargo::rerun-if-env-changed=VULKAN_SDK".to_owned(),
        format!("cargo::rerun-if-changed={}", tag_header().display()),
    ] {
        assert!(
            directives.contains(&directive),
            "{directives:?} lacks {directive:?}"
        );
    }
}

fn assert_no_link_directives(directives: &[String]) {
    assert!(
        directives
            .iter()
            .all(|directive| !directive.starts_with("cargo::rustc-link")
                && !directive.starts_with("cargo::metadata")),
        "{directives:?}"
    );
}

fn link_directives() -> [String; 4] {
    let root = sdk_root();
    [
        format!("cargo::rustc-link-search=native={}", lib_dir().display()),
        "cargo::rustc-link-lib=dylib=slang-compiler".to_owned(),
        format!("cargo::metadata=include={}", root.join("include").display()),
        format!("cargo::metadata=lib={}", lib_dir().display()),
    ]
}

#[test]
fn configure_links_the_sdk_library_when_the_build_tag_matches_the_pin() {
    let (directives, result) = run(&pinned_sdk_host());
    assert_eq!(result, Ok(()));
    let mut expected = vec![
        "cargo::rerun-if-env-changed=DOCS_RS".to_owned(),
        "cargo::rerun-if-env-changed=SLANG_SOURCE_DIR".to_owned(),
        "cargo::rerun-if-env-changed=VULKAN_SDK".to_owned(),
        format!("cargo::rerun-if-changed={}", tag_header().display()),
    ];
    expected.extend(link_directives());
    assert_eq!(directives, expected);
}

#[test]
fn configure_with_docs_rs_emits_only_its_rerun_directive() {
    for vars in [
        vec![(DOCS_RS, OsString::from("1"))],
        vec![
            (DOCS_RS, OsString::new()),
            (SLANG_SOURCE_DIR, OsString::from("/tmp/slang")),
            (VULKAN_SDK, OsString::from("relative")),
        ],
    ] {
        let host = FakeHost {
            vars,
            ..empty_host()
        };
        let (directives, result) = run(&host);
        assert_eq!(result, Ok(()));
        assert_eq!(directives, ["cargo::rerun-if-env-changed=DOCS_RS"]);
    }
}

#[test]
fn configure_without_vulkan_sdk_names_the_variable_the_version_and_the_static_route() {
    let empty = FakeHost {
        vars: vec![(VULKAN_SDK, OsString::new())],
        ..empty_host()
    };
    for (host, state) in [(empty_host(), "is not set"), (empty, "is set but empty")] {
        let message = run(&host).1.expect_err("the build must stop");
        assert_contains_all(
            &message,
            &[
                &format!("{VULKAN_SDK} {state}"),
                VULKAN_SDK_VERSION,
                SLANG_SOURCE_DIR,
            ],
        );
    }
}

#[cfg(unix)]
fn non_utf8_path() -> OsString {
    std::os::unix::ffi::OsStringExt::from_vec(vec![b'/', 0xff])
}

#[cfg(windows)]
fn non_utf8_path() -> OsString {
    std::os::windows::ffi::OsStringExt::from_wide(&[0x43, 0x3a, 0x5c, 0xd800])
}

#[test]
fn configure_rejects_a_vulkan_sdk_that_is_relative_not_utf8_or_has_line_breaks() {
    let values = [
        OsString::from("vulkan-sdk"),
        sdk_root()
            .join("x\ncargo::rustc-link-lib=other")
            .into_os_string(),
        sdk_root()
            .join("x\rcargo::rustc-link-lib=other")
            .into_os_string(),
        non_utf8_path(),
    ];
    for value in values {
        let host = FakeHost {
            vars: vec![(VULKAN_SDK, value)],
            ..empty_host()
        };
        let (directives, result) = run(&host);
        let message = result.expect_err("the path must be rejected");
        assert_contains_all(
            &message,
            &[
                VULKAN_SDK,
                "is not an absolute path in UTF-8 without line breaks",
                VULKAN_SDK_VERSION,
            ],
        );
        assert!(!message.contains(['\n', '\r']), "{message:?}");
        assert_no_link_directives(&directives);
    }
}

#[test]
fn configure_without_the_tag_header_names_its_path_and_the_sdk_version() {
    let mut host = pinned_sdk_host();
    host.files.clear();
    let (directives, result) = run(&host);
    let message = result.expect_err("the build must stop");
    assert_contains_all(
        &message,
        &[&tag_header().display().to_string(), VULKAN_SDK_VERSION],
    );
    assert_reruns_on_the_sdk(&directives);
    assert_no_link_directives(&directives);
}

#[test]
fn configure_without_the_tag_definition_names_the_path_and_the_pinned_tag() {
    let (directives, result) = run(&sdk_host("#pragma once\n"));
    let message = result.expect_err("the build must stop");
    assert_contains_all(
        &message,
        &[
            &tag_header().display().to_string(),
            &format!("does not define {TAG_MACRO}"),
            SLANG_BUILD_TAG,
        ],
    );
    assert_reruns_on_the_sdk(&directives);
    assert_no_link_directives(&directives);
}

#[test]
fn configure_with_a_malformed_tag_names_the_line_the_path_and_the_pinned_tag() {
    for (text, line) in [
        ("#define SLANG_TAG_VERSION 2026\n", 1),
        ("\n#define SLANG_TAG_VERSION \"a\" \"b\"\n", 2),
        ("#define SLANG_TAG_VERSION \"a\"b\"\n", 1),
        ("#define SLANG_TAG_VERSION \"2026.17-25-gc7954ebd1\n", 1),
        ("#define SLANG_TAG_VERSION\n", 1),
    ] {
        let (directives, result) = run(&sdk_host(text));
        let message = result.expect_err("a malformed tag must stop the build");
        assert_contains_all(
            &message,
            &[
                &format!("line {line} of {}", tag_header().display()),
                SLANG_BUILD_TAG,
            ],
        );
        assert_no_link_directives(&directives);
    }
}

#[test]
fn configure_with_another_build_tag_names_both_tags_the_version_and_the_path() {
    let (directives, result) = run(&sdk_host(&header_text("2000.1-1-gdeadbeef")));
    let message = result.expect_err("another build tag must stop the build");
    assert_contains_all(
        &message,
        &[
            "2000.1-1-gdeadbeef",
            SLANG_BUILD_TAG,
            VULKAN_SDK_VERSION,
            &tag_header().display().to_string(),
        ],
    );
    assert_reruns_on_the_sdk(&directives);
    assert_no_link_directives(&directives);
}

#[test]
fn configure_reads_the_last_tag_definition_and_accepts_crlf_lines() {
    let redefined = format!(
        "#define {TAG_MACRO} \"2000.1-1-gdeadbeef\"\n#undef {TAG_MACRO}\n#define {TAG_MACRO} \"{SLANG_BUILD_TAG}\"\n"
    );
    let crlf = header_text(SLANG_BUILD_TAG).replace('\n', "\r\n");
    for text in [redefined, crlf] {
        assert_eq!(run(&sdk_host(&text)).1, Ok(()));
    }
}

fn assert_library_failure(host: &FakeHost, problem: &str) {
    let (directives, result) = run(host);
    let message = result.err().unwrap_or_default();
    assert_contains_all(
        &message,
        &[
            problem,
            &lib_dir().display().to_string(),
            VULKAN_SDK_VERSION,
        ],
    );
    assert_reruns_on_the_sdk(&directives);
    assert_no_link_directives(&directives);
}

#[test]
fn configure_reports_why_the_library_directory_cannot_be_used() {
    let lib = lib_dir().display().to_string();
    let missing = io::Error::from(io::ErrorKind::NotFound).to_string();
    let denied = io::Error::from(io::ErrorKind::PermissionDenied).to_string();
    for (kind, problem) in [
        (
            Err(io::ErrorKind::NotFound),
            format!("cannot access {lib}: {missing}"),
        ),
        (
            Err(io::ErrorKind::PermissionDenied),
            format!("cannot access {lib}: {denied}"),
        ),
        (Ok(EntryKind::File), format!("{lib} is not a directory")),
        (Ok(EntryKind::Other), format!("{lib} is not a directory")),
    ] {
        let mut host = pinned_sdk_host();
        set_entry(&mut host, lib_dir(), kind);
        assert_library_failure(&host, &problem);
    }
}

#[test]
fn configure_for_the_host_os_requires_the_library_file_of_the_target() {
    let missing = io::Error::from(io::ErrorKind::NotFound).to_string();
    let denied = io::Error::from(io::ErrorKind::PermissionDenied).to_string();
    for (os, name) in [
        ("linux", "libslang-compiler.so"),
        ("macos", "libslang-compiler.dylib"),
        ("windows", "slang-compiler.lib"),
    ] {
        let file = lib_dir().join(name);
        let path = file.display().to_string();
        let mut host = pinned_sdk_host();
        host.host_os = os;
        set_target_os(&mut host, os);
        for (kind, problem) in [
            (
                Err(io::ErrorKind::NotFound),
                format!("cannot access {path}: {missing}"),
            ),
            (
                Err(io::ErrorKind::PermissionDenied),
                format!("cannot access {path}: {denied}"),
            ),
            (Ok(EntryKind::Directory), format!("{path} is not a file")),
        ] {
            set_entry(&mut host, file.clone(), kind);
            assert_library_failure(&host, &problem);
        }
        set_entry(&mut host, file, Ok(EntryKind::File));
        let (directives, result) = run(&host);
        assert_eq!(result, Ok(()));
        assert!(directives.ends_with(&link_directives()), "{directives:?}");
    }
}

#[test]
fn configure_for_another_os_or_an_unknown_one_links_without_the_library_file() {
    for (host_os, target_os) in [
        ("linux", Some("windows")),
        ("linux", Some("macos")),
        ("macos", Some("linux")),
        ("freebsd", Some("freebsd")),
        ("linux", None),
    ] {
        let mut host = pinned_sdk_host();
        host.host_os = host_os;
        host.vars.retain(|(name, _)| *name != TARGET_OS);
        host.vars
            .extend(target_os.map(|os| (TARGET_OS, OsString::from(os))));
        host.entries.retain(|(entry, _)| *entry == lib_dir());
        let (directives, result) = run(&host);
        assert_eq!(result, Ok(()), "{host_os} {target_os:?}");
        assert!(directives.ends_with(&link_directives()), "{directives:?}");
    }
}

const TRIPLE: &str = "x86_64-unknown-linux-gnu";

fn source_root() -> PathBuf {
    std::env::temp_dir().join("slang")
}

fn out_root() -> PathBuf {
    std::env::temp_dir().join("out")
}

fn copy_dir() -> PathBuf {
    out_root().join("slang").join("source")
}

fn build_dir() -> PathBuf {
    out_root().join("slang").join("build")
}

fn archive_dir() -> PathBuf {
    out_root().join("slang").join("lib")
}

fn log_file() -> PathBuf {
    out_root().join("slang").join("build.log")
}

fn stamp_file() -> PathBuf {
    out_root().join("slang").join("stamp")
}

fn built_include() -> PathBuf {
    build_dir().join("Release").join("include")
}

fn finished(code: i32, stdout: &str, stderr: &str) -> Finished {
    Finished {
        exit: Exit { code: Some(code) },
        stdout: stdout.to_owned(),
        stderr: stderr.to_owned(),
    }
}

fn superproject_listing() -> String {
    let mut entries = vec![
        "100644 1111111111111111111111111111111111111111 0\tCMakeLists.txt".to_owned(),
        "100644 2222222222222222222222222222222222222222 0\tsource/slang/slang.cpp".to_owned(),
        "160000 3333333333333333333333333333333333333333 0\texternal/glslang".to_owned(),
    ];
    entries.extend(REQUIRED_SUBMODULES.iter().map(|submodule| {
        format!("160000 4444444444444444444444444444444444444444 0\t{submodule}")
    }));
    entries.join("\0") + "\0"
}

fn subcommand(invocation: &Invocation) -> (&str, &str) {
    (
        invocation.program,
        invocation.args.first().map_or("", String::as_str),
    )
}

fn happy(invocation: &Invocation) -> io::Result<Finished> {
    match subcommand(invocation) {
        ("git", "rev-parse") => Ok(finished(
            0,
            &format!("true\n\n.git/HEAD\n.git/index\n{SLANG_COMMIT}\n"),
            "",
        )),
        ("git", "status") | ("cmake", _) => Ok(finished(0, "", "")),
        ("git", "ls-files") if invocation.dir == Some(source_root()) => {
            Ok(finished(0, &superproject_listing(), ""))
        }
        ("git", "ls-files") => Ok(finished(
            0,
            "100644 5555555555555555555555555555555555555555 0\tCMakeLists.txt\0",
            "",
        )),
        ("ninja", "--version") => Ok(finished(0, "1.12.1\n", "")),
        _ => Err(io::Error::from(io::ErrorKind::NotFound)),
    }
}

fn static_host() -> FakeHost {
    let mut entries = vec![(source_root(), Ok(EntryKind::Directory))];
    for submodule in REQUIRED_SUBMODULES {
        entries.push((
            source_root().join(submodule).join(".git"),
            Ok(EntryKind::File),
        ));
    }
    for library in STATIC_LIBRARIES {
        entries.push((
            archive_dir().join(format!("lib{library}.a")),
            Ok(EntryKind::File),
        ));
    }
    FakeHost {
        vars: vec![
            (SLANG_SOURCE_DIR, source_root().into_os_string()),
            (OUT_DIR, out_root().into_os_string()),
            (TARGET, OsString::from(TRIPLE)),
            (HOST, OsString::from(TRIPLE)),
            (TARGET_OS, OsString::from("linux")),
            (TARGET_ENV, OsString::from("gnu")),
            (TARGET_FEATURE, OsString::from("fxsr,sse,sse2")),
            (NUM_JOBS, OsString::from("8")),
            (VULKAN_SDK, OsString::from("relative")),
        ],
        files: vec![(
            built_include().join("slang-tag-version.h"),
            header_text(SLANG_BUILD_TAG),
        )],
        entries,
        respond: Box::new(happy),
        ..empty_host()
    }
}

fn respond_with(
    host: &mut FakeHost,
    program: &'static str,
    first: &'static str,
    result: fn() -> io::Result<Finished>,
) {
    host.respond = Box::new(move |invocation| {
        if subcommand(invocation) == (program, first) {
            result()
        } else {
            happy(invocation)
        }
    });
}

fn set_var(host: &mut FakeHost, name: &'static str, value: OsString) {
    host.vars.retain(|(key, _)| *key != name);
    host.vars.push((name, value));
}

fn ran(host: &FakeHost) -> Vec<String> {
    host.runs
        .borrow()
        .iter()
        .map(|(invocation, _)| {
            let (program, first) = subcommand(invocation);
            format!("{program} {first}")
        })
        .collect()
}

fn static_links() -> Vec<String> {
    let mut directives = vec![format!(
        "cargo::rustc-link-search=native={}",
        archive_dir().display()
    )];
    for library in [
        "slang-compiler",
        "compiler-core",
        "core",
        "cmark-gfm",
        "miniz",
        "lz4",
    ] {
        directives.push(format!("cargo::rustc-link-lib=static={library}"));
    }
    directives
}

fn git_reruns() -> [String; 2] {
    [
        format!(
            "cargo::rerun-if-changed={}",
            source_root().join(".git/HEAD").display()
        ),
        format!(
            "cargo::rerun-if-changed={}",
            source_root().join(".git/index").display()
        ),
    ]
}

fn static_failure(host: &FakeHost, parts: &[&str]) -> Vec<String> {
    let (directives, result) = run(host);
    let message = result.err().unwrap_or_default();
    assert_contains_all(&message, parts);
    assert_no_link_directives(&directives);
    assert!(
        !host
            .read_vars
            .borrow()
            .iter()
            .any(|name| name == VULKAN_SDK),
        "the static route read {VULKAN_SDK}"
    );
    assert!(
        !host
            .written
            .borrow()
            .iter()
            .any(|(path, _)| *path == stamp_file()),
        "a failed build wrote the stamp"
    );
    directives
}

#[test]
fn static_route_links_the_static_libraries_without_reading_vulkan_sdk() {
    let host = static_host();
    let (directives, result) = run(&host);
    assert_eq!(result, Ok(()));
    let mut expected = vec![
        "cargo::rerun-if-env-changed=DOCS_RS".to_owned(),
        "cargo::rerun-if-env-changed=SLANG_SOURCE_DIR".to_owned(),
    ];
    expected.extend(git_reruns());
    expected.extend(static_links());
    expected.extend([
        "cargo::rustc-link-lib=dylib=stdc++".to_owned(),
        format!("cargo::metadata=include={}", built_include().display()),
        format!("cargo::metadata=lib={}", archive_dir().display()),
    ]);
    assert_eq!(directives, expected);
    assert!(
        !host
            .read_vars
            .borrow()
            .iter()
            .any(|name| name == VULKAN_SDK),
        "the static route read {VULKAN_SDK}"
    );
}

#[test]
fn static_route_runs_git_ninja_and_cmake_with_their_arguments_and_environment() {
    let host = static_host();
    assert_eq!(run(&host).1, Ok(()));
    let mut expected_runs = vec![
        "git rev-parse".to_owned(),
        "git status".to_owned(),
        "ninja --version".to_owned(),
        "git ls-files".to_owned(),
    ];
    expected_runs.extend(
        REQUIRED_SUBMODULES
            .iter()
            .map(|_| "git ls-files".to_owned()),
    );
    expected_runs.extend(["cmake -S".to_owned(), "cmake --build".to_owned()]);
    assert_eq!(ran(&host), expected_runs);

    let runs = host.runs.borrow();
    for (invocation, log) in runs
        .iter()
        .filter(|(invocation, _)| invocation.program != "cmake")
    {
        assert_eq!(*log, None, "{invocation}");
    }
    for (invocation, _) in runs
        .iter()
        .filter(|(invocation, _)| invocation.program == "git")
    {
        assert_eq!(
            invocation.env,
            [("GIT_OPTIONAL_LOCKS", "0")],
            "{invocation}"
        );
        assert!(invocation.env_remove.contains(&"GIT_DIR"), "{invocation}");
        assert!(
            invocation.env_remove.contains(&"GIT_INDEX_FILE"),
            "{invocation}"
        );
        let dir = invocation.dir.clone().unwrap_or_default();
        assert!(dir.starts_with(source_root()), "{invocation}");
    }
    let numeric = SLANG_BUILD_TAG.split('-').next().unwrap_or_default();
    let (configure, configure_log) = &runs[runs.len() - 2];
    for arg in [
        "-S".to_owned(),
        copy_dir().display().to_string(),
        "-B".to_owned(),
        build_dir().display().to_string(),
        "Ninja".to_owned(),
        "-DCMAKE_BUILD_TYPE=Release".to_owned(),
        "-DSLANG_LIB_TYPE=STATIC".to_owned(),
        format!("-DSLANG_VERSION_FULL={SLANG_BUILD_TAG}"),
        format!("-DSLANG_VERSION_NUMERIC={numeric}"),
        format!(
            "-DCMAKE_ARCHIVE_OUTPUT_DIRECTORY={}",
            archive_dir().display()
        ),
        "-DSLANG_SLANG_LLVM_FLAVOR=DISABLE".to_owned(),
        "-DSLANG_ENABLE_DXIL=OFF".to_owned(),
        "-DSLANG_ENABLE_SLANG_GLSLANG=OFF".to_owned(),
        "-DFETCHCONTENT_FULLY_DISCONNECTED=ON".to_owned(),
    ] {
        assert!(configure.args.contains(&arg), "{configure} lacks {arg}");
    }
    assert!(
        !configure
            .args
            .iter()
            .any(|arg| arg.starts_with("-DCMAKE_MSVC_RUNTIME_LIBRARY")),
        "{configure}"
    );
    let (build, build_log) = &runs[runs.len() - 1];
    assert_eq!(
        build.to_string(),
        format!(
            "cmake --build {} --target slang --parallel 8",
            build_dir().display()
        )
    );
    for (invocation, log) in [(configure, configure_log), (build, build_log)] {
        assert_eq!(*log, Some(log_file()), "{invocation}");
        assert_eq!(invocation.env_remove, ["SLANG_USE_SCCACHE"], "{invocation}");
        assert_eq!(invocation.dir, None, "{invocation}");
    }
}

#[test]
fn static_route_copies_the_tracked_files_and_writes_the_stamp_after_the_build() {
    let host = static_host();
    assert_eq!(run(&host).1, Ok(()));
    let mut files = vec![
        PathBuf::from("CMakeLists.txt"),
        PathBuf::from("source/slang/slang.cpp"),
    ];
    files.extend(
        REQUIRED_SUBMODULES
            .iter()
            .map(|submodule| Path::new(submodule).join("CMakeLists.txt")),
    );
    let expected_sync: Vec<(PathBuf, PathBuf)> = files
        .iter()
        .map(|file| (source_root().join(file), copy_dir().join(file)))
        .collect();
    assert_eq!(*host.synced.borrow(), expected_sync);
    assert_eq!(*host.removed.borrow(), [stamp_file()]);
    let events = host.events.borrow();
    let position = |prefix: &str| {
        events
            .iter()
            .position(|event| event.starts_with(prefix))
            .unwrap_or(usize::MAX)
    };
    let removal = position(&format!("Remove {}", stamp_file().display()));
    assert!(removal < position("Sync "), "{events:?}");
    assert!(removal < position("Run cmake -S"), "{events:?}");
    assert!(
        position("Run cmake --build") < position(&format!("Write {}", stamp_file().display())),
        "{events:?}"
    );
    let written = host.written.borrow();
    assert_eq!(written.len(), 2, "{written:?}");
    assert_eq!(written[0], (log_file(), String::new()));
    assert_eq!(written[1].0, stamp_file());
    for part in [
        &format!("source {}", source_root().display()),
        &format!("commit {SLANG_COMMIT}"),
        &format!("-DSLANG_VERSION_FULL={SLANG_BUILD_TAG}"),
        "external/vulkan",
    ] {
        assert!(
            written[1].1.contains(part),
            "{:?} lacks {part:?}",
            written[1].1
        );
    }
}

#[test]
fn static_route_with_a_matching_stamp_skips_the_copy_and_the_configure_step() {
    let first = static_host();
    assert_eq!(run(&first).1, Ok(()));
    let stamp = first.written.borrow()[1].1.clone();

    let mut host = static_host();
    host.files.push((stamp_file(), stamp.clone()));
    let (directives, result) = run(&host);
    assert_eq!(result, Ok(()));
    assert!(directives.ends_with(&[
        "cargo::rustc-link-lib=dylib=stdc++".to_owned(),
        format!("cargo::metadata=include={}", built_include().display()),
        format!("cargo::metadata=lib={}", archive_dir().display()),
    ]));
    assert_eq!(
        ran(&host),
        [
            "git rev-parse",
            "git status",
            "ninja --version",
            "cmake --build"
        ]
    );
    assert!(host.synced.borrow().is_empty());
    assert!(host.removed.borrow().is_empty());
    assert_eq!(*host.written.borrow(), [(log_file(), String::new())]);

    let mut moved = static_host();
    moved.files.push((stamp_file(), stamp));
    let other = std::env::temp_dir().join("other-slang");
    set_var(&mut moved, SLANG_SOURCE_DIR, other.clone().into_os_string());
    moved
        .entries
        .push((other.clone(), Ok(EntryKind::Directory)));
    for submodule in REQUIRED_SUBMODULES {
        moved
            .entries
            .push((other.join(submodule).join(".git"), Ok(EntryKind::File)));
    }
    moved.respond = Box::new(move |invocation| {
        if subcommand(invocation) == ("git", "ls-files") && invocation.dir == Some(other.clone()) {
            Ok(finished(0, &superproject_listing(), ""))
        } else {
            happy(invocation)
        }
    });
    assert_eq!(run(&moved).1, Ok(()));
    assert!(ran(&moved).contains(&"cmake -S".to_owned()));
    assert!(!moved.synced.borrow().is_empty());
    assert_eq!(moved.written.borrow().len(), 2);

    let windows = |feature: &str| {
        let mut host = target_host("windows", "msvc", feature, "x86_64-pc-windows-msvc");
        for library in STATIC_LIBRARIES {
            set_entry(
                &mut host,
                archive_dir().join(format!("{library}.lib")),
                Ok(EntryKind::File),
            );
        }
        host
    };
    let shared_runtime = windows("");
    assert_eq!(run(&shared_runtime).1, Ok(()));
    let mut static_runtime = windows("crt-static");
    static_runtime
        .files
        .push((stamp_file(), shared_runtime.written.borrow()[1].1.clone()));
    assert_eq!(run(&static_runtime).1, Ok(()));
    assert!(ran(&static_runtime).contains(&"cmake -S".to_owned()));
    assert!(!static_runtime.synced.borrow().is_empty());
    assert_eq!(static_runtime.written.borrow().len(), 2);
}

#[test]
fn static_route_removes_files_of_the_copy_that_git_does_not_track() {
    let mut host = static_host();
    host.copied = Some(vec![
        PathBuf::from("CMakeLists.txt"),
        PathBuf::from("source/slang/core.meta.slang.temp.h"),
    ]);
    assert_eq!(run(&host).1, Ok(()));
    assert_eq!(
        *host.removed.borrow(),
        [
            stamp_file(),
            copy_dir().join("source/slang/core.meta.slang.temp.h")
        ]
    );
}

fn target_host(os: &str, env: &str, feature: &str, triple: &str) -> FakeHost {
    let mut host = static_host();
    set_var(&mut host, TARGET_OS, OsString::from(os));
    set_var(&mut host, TARGET_ENV, OsString::from(env));
    set_var(&mut host, TARGET_FEATURE, OsString::from(feature));
    set_var(&mut host, TARGET, OsString::from(triple));
    set_var(&mut host, HOST, OsString::from(triple));
    host
}

#[test]
fn static_route_links_the_cxx_library_and_archives_of_each_supported_target() {
    for (os, env, feature, triple, cxx, archive, runtime) in [
        (
            "macos",
            "",
            "",
            "aarch64-apple-darwin",
            Some("c++"),
            "lib{}.a",
            None,
        ),
        (
            "windows",
            "msvc",
            "",
            "x86_64-pc-windows-msvc",
            None,
            "{}.lib",
            Some("MultiThreadedDLL"),
        ),
        (
            "windows",
            "msvc",
            "crt-static",
            "x86_64-pc-windows-msvc",
            None,
            "{}.lib",
            Some("MultiThreaded"),
        ),
    ] {
        let archives: Vec<PathBuf> = STATIC_LIBRARIES
            .iter()
            .map(|library| archive_dir().join(archive.replace("{}", library)))
            .collect();
        let mut host = target_host(os, env, feature, triple);
        for path in &archives {
            set_entry(&mut host, path.clone(), Ok(EntryKind::File));
        }
        let (directives, result) = run(&host);
        assert_eq!(result, Ok(()), "{triple}");
        let cxx_directive = cxx.map(|library| format!("cargo::rustc-link-lib=dylib={library}"));
        let links: Vec<&String> = directives
            .iter()
            .filter(|directive| directive.starts_with("cargo::rustc-link-lib=dylib="))
            .collect();
        assert_eq!(links, cxx_directive.iter().collect::<Vec<_>>(), "{triple}");
        let configure = host
            .runs
            .borrow()
            .iter()
            .find(|(invocation, _)| subcommand(invocation) == ("cmake", "-S"))
            .map(|(invocation, _)| invocation.clone())
            .expect("the route configures");
        let option = runtime.map(|runtime| format!("-DCMAKE_MSVC_RUNTIME_LIBRARY={runtime}"));
        assert_eq!(
            configure
                .args
                .iter()
                .filter(|arg| arg.starts_with("-DCMAKE_MSVC_RUNTIME_LIBRARY"))
                .collect::<Vec<_>>(),
            option.iter().collect::<Vec<_>>(),
            "{triple}"
        );

        let missing = archive_dir().join(archive.replace("{}", "slang-compiler"));
        let mut lacking = target_host(os, env, feature, triple);
        for path in archives.iter().filter(|path| **path != missing) {
            set_entry(&mut lacking, path.clone(), Ok(EntryKind::File));
        }
        lacking.entries.retain(|(path, _)| *path != missing);
        static_failure(
            &lacking,
            &[
                &format!("cannot access {}", missing.display()),
                "another layout",
            ],
        );
    }
}

#[test]
fn static_route_with_an_empty_or_invalid_source_dir_names_the_variable() {
    let empty = OsString::new();
    let relative = OsString::from("slang");
    let line_break = source_root()
        .join("x\ncargo::rustc-link-lib=other")
        .into_os_string();
    for (value, part) in [
        (empty, "SLANG_SOURCE_DIR is set but empty"),
        (
            relative,
            "is not an absolute path in UTF-8 without line breaks",
        ),
        (
            line_break,
            "is not an absolute path in UTF-8 without line breaks",
        ),
        (
            non_utf8_path(),
            "is not an absolute path in UTF-8 without line breaks",
        ),
    ] {
        let mut host = static_host();
        set_var(&mut host, SLANG_SOURCE_DIR, value);
        let directives = static_failure(&host, &[part, SLANG_COMMIT, VULKAN_SDK_VERSION]);
        assert_eq!(
            directives,
            [
                "cargo::rerun-if-env-changed=DOCS_RS",
                "cargo::rerun-if-env-changed=SLANG_SOURCE_DIR",
            ]
        );
        assert!(host.runs.borrow().is_empty());
    }
}

#[test]
fn static_route_with_a_source_dir_that_is_not_a_directory_names_it() {
    let path = source_root().display().to_string();
    let missing = io::Error::from(io::ErrorKind::NotFound).to_string();
    for (kind, part) in [
        (Ok(EntryKind::File), "which is not a directory".to_owned()),
        (
            Err(io::ErrorKind::NotFound),
            format!("which cannot be accessed: {missing}"),
        ),
    ] {
        let mut host = static_host();
        set_entry(&mut host, source_root(), kind);
        static_failure(&host, &[SLANG_SOURCE_DIR, &path, &part, SLANG_COMMIT]);
        assert!(host.runs.borrow().is_empty());
    }
}

#[test]
fn static_route_without_out_dir_says_that_cargo_sets_it() {
    let mut host = static_host();
    host.vars.retain(|(name, _)| *name != OUT_DIR);
    static_failure(&host, &["OUT_DIR is not set", "Cargo sets OUT_DIR"]);
    assert!(host.runs.borrow().is_empty());
}

#[test]
fn static_route_refuses_cross_compilation_and_unsupported_targets() {
    let mut cross = static_host();
    set_var(
        &mut cross,
        TARGET,
        OsString::from("aarch64-unknown-linux-gnu"),
    );
    static_failure(
        &cross,
        &[
            "the target aarch64-unknown-linux-gnu differs from the host x86_64-unknown-linux-gnu",
            "cannot cross-compile",
            SLANG_SOURCE_DIR,
        ],
    );
    assert!(cross.runs.borrow().is_empty());
    for (os, env, triple) in [
        ("linux", "musl", "x86_64-unknown-linux-musl"),
        ("freebsd", "", "x86_64-unknown-freebsd"),
        ("windows", "gnu", "x86_64-pc-windows-gnu"),
    ] {
        let mut host = static_host();
        set_var(&mut host, TARGET_OS, OsString::from(os));
        set_var(&mut host, TARGET_ENV, OsString::from(env));
        set_var(&mut host, TARGET, OsString::from(triple));
        set_var(&mut host, HOST, OsString::from(triple));
        static_failure(
            &host,
            &[
                &format!("the target {triple} is none of these"),
                SLANG_SOURCE_DIR,
            ],
        );
        assert!(host.runs.borrow().is_empty());
    }
}

#[test]
fn static_route_names_each_program_that_cannot_start() {
    for (program, first) in [
        ("git", "rev-parse"),
        ("ninja", "--version"),
        ("cmake", "-S"),
        ("cmake", "--build"),
    ] {
        let mut host = static_host();
        respond_with(&mut host, program, first, || {
            Err(io::Error::from(io::ErrorKind::NotFound))
        });
        static_failure(
            &host,
            &[
                &format!("cannot start {program} for `{program} {first}"),
                &io::Error::from(io::ErrorKind::NotFound).to_string(),
                "needs git, CMake 3.22 or later, and Ninja on PATH",
            ],
        );
    }
}

fn start_message(program: &str, args: &str) -> String {
    format!(
        "cannot start {program} for `{program} {args}`: {}. The build of Slang from source that SLANG_SOURCE_DIR selects needs git, CMake 3.22 or later, and Ninja on PATH to build Slang. Install the missing program, or unset SLANG_SOURCE_DIR to link the Slang of the Vulkan SDK {VULKAN_SDK_VERSION} that VULKAN_SDK names.",
        io::Error::from(io::ErrorKind::NotFound)
    )
}

fn logged_args(host: &FakeHost, first: &str) -> String {
    host.runs
        .borrow()
        .iter()
        .find(|(invocation, _)| subcommand(invocation) == ("cmake", first))
        .map(|(invocation, _)| invocation.args.join(" "))
        .unwrap_or_default()
}

#[test]
fn static_route_reports_a_logged_step_whose_program_cannot_start() {
    for first in ["-S", "--build"] {
        let mut host = static_host();
        respond_with(&mut host, "cmake", first, || {
            Err(io::Error::from(io::ErrorKind::NotFound))
        });
        let (_, result) = run(&host);
        let expected = start_message("cmake", &logged_args(&host, first));
        assert_eq!(result, Err(expected.clone()));
        static_failure(&host, &[&expected]);
    }
}

#[test]
fn static_route_reports_a_log_that_cannot_be_opened_without_blaming_the_program() {
    let mut host = static_host();
    host.failures
        .push((FileOp::OpenLog, log_file(), io::ErrorKind::PermissionDenied));
    let expected = file_message("open the log", &log_file());
    assert_eq!(run(&host).1, Err(expected.clone()));
    static_failure(&host, &[&expected]);
    assert!(!ran(&host).contains(&"cmake -S".to_owned()));
}

#[test]
fn static_route_without_git_metadata_says_that_the_commit_cannot_be_verified() {
    let mut host = static_host();
    respond_with(&mut host, "git", "rev-parse", || {
        Ok(finished(
            128,
            "",
            "fatal: not a git repository (or any of the parent directories): .git\n",
        ))
    });
    let directives = static_failure(
        &host,
        &[
            "git rev-parse",
            "exit status 128",
            "fatal: not a git repository",
            "a source archive, cannot be verified",
            "git clone --recursive",
            SLANG_COMMIT,
        ],
    );
    assert_eq!(
        directives,
        [
            "cargo::rerun-if-env-changed=DOCS_RS",
            "cargo::rerun-if-env-changed=SLANG_SOURCE_DIR",
        ]
    );
}

#[test]
fn static_route_refuses_a_directory_that_is_not_the_top_of_a_work_tree() {
    for (output, part) in [
        (
            "true\nsource/\n../.git/HEAD\n../.git/index\nc7954ebd1f1984ad2bfde15f76248db41dc34aa3\n",
            "lies at source/ inside a git work tree whose top level is another directory",
        ),
        (
            "false\n\nHEAD\nindex\nc7954ebd1f1984ad2bfde15f76248db41dc34aa3\n",
            "is not inside a git work tree",
        ),
    ] {
        let mut host = static_host();
        host.respond = Box::new(move |invocation| {
            if subcommand(invocation) == ("git", "rev-parse") {
                Ok(finished(0, output, ""))
            } else {
                happy(invocation)
            }
        });
        static_failure(
            &host,
            &[part, &source_root().display().to_string(), SLANG_COMMIT],
        );
        assert_eq!(ran(&host), ["git rev-parse"]);
    }
}

#[test]
fn static_route_with_unexpected_git_output_stops() {
    let mut host = static_host();
    respond_with(&mut host, "git", "rev-parse", || {
        Ok(finished(0, "true\n", ""))
    });
    static_failure(&host, &["printed \"true\\n\"", "git rev-parse"]);
}

#[test]
fn static_route_at_another_commit_names_both_commits_and_the_checkout_command() {
    let mut host = static_host();
    respond_with(&mut host, "git", "rev-parse", || {
        Ok(finished(
            0,
            "true\n\n.git/HEAD\n.git/index\n0123456789012345678901234567890123456789\n",
            "",
        ))
    });
    let directives = static_failure(
        &host,
        &[
            "0123456789012345678901234567890123456789",
            SLANG_COMMIT,
            SLANG_BUILD_TAG,
            &format!("git -C {} checkout {SLANG_COMMIT}", source_root().display()),
        ],
    );
    assert!(directives.ends_with(&git_reruns()), "{directives:?}");
    assert_eq!(ran(&host), ["git rev-parse"]);
}

#[test]
fn static_route_refuses_a_tree_with_changed_tracked_files() {
    let mut host = static_host();
    respond_with(&mut host, "git", "status", || {
        Ok(finished(
            0,
            " M source/slang/slang.cpp\n M external/lz4\n",
            "",
        ))
    });
    let directives = static_failure(
        &host,
        &[
            "\" M source/slang/slang.cpp\", \" M external/lz4\".",
            "git status --porcelain=v1 --untracked-files=no --ignore-submodules=none",
            "differ from the commit",
            SLANG_COMMIT,
        ],
    );
    assert!(directives.ends_with(&git_reruns()), "{directives:?}");
    assert!(host.synced.borrow().is_empty());
}

#[test]
fn static_route_names_a_missing_submodule_and_the_command_that_fetches_it() {
    let command = format!(
        "git -C {} submodule update --init external/lz4",
        source_root().display()
    );
    let mut empty = static_host();
    empty
        .entries
        .retain(|(path, _)| *path != source_root().join("external/lz4/.git"));
    let mut unrecorded = static_host();
    unrecorded.respond = Box::new(|invocation| {
        if subcommand(invocation) == ("git", "ls-files") && invocation.dir == Some(source_root()) {
            Ok(finished(
                0,
                &superproject_listing().replace("\texternal/lz4\0", "\texternal/other\0"),
                "",
            ))
        } else {
            happy(invocation)
        }
    });
    for host in [empty, unrecorded] {
        static_failure(
            &host,
            &[
                "the submodule external/lz4",
                "is missing or empty",
                &command,
            ],
        );
        assert!(host.synced.borrow().is_empty());
        assert!(!ran(&host).contains(&"cmake -S".to_owned()));
    }
}

#[test]
fn static_route_reports_a_submodule_git_entry_that_cannot_be_accessed() {
    let entry = source_root().join("external/lz4").join(".git");
    let mut host = static_host();
    set_entry(
        &mut host,
        entry.clone(),
        Err(io::ErrorKind::PermissionDenied),
    );
    let (_, result) = run(&host);
    let message = result.err().unwrap_or_default();
    assert!(!message.contains("missing or empty"), "{message}");
    static_failure(
        &host,
        &[
            &format!("cannot access {}: {}", entry.display(), denied()),
            "the submodule external/lz4",
            &source_root().display().to_string(),
        ],
    );
    assert!(host.synced.borrow().is_empty());
    assert!(!ran(&host).contains(&"cmake -S".to_owned()));
}

#[test]
fn static_route_refuses_a_tracked_path_that_leaves_the_tree() {
    for path in ["../escape", "/tmp/escape"] {
        let mut host = static_host();
        host.respond = Box::new(move |invocation| {
            if subcommand(invocation) == ("git", "ls-files") {
                Ok(finished(
                    0,
                    &format!("100644 1111111111111111111111111111111111111111 0\t{path}\0"),
                    "",
                ))
            } else {
                happy(invocation)
            }
        });
        static_failure(&host, &["git ls-files -z --stage", path]);
        assert!(host.synced.borrow().is_empty());
    }
}

#[test]
fn static_route_reports_a_failed_step_with_the_tail_of_its_log() {
    let log = (1..=50)
        .map(|line| format!("log line {line}\n"))
        .collect::<Vec<_>>()
        .concat();
    for first in ["-S", "--build"] {
        let mut host = static_host();
        host.files.push((log_file(), log.clone()));
        host.respond = Box::new(move |invocation| {
            if subcommand(invocation) == ("cmake", first) {
                Ok(finished(1, "", ""))
            } else {
                happy(invocation)
            }
        });
        let step = if first == "-S" { "configure" } else { "build" };
        let directives = static_failure(
            &host,
            &[
                &format!("`cmake {first}"),
                "failed with exit status 1",
                &format!("the {step} step of the build of Slang from source failed"),
                &log_file().display().to_string(),
                "\nlog line 11\n",
                "log line 50",
            ],
        );
        let (_, result) = run(&host);
        assert!(
            !result.unwrap_err().contains("log line 10\n"),
            "the message holds more than the tail"
        );
        assert!(directives.ends_with(&git_reruns()), "{directives:?}");
    }
}

#[test]
fn static_route_reports_a_build_without_an_archive_or_with_another_tag() {
    let mut missing = static_host();
    missing
        .entries
        .retain(|(path, _)| *path != archive_dir().join("libcmark-gfm.a"));
    static_failure(
        &missing,
        &[
            &format!(
                "cannot access {}",
                archive_dir().join("libcmark-gfm.a").display()
            ),
            "another layout",
        ],
    );

    let header = built_include().join("slang-tag-version.h");
    let mut other = static_host();
    other.files = vec![(header.clone(), header_text("0.0.0-unknown"))];
    static_failure(
        &other,
        &[
            "reports the build tag 0.0.0-unknown",
            &header.display().to_string(),
            SLANG_BUILD_TAG,
        ],
    );

    let mut absent = static_host();
    absent.files.clear();
    static_failure(
        &absent,
        &[
            &format!("cannot read {}", header.display()),
            "another layout",
        ],
    );
}

#[test]
fn static_route_omits_the_job_count_when_cargo_gives_none_or_an_unusable_one() {
    let jobs: [Option<OsString>; 4] = [
        None,
        Some(OsString::from("x")),
        Some(OsString::new()),
        Some(non_utf8_path()),
    ];
    for value in jobs {
        let mut host = static_host();
        host.vars.retain(|(name, _)| *name != NUM_JOBS);
        if let Some(value) = &value {
            host.vars.push((NUM_JOBS, value.clone()));
        }
        assert_eq!(run(&host).1, Ok(()), "{value:?}");
        let build = host
            .runs
            .borrow()
            .last()
            .map(|(invocation, _)| invocation.to_string())
            .expect("the route builds");
        assert_eq!(
            build,
            format!("cmake --build {} --target slang", build_dir().display()),
            "{value:?}"
        );
    }
}

#[test]
fn numeric_version_drops_everything_after_the_first_hyphen() {
    assert_eq!(numeric_version("2026.17-25-gc7954ebd1"), "2026.17");
    assert_eq!(numeric_version("2026.17"), "2026.17");
}

#[test]
fn static_route_without_target_or_host_names_the_missing_variable() {
    for (absent, named) in [
        (&[TARGET][..], TARGET),
        (&[HOST][..], HOST),
        (&[TARGET, HOST][..], TARGET),
    ] {
        let mut host = static_host();
        host.vars.retain(|(name, _)| !absent.contains(name));
        static_failure(
            &host,
            &[
                &format!("{named} is not set"),
                &format!("Cargo sets {named} for every build script"),
                SLANG_SOURCE_DIR,
            ],
        );
        assert!(host.runs.borrow().is_empty(), "{absent:?}");
    }
}

#[test]
fn static_route_names_a_program_that_exits_with_a_failure() {
    let submodule = source_root().join("external/lz4");
    for (program, first, dir) in [
        ("ninja", "--version", None),
        ("git", "status", Some(source_root())),
        ("git", "ls-files", Some(source_root())),
        ("git", "ls-files", Some(submodule)),
    ] {
        let mut host = static_host();
        let failing = dir.clone();
        host.respond = Box::new(move |invocation| {
            if subcommand(invocation) == (program, first) && invocation.dir == failing {
                Ok(finished(1, "", "fatal: boom\n"))
            } else {
                happy(invocation)
            }
        });
        let place = dir
            .as_ref()
            .map(|dir| format!(" in {}", dir.display()))
            .unwrap_or_default();
        static_failure(
            &host,
            &[
                &format!("`{program} {first}"),
                &format!("`{place} failed with exit status 1: fatal: boom"),
            ],
        );
        assert!(host.synced.borrow().is_empty(), "{program} {first}");
        assert!(
            !ran(&host).contains(&"cmake -S".to_owned()),
            "{program} {first}"
        );
    }
}

#[test]
fn static_route_reports_a_signal_that_ends_a_program() {
    let signalled = || {
        Ok(Finished {
            exit: Exit { code: None },
            stdout: String::new(),
            stderr: String::new(),
        })
    };
    let mut status = static_host();
    respond_with(&mut status, "git", "status", signalled);
    static_failure(
        &status,
        &["`git status", "no exit status, because a signal ended it"],
    );
    let mut build = static_host();
    respond_with(&mut build, "cmake", "--build", signalled);
    static_failure(
        &build,
        &[
            "`cmake --build",
            "failed with no exit status, because a signal ended it, so the build step",
        ],
    );
}

#[test]
fn static_route_reports_a_failed_step_whose_log_cannot_be_read() {
    let mut host = static_host();
    respond_with(&mut host, "cmake", "-S", || Ok(finished(1, "", "")));
    static_failure(
        &host,
        &[
            "the configure step of the build of Slang from source failed",
            &format!(
                "the log cannot be read: {}",
                io::Error::from(io::ErrorKind::NotFound)
            ),
        ],
    );
}

#[test]
fn static_route_lists_exactly_ten_changed_files_without_a_remainder() {
    for (count, tail) in [(10, "\" M a\". The build"), (11, "\" M a\", and 1 more.")] {
        let mut host = static_host();
        host.respond = Box::new(move |invocation| {
            if subcommand(invocation) == ("git", "status") {
                Ok(finished(0, &" M a\n".repeat(count), ""))
            } else {
                happy(invocation)
            }
        });
        let (_, result) = run(&host);
        let message = result.err().unwrap_or_default();
        assert_eq!(message.matches("\" M a\"").count(), 10, "{message}");
        assert_contains_all(&message, &[tail]);
    }
}

#[test]
fn static_route_accepts_a_submodule_whose_git_entry_is_a_directory() {
    let mut host = static_host();
    for submodule in REQUIRED_SUBMODULES {
        set_entry(
            &mut host,
            source_root().join(submodule).join(".git"),
            Ok(EntryKind::Directory),
        );
    }
    assert_eq!(run(&host).1, Ok(()));
}

#[test]
fn static_route_reports_a_built_archive_or_tag_header_that_is_unusable() {
    let archive = archive_dir().join("libcmark-gfm.a");
    let mut directory = static_host();
    set_entry(&mut directory, archive.clone(), Ok(EntryKind::Directory));
    static_failure(
        &directory,
        &[
            &format!("{} is not a file", archive.display()),
            "another layout",
        ],
    );

    let header = built_include().join("slang-tag-version.h");
    for (text, part) in [
        (
            "#pragma once\n".to_owned(),
            format!("{} does not define {TAG_MACRO}", header.display()),
        ),
        (
            format!("#pragma once\n#define {TAG_MACRO} 2026\n"),
            format!(
                "line 2 of {} does not define {TAG_MACRO} as one string literal",
                header.display()
            ),
        ),
    ] {
        let mut host = static_host();
        host.files = vec![(header.clone(), text)];
        static_failure(&host, &[&part, "another layout"]);
    }
}

fn denied() -> String {
    io::Error::from(io::ErrorKind::PermissionDenied).to_string()
}

fn file_message(action: &str, path: &Path) -> String {
    format!(
        "cannot {action} {}: {}. The build of Slang from source keeps its copy of the source tree, its build, and its log under OUT_DIR.",
        path.display(),
        denied()
    )
}

#[test]
fn static_route_names_the_action_and_the_path_of_each_file_operation_that_fails() {
    let stale = copy_dir().join("stale.h");
    let tracked = copy_dir().join("CMakeLists.txt");
    let cases = [
        (
            FileOp::Write,
            log_file(),
            file_message("write", &log_file()),
            false,
        ),
        (
            FileOp::Remove,
            stamp_file(),
            file_message("remove", &stamp_file()),
            false,
        ),
        (
            FileOp::List,
            copy_dir(),
            file_message("list the files of", &copy_dir()),
            false,
        ),
        (
            FileOp::Remove,
            stale.clone(),
            file_message("remove", &stale),
            false,
        ),
        (
            FileOp::Sync,
            tracked.clone(),
            format!(
                "cannot copy {} to {}: {}. The build copies the tracked files of the Slang source tree into OUT_DIR and builds the copy, because Slang's build writes into its source tree.",
                source_root().join("CMakeLists.txt").display(),
                tracked.display(),
                denied()
            ),
            false,
        ),
        (
            FileOp::Write,
            stamp_file(),
            file_message("write", &stamp_file()),
            true,
        ),
    ];
    for (op, path, part, configured) in cases {
        let mut host = static_host();
        host.copied = Some(vec![PathBuf::from("stale.h")]);
        host.failures
            .push((op, path.clone(), io::ErrorKind::PermissionDenied));
        static_failure(&host, &[&part]);
        assert_eq!(
            ran(&host).contains(&"cmake -S".to_owned()),
            configured,
            "{op:?} {}",
            path.display()
        );
    }
}

#[test]
fn static_route_accepts_a_stamp_that_does_not_exist() {
    let mut host = static_host();
    host.failures
        .push((FileOp::Remove, stamp_file(), io::ErrorKind::NotFound));
    assert_eq!(run(&host).1, Ok(()));
    assert!(
        host.written
            .borrow()
            .iter()
            .any(|(path, _)| *path == stamp_file())
    );
}

#[test]
fn static_route_removes_a_stale_stamp_before_the_copy_and_keeps_it_removed_on_failure() {
    let mut host = static_host();
    host.files.push((stamp_file(), "stale\n".to_owned()));
    respond_with(&mut host, "cmake", "-S", || Ok(finished(1, "", "")));
    static_failure(
        &host,
        &["the configure step of the build of Slang from source failed"],
    );
    let events = host.events.borrow();
    let removal = format!("Remove {}", stamp_file().display());
    assert_eq!(
        events.iter().position(|event| event.starts_with("Remove ")),
        events.iter().position(|event| *event == removal),
        "{events:?}"
    );
    let first_sync = events
        .iter()
        .position(|event| event.starts_with("Sync "))
        .unwrap_or(usize::MAX);
    let configure = events
        .iter()
        .position(|event| event.starts_with("Run cmake -S"))
        .unwrap_or(usize::MAX);
    let removed = events
        .iter()
        .position(|event| *event == removal)
        .unwrap_or(usize::MAX);
    assert!(removed < first_sync && first_sync < configure, "{events:?}");
    assert!(
        !events
            .iter()
            .any(|event| *event == format!("Write {}", stamp_file().display())),
        "{events:?}"
    );
}
