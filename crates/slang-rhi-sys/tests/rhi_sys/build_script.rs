use std::cell::RefCell;

include!("../../build_logic.rs");

mod ffi;

type Responder = Box<dyn Fn(&Invocation) -> io::Result<Finished>>;

const BUILD_TAG: &str = "2026.17-25-gc7954ebd1";
const COMMIT: &str = "c7954ebd1f1984ad2bfde15f76248db41dc34aa3";
const LINUX: &str = "x86_64-unknown-linux-gnu";
const PROCESS_ID: u32 = 4242;
const CMAKE_VERSION: &str = "cmake version 3.31.6";
const COMPILER_VERSION: &str = "c++ (GCC) 15.2.1 20260101";
const LINUX_LINKS: &[&str] = &["stdc++", "dl", "pthread"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FileOp {
    ListDir,
    OpenLog,
    Copy,
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
    listings: Vec<(PathBuf, Vec<OsString>)>,
    respond: Responder,
    failures: Vec<(FileOp, PathBuf, io::ErrorKind)>,
    read_vars: RefCell<Vec<String>>,
    runs: RefCell<Vec<(Invocation, Option<PathBuf>)>>,
    copied: RefCell<Vec<(PathBuf, PathBuf)>>,
    written: RefCell<Vec<(PathBuf, String)>>,
    modified: RefCell<Vec<(PathBuf, SystemTime)>>,
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

    fn list(&self, dir: &Path) -> io::Result<Vec<OsString>> {
        self.attempt(FileOp::ListDir, dir)?;
        Ok(self
            .listings
            .iter()
            .find(|(listed, _)| listed == dir)
            .map_or_default(|(_, names)| names.clone()))
    }

    fn copy_file(&self, from: &Path, to: &Path) -> io::Result<()> {
        self.attempt(FileOp::Copy, to)?;
        self.copied
            .borrow_mut()
            .push((from.to_path_buf(), to.to_path_buf()));
        Ok(())
    }

    fn remove_file(&self, path: &Path) -> io::Result<()> {
        self.attempt(FileOp::Remove, path)
    }

    fn remove_dir_all(&self, path: &Path) -> io::Result<()> {
        self.attempt(FileOp::RemoveDir, path)
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
        self.events
            .borrow_mut()
            .push(format!("Renamed {}", from.display()));
        Ok(())
    }

    fn lock(&self, path: &Path) -> io::Result<()> {
        self.attempt(FileOp::Lock, path)
    }

    fn process_id(&self) -> u32 {
        PROCESS_ID
    }
}

fn root() -> PathBuf {
    std::env::temp_dir().join("rhi-fake")
}

fn source_root() -> PathBuf {
    root()
        .join("shared")
        .join("src")
        .join(format!("{COMMIT}-0123456789abcdef"))
        .join("tree")
}

fn slang_entry() -> PathBuf {
    root()
        .join("shared")
        .join("slang-compiler-sys")
        .join("fedcba9876543210")
}

fn shared_dir() -> PathBuf {
    root().join("shared")
}

fn entries_dir() -> PathBuf {
    shared_dir().join(ENTRY_FOLDER)
}

fn out_dir() -> PathBuf {
    root().join("out")
}

fn manifest_dir() -> PathBuf {
    root().join("crate")
}

fn finished(code: i32, stdout: &str, stderr: &str) -> Finished {
    Finished {
        exit: Exit { code: Some(code) },
        stdout: stdout.to_owned(),
        stderr: stderr.to_owned(),
    }
}

fn happy(invocation: &Invocation) -> Finished {
    match (invocation.program.as_str(), invocation.args.first()) {
        ("cmake", Some(arg)) if arg == "--version" => {
            finished(0, &format!("{CMAKE_VERSION}\n\nCMake suite\n"), "")
        }
        (_, Some(arg)) if arg == "--version" => finished(0, &format!("{COMPILER_VERSION}\n"), ""),
        _ => finished(0, "", ""),
    }
}

fn archives() -> Vec<String> {
    STATIC_LIBRARIES
        .iter()
        .map(|library| format!("lib{library}.a"))
        .collect()
}

fn built_header() -> PathBuf {
    out_dir()
        .join("build")
        .join("slang-rhi")
        .join("include")
        .join("slang-rhi-config.h")
}

fn host_for(triple: &str, os: &str, env: &str) -> FakeHost {
    let built = archives()
        .into_iter()
        .map(|name| (out_dir().join("lib").join(name), Ok(EntryKind::File)))
        .chain([(built_header(), Ok(EntryKind::File))])
        .collect();
    FakeHost {
        vars: vec![
            (SRC_ROOT, source_root().into_os_string()),
            (SRC_COMMIT, OsString::from(COMMIT)),
            (SRC_BUILD_TAG, OsString::from(BUILD_TAG)),
            (SRC_SHARED_DIR, shared_dir().into_os_string()),
            (
                SLANG_INCLUDE,
                slang_entry().join("include").into_os_string(),
            ),
            (SLANG_LIB, slang_entry().join("lib").into_os_string()),
            (SLANG_LINKAGE, OsString::from("static")),
            (SLANG_BUILD_TAG, OsString::from(BUILD_TAG)),
            (OUT_DIR, out_dir().into_os_string()),
            (MANIFEST_DIR, manifest_dir().into_os_string()),
            (TARGET, OsString::from(triple)),
            (HOST, OsString::from(triple)),
            (TARGET_OS, OsString::from(os)),
            (TARGET_ENV, OsString::from(env)),
        ],
        files: Vec::new(),
        entries: built,
        listings: Vec::new(),
        respond: Box::new(|invocation| Ok(happy(invocation))),
        failures: Vec::new(),
        read_vars: RefCell::default(),
        runs: RefCell::default(),
        copied: RefCell::default(),
        written: RefCell::default(),
        modified: RefCell::default(),
        events: RefCell::default(),
    }
}

fn linux_host() -> FakeHost {
    host_for(LINUX, "linux", "gnu")
}

fn set_var(host: &mut FakeHost, name: &'static str, value: Option<&str>) {
    host.vars.retain(|(key, _)| *key != name);
    host.vars
        .extend(value.map(|value| (name, OsString::from(value))));
}

fn run(host: &FakeHost) -> (Vec<String>, Result<(), String>) {
    let mut directives = Vec::new();
    let result = configure(host, &mut directives).map_err(|error| format!("{error:?}"));
    (directives, result)
}

fn failure(host: &FakeHost, parts: &[&str]) -> Vec<String> {
    let (directives, result) = run(host);
    let message = result.err().unwrap_or_default();
    for part in parts {
        assert!(message.contains(part), "{message:?} lacks {part:?}");
    }
    assert!(
        directives
            .iter()
            .all(|directive| !directive.starts_with("cargo::rustc-link")),
        "{directives:?}"
    );
    assert!(
        host.events
            .borrow()
            .iter()
            .all(|event| !event.starts_with("Renamed"))
    );
    directives
}

const KEY_OPTIONS: [&str; 3] = [
    "-DCMAKE_BUILD_TYPE=Release",
    "-DFETCHCONTENT_FULLY_DISCONNECTED=ON",
    "-DFETCHCONTENT_SOURCE_DIR_VULKAN_HEADERS=external/vulkan",
];

fn expected_inputs(triple: &str, variables: &[(&str, &str)], compiler: &str) -> String {
    let mut lines = vec![
        format!("crate={CRATE_NAME} {CRATE_VERSION}"),
        format!("files={}", native_files_hash()),
        format!("commit={COMMIT}"),
        format!("build_tag={BUILD_TAG}"),
        format!("target={triple}"),
        format!("cmake={CMAKE_VERSION}"),
    ];
    lines.extend(
        KEY_OPTIONS
            .iter()
            .map(|option| format!("configure={option}")),
    );
    for name in CMAKE_VARIABLES.into_iter().chain([MSVC_TOOLS_VERSION]) {
        let value = variables
            .iter()
            .find(|(variable, _)| *variable == name)
            .map_or("", |(_, value)| value);
        lines.push(format!("{name}={value}"));
    }
    lines.push(format!("compiler={compiler}"));
    lines.join("\n") + "\n"
}

fn linux_inputs() -> String {
    expected_inputs(LINUX, &[], COMPILER_VERSION)
}

fn entry_for(inputs: &str) -> PathBuf {
    entries_dir().join(fnv1a(inputs.as_bytes()))
}

fn reruns() -> Vec<String> {
    CMAKE_VARIABLES
        .into_iter()
        .chain(MSVC_VARIABLES)
        .map(|name| format!("cargo::rerun-if-env-changed={name}"))
        .collect()
}

fn links(entry: &Path) -> Vec<String> {
    let mut links = vec![format!(
        "cargo::rustc-link-search=native={}",
        entry.join("lib").display()
    )];
    links.extend(
        STATIC_LIBRARIES
            .iter()
            .map(|library| format!("cargo::rustc-link-lib=static={library}")),
    );
    links.extend(
        LINUX_LINKS
            .iter()
            .map(|library| format!("cargo::rustc-link-lib=dylib={library}")),
    );
    links
}

fn expected_directives(entry: &Path) -> Vec<String> {
    let mut directives = reruns();
    directives.push(format!(
        "cargo::rerun-if-changed={}",
        entry.join("stamp").display()
    ));
    directives.extend(links(entry));
    directives
}

fn cmake_runs(host: &FakeHost) -> Vec<(Invocation, Option<PathBuf>)> {
    host.runs
        .borrow()
        .iter()
        .filter(|(invocation, _)| invocation.program == "cmake")
        .cloned()
        .collect()
}

fn configure_args() -> Vec<String> {
    let lib = out_dir().join("lib");
    let mut args = vec![
        "-S".to_owned(),
        manifest_dir().join("native").display().to_string(),
        "-B".to_owned(),
        out_dir().join("build").display().to_string(),
        "-G".to_owned(),
        "Ninja".to_owned(),
    ];
    args.extend(CONFIGURE_OPTIONS.iter().map(|option| (*option).to_owned()));
    args.push(format!(
        "-DFETCHCONTENT_SOURCE_DIR_VULKAN_HEADERS={}",
        source_root().join("external").join("vulkan").display()
    ));
    args.push(format!("-DSLANG_TREE_DIR={}", source_root().display()));
    args.push(format!(
        "-DSLANG_INCLUDE_DIR={}",
        slang_entry().join("include").display()
    ));
    args.push(format!(
        "-DCMAKE_ARCHIVE_OUTPUT_DIRECTORY={}",
        lib.display()
    ));
    args
}

fn entry_files_of(entry: &Path) -> Vec<PathBuf> {
    archives()
        .into_iter()
        .map(|name| entry.join("lib").join(name))
        .chain([entry.join("include").join("slang-rhi-config.h")])
        .collect()
}

#[test]
fn fnv1a_gives_the_published_test_values() {
    assert_eq!(fnv1a(b""), "cbf29ce484222325");
    assert_eq!(fnv1a(b"a"), "af63dc4c8601ec8c");
    assert_eq!(fnv1a(b"foobar"), "85944171f73967e8");
}

#[test]
fn the_files_input_hashes_the_wrapper_project_and_its_name() {
    let name = "CMakeLists.txt";
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("native")
        .join(name);
    let text = std::fs::read_to_string(&path).expect("the wrapper project is readable");
    let mut bytes = Vec::new();
    bytes.extend_from_slice(name.as_bytes());
    bytes.push(0);
    bytes.extend_from_slice(text.as_bytes());
    bytes.push(0);
    assert_eq!(native_files_hash(), fnv1a(&bytes));
}

#[test]
fn configure_builds_stores_and_links_the_three_libraries_on_linux() {
    let host = linux_host();
    let (directives, result) = run(&host);
    result.expect("the build succeeds");
    let inputs = linux_inputs();
    let entry = entry_for(&inputs);
    assert_eq!(directives, expected_directives(&entry));
    let key = fnv1a(inputs.as_bytes());
    let temporary = entries_dir().join(format!("{key}.tmp-{PROCESS_ID}"));
    assert_eq!(
        host.written.borrow().as_slice(),
        [
            (out_dir().join("build.log"), String::new()),
            (temporary.join("inputs"), inputs.clone()),
            (temporary.join("stamp"), String::new()),
        ]
    );
    assert_eq!(
        host.modified.borrow().as_slice(),
        [(temporary.join("stamp"), SystemTime::UNIX_EPOCH + STAMP_TIME)]
    );
    let mut sources: Vec<PathBuf> = archives()
        .into_iter()
        .map(|name| out_dir().join("lib").join(name))
        .collect();
    sources.push(built_header());
    let copies: Vec<(PathBuf, PathBuf)> = sources
        .into_iter()
        .zip(entry_files_of(&temporary))
        .collect();
    assert_eq!(host.copied.borrow().as_slice(), copies);
    let events = host.events.borrow();
    assert_eq!(
        events.iter().find(|event| !event.starts_with("Run ")),
        Some(&format!(
            "Lock {}",
            entries_dir().join(format!("{key}.lock")).display()
        ))
    );
    assert_eq!(
        events.last(),
        Some(&format!("Renamed {}", temporary.display()))
    );
    assert!(
        host.read_vars
            .borrow()
            .iter()
            .all(|name| name != "VULKAN_SDK"),
        "{:?}",
        host.read_vars.borrow()
    );
}

#[test]
fn configure_links_slang_rhi_before_the_libraries_that_it_uses() {
    let (directives, result) = run(&linux_host());
    result.expect("the build succeeds");
    let links: Vec<&str> = directives
        .iter()
        .filter_map(|directive| directive.strip_prefix("cargo::rustc-link-lib="))
        .collect();
    assert_eq!(
        links,
        [
            "static=slang-rhi",
            "static=slang-rhi-vma",
            "static=slang-rhi-resources",
            "dylib=stdc++",
            "dylib=dl",
            "dylib=pthread",
        ]
    );
    assert!(
        directives
            .iter()
            .all(|directive| !directive.starts_with("cargo::metadata")),
        "{directives:?}"
    );
}

#[test]
fn configure_runs_ninja_the_compiler_and_cmake_with_the_tree_and_the_slang_headers() {
    let mut host = linux_host();
    set_var(&mut host, NUM_JOBS, Some("12"));
    let (_, result) = run(&host);
    result.expect("the build succeeds");
    let runs = host.runs.borrow();
    let commands: Vec<String> = runs
        .iter()
        .map(|(invocation, _)| invocation.to_string())
        .collect();
    let build_dir = out_dir().join("build").display().to_string();
    assert_eq!(
        commands,
        [
            "ninja --version".to_owned(),
            "c++ --version".to_owned(),
            "cmake --version".to_owned(),
            format!("cmake {}", configure_args().join(" ")),
            format!("cmake --build {build_dir} --target slang-rhi --parallel 12"),
        ]
    );
    let log = out_dir().join("build.log");
    for (invocation, logged) in cmake_runs(&host) {
        let to_log = invocation
            .args
            .first()
            .is_some_and(|arg| arg != "--version");
        assert_eq!(
            logged.as_deref(),
            to_log.then_some(log.as_path()),
            "{invocation}"
        );
    }
    assert!(host.events.borrow().contains(&format!(
        "Remove {}",
        out_dir().join("build").join("CMakeCache.txt").display()
    )));
}

#[test]
fn configure_builds_without_parallel_jobs_when_num_jobs_is_not_a_number() {
    for value in [None, Some(""), Some("many")] {
        let mut host = linux_host();
        set_var(&mut host, NUM_JOBS, value);
        run(&host).1.expect("the build succeeds");
        let runs = cmake_runs(&host);
        assert_eq!(
            runs.last().map(|(invocation, _)| invocation.args.clone()),
            Some(vec![
                "--build".to_owned(),
                out_dir().join("build").display().to_string(),
                "--target".to_owned(),
                "slang-rhi".to_owned(),
            ]),
            "{value:?}"
        );
    }
}

#[test]
fn configure_on_another_architecture_of_linux_with_glibc_builds() {
    let triple = "aarch64-unknown-linux-gnu";
    let host = host_for(triple, "linux", "gnu");
    let (directives, result) = run(&host);
    result.expect("the build succeeds");
    let entry = entry_for(&expected_inputs(triple, &[], COMPILER_VERSION));
    assert_eq!(directives, expected_directives(&entry));
}

#[test]
fn configure_refuses_every_target_that_is_not_linux_with_glibc() {
    for (triple, os, env) in [
        ("x86_64-pc-windows-msvc", "windows", "msvc"),
        ("x86_64-pc-windows-gnu", "windows", "gnu"),
        ("aarch64-apple-darwin", "macos", ""),
        ("x86_64-unknown-linux-musl", "linux", "musl"),
        ("x86_64-unknown-freebsd", "freebsd", ""),
    ] {
        let host = host_for(triple, os, env);
        failure(
            &host,
            &[
                &format!("the target {triple} is not Linux with the GNU C library"),
                "slang-rhi-sys builds only for Linux with the GNU C library",
                "Direct3D 12 backend of Windows and the Metal backend of macOS are not designed",
            ],
        );
        assert!(host.runs.borrow().is_empty(), "{triple}");
    }
}

#[test]
fn configure_refuses_another_target_before_it_reads_the_metadata_of_its_dependencies() {
    let mut host = host_for("aarch64-apple-darwin", "macos", "");
    for name in [
        SRC_ROOT,
        SRC_COMMIT,
        SRC_BUILD_TAG,
        SRC_SHARED_DIR,
        SLANG_INCLUDE,
        SLANG_LIB,
        SLANG_LINKAGE,
        SLANG_BUILD_TAG,
    ] {
        set_var(&mut host, name, None);
    }
    failure(
        &host,
        &["the target aarch64-apple-darwin is not Linux with the GNU C library"],
    );
    let read = host.read_vars.borrow();
    assert!(
        !read.iter().any(|name| name.starts_with("DEP_")),
        "{read:?}"
    );
    assert!(host.runs.borrow().is_empty());
}

#[test]
fn configure_refuses_cross_compilation_and_names_both_triples() {
    let mut host = linux_host();
    set_var(&mut host, HOST, Some("aarch64-unknown-linux-gnu"));
    failure(
        &host,
        &[
            "the target x86_64-unknown-linux-gnu differs from the host aarch64-unknown-linux-gnu",
            "cannot cross-compile",
            "Build on a host of the target",
        ],
    );
    assert!(host.runs.borrow().is_empty());
}

#[test]
fn configure_without_the_metadata_of_its_dependencies_says_to_depend_on_them_directly() {
    let advice = "the crate has to depend on slang-src and on slang-compiler-sys with the feature `vendored` directly";
    for name in [
        SRC_ROOT,
        SRC_COMMIT,
        SRC_BUILD_TAG,
        SRC_SHARED_DIR,
        SLANG_INCLUDE,
        SLANG_LIB,
        SLANG_LINKAGE,
        SLANG_BUILD_TAG,
    ] {
        let mut host = linux_host();
        set_var(&mut host, name, None);
        failure(&host, &[&format!("{name} is not set"), advice]);
        assert!(host.runs.borrow().is_empty(), "{name}");
    }
    let mut host = linux_host();
    set_var(&mut host, SLANG_INCLUDE, Some("relative/include"));
    failure(
        &host,
        &[
            "DEP_SLANG_COMPILER_INCLUDE is \"relative/include\", which is not an absolute path in UTF-8 without line breaks",
        ],
    );
    let mut host = linux_host();
    set_var(&mut host, SRC_COMMIT, Some("two\nlines"));
    failure(
        &host,
        &["DEP_SLANG_SRC_COMMIT is \"two\\nlines\", which is not one line of UTF-8 text"],
    );
}

#[test]
fn configure_refuses_a_slang_that_is_not_linked_statically() {
    let mut host = linux_host();
    set_var(&mut host, SLANG_LINKAGE, Some("dynamic"));
    failure(
        &host,
        &[
            "DEP_SLANG_COMPILER_LINKAGE is \"dynamic\", not \"static\"",
            "slang-compiler-sys with the feature `vendored`",
        ],
    );
    assert!(host.runs.borrow().is_empty());
}

#[test]
fn configure_without_a_path_or_a_triple_from_cargo_says_that_cargo_sets_it() {
    for (name, purpose) in [
        (OUT_DIR, "build slang-rhi"),
        (MANIFEST_DIR, "find the wrapper project"),
    ] {
        let mut host = linux_host();
        set_var(&mut host, name, None);
        failure(
            &host,
            &[
                &format!("{name} is not set"),
                purpose,
                &format!("Cargo sets {name}"),
            ],
        );
        let mut host = linux_host();
        set_var(&mut host, name, Some("relative"));
        failure(
            &host,
            &[&format!(
                "{name} is \"relative\", which is not an absolute path"
            )],
        );
    }
    for name in [TARGET, HOST] {
        let mut host = linux_host();
        set_var(&mut host, name, None);
        failure(
            &host,
            &[&format!("{name} is not set"), &format!("Cargo sets {name}")],
        );
    }
}

#[test]
fn configure_names_the_program_that_cannot_start_and_every_build_requirement() {
    for program in ["ninja", "c++", "cmake"] {
        let mut host = linux_host();
        host.respond = Box::new(move |invocation| {
            if invocation.program == program {
                Err(io::Error::from(io::ErrorKind::NotFound))
            } else {
                Ok(happy(invocation))
            }
        });
        failure(
            &host,
            &[
                &format!("cannot start {program} for `{program} --version`"),
                "CMake 3.22 or later, Ninja, a C++20 compiler, and the development headers of Xlib",
            ],
        );
    }
}

#[test]
fn configure_reports_a_probe_that_fails_with_its_status_and_error_output() {
    let mut host = linux_host();
    host.respond = Box::new(|invocation| {
        if invocation.program == "ninja" {
            Ok(finished(3, "", "broken ninja\n"))
        } else {
            Ok(happy(invocation))
        }
    });
    failure(
        &host,
        &["`ninja --version` failed with exit status 3: broken ninja"],
    );
}

#[test]
fn configure_probes_the_compiler_that_cxx_names_as_cmake_reads_it() {
    let cases = [
        ("", "c++ --version"),
        ("   ", "c++ --version"),
        ("ccache g++", "ccache g++ --version"),
        ("clang++", "clang++ --version"),
    ];
    for (value, probe) in cases {
        let mut host = linux_host();
        set_var(&mut host, CXX, Some(value));
        run(&host).1.expect("the build succeeds");
        assert_eq!(host.runs.borrow()[1].0.to_string(), probe);
    }
    let file = root().join("bin").join("my compiler");
    let mut host = linux_host();
    set_var(&mut host, CXX, Some(&file.display().to_string()));
    host.entries.push((file.clone(), Ok(EntryKind::File)));
    run(&host).1.expect("the build succeeds");
    assert_eq!(host.runs.borrow()[1].0.program, file.display().to_string());
}

#[test]
fn configure_keys_the_entry_by_every_variable_that_cmake_reads() {
    let mut host = linux_host();
    set_var(&mut host, "CXXFLAGS", Some("-O1"));
    set_var(&mut host, "VCToolsVersion", Some("14.51.36231"));
    let (directives, result) = run(&host);
    result.expect("the build succeeds");
    let inputs = expected_inputs(
        LINUX,
        &[("CXXFLAGS", "-O1"), ("VCToolsVersion", "14.51.36231")],
        COMPILER_VERSION,
    );
    assert_ne!(inputs, linux_inputs());
    assert_eq!(directives, expected_directives(&entry_for(&inputs)));
}

#[test]
fn configure_keys_the_entry_by_no_path_of_the_build() {
    let mut host = linux_host();
    set_var(&mut host, OUT_DIR, Some("/elsewhere/out"));
    set_var(&mut host, SRC_ROOT, Some("/elsewhere/tree"));
    set_var(&mut host, SLANG_INCLUDE, Some("/elsewhere/include"));
    host.entries = archives()
        .into_iter()
        .map(|name| {
            (
                PathBuf::from("/elsewhere/out/lib").join(name),
                Ok(EntryKind::File),
            )
        })
        .chain([(
            PathBuf::from("/elsewhere/out/build/slang-rhi/include/slang-rhi-config.h"),
            Ok(EntryKind::File),
        )])
        .collect();
    let (directives, result) = run(&host);
    result.expect("the build succeeds");
    assert_eq!(directives, expected_directives(&entry_for(&linux_inputs())));
}

fn log_lines(count: usize) -> String {
    let lines: Vec<String> = (1..=count).map(|line| format!("line {line}")).collect();
    lines.join("\n") + "\n"
}

#[test]
fn configure_reports_a_failed_configure_or_build_step_with_the_tail_of_its_log() {
    for (step, failing) in [("configure", "-S"), ("build", "--build")] {
        let mut host = linux_host();
        host.files
            .push((out_dir().join("build.log"), log_lines(50)));
        host.respond = Box::new(move |invocation| {
            if invocation.args.first().is_some_and(|arg| arg == failing) {
                Ok(finished(1, "", ""))
            } else {
                Ok(happy(invocation))
            }
        });
        failure(
            &host,
            &[
                "failed with exit status 1",
                &format!("so the {step} step of the build of slang-rhi failed"),
                &out_dir().join("build.log").display().to_string(),
                "ends with these lines:\nline 11\n",
                "line 50",
            ],
        );
        let (_, result) = run(&host);
        assert!(!result.expect_err("fails").contains("line 10\n"));
        assert!(host.copied.borrow().is_empty());
    }
}

#[test]
fn configure_reports_a_signal_and_a_log_that_cannot_be_opened_or_read() {
    let mut host = linux_host();
    host.respond = Box::new(|invocation| {
        if invocation.args.first().is_some_and(|arg| arg == "--build") {
            Ok(Finished {
                exit: Exit { code: None },
                stdout: String::new(),
                stderr: String::new(),
            })
        } else {
            Ok(happy(invocation))
        }
    });
    failure(
        &host,
        &[
            "no exit status, because a signal ended it",
            "the log cannot be read",
        ],
    );
    let mut host = linux_host();
    host.failures.push((
        FileOp::OpenLog,
        out_dir().join("build.log"),
        io::ErrorKind::PermissionDenied,
    ));
    failure(
        &host,
        &[
            "cannot open the log",
            "keeps its build tree, its libraries, and its log under OUT_DIR",
        ],
    );
    let mut host = linux_host();
    host.failures.push((
        FileOp::Write,
        out_dir().join("build.log"),
        io::ErrorKind::PermissionDenied,
    ));
    failure(&host, &["cannot write", "build.log"]);
    let mut host = linux_host();
    let cache = out_dir().join("build").join("CMakeCache.txt");
    host.failures.push((
        FileOp::Remove,
        cache.clone(),
        io::ErrorKind::PermissionDenied,
    ));
    failure(&host, &["cannot remove", &cache.display().to_string()]);
}

#[test]
fn configure_reports_a_library_or_the_header_that_the_build_did_not_produce() {
    let missing_files = [
        out_dir().join("lib").join("libslang-rhi-vma.a"),
        built_header(),
    ];
    for missing in missing_files {
        for (kind, problem) in [
            (Err(io::ErrorKind::NotFound), "cannot access"),
            (Ok(EntryKind::Directory), "is not a file"),
        ] {
            let mut host = linux_host();
            host.entries.retain(|(path, _)| *path != missing);
            host.entries.push((missing.clone(), kind));
            failure(
                &host,
                &[
                    problem,
                    &missing.display().to_string(),
                    "another layout than the one that this version of slang-rhi-sys expects",
                ],
            );
            assert!(host.copied.borrow().is_empty());
        }
    }
}

fn complete_entry(host: &mut FakeHost, inputs: &str) -> PathBuf {
    let entry = entry_for(inputs);
    host.entries.push((entry.clone(), Ok(EntryKind::Directory)));
    host.files.push((entry.join("inputs"), inputs.to_owned()));
    for file in entry_files_of(&entry) {
        host.entries.push((file, Ok(EntryKind::File)));
    }
    entry
}

#[test]
fn configure_uses_a_complete_entry_without_building() {
    let mut host = linux_host();
    let entry = complete_entry(&mut host, &linux_inputs());
    let (directives, result) = run(&host);
    result.expect("the build succeeds");
    assert_eq!(directives, expected_directives(&entry));
    assert!(cmake_runs(&host).iter().all(|(_, log)| log.is_none()));
    assert!(host.written.borrow().is_empty());
    assert!(host.copied.borrow().is_empty());
}

#[test]
fn configure_replaces_an_entry_whose_inputs_libraries_or_header_differ() {
    let inputs = linux_inputs();
    for case in ["inputs", "library", "header"] {
        let mut host = linux_host();
        let entry = complete_entry(&mut host, &inputs);
        match case {
            "inputs" => {
                host.files.retain(|(path, _)| *path != entry.join("inputs"));
                host.files
                    .push((entry.join("inputs"), "crate=other\n".to_owned()));
            }
            "library" => {
                let library = entry.join("lib").join("libslang-rhi-resources.a");
                host.entries.retain(|(path, _)| *path != library);
            }
            _ => {
                let header = entry.join("include").join("slang-rhi-config.h");
                host.entries.retain(|(path, _)| *path != header);
            }
        }
        run(&host).1.expect("the build succeeds");
        let events = host.events.borrow();
        let removed = events
            .iter()
            .position(|event| *event == format!("RemoveDir {}", entry.display()))
            .unwrap_or_else(|| panic!("{case}: the old entry is not removed: {events:?}"));
        let renamed = events
            .iter()
            .position(|event| event.starts_with("Renamed"))
            .expect("the new entry is renamed into place");
        assert_eq!(renamed, removed + 2, "{case}: {events:?}");
    }
}

#[test]
fn configure_removes_only_the_numbered_temporary_directories_of_its_entry() {
    let mut host = linux_host();
    let key = fnv1a(linux_inputs().as_bytes());
    host.listings.push((
        entries_dir(),
        [
            format!("{key}.tmp-17"),
            format!("{key}.tmp-"),
            format!("{key}.tmp-1x"),
            format!("{key}.lock"),
            "0123456789abcdef.tmp-17".to_owned(),
            key.clone(),
        ]
        .into_iter()
        .map(OsString::from)
        .collect(),
    ));
    run(&host).1.expect("the build succeeds");
    let removed: Vec<String> = host
        .events
        .borrow()
        .iter()
        .filter(|event| event.starts_with("RemoveDir"))
        .cloned()
        .collect();
    assert_eq!(
        removed,
        [format!(
            "RemoveDir {}",
            entries_dir().join(format!("{key}.tmp-17")).display()
        )]
    );
}

#[test]
fn configure_reports_each_operation_on_the_shared_folder_that_fails() {
    let key = fnv1a(linux_inputs().as_bytes());
    let entry = entries_dir().join(&key);
    let temporary = entries_dir().join(format!("{key}.tmp-{PROCESS_ID}"));
    let cases = [
        (
            FileOp::Lock,
            entries_dir().join(format!("{key}.lock")),
            "cannot lock",
        ),
        (FileOp::ListDir, entries_dir(), "cannot list"),
        (
            FileOp::Copy,
            temporary.join("lib").join("libslang-rhi.a"),
            "cannot copy",
        ),
        (
            FileOp::Copy,
            temporary.join("include").join("slang-rhi-config.h"),
            "cannot copy",
        ),
        (FileOp::Write, temporary.join("inputs"), "cannot write"),
        (FileOp::Write, temporary.join("stamp"), "cannot write"),
        (
            FileOp::SetModified,
            temporary.join("stamp"),
            "cannot set the modification time of",
        ),
        (FileOp::Rename, entry.clone(), "cannot rename"),
    ];
    for (op, path, action) in cases {
        let mut host = linux_host();
        host.failures
            .push((op, path.clone(), io::ErrorKind::PermissionDenied));
        failure(
            &host,
            &[
                action,
                &path.display().to_string(),
                "the shared folder of the Cargo target directory",
                "removing the entry, or running cargo clean, when no build runs is safe",
            ],
        );
    }
    let mut host = linux_host();
    host.entries
        .push((entry.clone(), Err(io::ErrorKind::PermissionDenied)));
    failure(&host, &["cannot inspect", &entry.display().to_string()]);
    let mut host = linux_host();
    let entry = complete_entry(&mut host, "crate=other\n");
    let inputs = linux_inputs();
    let actual = entry_for(&inputs);
    assert_ne!(entry, actual);
    host.entries
        .push((actual.clone(), Ok(EntryKind::Directory)));
    host.failures.push((
        FileOp::RemoveDir,
        actual.clone(),
        io::ErrorKind::PermissionDenied,
    ));
    failure(&host, &["cannot remove", &actual.display().to_string()]);
}
