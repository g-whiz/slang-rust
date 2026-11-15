use std::cell::RefCell;

include!("../../route.rs");

mod ffi;

type Generator = Box<dyn Fn(&Path, &str) -> Result<Generated, GenerationError>>;
type Responder = Box<dyn Fn(&Invocation) -> io::Result<Finished>>;

const BUILD_TAG: &str = "2026.17-25-gc7954ebd1";
const COMMIT: &str = "c7954ebd1f1984ad2bfde15f76248db41dc34aa3";
const TRIPLE: &str = "x86_64-unknown-linux-gnu";
const PROCESS_ID: u32 = 4242;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FileOp {
    List,
    ListDir,
    OpenLog,
    Sync,
    Remove,
    RemoveDir,
    Write,
    SetModified,
    Rename,
    Lock,
}

struct FakeHost {
    vars: Vec<(&'static str, OsString)>,
    files: Vec<(PathBuf, String)>,
    entries: Vec<(PathBuf, Result<EntryKind, io::ErrorKind>)>,
    trees: Vec<(PathBuf, Vec<PathBuf>)>,
    listings: Vec<(PathBuf, Vec<OsString>)>,
    host_os: &'static str,
    generator: Generator,
    respond: Responder,
    failures: Vec<(FileOp, PathBuf, io::ErrorKind)>,
    read_vars: RefCell<Vec<String>>,
    generations: RefCell<Vec<(PathBuf, String)>>,
    runs: RefCell<Vec<(Invocation, Option<PathBuf>)>>,
    synced: RefCell<Vec<(PathBuf, PathBuf)>>,
    removed: RefCell<Vec<PathBuf>>,
    removed_dirs: RefCell<Vec<PathBuf>>,
    written: RefCell<Vec<(PathBuf, String)>>,
    modified: RefCell<Vec<(PathBuf, SystemTime)>>,
    renamed: RefCell<Vec<(PathBuf, PathBuf)>>,
    locked: RefCell<Vec<PathBuf>>,
    msvc: Option<MsvcCompiler>,
    msvc_lookups: RefCell<Vec<String>>,
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
        self.trees
            .iter()
            .find(|(tree, _)| tree == root)
            .map(|(_, files)| files.clone())
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
    }

    fn list(&self, dir: &Path) -> io::Result<Vec<OsString>> {
        self.attempt(FileOp::ListDir, dir)?;
        Ok(self
            .listings
            .iter()
            .find(|(listed, _)| listed == dir)
            .map_or_default(|(_, names)| names.clone()))
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

    fn remove_dir_all(&self, path: &Path) -> io::Result<()> {
        self.attempt(FileOp::RemoveDir, path)?;
        self.removed_dirs.borrow_mut().push(path.to_path_buf());
        Ok(())
    }

    fn write(&self, path: &Path, text: &str) -> io::Result<()> {
        self.attempt(FileOp::Write, path)?;
        self.written
            .borrow_mut()
            .push((path.to_path_buf(), text.to_owned()));
        Ok(())
    }

    fn set_modified(&self, path: &Path, time: SystemTime) -> io::Result<()> {
        self.attempt(FileOp::SetModified, path)?;
        self.modified.borrow_mut().push((path.to_path_buf(), time));
        Ok(())
    }

    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        self.attempt(FileOp::Rename, to)?;
        self.renamed
            .borrow_mut()
            .push((from.to_path_buf(), to.to_path_buf()));
        Ok(())
    }

    fn lock(&self, path: &Path) -> io::Result<()> {
        self.attempt(FileOp::Lock, path)?;
        self.locked.borrow_mut().push(path.to_path_buf());
        Ok(())
    }

    fn process_id(&self) -> u32 {
        PROCESS_ID
    }

    fn msvc_compiler(&self, target: &str) -> Option<MsvcCompiler> {
        self.msvc_lookups.borrow_mut().push(target.to_owned());
        self.msvc.clone()
    }

    fn generate(&self, header_dir: &Path, target: &str) -> Result<Generated, GenerationError> {
        self.generations
            .borrow_mut()
            .push((header_dir.to_path_buf(), target.to_owned()));
        (self.generator)(header_dir, target)
    }
}

fn sdk_root() -> PathBuf {
    std::env::temp_dir().join("vulkan-sdk")
}

fn include_dir() -> PathBuf {
    sdk_root().join("include").join("slang")
}

fn tag_header() -> PathBuf {
    include_dir().join("slang-tag-version.h")
}

fn header_text(tag: &str) -> String {
    format!("#pragma once\n#define {TAG_MACRO} \"{tag}\"\n#define SLANG_VERSION_NUMERIC \"0\"\n")
}

fn lib_dir() -> PathBuf {
    sdk_root().join("lib")
}

fn out_root() -> PathBuf {
    std::env::temp_dir().join("out")
}

fn generated() -> Generated {
    Generated {
        bindings: "pub struct Bindings;\n".to_owned(),
        symbols: "pub(super) const C_FUNCTIONS: &[(&str, *const ())] = &[];\n".to_owned(),
    }
}

fn empty_host() -> FakeHost {
    FakeHost {
        vars: Vec::new(),
        files: Vec::new(),
        entries: Vec::new(),
        trees: Vec::new(),
        listings: Vec::new(),
        host_os: "linux",
        generator: Box::new(|_, _| Ok(generated())),
        respond: Box::new(|_| Err(io::Error::from(io::ErrorKind::NotFound))),
        failures: Vec::new(),
        read_vars: RefCell::default(),
        generations: RefCell::default(),
        runs: RefCell::default(),
        synced: RefCell::default(),
        removed: RefCell::default(),
        removed_dirs: RefCell::default(),
        written: RefCell::default(),
        modified: RefCell::default(),
        renamed: RefCell::default(),
        locked: RefCell::default(),
        msvc: None,
        msvc_lookups: RefCell::default(),
        events: RefCell::default(),
    }
}

fn sdk_host(header: &str) -> FakeHost {
    FakeHost {
        vars: vec![
            (VULKAN_SDK, sdk_root().into_os_string()),
            (TARGET_OS, OsString::from("linux")),
            (TARGET, OsString::from(TRIPLE)),
            (POINTER_WIDTH, OsString::from("64")),
            (OUT_DIR, out_root().into_os_string()),
        ],
        files: vec![(tag_header(), header.to_owned())],
        entries: vec![
            (lib_dir(), Ok(EntryKind::Directory)),
            (lib_dir().join("libslang-compiler.so"), Ok(EntryKind::File)),
        ],
        ..empty_host()
    }
}

fn recorded_sdk_host() -> FakeHost {
    sdk_host(&header_text(BUILD_TAG))
}

fn set_var(host: &mut FakeHost, name: &'static str, value: Option<OsString>) {
    host.vars.retain(|(key, _)| *key != name);
    host.vars.extend(value.map(|value| (name, value)));
}

