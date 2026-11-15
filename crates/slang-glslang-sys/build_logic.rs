use std::ffi::{OsStr, OsString};
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

const TARGET_OS: &str = "CARGO_CFG_TARGET_OS";
const TARGET_ENV: &str = "CARGO_CFG_TARGET_ENV";
const TARGET_FEATURE: &str = "CARGO_CFG_TARGET_FEATURE";
const TARGET: &str = "TARGET";
const HOST: &str = "HOST";
const OUT_DIR: &str = "OUT_DIR";
const MANIFEST_DIR: &str = "CARGO_MANIFEST_DIR";
const NUM_JOBS: &str = "NUM_JOBS";
const CXX: &str = "CXX";
const SRC_ROOT: &str = "DEP_SLANG_SRC_ROOT";
const SRC_COMMIT: &str = "DEP_SLANG_SRC_COMMIT";
const SRC_BUILD_TAG: &str = "DEP_SLANG_SRC_BUILD_TAG";
const SRC_SHARED_DIR: &str = "DEP_SLANG_SRC_SHARED_DIR";
const CRATE_NAME: &str = env!("CARGO_PKG_NAME");
const CRATE_VERSION: &str = env!("CARGO_PKG_VERSION");

const CMAKE_VARIABLES: [&str; 7] = [
    "CC",
    "CXX",
    "CFLAGS",
    "CXXFLAGS",
    "LDFLAGS",
    "CMAKE_TOOLCHAIN_FILE",
    "MACOSX_DEPLOYMENT_TARGET",
];

const MSVC_TOOLS_VERSION: &str = "VCToolsVersion";
const MSVC_DEVELOPER_PROMPT: &str = "VCINSTALLDIR";
const MSVC_VARIABLES: [&str; 2] = [MSVC_DEVELOPER_PROMPT, MSVC_TOOLS_VERSION];

const NATIVE_FILES: [(&str, &str); 2] = [
    ("CMakeLists.txt", include_str!("native/CMakeLists.txt")),
    ("loader.cpp", include_str!("native/loader.cpp")),
];

const STATIC_LIBRARIES: [&str; 5] = [
    "slang-glslang-static",
    "glslang",
    "SPIRV-Tools-opt",
    "SPIRV-Tools-link",
    "SPIRV-Tools",
];

const LINUX_SYSTEM_LIBRARIES: [&str; 1] = ["pthread"];

const MSVC_DEFAULT_LIBRARIES: [&str; 10] = [
    "kernel32", "user32", "gdi32", "winspool", "shell32", "ole32", "oleaut32", "uuid", "comdlg32",
    "advapi32",
];

const BUILD_TARGET: &str = "slang-glslang-static";

const CONFIGURE_OPTIONS: [&str; 2] = [
    "-DCMAKE_BUILD_TYPE=Release",
    "-DFETCHCONTENT_FULLY_DISCONNECTED=ON",
];

const PYTHON_NO_BYTECODE: (&str, &str) = ("PYTHONDONTWRITEBYTECODE", "1");

const LOG_TAIL_LINES: usize = 40;
const ENTRY_FOLDER: &str = "slang-glslang-sys";
const STAMP_TIME: Duration = Duration::from_hours(262_968);
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

const REMEDY: &str = "build without the Cargo feature `vendored` of the crate that enables slang-glslang-sys, such as slang-compiler, to link the Slang of a Vulkan SDK";

