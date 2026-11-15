use std::cell::RefCell;

include!("../../build_logic.rs");

mod ffi;

type Responder = Box<dyn Fn(&Invocation) -> io::Result<Finished>>;

const BUILD_TAG: &str = "2026.17-25-gc7954ebd1";
const COMMIT: &str = "c7954ebd1f1984ad2bfde15f76248db41dc34aa3";
const LINUX: &str = "x86_64-unknown-linux-gnu";
const MACOS: &str = "aarch64-apple-darwin";
const WINDOWS: &str = "x86_64-pc-windows-msvc";
const PROCESS_ID: u32 = 4242;
const CMAKE_VERSION: &str = "cmake version 3.31.6";
const COMPILER_VERSION: &str = "c++ (GCC) 15.2.1 20260101";
const LINUX_LINKS: &[&str] = &["stdc++", "pthread"];
const MACOS_LINKS: &[&str] = &["c++"];
const WINDOWS_LINKS: &[&str] = &[
    "kernel32", "user32", "gdi32", "winspool", "shell32", "ole32", "oleaut32", "uuid", "comdlg32",
    "advapi32",
];

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
    msvc: Option<MsvcCompiler>,
    read_vars: RefCell<Vec<String>>,
    runs: RefCell<Vec<(Invocation, Option<PathBuf>)>>,
    copied: RefCell<Vec<(PathBuf, PathBuf)>>,
    written: RefCell<Vec<(PathBuf, String)>>,
    modified: RefCell<Vec<(PathBuf, SystemTime)>>,
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

    fn msvc_compiler(&self, target: &str) -> Option<MsvcCompiler> {
        self.msvc_lookups.borrow_mut().push(target.to_owned());
        self.msvc.clone()
    }
}

fn root() -> PathBuf {
    std::env::temp_dir().join("glslang-fake")
}