fn set_entry(host: &mut FakeHost, path: PathBuf, kind: Result<EntryKind, io::ErrorKind>) {
    host.entries.retain(|(entry, _)| *entry != path);
    host.entries.push((path, kind));
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

fn reruns() -> [String; 3] {
    [
        "cargo::rerun-if-env-changed=LIBCLANG_PATH".to_owned(),
        "cargo::rerun-if-env-changed=VULKAN_SDK".to_owned(),
        format!("cargo::rerun-if-changed={}", include_dir().display()),
    ]
}

fn assert_reruns_on_the_sdk(directives: &[String]) {
    assert!(directives.starts_with(&reruns()), "{directives:?}");
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

fn link_directives(tag: &str) -> [String; 6] {
    [
        format!("cargo::rustc-link-search=native={}", lib_dir().display()),
        "cargo::rustc-link-lib=dylib=slang-compiler".to_owned(),
        format!("cargo::metadata=include={}", include_dir().display()),
        format!("cargo::metadata=lib={}", lib_dir().display()),
        "cargo::metadata=linkage=dynamic".to_owned(),
        format!("cargo::metadata=build_tag={tag}"),
    ]
}

fn written(host: &FakeHost, name: &str) -> Option<String> {
    let path = out_root().join(name);
    host.written
        .borrow()
        .iter()
        .find(|(file, _)| *file == path)
        .map(|(_, text)| text.clone())
}

fn build_info_text(tag: &str, tested: bool, version: &str) -> String {
    format!(
        "BuildInfo {{ slang_build_tag: {tag:?}, slang_commit: None, linkage: Linkage::Dynamic, tested_target: {tested}, version: {version:?} }}\n"
    )
}

fn assert_no_vendored_advice(message: &str) {
    assert!(!message.contains(VENDORED_ADVICE), "{message}");
}

#[test]
fn configure_generates_the_bindings_from_the_sdk_and_links_its_library() {
    let host = recorded_sdk_host();
    let (directives, result) = run(&host);
    assert_eq!(result, Ok(()));
    let mut expected = reruns().to_vec();
    expected.extend(link_directives(BUILD_TAG));
    assert_eq!(directives, expected);
    assert_eq!(
        *host.generations.borrow(),
        [(include_dir(), TRIPLE.to_owned())]
    );
    assert_eq!(written(&host, "bindings.rs"), Some(generated().bindings));
    assert_eq!(written(&host, "symbols.rs"), Some(generated().symbols));
    assert_eq!(
        written(&host, "build_info.rs"),
        Some(build_info_text(
            BUILD_TAG,
            true,
            &format!("{CRATE_VERSION}+slang.2026.17.0.25.gc7954ebd1.dynamic")
        ))
    );
    assert_eq!(
        *host.read_vars.borrow(),
        [
            VENDORED,
            VULKAN_SDK,
            TARGET_OS,
            TARGET,
            POINTER_WIDTH,
            OUT_DIR
        ]
    );
}

#[test]
fn configure_records_any_build_tag_without_comparing_it() {
    for (tag, identifiers) in [
        ("2000.1-1-gdeadbeef0", "2000.1.0.1.gdeadbeef0"),
        (
            "2026.22.3-7-g0123456789abcdef",
            "2026.22.3.7.g0123456789abcdef",
        ),
        ("2026.17", "2026.17.0.0"),
        ("2026.17-25-gc7954eb", "2026-17-25-gc7954eb"),
        ("commit 1/2+x", "commit-1-2-x"),
        ("0.0.0-unknown", "0-0-0-unknown"),
    ] {
        let host = sdk_host(&header_text(tag));
        let (directives, result) = run(&host);
        assert_eq!(result, Ok(()), "{tag}");
        assert!(
            directives.ends_with(&link_directives(tag)),
            "{directives:?}"
        );
        assert_eq!(
            written(&host, "build_info.rs"),
            Some(build_info_text(
                tag,
                true,
                &format!("{CRATE_VERSION}+slang.{identifiers}.dynamic")
            )),
            "{tag}"
        );
    }
}

#[test]
fn configure_without_vulkan_sdk_names_the_variable_and_the_sdk() {
    for (value, state) in [
        (None, "is not set"),
        (Some(OsString::new()), "is set but empty"),
    ] {
        let mut host = recorded_sdk_host();
        set_var(&mut host, VULKAN_SDK, value);
        let (directives, result) = run(&host);
        let message = result.expect_err("the build must stop");
        assert_contains_all(
            &message,
            &[
                &format!(
                    "{VULKAN_SDK} {state}, so slang-compiler-sys cannot find the Slang library to link"
                ),
                "https://vulkan.lunarg.com",
                "include/slang and lib",
                VENDORED_ADVICE,
            ],
        );
        assert_eq!(
            directives,
            [
                "cargo::rerun-if-env-changed=LIBCLANG_PATH",
                "cargo::rerun-if-env-changed=VULKAN_SDK"
            ]
        );
        assert!(host.generations.borrow().is_empty());
        assert!(host.written.borrow().is_empty());
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
        let mut host = recorded_sdk_host();
        set_var(&mut host, VULKAN_SDK, Some(value));
        let (directives, result) = run(&host);
        let message = result.expect_err("the path must be rejected");
        assert_contains_all(
            &message,
            &[
                VULKAN_SDK,
                "is not an absolute path in UTF-8 without line breaks",
                "https://vulkan.lunarg.com",
            ],
        );
        assert!(!message.contains(['\n', '\r']), "{message:?}");
        assert_no_vendored_advice(&message);
        assert_no_link_directives(&directives);
        assert_eq!(directives.len(), 2, "{directives:?}");
    }
}

#[test]
fn configure_without_the_tag_header_names_its_path_and_the_reason() {
    let mut host = recorded_sdk_host();
    host.files.clear();
    let (directives, result) = run(&host);
    let message = result.expect_err("the build must stop");
    let missing = io::Error::from(io::ErrorKind::NotFound).to_string();
    assert_contains_all(
        &message,
        &[
            &format!("cannot read {}: {missing}", tag_header().display()),
            "to record the build tag",
            "https://vulkan.lunarg.com",
        ],
    );
    assert_reruns_on_the_sdk(&directives);
    assert_no_link_directives(&directives);
}

#[test]
fn configure_without_the_tag_definition_names_the_path_and_the_macro() {
    let (directives, result) = run(&sdk_host(
        "#pragma once\n#define SLANG_TAG_VERSIONS \"1\"\n",
    ));
    let message = result.expect_err("the build must stop");
    assert_contains_all(
        &message,
        &[
            &format!("{} does not define {TAG_MACRO}", tag_header().display()),
            "https://vulkan.lunarg.com",
        ],
    );
    assert_reruns_on_the_sdk(&directives);
    assert_no_link_directives(&directives);
}

#[test]
fn configure_with_a_malformed_tag_names_the_line_and_the_path() {
    for (text, line) in [
        ("#define SLANG_TAG_VERSION 2026\n", 1),
        ("\n#define SLANG_TAG_VERSION \"a\" \"b\"\n", 2),
        ("#define SLANG_TAG_VERSION \"a\"b\"\n", 1),
        ("#define SLANG_TAG_VERSION \"2026.17-25-gc7954ebd1\n", 1),
        ("#define SLANG_TAG_VERSION\n", 1),
        ("#define SLANG_TAG_VERSION \"\"\n", 1),
        ("#define SLANG_TAG_VERSION \"a\tb\"\n", 1),
    ] {
        let host = sdk_host(text);
        let (directives, result) = run(&host);
        let message = result.expect_err("a malformed tag must stop the build");
        assert_contains_all(
            &message,
            &[
                &format!("line {line} of {}", tag_header().display()),
                TAG_MACRO,
                "https://vulkan.lunarg.com",
            ],
        );
        assert_reruns_on_the_sdk(&directives);
        assert_no_link_directives(&directives);
        assert!(host.generations.borrow().is_empty());
    }
}

#[test]
fn configure_reads_the_last_tag_definition_and_accepts_crlf_lines() {
    let redefined = format!(
        "#define {TAG_MACRO} \"2000.1-1-gdeadbeef0\"\n#undef {TAG_MACRO}\n#define {TAG_MACRO} \"{BUILD_TAG}\"\n"
    );
    let crlf = header_text(BUILD_TAG).replace('\n', "\r\n");
    let spaced = format!("  #define\t{TAG_MACRO}   \"{BUILD_TAG}\"  \n");
    for text in [redefined, crlf, spaced] {
        let (directives, result) = run(&sdk_host(&text));
        assert_eq!(result, Ok(()), "{text:?}");
        assert!(
            directives.ends_with(&link_directives(BUILD_TAG)),
            "{directives:?}"
        );
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
            "libslang-compiler.so on Linux, libslang-compiler.dylib on macOS, and the import library slang-compiler.lib on Windows",
            "https://vulkan.lunarg.com",
        ],
    );
    assert_reruns_on_the_sdk(&directives);
    assert_no_link_directives(&directives);
    assert!(host.generations.borrow().is_empty());
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
        let mut host = recorded_sdk_host();
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
        let mut host = recorded_sdk_host();
        host.host_os = os;
        set_var(&mut host, TARGET_OS, Some(OsString::from(os)));
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
        assert!(
            directives.ends_with(&link_directives(BUILD_TAG)),
            "{directives:?}"
        );
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
        let mut host = recorded_sdk_host();
        host.host_os = host_os;
        set_var(&mut host, TARGET_OS, target_os.map(OsString::from));
        host.entries.retain(|(entry, _)| *entry == lib_dir());
        let (directives, result) = run(&host);
        assert_eq!(result, Ok(()), "{host_os} {target_os:?}");
        assert!(
            directives.ends_with(&link_directives(BUILD_TAG)),
            "{directives:?}"
        );
    }
}

#[test]
fn configure_on_each_tested_target_emits_no_warning() {
    for target in TESTED_TARGETS {
        let mut host = recorded_sdk_host();
        set_var(&mut host, TARGET, Some(OsString::from(target)));
        let (directives, result) = run(&host);
        assert_eq!(result, Ok(()), "{target}");
        assert!(
            directives
                .iter()
                .all(|directive| !directive.starts_with("cargo::warning")),
            "{directives:?}"
        );
        assert_eq!(
            *host.generations.borrow(),
            [(include_dir(), target.to_owned())]
        );
        assert!(
            written(&host, "build_info.rs")
                .is_some_and(|text| text.contains("tested_target: true")),
            "{target}"
        );
    }
}

#[test]
fn configure_on_an_untested_target_warns_once_and_goes_on() {
    let mut host = recorded_sdk_host();
    set_var(
        &mut host,
        TARGET,
        Some(OsString::from("aarch64-unknown-linux-gnu")),
    );
    let (directives, result) = run(&host);
    assert_eq!(result, Ok(()));
    let warnings: Vec<&String> = directives
        .iter()
        .filter(|directive| directive.starts_with("cargo::warning="))
        .collect();
    assert_eq!(warnings.len(), 1, "{directives:?}");
    assert_contains_all(
        warnings[0],
        &[
            "untested and unsupported on the target aarch64-unknown-linux-gnu",
            TRIPLE,
        ],
    );
    assert!(
        directives.ends_with(&link_directives(BUILD_TAG)),
        "{directives:?}"
    );
    assert_eq!(
        *host.generations.borrow(),
        [(include_dir(), "aarch64-unknown-linux-gnu".to_owned())]
    );
    assert!(
        written(&host, "build_info.rs").is_some_and(|text| text.contains("tested_target: false"))
    );
}

#[test]
fn configure_stops_on_a_target_whose_pointer_width_is_not_64() {
    for (width, shown) in [
        (Some("32"), "a pointer width of \"32\" bits"),
        (None, "no pointer width"),
    ] {
        let mut host = recorded_sdk_host();
        set_var(
            &mut host,
            TARGET,
            Some(OsString::from("i686-pc-windows-msvc")),
        );
        set_var(&mut host, POINTER_WIDTH, width.map(OsString::from));
        let (directives, result) = run(&host);
        let message = result.expect_err("the build must stop");
        assert_contains_all(
            &message,
            &[
                "i686-pc-windows-msvc",
                shown,
                "support only 64-bit targets",
                "other struct layouts and calling conventions",
            ],
        );
        assert_reruns_on_the_sdk(&directives);
        assert_no_link_directives(&directives);
        assert!(host.generations.borrow().is_empty());
        assert!(host.written.borrow().is_empty());
    }
}

#[test]
fn configure_without_target_or_out_dir_says_that_cargo_sets_it() {
    for (name, value, problem) in [
        (TARGET, None, "TARGET is not set".to_owned()),
        (
            TARGET,
            Some(OsString::new()),
            "TARGET is \"\", which is not a target triple".to_owned(),
        ),
        (OUT_DIR, None, "OUT_DIR is not set".to_owned()),
        (
            OUT_DIR,
            Some(OsString::from("out")),
            "OUT_DIR is \"out\", which is not an absolute path in UTF-8 without line breaks"
                .to_owned(),
        ),
    ] {
        let mut host = recorded_sdk_host();
        set_var(&mut host, name, value);
        let (directives, result) = run(&host);
        let message = result.expect_err("the build must stop");
        assert_contains_all(&message, &[&problem, &format!("Cargo sets {name}")]);
        assert_reruns_on_the_sdk(&directives);
        assert_no_link_directives(&directives);
        assert!(host.generations.borrow().is_empty());
        assert!(host.written.borrow().is_empty());
    }
}