trait Host {
    fn var(&self, name: &str) -> Option<OsString>;
    fn read(&self, path: &Path) -> io::Result<String>;
    fn kind(&self, path: &Path) -> io::Result<EntryKind>;
    fn run(&self, invocation: &Invocation) -> io::Result<Finished>;
    fn run_to_log(&self, invocation: &Invocation, log: &Path) -> Result<Exit, LoggedRunError>;
    fn list(&self, dir: &Path) -> io::Result<Vec<OsString>>;
    fn copy_file(&self, from: &Path, to: &Path) -> io::Result<()>;
    fn remove_file(&self, path: &Path) -> io::Result<()>;
    fn remove_dir_all(&self, path: &Path) -> io::Result<()>;
    fn write(&self, path: &Path, text: &str) -> io::Result<()>;
    fn set_modified(&self, path: &Path, time: SystemTime) -> io::Result<()>;
    fn rename(&self, from: &Path, to: &Path) -> io::Result<()>;
    fn lock(&self, path: &Path) -> io::Result<()>;
    fn process_id(&self) -> u32;
    fn msvc_compiler(&self, target: &str) -> Option<MsvcCompiler>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EntryKind {
    Directory,
    File,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Invocation {
    program: String,
    args: Vec<String>,
    env: Vec<(OsString, OsString)>,
}

impl fmt::Display for Invocation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.program)?;
        for arg in &self.args {
            write!(formatter, " {arg}")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Exit {
    code: Option<i32>,
}

impl Exit {
    fn success(self) -> bool {
        self.code == Some(0)
    }

    fn status(self) -> String {
        self.code.map_or_else(
            || "no exit status, because a signal ended it".to_owned(),
            |code| format!("exit status {code}"),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Finished {
    exit: Exit,
    stdout: String,
    stderr: String,
}

enum LoggedRunError {
    Log(io::Error),
    Start(io::Error),
}

struct BuildError(String);

impl fmt::Debug for BuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MsvcCompiler {
    path: PathBuf,
    env: Vec<(OsString, OsString)>,
}

struct Toolchain {
    identity: String,
    env: Vec<(OsString, OsString)>,
}

struct Source {
    root: PathBuf,
    commit: String,
    build_tag: String,
    shared: PathBuf,
}

struct Platform {
    msvc: bool,
    cxx_library: Option<&'static str>,
    system_libraries: &'static [&'static str],
    configure_options: Vec<String>,
}

impl Platform {
    fn archive(&self, library: &str) -> String {
        if self.msvc {
            format!("{library}.lib")
        } else {
            format!("lib{library}.a")
        }
    }
}

struct Layout {
    build: PathBuf,
    lib: PathBuf,
    log: PathBuf,
}

impl Layout {
    fn new(out: &Path) -> Self {
        Self {
            build: out.join("build"),
            lib: out.join("lib"),
            log: out.join("build.log"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EntryState {
    Missing,
    Complete,
    Unusable,
}

fn configure(host: &impl Host, directives: &mut Vec<String>) -> Result<(), BuildError> {
    for name in CMAKE_VARIABLES.into_iter().chain(MSVC_VARIABLES) {
        directives.push(format!("cargo::rerun-if-env-changed={name}"));
    }
    let source = read_source(host)?;
    let out = cargo_path(host, OUT_DIR, "build slang-glslang")?;
    let native = cargo_path(host, MANIFEST_DIR, "find the wrapper project")?.join("native");
    let target = required(host, TARGET)?;
    let host_triple = required(host, HOST)?;
    let platform = platform(host, &target, &host_triple)?;
    check_success(host, &command("ninja", &["--version"]))?;
    let toolchain = toolchain(host, &target, &platform)?;
    let layout = Layout::new(&out);
    let options = configure_options(&platform);
    let inputs = entry_inputs(host, &source, &target, &toolchain, &options)?;
    let entries = source.shared.join(ENTRY_FOLDER);
    let key = fnv1a(inputs.as_bytes());
    let lock = entries.join(format!("{key}.lock"));
    host.lock(&lock)
        .map_err(|error| shared_error("lock", &lock, &error))?;
    let entry = entries.join(&key);
    directives.push(format!(
        "cargo::rerun-if-changed={}",
        entry.join("stamp").display()
    ));
    let state = entry_state(host, &entry, &inputs, &platform)?;
    if state != EntryState::Complete {
        let build = Build {
            layout: &layout,
            source: &source,
            native: &native,
            platform: &platform,
            toolchain: &toolchain,
            options: &options,
        };
        build_libraries(host, &build)?;
        store_entry(
            host,
            &entries,
            &key,
            &layout,
            &platform,
            &inputs,
            state == EntryState::Unusable,
        )?;
    }
    directives.push(format!(
        "cargo::rustc-link-search=native={}",
        entry.join("lib").display()
    ));
    for library in STATIC_LIBRARIES {
        directives.push(format!("cargo::rustc-link-lib=static={library}"));
    }
    for library in platform.cxx_library.iter().chain(platform.system_libraries) {
        directives.push(format!("cargo::rustc-link-lib=dylib={library}"));
    }
    Ok(())
}

fn directive_safe_path(value: &OsStr) -> Option<PathBuf> {
    value
        .to_str()
        .filter(|text| !text.contains(['\n', '\r']) && Path::new(text).is_absolute())
        .map(PathBuf::from)
}

fn quoted(value: &OsStr) -> String {
    format!("\"{}\"", value.to_string_lossy().escape_debug())
}

fn path_problem(name: &str, value: &OsStr) -> String {
    format!(
        "{name} is {}, which is not an absolute path in UTF-8 without line breaks",
        quoted(value)
    )
}

fn cargo_path(host: &impl Host, name: &str, purpose: &str) -> Result<PathBuf, BuildError> {
    let value = host.var(name);
    let problem = match &value {
        None => format!("{name} is not set"),
        Some(value) => match directive_safe_path(value) {
            Some(path) => return Ok(path),
            None => path_problem(name, value),
        },
    };
    Err(BuildError(format!(
        "{problem}, so the build has no directory in which to {purpose}. Cargo sets {name}, so build the crate through Cargo, with a target directory whose path is UTF-8."
    )))
}

fn read_source(host: &impl Host) -> Result<Source, BuildError> {
    let root = metadata_path(host, SRC_ROOT)?;
    let commit = metadata_text(host, SRC_COMMIT)?;
    let build_tag = metadata_text(host, SRC_BUILD_TAG)?;
    let shared = metadata_path(host, SRC_SHARED_DIR)?;
    Ok(Source {
        root,
        commit,
        build_tag,
        shared,
    })
}

fn metadata_path(host: &impl Host, name: &str) -> Result<PathBuf, BuildError> {
    let value = host.var(name);
    value
        .as_deref()
        .and_then(directive_safe_path)
        .ok_or_else(|| {
            metadata_error(&value.map_or_else(
                || format!("{name} is not set"),
                |value| path_problem(name, &value),
            ))
        })
}

fn metadata_text(host: &impl Host, name: &str) -> Result<String, BuildError> {
    let value = host.var(name);
    value
        .as_deref()
        .and_then(OsStr::to_str)
        .filter(|text| !text.is_empty() && !text.contains(['\n', '\r']))
        .map(str::to_owned)
        .ok_or_else(|| {
            metadata_error(&value.map_or_else(
                || format!("{name} is not set"),
                |value| {
                    format!(
                        "{name} is {}, which is not one line of UTF-8 text",
                        quoted(&value)
                    )
                },
            ))
        })
}

fn metadata_error(problem: &str) -> BuildError {
    BuildError(format!(
        "{problem}, so the build of slang-glslang has no valid source tree. The build reads the Slang source tree and the shared folder from the metadata of slang-src, which Cargo passes only when slang-src is a direct dependency of the version that slang-glslang-sys requires, so the crate has to depend on slang-src directly."
    ))
}

fn required(host: &impl Host, name: &str) -> Result<String, BuildError> {
    host.var(name)
        .map(|value| value.to_string_lossy().into_owned())
        .ok_or_else(|| {
            BuildError(format!(
                "{name} is not set, so the build of slang-glslang cannot compare the target with the host. Cargo sets {name}, so build the crate through Cargo."
            ))
        })
}

fn platform(host: &impl Host, target: &str, host_triple: &str) -> Result<Platform, BuildError> {
    if target != host_triple {
        return Err(BuildError(format!(
            "the target {target} differs from the host {host_triple}, and the build of slang-glslang cannot cross-compile, because the Slang that it serves is built from source for the host only. Build on a host of the target, or {REMEDY}."
        )));
    }
    let text = |name: &str| {
        host.var(name)
            .map_or_default(|value| value.to_string_lossy().into_owned())
    };
    let os = text(TARGET_OS);
    let env = text(TARGET_ENV);
    let crt_static = text(TARGET_FEATURE)
        .split(',')
        .any(|feature| feature == "crt-static");
    match (os.as_str(), env.as_str()) {
        ("linux", "gnu") => Ok(Platform {
            msvc: false,
            cxx_library: Some("stdc++"),
            system_libraries: &LINUX_SYSTEM_LIBRARIES,
            configure_options: Vec::new(),
        }),
        ("macos", _) => Ok(Platform {
            msvc: false,
            cxx_library: Some("c++"),
            system_libraries: &[],
            configure_options: Vec::new(),
        }),
        ("windows", "msvc") => Ok(Platform {
            msvc: true,
            cxx_library: None,
            system_libraries: &MSVC_DEFAULT_LIBRARIES,
            configure_options: vec![format!(
                "-DCMAKE_MSVC_RUNTIME_LIBRARY={}",
                if crt_static {
                    "MultiThreaded"
                } else {
                    "MultiThreadedDLL"
                }
            )],
        }),
        _ => Err(BuildError(format!(
            "the target {target} has no link rule for its C++ runtime library, so the build of slang-glslang cannot link the C++ code of glslang and SPIRV-Tools for it. The build has a link rule for Linux with the GNU C library, macOS, and Windows with the Microsoft toolchain. Build on a host of one of these targets, or {REMEDY}."
        ))),
    }
}

fn command(program: &str, args: &[&str]) -> Invocation {
    Invocation {
        program: program.to_owned(),
        args: args.iter().map(|arg| (*arg).to_owned()).collect(),
        env: Vec::new(),
    }
}

fn cmake(args: Vec<String>, toolchain: &Toolchain) -> Invocation {
    let mut env = toolchain.env.clone();
    env.push((
        OsString::from(PYTHON_NO_BYTECODE.0),
        OsString::from(PYTHON_NO_BYTECODE.1),
    ));
    Invocation {
        program: "cmake".to_owned(),
        args,
        env,
    }
}

fn configure_options(platform: &Platform) -> Vec<String> {
    let mut options: Vec<String> = CONFIGURE_OPTIONS
        .iter()
        .map(|option| (*option).to_owned())
        .collect();
    options.extend(platform.configure_options.iter().cloned());
    options
}

fn first_line(host: &impl Host, invocation: &Invocation) -> Result<String, BuildError> {
    let finished = check_success(host, invocation)?;
    Ok(finished
        .stdout
        .lines()
        .next()
        .unwrap_or_default()
        .trim_end()
        .to_owned())
}

fn toolchain(host: &impl Host, target: &str, platform: &Platform) -> Result<Toolchain, BuildError> {
    if platform.msvc {
        let compiler = host
            .msvc_compiler(target)
            .ok_or_else(|| msvc_error(host, target))?;
        return Ok(Toolchain {
            identity: compiler.path.display().to_string(),
            env: compiler.env,
        });
    }
    let value = host
        .var(CXX)
        .map_or_default(|value| value.to_string_lossy().into_owned());
    let invocation = compiler_probe(host, &value);
    Ok(Toolchain {
        identity: first_line(host, &invocation)?,
        env: Vec::new(),
    })
}

fn compiler_probe(host: &impl Host, value: &str) -> Invocation {
    let value = value.trim();
    if value.is_empty() {
        return command("c++", &["--version"]);
    }
    if matches!(host.kind(Path::new(value)), Ok(EntryKind::File)) {
        return command(value, &["--version"]);
    }
    let mut words: Vec<&str> = value.split_ascii_whitespace().collect();
    let program = words.remove(0);
    words.push("--version");
    command(program, &words)
}

fn msvc_error(host: &impl Host, target: &str) -> BuildError {
    let shown = |name: &str| {
        host.var(name)
            .map_or_else(|| "not set".to_owned(), |value| quoted(&value))
    };
    BuildError(format!(
        "cannot find the Microsoft C++ compiler cl.exe for the target {target}. The build of slang-glslang runs CMake with Ninja, which needs the environment of the Microsoft toolchain, so the build script looked for cl.exe first in the environment of a developer command prompt, which VCINSTALLDIR marks and which is {}, and then in every installation of Visual Studio 2015 or later that the Visual Studio setup configuration or the registry lists, limited to the toolset that VCToolsVersion names, which is {}, and found none. Install Visual Studio or its Build Tools with the workload for desktop development with C++, or {REMEDY}.",
        shown(MSVC_DEVELOPER_PROMPT),
        shown(MSVC_TOOLS_VERSION)
    ))
}

fn native_files_hash() -> String {
    let mut bytes = Vec::new();
    for (name, text) in NATIVE_FILES {
        bytes.extend_from_slice(name.as_bytes());
        bytes.push(0);
        bytes.extend_from_slice(text.as_bytes());
        bytes.push(0);
    }
    fnv1a(&bytes)
}

fn entry_inputs(
    host: &impl Host,
    source: &Source,
    target: &str,
    toolchain: &Toolchain,
    options: &[String],
) -> Result<String, BuildError> {
    let cmake_version = first_line(host, &cmake(vec!["--version".to_owned()], toolchain))?;
    let mut lines = vec![
        format!("crate={CRATE_NAME} {CRATE_VERSION}"),
        format!("files={}", native_files_hash()),
        format!("commit={}", source.commit),
        format!("build_tag={}", source.build_tag),
        format!("target={target}"),
        format!("cmake={cmake_version}"),
    ];
    lines.extend(options.iter().map(|option| format!("configure={option}")));
    for name in CMAKE_VARIABLES.into_iter().chain([MSVC_TOOLS_VERSION]) {
        let value = host
            .var(name)
            .map_or_default(|value| value.to_string_lossy().into_owned());
        lines.push(format!("{name}={value}"));
    }
    lines.push(format!("compiler={}", toolchain.identity));
    Ok(lines.join("\n") + "\n")
}

fn fnv1a(bytes: &[u8]) -> String {
    let hash = bytes.iter().fold(FNV_OFFSET, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(FNV_PRIME)
    });
    format!("{hash:016x}")
}

fn entry_state(
    host: &impl Host,
    entry: &Path,
    inputs: &str,
    platform: &Platform,
) -> Result<EntryState, BuildError> {
    match host.kind(entry) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(EntryState::Missing),
        Err(error) => return Err(shared_error("inspect", entry, &error)),
        Ok(_) => {}
    }
    let same_inputs = host
        .read(&entry.join("inputs"))
        .is_ok_and(|text| text == inputs);
    let lib = entry.join("lib");
    let complete = same_inputs
        && STATIC_LIBRARIES.iter().all(|library| {
            matches!(
                host.kind(&lib.join(platform.archive(library))),
                Ok(EntryKind::File)
            )
        });
    Ok(if complete {
        EntryState::Complete
    } else {
        EntryState::Unusable
    })
}

struct Build<'a> {
    layout: &'a Layout,
    source: &'a Source,
    native: &'a Path,
    platform: &'a Platform,
    toolchain: &'a Toolchain,
    options: &'a [String],
}

fn build_libraries(host: &impl Host, build: &Build<'_>) -> Result<(), BuildError> {
    let Build {
        layout,
        source,
        native,
        platform,
        toolchain,
        options,
    } = *build;
    write_file(host, &layout.log, "")?;
    remove_if_present(host, &layout.build.join("CMakeCache.txt"))?;
    let mut configure_args = vec![
        "-S".to_owned(),
        native.display().to_string(),
        "-B".to_owned(),
        layout.build.display().to_string(),
        "-G".to_owned(),
        "Ninja".to_owned(),
    ];
    configure_args.extend(options.iter().cloned());
    configure_args.push(format!("-DSLANG_TREE_DIR={}", source.root.display()));
    configure_args.push(format!(
        "-DCMAKE_ARCHIVE_OUTPUT_DIRECTORY={}",
        layout.lib.display()
    ));
    run_logged(
        host,
        &cmake(configure_args, toolchain),
        &layout.log,
        "configure",
    )?;
    let mut build_args = vec![
        "--build".to_owned(),
        layout.build.display().to_string(),
        "--target".to_owned(),
        BUILD_TARGET.to_owned(),
    ];
    if let Some(jobs) = host
        .var(NUM_JOBS)
        .and_then(|jobs| jobs.to_str()?.parse::<u32>().ok())
    {
        build_args.extend(["--parallel".to_owned(), jobs.to_string()]);
    }
    run_logged(host, &cmake(build_args, toolchain), &layout.log, "build")?;
    for library in STATIC_LIBRARIES {
        let file = layout.lib.join(platform.archive(library));
        match host.kind(&file) {
            Ok(EntryKind::File) => {}
            Ok(_) => return Err(layout_error(&format!("{} is not a file", file.display()))),
            Err(error) => {
                return Err(layout_error(&format!(
                    "cannot access {}: {error}",
                    file.display()
                )));
            }
        }
    }
    Ok(())
}

fn store_entry(
    host: &impl Host,
    entries: &Path,
    key: &str,
    layout: &Layout,
    platform: &Platform,
    inputs: &str,
    replace: bool,
) -> Result<(), BuildError> {
    let prefix = format!("{key}.tmp-");
    let names = host
        .list(entries)
        .map_err(|error| shared_error("list", entries, &error))?;
    for name in names {
        let stale = name
            .to_str()
            .and_then(|name| name.strip_prefix(&prefix))
            .is_some_and(|number| {
                !number.is_empty() && number.bytes().all(|byte| byte.is_ascii_digit())
            });
        if stale {
            let path = entries.join(name);
            host.remove_dir_all(&path)
                .map_err(|error| shared_error("remove", &path, &error))?;
        }
    }
    let temporary = entries.join(format!("{prefix}{}", host.process_id()));
    for library in STATIC_LIBRARIES {
        let name = platform.archive(library);
        let (from, to) = (layout.lib.join(&name), temporary.join("lib").join(&name));
        host.copy_file(&from, &to)
            .map_err(|error| shared_error(&format!("copy {} to", from.display()), &to, &error))?;
    }
    let inputs_file = temporary.join("inputs");
    host.write(&inputs_file, inputs)
        .map_err(|error| shared_error("write", &inputs_file, &error))?;
    let stamp = temporary.join("stamp");
    host.write(&stamp, "")
        .map_err(|error| shared_error("write", &stamp, &error))?;
    host.set_modified(&stamp, SystemTime::UNIX_EPOCH + STAMP_TIME)
        .map_err(|error| shared_error("set the modification time of", &stamp, &error))?;
    let entry = entries.join(key);
    if replace {
        host.remove_dir_all(&entry)
            .map_err(|error| shared_error("remove", &entry, &error))?;
    }
    host.rename(&temporary, &entry).map_err(|error| {
        shared_error(
            &format!("rename {} to", temporary.display()),
            &entry,
            &error,
        )
    })
}

fn run_logged(
    host: &impl Host,
    invocation: &Invocation,
    log: &Path,
    step: &str,
) -> Result<(), BuildError> {
    let exit = host
        .run_to_log(invocation, log)
        .map_err(|error| match error {
            LoggedRunError::Log(error) => file_error("open the log", log, &error),
            LoggedRunError::Start(error) => start_error(invocation, &error),
        })?;
    if exit.success() {
        return Ok(());
    }
    let tail = match host.read(log) {
        Ok(text) => {
            let lines: Vec<&str> = text.lines().collect();
            lines[lines.len().saturating_sub(LOG_TAIL_LINES)..].join("\n")
        }
        Err(error) => format!("the log cannot be read: {error}"),
    };
    Err(BuildError(format!(
        "`{invocation}` failed with {}, so the {step} step of the build of slang-glslang failed. The full output is in {}, and it ends with these lines:\n{tail}",
        exit.status(),
        log.display()
    )))
}

fn check_success(host: &impl Host, invocation: &Invocation) -> Result<Finished, BuildError> {
    let finished = host
        .run(invocation)
        .map_err(|error| start_error(invocation, &error))?;
    if finished.exit.success() {
        return Ok(finished);
    }
    Err(BuildError(format!(
        "`{invocation}` failed with {}: {}",
        finished.exit.status(),
        finished.stderr.trim()
    )))
}

fn start_error(invocation: &Invocation, error: &io::Error) -> BuildError {
    BuildError(format!(
        "cannot start {} for `{invocation}`: {error}. The build of slang-glslang needs CMake 3.22 or later, Ninja, and Python 3 on PATH, and a C++20 compiler. Install the missing program, or {REMEDY}.",
        invocation.program
    ))
}

fn write_file(host: &impl Host, path: &Path, text: &str) -> Result<(), BuildError> {
    host.write(path, text)
        .map_err(|error| file_error("write", path, &error))
}

fn remove_if_present(host: &impl Host, path: &Path) -> Result<(), BuildError> {
    match host.remove_file(path) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => {
            Err(file_error("remove", path, &error))
        }
        _ => Ok(()),
    }
}

fn file_error(action: &str, path: &Path, error: &io::Error) -> BuildError {
    BuildError(format!(
        "cannot {action} {}: {error}. The build of slang-glslang keeps its build tree, its libraries, and its log under {OUT_DIR}.",
        path.display()
    ))
}

fn shared_error(action: &str, path: &Path, error: &io::Error) -> BuildError {
    BuildError(format!(
        "cannot {action} {}: {error}. The build keeps the libraries of slang-glslang that it built in the shared folder of the Cargo target directory, and removing the entry, or running cargo clean, when no build runs is safe.",
        path.display()
    ))
}

fn layout_error(problem: &str) -> BuildError {
    BuildError(format!(
        "the build of slang-glslang finished, but {problem}, so the build produced another layout than the one that this version of slang-glslang-sys expects. Check the source tree of slang-src."
    ))
}