fn source_root() -> PathBuf {
    root().join("shared").join("src").join(COMMIT).join("tree")
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

fn archives(triple: &str) -> Vec<String> {
    STATIC_LIBRARIES
        .iter()
        .map(|library| {
            if triple == WINDOWS {
                format!("{library}.lib")
            } else {
                format!("lib{library}.a")
            }
        })
        .collect()
}

fn host_for(triple: &str) -> FakeHost {
    let (os, env) = match triple {
        WINDOWS => ("windows", "msvc"),
        MACOS => ("macos", ""),
        _ => ("linux", "gnu"),
    };
    let built = archives(triple)
        .into_iter()
        .map(|name| (out_dir().join("lib").join(name), Ok(EntryKind::File)))
        .collect();
    FakeHost {
        vars: vec![
            (SRC_ROOT, source_root().into_os_string()),
            (SRC_COMMIT, OsString::from(COMMIT)),
            (SRC_BUILD_TAG, OsString::from(BUILD_TAG)),
            (SRC_SHARED_DIR, shared_dir().into_os_string()),
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
        msvc: None,
        read_vars: RefCell::default(),
        runs: RefCell::default(),
        copied: RefCell::default(),
        written: RefCell::default(),
        modified: RefCell::default(),
        msvc_lookups: RefCell::default(),
        events: RefCell::default(),
    }
}

fn linux_host() -> FakeHost {
    host_for(LINUX)
}

fn msvc_compiler() -> MsvcCompiler {
    MsvcCompiler {
        path: root().join("VC").join("bin").join("cl.exe"),
        env: vec![(OsString::from("INCLUDE"), OsString::from("msvc-include"))],
    }
}

fn windows_host() -> FakeHost {
    FakeHost {
        msvc: Some(msvc_compiler()),
        ..host_for(WINDOWS)
    }
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

fn expected_inputs(
    triple: &str,
    configure: &[&str],
    variables: &[(&str, &str)],
    compiler: &str,
) -> String {
    let mut lines = vec![
        format!("crate={CRATE_NAME} {CRATE_VERSION}"),
        format!("files={}", native_files_hash()),
        format!("commit={COMMIT}"),
        format!("build_tag={BUILD_TAG}"),
        format!("target={triple}"),
        format!("cmake={CMAKE_VERSION}"),
    ];
    lines.extend(configure.iter().map(|option| format!("configure={option}")));
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
    expected_inputs(LINUX, &CONFIGURE_OPTIONS, &[], COMPILER_VERSION)
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

fn links(entry: &Path, dynamic: &[&str]) -> Vec<String> {
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
        dynamic
            .iter()
            .map(|library| format!("cargo::rustc-link-lib=dylib={library}")),
    );
    links
}

fn expected_directives(entry: &Path, dynamic: &[&str]) -> Vec<String> {
    let mut directives = reruns();
    directives.push(format!(
        "cargo::rerun-if-changed={}",
        entry.join("stamp").display()
    ));
    directives.extend(links(entry, dynamic));
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

fn python_setting() -> (OsString, OsString) {
    (
        OsString::from("PYTHONDONTWRITEBYTECODE"),
        OsString::from("1"),
    )
}

fn configure_args(options: &[&str]) -> Vec<String> {
    let lib = out_dir().join("lib");
    let mut args = vec![
        "-S".to_owned(),
        manifest_dir().join("native").display().to_string(),
        "-B".to_owned(),
        out_dir().join("build").display().to_string(),
        "-G".to_owned(),
        "Ninja".to_owned(),
    ];
    args.extend(options.iter().map(|option| (*option).to_owned()));
    args.push(format!("-DSLANG_TREE_DIR={}", source_root().display()));
    args.push(format!(
        "-DCMAKE_ARCHIVE_OUTPUT_DIRECTORY={}",
        lib.display()
    ));
    args
}

#[test]
fn fnv1a_gives_the_published_test_values() {
    assert_eq!(fnv1a(b""), "cbf29ce484222325");
    assert_eq!(fnv1a(b"a"), "af63dc4c8601ec8c");
    assert_eq!(fnv1a(b"foobar"), "85944171f73967e8");
}

#[test]
fn the_files_input_hashes_both_native_files_and_their_names() {
    let mut bytes = Vec::new();
    for name in ["CMakeLists.txt", "loader.cpp"] {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("native")
            .join(name);
        let text = std::fs::read_to_string(&path).expect("the native file is readable");
        bytes.extend_from_slice(name.as_bytes());
        bytes.push(0);
        bytes.extend_from_slice(text.as_bytes());
        bytes.push(0);
    }
    assert_eq!(native_files_hash(), fnv1a(&bytes));
}

#[test]
fn configure_builds_stores_and_links_the_five_libraries_on_linux() {
    let host = linux_host();
    let (directives, result) = run(&host);
    result.expect("the build succeeds");
    let inputs = linux_inputs();
    let entry = entry_for(&inputs);
    assert_eq!(directives, expected_directives(&entry, LINUX_LINKS));
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
    let copies: Vec<(PathBuf, PathBuf)> = archives(LINUX)
        .into_iter()
        .map(|name| {
            (
                out_dir().join("lib").join(&name),
                temporary.join("lib").join(&name),
            )
        })
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
fn configure_runs_ninja_the_compiler_and_cmake_with_python_bytecode_off() {
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
            format!("cmake {}", configure_args(&CONFIGURE_OPTIONS).join(" ")),
            format!("cmake --build {build_dir} --target slang-glslang-static --parallel 12"),
        ]
    );
    let log = out_dir().join("build.log");
    for (invocation, logged) in cmake_runs(&host) {
        assert_eq!(invocation.env, [python_setting()], "{invocation}");
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
fn configure_on_macos_links_libc_plus_plus() {
    let host = host_for(MACOS);
    let (directives, result) = run(&host);
    result.expect("the build succeeds");
    let entry = entry_for(&expected_inputs(
        MACOS,
        &CONFIGURE_OPTIONS,
        &[],
        COMPILER_VERSION,
    ));
    assert_eq!(directives, expected_directives(&entry, MACOS_LINKS));
}

#[test]
fn configure_with_the_microsoft_toolchain_uses_the_found_compiler_and_its_runtime() {
    for (features, runtime) in [
        ("", "MultiThreadedDLL"),
        ("crt-static,sse2", "MultiThreaded"),
    ] {
        let mut host = windows_host();
        set_var(&mut host, TARGET_FEATURE, Some(features));
        let (directives, result) = run(&host);
        result.expect("the build succeeds");
        let option = format!("-DCMAKE_MSVC_RUNTIME_LIBRARY={runtime}");
        let mut options: Vec<&str> = CONFIGURE_OPTIONS.to_vec();
        options.push(&option);
        let compiler = msvc_compiler().path.display().to_string();
        let entry = entry_for(&expected_inputs(WINDOWS, &options, &[], &compiler));
        assert_eq!(directives, expected_directives(&entry, WINDOWS_LINKS));
        assert_eq!(host.msvc_lookups.borrow().as_slice(), [WINDOWS]);
        let runs = cmake_runs(&host);
        assert_eq!(runs.len(), 3);
        for (invocation, _) in &runs {
            assert_eq!(
                invocation.env,
                [msvc_compiler().env[0].clone(), python_setting()]
            );
        }
        assert_eq!(runs[1].0.args, configure_args(&options));
        assert!(
            host.runs
                .borrow()
                .iter()
                .all(|(invocation, _)| invocation.program != "c++")
        );
        let copied: Vec<PathBuf> = host
            .copied
            .borrow()
            .iter()
            .map(|(from, _)| from.clone())
            .collect();
        let expected: Vec<PathBuf> = archives(WINDOWS)
            .into_iter()
            .map(|name| out_dir().join("lib").join(name))
            .collect();
        assert_eq!(copied, expected);
    }
}

#[test]
fn configure_without_a_microsoft_compiler_says_where_it_looked() {
    let mut host = host_for(WINDOWS);
    set_var(&mut host, "VCToolsVersion", Some("14.51.36231"));
    failure(
        &host,
        &[
            "cannot find the Microsoft C++ compiler cl.exe for the target x86_64-pc-windows-msvc",
            "VCINSTALLDIR marks and which is not set",
            "VCToolsVersion names, which is \"14.51.36231\"",
            "Install Visual Studio or its Build Tools",
            "without the Cargo feature `vendored`",
        ],
    );
}

#[test]
fn configure_on_an_untested_target_with_a_link_rule_builds_without_a_warning() {
    let triple = "aarch64-unknown-linux-gnu";
    let host = host_for(triple);
    let (directives, result) = run(&host);
    result.expect("the build succeeds");
    assert!(
        directives
            .iter()
            .all(|directive| !directive.starts_with("cargo::warning"))
    );
    let entry = entry_for(&expected_inputs(
        triple,
        &CONFIGURE_OPTIONS,
        &[],
        COMPILER_VERSION,
    ));
    assert_eq!(directives, expected_directives(&entry, LINUX_LINKS));
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
fn configure_refuses_a_target_without_a_link_rule_for_its_cxx_runtime() {
    let triple = "x86_64-unknown-linux-musl";
    let mut host = host_for(triple);
    set_var(&mut host, TARGET_ENV, Some("musl"));
    failure(
        &host,
        &[
            "the target x86_64-unknown-linux-musl has no link rule for its C++ runtime library",
            "Linux with the GNU C library, macOS, and Windows with the Microsoft toolchain",
        ],
    );
    assert!(host.runs.borrow().is_empty());
}

#[test]
fn configure_without_the_metadata_of_slang_src_says_to_depend_on_it_directly() {
    for name in [SRC_ROOT, SRC_COMMIT, SRC_BUILD_TAG, SRC_SHARED_DIR] {
        let mut host = linux_host();
        set_var(&mut host, name, None);
        failure(
            &host,
            &[
                &format!("{name} is not set"),
                "the crate has to depend on slang-src directly",
            ],
        );
    }
    let mut host = linux_host();
    set_var(&mut host, SRC_ROOT, Some("relative/tree"));
    failure(
        &host,
        &[
            "DEP_SLANG_SRC_ROOT is \"relative/tree\", which is not an absolute path in UTF-8 without line breaks",
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
fn configure_without_a_path_or_a_triple_from_cargo_says_that_cargo_sets_it() {
    for (name, purpose) in [
        (OUT_DIR, "build slang-glslang"),
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
                "CMake 3.22 or later, Ninja, and Python 3 on PATH, and a C++20 compiler",
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
    set_var(&mut host, "MACOSX_DEPLOYMENT_TARGET", Some("13.0"));
    let (directives, result) = run(&host);
    result.expect("the build succeeds");
    let inputs = expected_inputs(
        LINUX,
        &CONFIGURE_OPTIONS,
        &[("CXXFLAGS", "-O1"), ("MACOSX_DEPLOYMENT_TARGET", "13.0")],
        COMPILER_VERSION,
    );
    assert_ne!(inputs, linux_inputs());
    assert_eq!(
        directives,
        expected_directives(&entry_for(&inputs), LINUX_LINKS)
    );
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
                &format!("so the {step} step of the build of slang-glslang failed"),
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
fn configure_reports_a_library_that_the_build_did_not_produce() {
    for (kind, problem) in [
        (Err(io::ErrorKind::NotFound), "cannot access"),
        (Ok(EntryKind::Directory), "is not a file"),
    ] {
        let mut host = linux_host();
        let missing = out_dir().join("lib").join("libSPIRV-Tools-link.a");
        host.entries.retain(|(path, _)| *path != missing);
        host.entries.push((missing.clone(), kind));
        failure(
            &host,
            &[
                problem,
                &missing.display().to_string(),
                "another layout than the one that this version of slang-glslang-sys expects",
            ],
        );
        assert!(host.copied.borrow().is_empty());
    }
}

fn complete_entry(host: &mut FakeHost, inputs: &str) -> PathBuf {
    let entry = entry_for(inputs);
    host.entries.push((entry.clone(), Ok(EntryKind::Directory)));
    host.files.push((entry.join("inputs"), inputs.to_owned()));
    for name in archives(LINUX) {
        host.entries
            .push((entry.join("lib").join(name), Ok(EntryKind::File)));
    }
    entry
}

#[test]
fn configure_uses_a_complete_entry_without_building() {
    let mut host = linux_host();
    let entry = complete_entry(&mut host, &linux_inputs());
    let (directives, result) = run(&host);
    result.expect("the build succeeds");
    assert_eq!(directives, expected_directives(&entry, LINUX_LINKS));
    assert!(cmake_runs(&host).iter().all(|(_, log)| log.is_none()));
    assert!(host.written.borrow().is_empty());
    assert!(host.copied.borrow().is_empty());
}

#[test]
fn configure_replaces_an_entry_whose_inputs_or_libraries_differ() {
    let inputs = linux_inputs();
    for case in ["inputs", "library"] {
        let mut host = linux_host();
        let entry = complete_entry(&mut host, &inputs);
        if case == "inputs" {
            host.files.retain(|(path, _)| *path != entry.join("inputs"));
            host.files
                .push((entry.join("inputs"), "crate=other\n".to_owned()));
        } else {
            let library = entry.join("lib").join("libglslang.a");
            host.entries.retain(|(path, _)| *path != library);
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
            temporary.join("lib").join("libglslang.a"),
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