#[test]
fn configure_reports_each_kind_of_generator_failure() {
    let header = include_dir().display().to_string();
    for (kind, parts) in [
        (
            GenerationErrorKind::Libclang,
            vec!["install libclang or set LIBCLANG_PATH to the directory that holds it"],
        ),
        (
            GenerationErrorKind::Headers,
            vec![
                "The headers are those of the Slang build 2026.17-25-gc7954ebd1",
                "another Slang is best effort",
                "slang-src",
                "https://vulkan.lunarg.com",
                VENDORED_ADVICE,
            ],
        ),
        (
            GenerationErrorKind::Target,
            vec![
                "libclang does not accept the target triple x86_64-unknown-linux-gnu",
                "tested only on x86_64-unknown-linux-gnu, x86_64-pc-windows-msvc, aarch64-apple-darwin, x86_64-apple-darwin",
            ],
        ),
        (GenerationErrorKind::Other, vec![]),
    ] {
        let mut host = recorded_sdk_host();
        host.generator = Box::new(move |_, _| {
            Err(GenerationError {
                kind,
                message: "slang.h:3: the failure, caused by: its source".to_owned(),
            })
        });
        let (directives, result) = run(&host);
        let message = result.expect_err("the build must stop");
        let mut expected = vec![
            "cannot generate the Slang bindings from",
            header.as_str(),
            "for the target x86_64-unknown-linux-gnu",
            "slang.h:3: the failure, caused by: its source",
        ];
        expected.extend(&parts);
        assert_contains_all(&message, &expected);
        if kind != GenerationErrorKind::Libclang {
            assert!(!message.contains("install libclang"), "{message}");
        }
        if kind != GenerationErrorKind::Headers {
            assert!(!message.contains("best effort"), "{message}");
            assert!(!message.contains(VENDORED_ADVICE), "{message}");
        }
        if kind != GenerationErrorKind::Target {
            assert!(
                !message.contains("does not accept the target triple"),
                "{message}"
            );
        }
        assert_reruns_on_the_sdk(&directives);
        assert_no_link_directives(&directives);
        assert!(host.written.borrow().is_empty());
    }
}

#[test]
fn configure_reports_a_generated_file_that_cannot_be_written() {
    let denied = io::Error::from(io::ErrorKind::PermissionDenied).to_string();
    for name in ["bindings.rs", "symbols.rs", "build_info.rs"] {
        let path = out_root().join(name);
        let mut host = recorded_sdk_host();
        host.failures
            .push((FileOp::Write, path.clone(), io::ErrorKind::PermissionDenied));
        let (directives, result) = run(&host);
        let message = result.expect_err("the build must stop");
        assert_contains_all(
            &message,
            &[
                &format!("cannot write {}: {denied}", path.display()),
                "under OUT_DIR",
            ],
        );
        assert_reruns_on_the_sdk(&directives);
        assert_no_link_directives(&directives);
    }
}

#[test]
fn slang_identifiers_follow_the_form_of_a_build_tag() {
    for (tag, expected) in [
        ("2026.17-25-gc7954ebd1", Some("2026.17.0.25.gc7954ebd1")),
        ("2026.17.1-3-g0123456789", Some("2026.17.1.3.g0123456789")),
        ("2026.17", Some("2026.17.0.0")),
        ("2026.17.2", Some("2026.17.2.0")),
        ("2026.017-25-gc7954ebd1", None),
        ("2026.17-25-gc7954eb", None),
        ("2026.17-25-gC7954EBD1", None),
        ("2026", None),
        ("2026.17.1.4", None),
        ("v2026.17", None),
    ] {
        assert_eq!(slang_identifiers(tag).as_deref(), expected, "{tag}");
    }
}

fn shared_dir() -> PathBuf {
    std::env::temp_dir().join("target").join("slang-src")
}

fn source_root() -> PathBuf {
    shared_dir().join("src").join(COMMIT).join("tree")
}

fn entries_dir() -> PathBuf {
    shared_dir().join("slang-compiler-sys")
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

fn built_include() -> PathBuf {
    build_dir().join("Release").join("include")
}

fn log_file() -> PathBuf {
    out_root().join("slang").join("build.log")
}

fn stamp_file() -> PathBuf {
    out_root().join("slang").join("stamp")
}

const CMAKE_VERSION: &str = "cmake version 3.31.6";
const LINUX_LINKS: &[&str] = &["stdc++", "pthread", "dl"];
const MACOS_LINKS: &[&str] = &["c++"];
const WINDOWS_LINKS: &[&str] = &[
    "kernel32", "user32", "gdi32", "winspool", "shell32", "ole32", "oleaut32", "uuid", "comdlg32",
    "advapi32",
];
const COMPILER_VERSION: &str = "c++ (GCC) 15.2.1 20260101";

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
        invocation.program.as_str(),
        invocation.args.first().map_or("", String::as_str),
    )
}

fn happy(invocation: &Invocation) -> io::Result<Finished> {
    match subcommand(invocation) {
        ("git", "ls-files") if invocation.dir == Some(source_root()) => {
            Ok(finished(0, &superproject_listing(), ""))
        }
        ("git", "ls-files") => Ok(finished(
            0,
            "100644 5555555555555555555555555555555555555555 0\tCMakeLists.txt\0",
            "",
        )),
        ("ninja", "--version") => Ok(finished(0, "1.12.1\n", "")),
        ("cmake", "--version") => Ok(finished(
            0,
            &format!("{CMAKE_VERSION}\r\n\r\nCMake suite maintained by Kitware\n"),
            "",
        )),
        ("c++" | "clang++", "--version") => {
            Ok(finished(0, &format!("{COMPILER_VERSION}\nCopyright\n"), ""))
        }
        ("cmake", _) => Ok(finished(0, "", "")),
        _ => Err(io::Error::from(io::ErrorKind::NotFound)),
    }
}

fn vendored_host() -> FakeHost {
    let entries = STATIC_LIBRARIES
        .iter()
        .map(|library| {
            (
                archive_dir().join(format!("lib{library}.a")),
                Ok(EntryKind::File),
            )
        })
        .collect();
    FakeHost {
        vars: vec![
            (VENDORED, OsString::from("1")),
            (SRC_ROOT, source_root().into_os_string()),
            (SRC_COMMIT, OsString::from(COMMIT)),
            (SRC_BUILD_TAG, OsString::from(BUILD_TAG)),
            (SRC_SHARED_DIR, shared_dir().into_os_string()),
            (OUT_DIR, out_root().into_os_string()),
            (TARGET, OsString::from(TRIPLE)),
            (HOST, OsString::from(TRIPLE)),
            (TARGET_OS, OsString::from("linux")),
            (TARGET_ENV, OsString::from("gnu")),
            (TARGET_FEATURE, OsString::from("fxsr,sse,sse2")),
            (POINTER_WIDTH, OsString::from("64")),
            (NUM_JOBS, OsString::from("8")),
            (VULKAN_SDK, OsString::from("relative")),
        ],
        files: vec![(
            built_include().join("slang-tag-version.h"),
            header_text(BUILD_TAG),
        )],
        entries,
        trees: vec![
            (
                built_include(),
                vec![
                    PathBuf::from("slang.h"),
                    PathBuf::from("slang-tag-version.h"),
                ],
            ),
            (
                source_root().join("include"),
                vec![PathBuf::from("slang.h")],
            ),
        ],
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

fn expected_inputs(target: &str, configure: &[String], variables: &str, compiler: &str) -> String {
    let configure: Vec<String> = configure
        .iter()
        .map(|arg| format!("configure={arg}\n"))
        .collect();
    format!(
        "crate=slang-compiler-sys {CRATE_VERSION}\nfiles=\ncommit={COMMIT}\nbuild_tag={BUILD_TAG}\ntarget={target}\ncmake={CMAKE_VERSION}\n{}{variables}compiler={compiler}\n",
        configure.concat()
    )
}

fn no_variables() -> &'static str {
    "CC=\nCXX=\nCFLAGS=\nCXXFLAGS=\nLDFLAGS=\nCMAKE_TOOLCHAIN_FILE=\nMACOSX_DEPLOYMENT_TARGET=\nVCToolsVersion=\n"
}

fn linux_configure() -> Vec<String> {
    let mut args = vec![
        format!("-DSLANG_VERSION_FULL={BUILD_TAG}"),
        "-DSLANG_VERSION_NUMERIC=2026.17".to_owned(),
        "-DFETCHCONTENT_FULLY_DISCONNECTED=ON".to_owned(),
    ];
    args.extend(SLANG_OPTIONS.iter().map(|option| (*option).to_owned()));
    args
}

fn linux_inputs() -> String {
    expected_inputs(TRIPLE, &linux_configure(), no_variables(), COMPILER_VERSION)
}

fn linux_key() -> String {
    fnv1a(linux_inputs().as_bytes())
}

fn entry_dir() -> PathBuf {
    entries_dir().join(linux_key())
}

fn temporary_dir() -> PathBuf {
    entries_dir().join(format!("{}.tmp-{PROCESS_ID}", linux_key()))
}

fn vendored_reruns() -> Vec<String> {
    let mut reruns = vec!["cargo::rerun-if-env-changed=LIBCLANG_PATH".to_owned()];
    reruns.extend(
        [
            "CC",
            "CXX",
            "CFLAGS",
            "CXXFLAGS",
            "LDFLAGS",
            "CMAKE_TOOLCHAIN_FILE",
            "MACOSX_DEPLOYMENT_TARGET",
            "VCINSTALLDIR",
            "VCToolsVersion",
        ]
        .map(|name| format!("cargo::rerun-if-env-changed={name}")),
    );
    reruns
}

fn vendored_links(entry: &Path, dynamic: &[&str]) -> Vec<String> {
    let lib = entry.join("lib");
    let mut directives = vec![format!("cargo::rustc-link-search=native={}", lib.display())];
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
    directives.extend(
        dynamic
            .iter()
            .map(|library| format!("cargo::rustc-link-lib=dylib={library}")),
    );
    directives.extend([
        format!(
            "cargo::metadata=include={}",
            entry.join("include").display()
        ),
        format!("cargo::metadata=lib={}", lib.display()),
        "cargo::metadata=linkage=static".to_owned(),
        format!("cargo::metadata=build_tag={BUILD_TAG}"),
    ]);
    directives
}

fn vendored_build_info(tested: bool) -> String {
    format!(
        "BuildInfo {{ slang_build_tag: {BUILD_TAG:?}, slang_commit: Some({COMMIT:?}), linkage: Linkage::Static, tested_target: {tested}, version: \"{CRATE_VERSION}+slang.2026.17.0.25.gc7954ebd1.vendored\" }}\n"
    )
}

fn assert_vulkan_sdk_unread(host: &FakeHost) {
    assert!(
        !host
            .read_vars
            .borrow()
            .iter()
            .any(|name| name == VULKAN_SDK),
        "the vendored route read {VULKAN_SDK}"
    );
}

fn vendored_failure(host: &FakeHost, parts: &[&str]) -> Vec<String> {
    let (directives, result) = run(host);
    let message = result.err().unwrap_or_default();
    assert_contains_all(&message, parts);
    assert_no_link_directives(&directives);
    assert!(directives.starts_with(&vendored_reruns()), "{directives:?}");
    assert_vulkan_sdk_unread(host);
    assert!(
        !host
            .written
            .borrow()
            .iter()
            .any(|(path, _)| *path == stamp_file() || path.starts_with(shared_dir())),
        "a failed build wrote the stamp or into the shared folder"
    );
    assert!(
        host.renamed.borrow().is_empty(),
        "a failed build stored an entry"
    );
    directives
}

#[test]
fn fnv1a_gives_the_published_test_values() {
    assert_eq!(fnv1a(b""), "cbf29ce484222325");
    assert_eq!(fnv1a(b"a"), "af63dc4c8601ec8c");
    assert_eq!(fnv1a(b"foobar"), "85944171f73967e8");
}

#[test]
fn vendored_route_builds_stores_and_links_a_static_slang_without_reading_vulkan_sdk() {
    let host = vendored_host();
    let (directives, result) = run(&host);
    assert_eq!(result, Ok(()));
    let mut expected = vendored_reruns();
    expected.push(format!(
        "cargo::rerun-if-changed={}",
        entry_dir().join("stamp").display()
    ));
    expected.extend(vendored_links(&entry_dir(), LINUX_LINKS));
    assert_eq!(directives, expected);
    assert_vulkan_sdk_unread(&host);
    assert_eq!(
        *host.generations.borrow(),
        [(entry_dir().join("include"), TRIPLE.to_owned())]
    );
    assert_eq!(written(&host, "bindings.rs"), Some(generated().bindings));
    assert_eq!(written(&host, "symbols.rs"), Some(generated().symbols));
    assert_eq!(
        written(&host, "build_info.rs"),
        Some(vendored_build_info(true))
    );
    assert_eq!(
        *host.locked.borrow(),
        [entries_dir().join(format!("{}.lock", linux_key()))]
    );
    assert_eq!(*host.renamed.borrow(), [(temporary_dir(), entry_dir())]);
    assert!(host.removed_dirs.borrow().is_empty());
    let read_vars = host.read_vars.borrow();
    assert_eq!(
        read_vars[..12],
        [
            VENDORED,
            SRC_ROOT,
            SRC_COMMIT,
            SRC_BUILD_TAG,
            SRC_SHARED_DIR,
            OUT_DIR,
            TARGET,
            HOST,
            TARGET_OS,
            TARGET_ENV,
            TARGET_FEATURE,
            POINTER_WIDTH,
        ]
    );
}

#[test]
fn vendored_route_stores_the_libraries_headers_inputs_and_an_old_stamp_in_the_entry() {
    let host = vendored_host();
    assert_eq!(run(&host).1, Ok(()));
    let temporary = temporary_dir();
    let synced = host.synced.borrow();
    let stored: Vec<&(PathBuf, PathBuf)> = synced
        .iter()
        .filter(|(_, to)| to.starts_with(&temporary))
        .collect();
    let mut expected: Vec<(PathBuf, PathBuf)> = STATIC_LIBRARIES
        .iter()
        .map(|library| {
            let name = format!("lib{library}.a");
            (archive_dir().join(&name), temporary.join("lib").join(&name))
        })
        .collect();
    for header in ["slang.h", "slang-tag-version.h"] {
        expected.push((
            built_include().join(header),
            temporary.join("include").join(header),
        ));
    }
    assert_eq!(stored, expected.iter().collect::<Vec<_>>());
    let written = host.written.borrow();
    let entry_writes: Vec<&(PathBuf, String)> = written
        .iter()
        .filter(|(path, _)| path.starts_with(shared_dir()))
        .collect();
    assert_eq!(
        entry_writes,
        [
            &(temporary.join("inputs"), linux_inputs()),
            &(temporary.join("stamp"), String::new()),
        ]
    );
    assert_eq!(
        *host.modified.borrow(),
        [(
            temporary.join("stamp"),
            SystemTime::UNIX_EPOCH + Duration::from_hours(262_968)
        )]
    );
    let events = host.events.borrow();
    let position = |event: String| {
        events
            .iter()
            .position(|recorded| *recorded == event)
            .unwrap_or(usize::MAX)
    };
    let lock = position(format!(
        "Lock {}",
        entries_dir()
            .join(format!("{}.lock", linux_key()))
            .display()
    ));
    let stamp = position(format!("SetModified {}", temporary.join("stamp").display()));
    let rename = position(format!("Rename {}", entry_dir().display()));
    assert!(lock < stamp && stamp < rename, "{events:?}");
    assert!(
        position(format!("Write {}", temporary.join("inputs").display()))
            < position(format!("Write {}", temporary.join("stamp").display())),
        "{events:?}"
    );
}

#[test]
fn vendored_route_runs_ninja_cmake_git_and_the_compiler_with_their_arguments_and_environment() {
    let host = vendored_host();
    assert_eq!(run(&host).1, Ok(()));
    let mut expected_runs = vec![
        "ninja --version".to_owned(),
        "c++ --version".to_owned(),
        "cmake --version".to_owned(),
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
    let git_dirs: Vec<Option<PathBuf>> = runs
        .iter()
        .filter(|(invocation, _)| invocation.program == "git")
        .map(|(invocation, _)| invocation.dir.clone())
        .collect();
    let mut expected_dirs = vec![Some(source_root())];
    expected_dirs.extend(
        REQUIRED_SUBMODULES
            .iter()
            .map(|submodule| Some(source_root().join(submodule))),
    );
    assert_eq!(git_dirs, expected_dirs);
    for (invocation, log) in runs.iter() {
        if invocation.program == "git" {
            assert_eq!(invocation.args, ["ls-files", "-z", "--stage"]);
            assert_eq!(
                invocation.env,
                [(OsString::from("GIT_OPTIONAL_LOCKS"), OsString::from("0"))]
            );
            assert_eq!(invocation.env_remove, GIT_REPOSITORY_VARIABLES);
            assert_eq!(*log, None);
        } else if invocation
            .args
            .first()
            .is_some_and(|arg| arg != "--version")
        {
            assert_eq!(*log, Some(log_file()), "{invocation}");
            assert_eq!(invocation.env_remove, ["SLANG_USE_SCCACHE"], "{invocation}");
            assert!(invocation.env.is_empty(), "{invocation}");
            assert_eq!(invocation.dir, None, "{invocation}");
        } else {
            assert_eq!(invocation.args, ["--version"], "{invocation}");
            assert_eq!(*log, None, "{invocation}");
            assert!(invocation.env.is_empty() && invocation.env_remove.is_empty());
        }
    }
    let mut configure = vec![
        "-S".to_owned(),
        copy_dir().display().to_string(),
        "-B".to_owned(),
        build_dir().display().to_string(),
        "-G".to_owned(),
        "Ninja".to_owned(),
        format!(
            "-DCMAKE_ARCHIVE_OUTPUT_DIRECTORY={}",
            archive_dir().display()
        ),
    ];
    configure.extend(linux_configure());
    assert_eq!(runs[runs.len() - 2].0.args, configure);
    assert_eq!(
        runs[runs.len() - 1].0.to_string(),
        format!(
            "cmake --build {} --target slang --parallel 8",
            build_dir().display()
        )
    );
}

#[test]
fn vendored_route_copies_the_tracked_files_and_writes_the_stamp_after_the_build() {
    let host = vendored_host();
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
    let synced = host.synced.borrow();
    assert_eq!(synced[..expected_sync.len()], expected_sync);
    assert_eq!(
        *host.removed.borrow(),
        [stamp_file(), build_dir().join("CMakeCache.txt")]
    );
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
    assert_eq!(written[0], (log_file(), String::new()));
    let stamp = written
        .iter()
        .find(|(path, _)| *path == stamp_file())
        .map_or_default(|(_, text)| text.clone());
    for part in [
        &format!("source {}", source_root().display()),
        &format!("commit {COMMIT}"),
        &format!("build_tag {BUILD_TAG}"),
        &format!("-DSLANG_VERSION_FULL={BUILD_TAG}"),
        &format!(
            "-DCMAKE_ARCHIVE_OUTPUT_DIRECTORY={}",
            archive_dir().display()
        ),
        "external/vulkan",
    ] {
        assert!(stamp.contains(part), "{stamp:?} lacks {part:?}");
    }
}

#[test]
fn vendored_route_with_a_matching_stamp_skips_the_copy_and_the_configure_step() {
    let first = vendored_host();
    assert_eq!(run(&first).1, Ok(()));
    let stamp = first
        .written
        .borrow()
        .iter()
        .find(|(path, _)| *path == stamp_file())
        .map_or_default(|(_, text)| text.clone());

    let mut host = vendored_host();
    host.files.push((stamp_file(), stamp.clone()));
    let (directives, result) = run(&host);
    assert_eq!(result, Ok(()));
    assert!(directives.ends_with(&vendored_links(&entry_dir(), LINUX_LINKS)));
    assert_eq!(
        ran(&host),
        [
            "ninja --version",
            "c++ --version",
            "cmake --version",
            "cmake --build"
        ]
    );
    assert!(
        host.synced
            .borrow()
            .iter()
            .all(|(_, to)| to.starts_with(temporary_dir())),
        "the build copied the source tree"
    );
    assert!(host.removed.borrow().is_empty());
    assert!(
        !host
            .written
            .borrow()
            .iter()
            .any(|(path, _)| *path == stamp_file())
    );
    assert_eq!(*host.renamed.borrow(), [(temporary_dir(), entry_dir())]);

    let mut moved = vendored_host();
    moved.files.push((stamp_file(), stamp));
    set_var(&mut moved, SRC_COMMIT, Some(OsString::from("0".repeat(40))));
    assert_eq!(run(&moved).1, Ok(()));
    assert!(ran(&moved).contains(&"cmake -S".to_owned()));
}

#[test]
fn vendored_route_removes_files_of_the_copy_that_git_does_not_track() {
    let mut host = vendored_host();
    host.trees.push((
        copy_dir(),
        vec![
            PathBuf::from("CMakeLists.txt"),
            PathBuf::from("source/slang/core.meta.slang.temp.h"),
        ],
    ));
    assert_eq!(run(&host).1, Ok(()));
    assert_eq!(
        *host.removed.borrow(),
        [
            stamp_file(),
            build_dir().join("CMakeCache.txt"),
            copy_dir().join("source/slang/core.meta.slang.temp.h")
        ]
    );
}

fn complete_entry(host: &mut FakeHost) {
    host.entries.push((entry_dir(), Ok(EntryKind::Directory)));
    for library in STATIC_LIBRARIES {
        host.entries.push((
            entry_dir().join("lib").join(format!("lib{library}.a")),
            Ok(EntryKind::File),
        ));
    }
    for header in ["slang.h", "slang-tag-version.h"] {
        host.entries.push((
            entry_dir().join("include").join(header),
            Ok(EntryKind::File),
        ));
    }
    host.files.push((
        entry_dir().join("include").join("slang-tag-version.h"),
        header_text(BUILD_TAG),
    ));
    host.files
        .push((entry_dir().join("inputs"), linux_inputs()));
}

#[test]
fn vendored_route_uses_a_complete_entry_without_building() {
    let mut host = vendored_host();
    complete_entry(&mut host);
    let (directives, result) = run(&host);
    assert_eq!(result, Ok(()));
    let mut expected = vendored_reruns();
    expected.push(format!(
        "cargo::rerun-if-changed={}",
        entry_dir().join("stamp").display()
    ));
    expected.extend(vendored_links(&entry_dir(), LINUX_LINKS));
    assert_eq!(directives, expected);
    assert_eq!(
        ran(&host),
        ["ninja --version", "c++ --version", "cmake --version"]
    );
    assert!(host.synced.borrow().is_empty());
    assert!(host.renamed.borrow().is_empty());
    assert!(host.removed_dirs.borrow().is_empty());
    let written: Vec<PathBuf> = host
        .written
        .borrow()
        .iter()
        .map(|(path, _)| path.clone())
        .collect();
    assert_eq!(
        written,
        ["bindings.rs", "symbols.rs", "build_info.rs"].map(|name| out_root().join(name))
    );
    assert_eq!(
        *host.generations.borrow(),
        [(entry_dir().join("include"), TRIPLE.to_owned())]
    );
}

type Damage<'a> = Box<dyn Fn(&mut FakeHost) + 'a>;

fn assert_replaced(host: &FakeHost, case: &str) {
    assert_eq!(run(host).1, Ok(()), "{case}");
    assert!(ran(host).contains(&"cmake --build".to_owned()), "{case}");
    assert_eq!(*host.removed_dirs.borrow(), [entry_dir()], "{case}");
    assert_eq!(
        *host.renamed.borrow(),
        [(temporary_dir(), entry_dir())],
        "{case}"
    );
    let events = host.events.borrow();
    let removal = events
        .iter()
        .position(|event| *event == format!("RemoveDir {}", entry_dir().display()));
    let rename = events
        .iter()
        .position(|event| *event == format!("Rename {}", entry_dir().display()));
    assert!(removal < rename, "{case}: {events:?}");
}

#[test]
fn vendored_route_replaces_an_entry_whose_inputs_libraries_or_headers_differ() {
    let inputs = entry_dir().join("inputs");
    let header = entry_dir().join("include").join("slang.h");
    let tag = entry_dir().join("include").join("slang-tag-version.h");
    let library = entry_dir().join("lib").join("liblz4.a");
    let cases: [(&str, Damage<'_>); 6] = [
        (
            "other inputs",
            Box::new(|host| {
                host.files.retain(|(path, _)| *path != inputs);
                host.files
                    .push((inputs.clone(), linux_inputs() + "extra\n"));
            }),
        ),
        (
            "no inputs",
            Box::new(|host| host.files.retain(|(path, _)| *path != inputs)),
        ),
        (
            "missing library",
            Box::new(|host| host.entries.retain(|(path, _)| *path != library)),
        ),
        (
            "missing header",
            Box::new(|host| host.entries.retain(|(path, _)| *path != header)),
        ),
        (
            "unreadable version header",
            Box::new(|host| host.files.retain(|(path, _)| *path != tag)),
        ),
        (
            "other build tag",
            Box::new(|host| {
                host.files.retain(|(path, _)| *path != tag);
                host.files.push((tag.clone(), header_text("0.0.0-unknown")));
            }),
        ),
    ];
    for (case, damage) in cases {
        let mut host = vendored_host();
        complete_entry(&mut host);
        damage(&mut host);
        assert_replaced(&host, case);
    }
}

#[test]
fn vendored_route_reports_a_source_include_directory_that_cannot_be_listed() {
    let mut host = vendored_host();
    complete_entry(&mut host);
    let headers = source_root().join("include");
    host.failures.push((
        FileOp::List,
        headers.clone(),
        io::ErrorKind::PermissionDenied,
    ));
    vendored_failure(&host, &[&shared_message("list the files of", &headers)]);
    assert!(!ran(&host).contains(&"cmake --build".to_owned()));
}

#[test]
fn vendored_route_removes_only_the_numbered_temporary_directories_of_its_entry() {
    let key = linux_key();
    let mut host = vendored_host();
    host.listings.push((
        entries_dir(),
        [
            format!("{key}.tmp-12"),
            format!("{key}.tmp-4242"),
            format!("{key}.tmp-"),
            format!("{key}.tmp-1a"),
            format!("{key}.lock"),
            "0123456789abcdef.tmp-3".to_owned(),
            key.clone(),
        ]
        .map(OsString::from)
        .to_vec(),
    ));
    assert_eq!(run(&host).1, Ok(()));
    assert_eq!(
        *host.removed_dirs.borrow(),
        [
            entries_dir().join(format!("{key}.tmp-12")),
            entries_dir().join(format!("{key}.tmp-4242")),
        ]
    );
}

fn msvc_root() -> PathBuf {
    std::env::temp_dir()
        .join("vs")
        .join("VC")
        .join("Tools")
        .join("MSVC")
        .join("14.44.35207")
}

fn msvc_compiler() -> MsvcCompiler {
    let bin = msvc_root().join("bin").join("Hostx64").join("x64");
    MsvcCompiler {
        path: bin.join("cl.exe"),
        env: vec![
            (OsString::from("PATH"), bin.into_os_string()),
            (
                OsString::from("INCLUDE"),
                msvc_root().join("include").into_os_string(),
            ),
        ],
    }
}

fn assert_toolchain(host: &FakeHost, triple: &str, msvc: bool) {
    let expected_lookups: &[&str] = if msvc { &[triple] } else { &[] };
    assert_eq!(*host.msvc_lookups.borrow(), expected_lookups, "{triple}");
    let expected_env = if msvc {
        msvc_compiler().env
    } else {
        Vec::new()
    };
    for (invocation, log) in host.runs.borrow().iter() {
        if log.is_some() {
            assert_eq!(invocation.env, expected_env, "{triple}: {invocation}");
        }
    }
}

fn target_host(os: &str, env: &str, feature: &str, triple: &str) -> FakeHost {
    let mut host = vendored_host();
    set_var(&mut host, TARGET_OS, Some(OsString::from(os)));
    set_var(&mut host, TARGET_ENV, Some(OsString::from(env)));
    set_var(&mut host, TARGET_FEATURE, Some(OsString::from(feature)));
    set_var(&mut host, TARGET, Some(OsString::from(triple)));
    set_var(&mut host, HOST, Some(OsString::from(triple)));
    if env == "msvc" {
        host.msvc = Some(msvc_compiler());
    }
    host
}

#[test]
fn vendored_route_links_the_archives_cxx_and_system_libraries_of_each_kind_of_target() {
    for (os, env, feature, triple, dynamic, archive, runtime) in [
        (
            "linux",
            "gnu",
            "",
            "aarch64-unknown-linux-gnu",
            LINUX_LINKS,
            "lib{}.a",
            None,
        ),
        (
            "macos",
            "",
            "",
            "aarch64-apple-darwin",
            MACOS_LINKS,
            "lib{}.a",
            None,
        ),
        (
            "windows",
            "msvc",
            "",
            "x86_64-pc-windows-msvc",
            WINDOWS_LINKS,
            "{}.lib",
            Some("MultiThreadedDLL"),
        ),
        (
            "windows",
            "msvc",
            "crt-static",
            "x86_64-pc-windows-msvc",
            WINDOWS_LINKS,
            "{}.lib",
            Some("MultiThreaded"),
        ),
    ] {
        let mut host = target_host(os, env, feature, triple);
        host.entries.clear();
        for library in STATIC_LIBRARIES {
            host.entries.push((
                archive_dir().join(archive.replace("{}", library)),
                Ok(EntryKind::File),
            ));
        }
        set_var(
            &mut host,
            "VCToolsVersion",
            Some(OsString::from("14.44.35207")),
        );
        let (directives, result) = run(&host);
        assert_eq!(result, Ok(()), "{triple}");
        let mut configure = linux_configure();
        if let Some(runtime) = runtime {
            configure.push(format!("-DCMAKE_MSVC_RUNTIME_LIBRARY={runtime}"));
            configure.push("-DSLANG_ENABLE_SPIRV_TOOLS_MIMALLOC=OFF".to_owned());
        }
        let msvc_path = msvc_compiler().path.display().to_string();
        let compiler = if runtime.is_some() {
            msvc_path.as_str()
        } else {
            COMPILER_VERSION
        };
        let inputs = expected_inputs(
            triple,
            &configure,
            &no_variables().replace("VCToolsVersion=", "VCToolsVersion=14.44.35207"),
            compiler,
        );
        assert_toolchain(&host, triple, runtime.is_some());
        let entry = entries_dir().join(fnv1a(inputs.as_bytes()));
        assert!(
            directives.ends_with(&vendored_links(&entry, dynamic)),
            "{triple}: {directives:?}"
        );
        assert_eq!(
            ran(&host).contains(&"c++ --version".to_owned()),
            runtime.is_none(),
            "{triple}"
        );
        let stored: Vec<PathBuf> = host
            .synced
            .borrow()
            .iter()
            .filter(|(_, to)| to.parent().is_some_and(|dir| dir.ends_with("lib")))
            .map(|(from, _)| from.clone())
            .collect();
        assert_eq!(
            stored,
            STATIC_LIBRARIES.map(|library| archive_dir().join(archive.replace("{}", library))),
            "{triple}"
        );
        let warned = directives
            .iter()
            .any(|directive| directive.starts_with("cargo::warning="));
        assert_eq!(warned, !TESTED_TARGETS.contains(&triple), "{triple}");
    }
}

#[test]
fn vendored_route_keys_the_entry_on_the_compiler_and_the_cmake_variables() {
    let base = vendored_host();
    assert_eq!(run(&base).1, Ok(()));
    let mut host = vendored_host();
    set_var(&mut host, CXX, Some(OsString::from("clang++")));
    set_var(&mut host, "CFLAGS", Some(OsString::from("-O2")));
    assert_eq!(run(&host).1, Ok(()));
    assert!(ran(&host).contains(&"clang++ --version".to_owned()));
    let inputs = expected_inputs(
        TRIPLE,
        &linux_configure(),
        &no_variables()
            .replace("CXX=", "CXX=clang++")
            .replace("CFLAGS=", "CFLAGS=-O2"),
        COMPILER_VERSION,
    );
    let entry = entries_dir().join(fnv1a(inputs.as_bytes()));
    assert_ne!(entry, entry_dir());
    assert_eq!(
        host.renamed.borrow()[0].1,
        entry,
        "the entry key ignores CXX or CFLAGS"
    );
}

#[test]
fn vendored_route_on_an_untested_target_warns_before_the_native_build() {
    let mut host = target_host("linux", "gnu", "", "aarch64-unknown-linux-gnu");
    respond_with(&mut host, "ninja", "--version", || {
        Err(io::Error::from(io::ErrorKind::NotFound))
    });
    let directives = vendored_failure(&host, &["cannot start ninja"]);
    let warnings: Vec<&String> = directives
        .iter()
        .filter(|directive| directive.starts_with("cargo::warning="))
        .collect();
    assert_eq!(warnings.len(), 1, "{directives:?}");
    assert!(
        warnings[0].contains("untested and unsupported on the target aarch64-unknown-linux-gnu")
    );

    let host = target_host("linux", "gnu", "", "aarch64-unknown-linux-gnu");
    assert_eq!(run(&host).1, Ok(()));
    assert!(
        written(&host, "build_info.rs").is_some_and(|text| text.contains("tested_target: false"))
    );
}

#[test]
fn vendored_route_on_each_tested_target_emits_no_warning() {
    for (os, env, triple) in [
        ("linux", "gnu", "x86_64-unknown-linux-gnu"),
        ("windows", "msvc", "x86_64-pc-windows-msvc"),
        ("macos", "", "aarch64-apple-darwin"),
        ("macos", "", "x86_64-apple-darwin"),
    ] {
        let mut host = target_host(os, env, "", triple);
        if env == "msvc" {
            host.entries = STATIC_LIBRARIES
                .iter()
                .map(|library| {
                    (
                        archive_dir().join(format!("{library}.lib")),
                        Ok(EntryKind::File),
                    )
                })
                .collect();
        }
        let (directives, result) = run(&host);
        assert_eq!(result, Ok(()), "{triple}");
        assert!(
            directives
                .iter()
                .all(|directive| !directive.starts_with("cargo::warning")),
            "{directives:?}"
        );
        assert_eq!(
            written(&host, "build_info.rs"),
            Some(vendored_build_info(true)),
            "{triple}"
        );
    }
}

#[test]
fn vendored_route_refuses_cross_compilation_and_targets_without_a_link_rule() {
    let mut cross = vendored_host();
    set_var(
        &mut cross,
        TARGET,
        Some(OsString::from("aarch64-unknown-linux-gnu")),
    );
    vendored_failure(
        &cross,
        &[
            "the target aarch64-unknown-linux-gnu differs from the host x86_64-unknown-linux-gnu",
            "cannot cross-compile, because Slang's build runs programs that it builds for the target",
            "Build on a host of the target",
            "build without the Cargo feature `vendored` to link the Slang of a Vulkan SDK",
        ],
    );
    assert!(cross.runs.borrow().is_empty());
    for (os, env, triple) in [
        ("linux", "musl", "x86_64-unknown-linux-musl"),
        ("freebsd", "", "x86_64-unknown-freebsd"),
        ("windows", "gnu", "x86_64-pc-windows-gnu"),
    ] {
        let host = target_host(os, env, "", triple);
        vendored_failure(
            &host,
            &[
                &format!("the target {triple} has no link rule for its C++ runtime library"),
                "cannot link Slang's C++ code",
                "Linux with the GNU C library, macOS, and Windows with the Microsoft toolchain",
                "build without the Cargo feature `vendored` to link the Slang of a Vulkan SDK",
            ],
        );
        assert!(host.runs.borrow().is_empty());
    }
}

#[test]
fn vendored_route_stops_on_a_target_whose_pointer_width_is_not_64() {
    let mut host = target_host("linux", "gnu", "", "i686-unknown-linux-gnu");
    set_var(&mut host, POINTER_WIDTH, Some(OsString::from("32")));
    vendored_failure(
        &host,
        &[
            "i686-unknown-linux-gnu",
            "a pointer width of \"32\" bits",
            "support only 64-bit targets",
        ],
    );
    assert!(host.runs.borrow().is_empty());
    assert!(host.generations.borrow().is_empty());
}

#[test]
fn vendored_route_checks_every_variable_of_slang_src() {
    let relative = OsString::from("relative/tree");
    let broken = source_root()
        .join("x\ncargo::rustc-link-lib=other")
        .into_os_string();
    for (name, value, problem) in [
        (SRC_ROOT, None, format!("{SRC_ROOT} is not set")),
        (
            SRC_ROOT,
            Some(relative),
            format!(
                "{SRC_ROOT} is \"relative/tree\", which is not an absolute path in UTF-8 without line breaks"
            ),
        ),
        (
            SRC_ROOT,
            Some(broken),
            "\\ncargo::rustc-link-lib=other\"".to_owned(),
        ),
        (
            SRC_ROOT,
            Some(non_utf8_path()),
            "which is not an absolute path in UTF-8 without line breaks".to_owned(),
        ),
        (SRC_COMMIT, None, format!("{SRC_COMMIT} is not set")),
        (
            SRC_COMMIT,
            Some(OsString::from("C7954EBD1F1984AD2BFDE15F76248DB41DC34AA3")),
            format!(
                "{SRC_COMMIT} is \"C7954EBD1F1984AD2BFDE15F76248DB41DC34AA3\", which is not a full commit hash of 40 lowercase hexadecimal digits"
            ),
        ),
        (
            SRC_COMMIT,
            Some(OsString::from("c7954ebd1")),
            "\"c7954ebd1\", which is not a full commit hash".to_owned(),
        ),
        (SRC_BUILD_TAG, None, format!("{SRC_BUILD_TAG} is not set")),
        (
            SRC_BUILD_TAG,
            Some(OsString::from("0.0.0-unknown")),
            format!("{SRC_BUILD_TAG} is \"0.0.0-unknown\", which is not a build tag"),
        ),
        (
            SRC_BUILD_TAG,
            Some(OsString::from("2026.17-25-gc7954eb")),
            "\"2026.17-25-gc7954eb\", which is not a build tag".to_owned(),
        ),
        (SRC_SHARED_DIR, None, format!("{SRC_SHARED_DIR} is not set")),
        (
            SRC_SHARED_DIR,
            Some(OsString::new()),
            format!(
                "{SRC_SHARED_DIR} is \"\", which is not an absolute path in UTF-8 without line breaks"
            ),
        ),
    ] {
        let mut host = vendored_host();
        set_var(&mut host, name, value);
        let (_, result) = run(&host);
        let message = result.err().unwrap_or_default();
        assert!(!message.contains(['\n', '\r']), "{message:?}");
        vendored_failure(
            &host,
            &[
                &problem,
                "from the metadata of slang-src",
                "direct dependency of the version that slang-compiler-sys requires",
            ],
        );
        assert!(host.runs.borrow().is_empty());
        assert!(host.locked.borrow().is_empty());
    }
}

#[test]
fn vendored_route_without_out_dir_target_or_host_says_that_cargo_sets_it() {
    for (name, value, problem) in [
        (OUT_DIR, None, "OUT_DIR is not set".to_owned()),
        (
            OUT_DIR,
            Some(OsString::from("out")),
            "OUT_DIR is \"out\", which is not an absolute path in UTF-8 without line breaks, so the build has no directory in which to build Slang"
                .to_owned(),
        ),
        (
            TARGET,
            None,
            "TARGET is not set, so the build of Slang from source cannot compare the target with the host"
                .to_owned(),
        ),
        (
            HOST,
            None,
            "HOST is not set, so the build of Slang from source cannot compare the target with the host"
                .to_owned(),
        ),
    ] {
        let mut host = vendored_host();
        set_var(&mut host, name, value);
        vendored_failure(&host, &[&problem, &format!("Cargo sets {name}")]);
        assert!(host.runs.borrow().is_empty());
    }
}

fn start_message(program: &str, args: &str) -> String {
    format!(
        "cannot start {program} for `{program} {args}`: {}. The build of Slang from source needs git, CMake 3.22 or later, and Ninja on PATH, and a C++ compiler. Install the missing program, or build without the Cargo feature `vendored` to link the Slang of a Vulkan SDK.",
        io::Error::from(io::ErrorKind::NotFound)
    )
}

#[test]
fn vendored_route_names_each_program_that_cannot_start() {
    for (program, first, args) in [
        ("ninja", "--version", "--version".to_owned()),
        ("cmake", "--version", "--version".to_owned()),
        ("c++", "--version", "--version".to_owned()),
        ("git", "ls-files", "ls-files -z --stage".to_owned()),
        (
            "cmake",
            "--build",
            format!(
                "--build {} --target slang --parallel 8",
                build_dir().display()
            ),
        ),
    ] {
        let mut host = vendored_host();
        respond_with(&mut host, program, first, || {
            Err(io::Error::from(io::ErrorKind::NotFound))
        });
        let (_, result) = run(&host);
        assert_eq!(
            result,
            Err(start_message(program, &args)),
            "{program} {first}"
        );
        vendored_failure(&host, &[&start_message(program, &args)]);
    }
}

#[test]
fn vendored_route_reports_a_configure_step_that_cannot_start() {
    let mut host = vendored_host();
    respond_with(&mut host, "cmake", "-S", || {
        Err(io::Error::from(io::ErrorKind::NotFound))
    });
    vendored_failure(
        &host,
        &[
            "cannot start cmake for `cmake -S",
            "needs git, CMake 3.22 or later, and Ninja on PATH",
        ],
    );
}

fn denied() -> String {
    io::Error::from(io::ErrorKind::PermissionDenied).to_string()
}

fn file_message(action: &str, path: &Path) -> String {
    format!(
        "cannot {action} {}: {}. The build of Slang from source keeps its copy of the source tree, its build tree, and its log under OUT_DIR.",
        path.display(),
        denied()
    )
}

fn shared_message(action: &str, path: &Path) -> String {
    format!(
        "cannot {action} {}: {}. The build keeps the libraries of Slang that it built in the shared folder of the Cargo target directory, and removing the entry, or running cargo clean, when no build runs is safe.",
        path.display(),
        denied()
    )
}

#[test]
fn vendored_route_reports_a_log_that_cannot_be_opened_without_blaming_the_program() {
    let mut host = vendored_host();
    host.failures
        .push((FileOp::OpenLog, log_file(), io::ErrorKind::PermissionDenied));
    let expected = file_message("open the log", &log_file());
    assert_eq!(run(&host).1, Err(expected.clone()));
    vendored_failure(&host, &[&expected]);
    assert!(!ran(&host).contains(&"cmake -S".to_owned()));
}

#[test]
fn vendored_route_names_a_program_that_exits_with_a_failure() {
    let submodule = source_root().join("external/lz4");
    for (program, first, dir) in [
        ("ninja", "--version", None),
        ("cmake", "--version", None),
        ("c++", "--version", None),
        ("git", "ls-files", Some(source_root())),
        ("git", "ls-files", Some(submodule)),
    ] {
        let mut host = vendored_host();
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
            .map_or_default(|dir| format!(" in {}", dir.display()));
        vendored_failure(
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
fn vendored_route_reports_a_signal_that_ends_a_program() {
    let signalled = || {
        Ok(Finished {
            exit: Exit { code: None },
            stdout: String::new(),
            stderr: String::new(),
        })
    };
    let mut probe = vendored_host();
    respond_with(&mut probe, "ninja", "--version", signalled);
    vendored_failure(
        &probe,
        &[
            "`ninja --version`",
            "failed with no exit status, because a signal ended it",
        ],
    );
    let mut build = vendored_host();
    respond_with(&mut build, "cmake", "--build", signalled);
    vendored_failure(
        &build,
        &[
            "`cmake --build",
            "failed with no exit status, because a signal ended it, so the build step",
        ],
    );
}

#[test]
fn vendored_route_refuses_unexpected_git_output_and_a_tracked_path_that_leaves_the_tree() {
    for entry in [
        "100644 1111111111111111111111111111111111111111 0\t../escape\0",
        "100644 1111111111111111111111111111111111111111 0\t/tmp/escape\0",
        "no tab here\0",
    ] {
        let mut host = vendored_host();
        host.respond = Box::new(move |invocation| {
            if subcommand(invocation) == ("git", "ls-files") {
                Ok(finished(0, entry, ""))
            } else {
                happy(invocation)
            }
        });
        let shown = entry.trim_end_matches('\0').escape_debug().to_string();
        vendored_failure(
            &host,
            &[
                "git ls-files -z --stage",
                &source_root().display().to_string(),
                &format!("printed \"{shown}\""),
                "not the output that the build expects from git",
            ],
        );
        assert!(host.synced.borrow().is_empty());
    }
}

#[test]
fn vendored_route_reports_a_failed_step_with_the_tail_of_its_log() {
    let log = (1..=50)
        .map(|line| format!("log line {line}\n"))
        .collect::<Vec<_>>()
        .concat();
    for first in ["-S", "--build"] {
        let mut host = vendored_host();
        host.files.push((log_file(), log.clone()));
        host.respond = Box::new(move |invocation| {
            if subcommand(invocation) == ("cmake", first) {
                Ok(finished(1, "", ""))
            } else {
                happy(invocation)
            }
        });
        let step = if first == "-S" { "configure" } else { "build" };
        vendored_failure(
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
        let message = run(&host).1.err().unwrap_or_default();
        assert!(
            !message.contains("log line 10\n"),
            "the message holds more than the tail"
        );
    }
}

#[test]
fn vendored_route_reports_a_failed_step_whose_log_cannot_be_read() {
    let mut host = vendored_host();
    respond_with(&mut host, "cmake", "-S", || Ok(finished(1, "", "")));
    vendored_failure(
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
fn vendored_route_omits_the_job_count_when_cargo_gives_none_or_an_unusable_one() {
    for jobs in [None, Some("many"), Some("-1"), Some("")] {
        let mut host = vendored_host();
        set_var(&mut host, NUM_JOBS, jobs.map(OsString::from));
        assert_eq!(run(&host).1, Ok(()));
        let runs = host.runs.borrow();
        assert_eq!(
            runs[runs.len() - 1].0.to_string(),
            format!("cmake --build {} --target slang", build_dir().display()),
            "{jobs:?}"
        );
    }
}

#[test]
fn vendored_route_reports_a_build_with_another_layout_or_build_tag() {
    let archive = archive_dir().join("libcmark-gfm.a");
    let header = built_include().join("slang-tag-version.h");
    let mut missing = vendored_host();
    missing.entries.retain(|(path, _)| *path != archive);
    let mut directory = vendored_host();
    set_entry(&mut directory, archive.clone(), Ok(EntryKind::Directory));
    let mut unreadable = vendored_host();
    unreadable.files.clear();
    let mut undefined = vendored_host();
    undefined.files = vec![(header.clone(), "#pragma once\n".to_owned())];
    let mut malformed = vendored_host();
    malformed.files = vec![(header.clone(), format!("#define {TAG_MACRO} 2026\n"))];
    for (host, part) in [
        (
            missing,
            format!(
                "cannot access {}: {}",
                archive.display(),
                io::Error::from(io::ErrorKind::NotFound)
            ),
        ),
        (directory, format!("{} is not a file", archive.display())),
        (unreadable, format!("cannot read {}", header.display())),
        (
            undefined,
            format!("{} does not define {TAG_MACRO}", header.display()),
        ),
        (
            malformed,
            format!("line 1 of {} does not define {TAG_MACRO}", header.display()),
        ),
    ] {
        vendored_failure(
            &host,
            &[
                "the build of Slang from source finished, but",
                &part,
                "another layout than the one that this version of slang-compiler-sys expects",
            ],
        );
    }

    let mut other = vendored_host();
    other.files = vec![(header.clone(), header_text("0.0.0-unknown"))];
    vendored_failure(
        &other,
        &[
            "reports the build tag 0.0.0-unknown",
            &header.display().to_string(),
            &format!("slang-src names the build tag {BUILD_TAG}"),
            "the bindings and the build information name",
        ],
    );
}

#[test]
fn vendored_route_names_the_action_and_the_path_of_each_file_operation_that_fails() {
    let stale = copy_dir().join("stale.h");
    let tracked = copy_dir().join("CMakeLists.txt");
    let cases = [
        (
            FileOp::Write,
            log_file(),
            file_message("write", &log_file()),
        ),
        (
            FileOp::Remove,
            stamp_file(),
            file_message("remove", &stamp_file()),
        ),
        (
            FileOp::List,
            copy_dir(),
            file_message("list the files of", &copy_dir()),
        ),
        (
            FileOp::Remove,
            stale.clone(),
            file_message("remove", &stale),
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
        ),
        (
            FileOp::Write,
            stamp_file(),
            file_message("write", &stamp_file()),
        ),
    ];
    for (op, path, part) in cases {
        let mut host = vendored_host();
        host.trees
            .push((copy_dir(), vec![PathBuf::from("stale.h")]));
        host.failures
            .push((op, path.clone(), io::ErrorKind::PermissionDenied));
        vendored_failure(&host, &[&part]);
    }

    let mut headers = vendored_host();
    headers.failures.push((
        FileOp::List,
        built_include(),
        io::ErrorKind::PermissionDenied,
    ));
    let (directives, result) = run(&headers);
    assert_eq!(
        result,
        Err(file_message("list the files of", &built_include()))
    );
    assert_no_link_directives(&directives);
    assert!(headers.renamed.borrow().is_empty());
}

#[test]
fn vendored_route_names_the_action_and_the_path_of_each_shared_folder_operation_that_fails() {
    let key = linux_key();
    let lock = entries_dir().join(format!("{key}.lock"));
    let stale = entries_dir().join(format!("{key}.tmp-7"));
    let temporary = temporary_dir();
    let stored_library = temporary.join("lib").join("libcore.a");
    let stored_header = temporary.join("include").join("slang.h");
    let cases = [
        (FileOp::Lock, lock.clone(), shared_message("lock", &lock)),
        (
            FileOp::ListDir,
            entries_dir(),
            shared_message("list", &entries_dir()),
        ),
        (
            FileOp::RemoveDir,
            stale.clone(),
            shared_message("remove", &stale),
        ),
        (
            FileOp::Sync,
            stored_library.clone(),
            shared_message(
                &format!("copy {} to", archive_dir().join("libcore.a").display()),
                &stored_library,
            ),
        ),
        (
            FileOp::Sync,
            stored_header.clone(),
            shared_message(
                &format!("copy {} to", built_include().join("slang.h").display()),
                &stored_header,
            ),
        ),
        (
            FileOp::Write,
            temporary.join("inputs"),
            shared_message("write", &temporary.join("inputs")),
        ),
        (
            FileOp::Write,
            temporary.join("stamp"),
            shared_message("write", &temporary.join("stamp")),
        ),
        (
            FileOp::SetModified,
            temporary.join("stamp"),
            shared_message("set the modification time of", &temporary.join("stamp")),
        ),
        (
            FileOp::Rename,
            entry_dir(),
            shared_message(&format!("rename {} to", temporary.display()), &entry_dir()),
        ),
    ];
    for (op, path, part) in cases {
        let mut host = vendored_host();
        host.listings
            .push((entries_dir(), vec![OsString::from(format!("{key}.tmp-7"))]));
        host.failures
            .push((op, path.clone(), io::ErrorKind::PermissionDenied));
        let (_, result) = run(&host);
        let message = result.err().unwrap_or_default();
        assert_contains_all(&message, &[&part]);
        assert_vulkan_sdk_unread(&host);
        assert!(host.generations.borrow().is_empty(), "{op:?}");
        assert!(host.renamed.borrow().is_empty(), "{op:?}");
    }

    let mut inspect = vendored_host();
    inspect
        .entries
        .push((entry_dir(), Err(io::ErrorKind::PermissionDenied)));
    vendored_failure(&inspect, &[&shared_message("inspect", &entry_dir())]);
    assert!(!ran(&inspect).contains(&"cmake -S".to_owned()));

    let mut replace = vendored_host();
    replace
        .entries
        .push((entry_dir(), Ok(EntryKind::Directory)));
    replace.failures.push((
        FileOp::RemoveDir,
        entry_dir(),
        io::ErrorKind::PermissionDenied,
    ));
    let (directives, result) = run(&replace);
    assert_contains_all(
        &result.err().unwrap_or_default(),
        &[&shared_message("remove", &entry_dir())],
    );
    assert_no_link_directives(&directives);
    assert!(replace.renamed.borrow().is_empty());
}

#[test]
fn vendored_route_accepts_a_stamp_that_does_not_exist() {
    let mut host = vendored_host();
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
fn vendored_route_removes_a_stale_stamp_before_the_copy_and_keeps_it_removed_on_failure() {
    let mut host = vendored_host();
    host.files.push((stamp_file(), "stale\n".to_owned()));
    respond_with(&mut host, "cmake", "-S", || Ok(finished(1, "", "")));
    vendored_failure(
        &host,
        &["the configure step of the build of Slang from source failed"],
    );
    let events = host.events.borrow();
    let position = |prefix: &str| {
        events
            .iter()
            .position(|event| event.starts_with(prefix))
            .unwrap_or(usize::MAX)
    };
    let removed = position(&format!("Remove {}", stamp_file().display()));
    assert!(removed < position("Sync "), "{events:?}");
    assert!(position("Sync ") < position("Run cmake -S"), "{events:?}");
    assert_eq!(
        position(&format!("Write {}", stamp_file().display())),
        usize::MAX,
        "{events:?}"
    );
}

#[test]
fn vendored_route_reports_a_generator_failure_for_the_headers_of_the_entry() {
    let mut host = vendored_host();
    host.generator = Box::new(|_, _| {
        Err(GenerationError {
            kind: GenerationErrorKind::Headers,
            message: "slang.h:3: the failure".to_owned(),
        })
    });
    let (directives, result) = run(&host);
    let message = result.err().unwrap_or_default();
    assert_contains_all(
        &message,
        &[
            &format!(
                "cannot generate the Slang bindings from {} for the target {TRIPLE}: slang.h:3: the failure",
                entry_dir().join("include").display()
            ),
            &format!("The headers are those of the Slang build {BUILD_TAG}"),
            VENDORED_ADVICE,
        ],
    );
    assert_no_link_directives(&directives);
    assert!(directives.starts_with(&vendored_reruns()));
    assert_vulkan_sdk_unread(&host);
    assert_eq!(*host.renamed.borrow(), [(temporary_dir(), entry_dir())]);
}

#[test]
fn numeric_version_drops_everything_after_the_first_hyphen() {
    assert_eq!(numeric_version("2026.17-25-gc7954ebd1"), "2026.17");
    assert_eq!(numeric_version("2026.17.1"), "2026.17.1");
}

#[test]
fn vendored_route_stops_when_it_finds_no_microsoft_compiler_and_says_where_it_looked() {
    for (developer, tools, shown_developer, shown_tools) in [
        (None, None, "not set", "not set"),
        (
            Some("C:\\VS\\VC\\"),
            Some("14.44.35207"),
            "\"C:\\\\VS\\\\VC\\\\\"",
            "\"14.44.35207\"",
        ),
    ] {
        let mut host = target_host("windows", "msvc", "", "x86_64-pc-windows-msvc");
        host.msvc = None;
        set_var(&mut host, "VCINSTALLDIR", developer.map(OsString::from));
        set_var(&mut host, "VCToolsVersion", tools.map(OsString::from));
        vendored_failure(
            &host,
            &[
                "cannot find the Microsoft C++ compiler cl.exe for the target x86_64-pc-windows-msvc",
                "runs CMake with Ninja, which needs the environment of the Microsoft toolchain",
                &format!("which VCINSTALLDIR marks and which is {shown_developer}"),
                "every installation of Visual Studio 2015 or later",
                &format!("the toolset that VCToolsVersion names, which is {shown_tools}"),
                "Install Visual Studio or its Build Tools with the workload for desktop development with C++",
                "build without the Cargo feature `vendored` to link the Slang of a Vulkan SDK",
            ],
        );
        assert_eq!(*host.msvc_lookups.borrow(), ["x86_64-pc-windows-msvc"]);
        assert_eq!(ran(&host), ["ninja --version"]);
        assert!(host.locked.borrow().is_empty());
    }
}

#[test]
fn vendored_route_probes_the_compiler_that_cxx_names_as_cmake_reads_it() {
    let spaced = std::env::temp_dir().join("my compilers").join("g++");
    let spaced_text = spaced.display().to_string();
    for (cxx, file, program, args) in [
        (Some("  "), false, "c++".to_owned(), vec!["--version"]),
        (
            Some("clang++"),
            false,
            "clang++".to_owned(),
            vec!["--version"],
        ),
        (
            Some("ccache g++ -m64"),
            false,
            "ccache".to_owned(),
            vec!["g++", "-m64", "--version"],
        ),
        (
            Some(spaced_text.as_str()),
            true,
            spaced_text.clone(),
            vec!["--version"],
        ),
    ] {
        let mut host = vendored_host();
        set_var(&mut host, CXX, cxx.map(OsString::from));
        if file {
            host.entries.push((spaced.clone(), Ok(EntryKind::File)));
        }
        let expected = program.clone();
        host.respond = Box::new(move |invocation| {
            if invocation.program == expected
                && invocation.args.last().is_some_and(|arg| arg == "--version")
            {
                Ok(finished(0, &format!("{COMPILER_VERSION}\n"), ""))
            } else {
                happy(invocation)
            }
        });
        assert_eq!(run(&host).1, Ok(()), "{cxx:?}");
        let runs = host.runs.borrow();
        let probe = &runs[1].0;
        assert_eq!(probe.program, program, "{cxx:?}");
        assert_eq!(probe.args, args, "{cxx:?}");
    }
}

#[test]
fn vendored_route_with_another_toolchain_configures_a_fresh_cmake_cache() {
    let first = vendored_host();
    assert_eq!(run(&first).1, Ok(()));
    let stamp = first
        .written
        .borrow()
        .iter()
        .find(|(path, _)| *path == stamp_file())
        .map_or_default(|(_, text)| text.clone());
    assert!(stamp.ends_with(&linux_inputs()), "{stamp:?}");

    let mut host = vendored_host();
    host.files.push((stamp_file(), stamp));
    set_var(&mut host, CXX, Some(OsString::from("clang++")));
    assert_eq!(run(&host).1, Ok(()));
    assert!(ran(&host).contains(&"cmake -S".to_owned()));
    assert_eq!(
        *host.removed.borrow(),
        [stamp_file(), build_dir().join("CMakeCache.txt")]
    );
}
