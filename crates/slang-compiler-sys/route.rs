use std::collections::HashSet;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, SystemTime};

const VENDORED: &str = "CARGO_FEATURE_VENDORED";
const VULKAN_SDK: &str = "VULKAN_SDK";
const LIBCLANG_PATH: &str = "LIBCLANG_PATH";
const TAG_MACRO: &str = "SLANG_TAG_VERSION";
const TARGET_OS: &str = "CARGO_CFG_TARGET_OS";
const TARGET_ENV: &str = "CARGO_CFG_TARGET_ENV";
const TARGET_FEATURE: &str = "CARGO_CFG_TARGET_FEATURE";
const POINTER_WIDTH: &str = "CARGO_CFG_TARGET_POINTER_WIDTH";
const TARGET: &str = "TARGET";
const HOST: &str = "HOST";
const OUT_DIR: &str = "OUT_DIR";
const NUM_JOBS: &str = "NUM_JOBS";
const CXX: &str = "CXX";
const SRC_ROOT: &str = "DEP_SLANG_SRC_ROOT";
const SRC_COMMIT: &str = "DEP_SLANG_SRC_COMMIT";
const SRC_BUILD_TAG: &str = "DEP_SLANG_SRC_BUILD_TAG";
const SRC_SHARED_DIR: &str = "DEP_SLANG_SRC_SHARED_DIR";
const CRATE_NAME: &str = env!("CARGO_PKG_NAME");
const CRATE_VERSION: &str = env!("CARGO_PKG_VERSION");

const TESTED_TARGETS: [&str; 4] = [
    "x86_64-unknown-linux-gnu",
    "x86_64-pc-windows-msvc",
    "aarch64-apple-darwin",
    "x86_64-apple-darwin",
];

const BINDINGS_FILE: &str = "bindings.rs";
const SYMBOLS_FILE: &str = "symbols.rs";
const BUILD_INFO_FILE: &str = "build_info.rs";

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

const REQUIRED_SUBMODULES: [&str; 8] = [
    "external/cmark",
    "external/fast_float",
    "external/lua",
    "external/lz4",
    "external/miniz",
    "external/spirv-headers",
    "external/unordered_dense",
    "external/vulkan",
];

const STATIC_LIBRARIES: [&str; 6] = [
    "slang-compiler",
    "compiler-core",
    "core",
    "cmark-gfm",
    "miniz",
    "lz4",
];

const SLANG_OPTIONS: [&str; 27] = [
    "-DCMAKE_BUILD_TYPE=Release",
    "-DSLANG_LIB_TYPE=STATIC",
    "-DSLANG_EMBED_CORE_MODULE=ON",
    "-DSLANG_EMBED_CORE_MODULE_SOURCE=ON",
    "-DSLANG_SLANG_LLVM_FLAVOR=DISABLE",
    "-DSLANG_ENABLE_DXIL=OFF",
    "-DSLANG_ENABLE_TESTS=OFF",
    "-DSLANG_ENABLE_EXAMPLES=OFF",
    "-DSLANG_ENABLE_GFX=OFF",
    "-DSLANG_ENABLE_SLANG_RHI=OFF",
    "-DSLANG_ENABLE_REPLAYER=OFF",
    "-DSLANG_ENABLE_SLANGD=OFF",
    "-DSLANG_ENABLE_SLANGC=OFF",
    "-DSLANG_ENABLE_SLANGI=OFF",
    "-DSLANG_ENABLE_SLANGRT=OFF",
    "-DSLANG_ENABLE_SLANG_GLSLANG=OFF",
    "-DSLANG_ENABLE_SLANG_PROXY=OFF",
    "-DSLANG_ENABLE_CUDA=OFF",
    "-DSLANG_ENABLE_OPTIX=OFF",
    "-DSLANG_ENABLE_NVAPI=OFF",
    "-DSLANG_ENABLE_XLIB=OFF",
    "-DSLANG_ENABLE_AFTERMATH=OFF",
    "-DSLANG_EXCLUDE_DAWN=ON",
    "-DSLANG_EXCLUDE_TINT=ON",
    "-DSLANG_ENABLE_RELEASE_DEBUG_INFO=OFF",
    "-DSLANG_ENABLE_SPLIT_DEBUG_INFO=OFF",
    "-DSLANG_USE_SCCACHE=OFF",
];

const LINUX_SYSTEM_LIBRARIES: [&str; 2] = ["pthread", "dl"];

const MSVC_DEFAULT_LIBRARIES: [&str; 10] = [
    "kernel32", "user32", "gdi32", "winspool", "shell32", "ole32", "oleaut32", "uuid", "comdlg32",
    "advapi32",
];

const ARCHIVE_OUTPUT_OPTION: &str = "-DCMAKE_ARCHIVE_OUTPUT_DIRECTORY=";

const GIT_REPOSITORY_VARIABLES: [&str; 15] = [
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_CONFIG",
    "GIT_CONFIG_PARAMETERS",
    "GIT_CONFIG_COUNT",
    "GIT_OBJECT_DIRECTORY",
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_IMPLICIT_WORK_TREE",
    "GIT_GRAFT_FILE",
    "GIT_INDEX_FILE",
    "GIT_NO_REPLACE_OBJECTS",
    "GIT_REPLACE_REF_BASE",
    "GIT_PREFIX",
    "GIT_SHALLOW_FILE",
    "GIT_COMMON_DIR",
];

const LOG_TAIL_LINES: usize = 40;
const ENTRY_FOLDER: &str = "slang-compiler-sys";
const STAMP_TIME: Duration = Duration::from_hours(262_968);
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

const VENDORED_ADVICE: &str = "Enabling the Cargo feature `vendored` builds Slang from source instead, which needs git, CMake, Ninja, a C++ compiler, and network access for the first build in a target directory";

const DYNAMIC_REMEDY: &str =
    "build without the Cargo feature `vendored` to link the Slang of a Vulkan SDK";

trait Host {
    fn var(&self, name: &str) -> Option<OsString>;
    fn read(&self, path: &Path) -> io::Result<String>;
    fn kind(&self, path: &Path) -> io::Result<EntryKind>;
    fn host_os(&self) -> &str;
    fn run(&self, invocation: &Invocation) -> io::Result<Finished>;
    fn run_to_log(&self, invocation: &Invocation, log: &Path) -> Result<Exit, LoggedRunError>;
    fn files(&self, root: &Path) -> io::Result<Vec<PathBuf>>;
    fn list(&self, dir: &Path) -> io::Result<Vec<OsString>>;
    fn sync_file(&self, from: &Path, to: &Path) -> io::Result<()>;
    fn remove_file(&self, path: &Path) -> io::Result<()>;
    fn remove_dir_all(&self, path: &Path) -> io::Result<()>;
    fn write(&self, path: &Path, text: &str) -> io::Result<()>;
    fn set_modified(&self, path: &Path, time: SystemTime) -> io::Result<()>;
    fn rename(&self, from: &Path, to: &Path) -> io::Result<()>;
    fn lock(&self, path: &Path) -> io::Result<()>;
    fn process_id(&self) -> u32;
    fn msvc_compiler(&self, target: &str) -> Option<MsvcCompiler>;
    fn generate(&self, header_dir: &Path, target: &str) -> Result<Generated, GenerationError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EntryKind {
    Directory,
    File,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Wanted {
    Directory,
    File,
}

impl Wanted {
    fn kind(self) -> EntryKind {
        match self {
            Self::Directory => EntryKind::Directory,
            Self::File => EntryKind::File,
        }
    }

    fn noun(self) -> &'static str {
        match self {
            Self::Directory => "directory",
            Self::File => "file",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Generated {
    bindings: String,
    symbols: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GenerationErrorKind {
    Libclang,
    Target,
    Headers,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GenerationError {
    kind: GenerationErrorKind,
    message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Invocation {
    program: String,
    args: Vec<String>,
    dir: Option<PathBuf>,
    env: Vec<(OsString, OsString)>,
    env_remove: Vec<&'static str>,
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

enum KindError {
    Wrong,
    Inaccessible(io::Error),
}

struct BuildError(String);

impl fmt::Debug for BuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, PartialEq, Eq)]
enum TagError {
    Missing,
    Malformed { line: usize },
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
    copy: PathBuf,
    build: PathBuf,
    include: PathBuf,
    lib: PathBuf,
    log: PathBuf,
    stamp: PathBuf,
}

impl Layout {
    fn new(out: &Path) -> Self {
        let root = out.join("slang");
        let build = root.join("build");
        Self {
            copy: root.join("source"),
            include: build.join("Release").join("include"),
            lib: root.join("lib"),
            log: root.join("build.log"),
            stamp: root.join("stamp"),
            build,
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
    directives.push(format!("cargo::rerun-if-env-changed={LIBCLANG_PATH}"));
    if host.var(VENDORED).is_some() {
        vendored_route(host, directives)
    } else {
        dynamic_route(host, directives)
    }
}

fn dynamic_route(host: &impl Host, directives: &mut Vec<String>) -> Result<(), BuildError> {
    directives.push(format!("cargo::rerun-if-env-changed={VULKAN_SDK}"));
    let sdk = sdk_dir(host.var(VULKAN_SDK))?;
    let include = sdk.join("include").join("slang");
    directives.push(format!("cargo::rerun-if-changed={}", include.display()));
    let tag_header = include.join("slang-tag-version.h");
    let text = host
        .read(&tag_header)
        .map_err(|error| unreadable_header_error(&tag_header, &error))?;
    let build_tag = parse_tag(&text)
        .map_err(|error| tag_error(&tag_header, &error))?
        .to_owned();
    let lib = sdk.join("lib");
    check_lib(host, &sdk, &lib)?;
    let target = target(host.var(TARGET).as_deref())?;
    check_pointer_width(&target, host.var(POINTER_WIDTH))?;
    let tested = warn_untested(directives, &target);
    let out = out_dir(
        host.var(OUT_DIR).as_deref(),
        "write the generated Slang bindings",
    )?;
    generate_files(
        host,
        &out,
        &include,
        &target,
        &build_tag,
        &build_info(&build_tag, None, tested),
    )?;
    directives.push(format!("cargo::rustc-link-search=native={}", lib.display()));
    directives.push("cargo::rustc-link-lib=dylib=slang-compiler".to_owned());
    push_metadata(directives, &include, &lib, "dynamic", &build_tag);
    Ok(())
}

fn vendored_route(host: &impl Host, directives: &mut Vec<String>) -> Result<(), BuildError> {
    for name in CMAKE_VARIABLES.into_iter().chain(MSVC_VARIABLES) {
        directives.push(format!("cargo::rerun-if-env-changed={name}"));
    }
    let source = read_source(host)?;
    let out = out_dir(host.var(OUT_DIR).as_deref(), "build Slang")?;
    let target = required(host, TARGET)?;
    let host_triple = required(host, HOST)?;
    let platform = platform(host, &target, &host_triple)?;
    check_pointer_width(&target, host.var(POINTER_WIDTH))?;
    let tested = warn_untested(directives, &target);
    check_success(host, &command("ninja", &["--version"]))?;
    let layout = Layout::new(&out);
    let configure_args = configure_arguments(&layout, &platform, &source.build_tag);
    let toolchain = toolchain(host, &target, &platform)?;
    let inputs = entry_inputs(host, &source, &target, &toolchain, &configure_args)?;
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
    let state = entry_state(host, &entry, &inputs, &source, &platform)?;
    if state != EntryState::Complete {
        let build = Build {
            layout: &layout,
            source: &source,
            platform: &platform,
            toolchain: &toolchain,
            configure_args: &configure_args,
            inputs: &inputs,
        };
        build_slang(host, &build)?;
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
    let include = entry.join("include");
    let lib = entry.join("lib");
    generate_files(
        host,
        &out,
        &include,
        &target,
        &source.build_tag,
        &build_info(&source.build_tag, Some(&source.commit), tested),
    )?;
    directives.push(format!("cargo::rustc-link-search=native={}", lib.display()));
    for library in STATIC_LIBRARIES {
        directives.push(format!("cargo::rustc-link-lib=static={library}"));
    }
    for library in platform.cxx_library.iter().chain(platform.system_libraries) {
        directives.push(format!("cargo::rustc-link-lib=dylib={library}"));
    }
    push_metadata(directives, &include, &lib, "static", &source.build_tag);
    Ok(())
}

fn push_metadata(
    directives: &mut Vec<String>,
    include: &Path,
    lib: &Path,
    linkage: &str,
    build_tag: &str,
) {
    directives.push(format!("cargo::metadata=include={}", include.display()));
    directives.push(format!("cargo::metadata=lib={}", lib.display()));
    directives.push(format!("cargo::metadata=linkage={linkage}"));
    directives.push(format!("cargo::metadata=build_tag={build_tag}"));
}

fn warn_untested(directives: &mut Vec<String>, target: &str) -> bool {
    let tested = TESTED_TARGETS.contains(&target);
    if !tested {
        directives.push(format!(
            "cargo::warning=slang-compiler-sys is untested and unsupported on the target {target}. It is tested on {}, and a build for another 64-bit target goes on but may fail or misbehave.",
            TESTED_TARGETS.join(", ")
        ));
    }
    tested
}

fn generate_files(
    host: &impl Host,
    out: &Path,
    include: &Path,
    target: &str,
    build_tag: &str,
    info: &str,
) -> Result<(), BuildError> {
    let generated = host
        .generate(include, target)
        .map_err(|error| generation_error(include, target, build_tag, &error))?;
    write_generated(host, &out.join(BINDINGS_FILE), &generated.bindings)?;
    write_generated(host, &out.join(SYMBOLS_FILE), &generated.symbols)?;
    write_generated(host, &out.join(BUILD_INFO_FILE), info)
}

fn build_info(build_tag: &str, commit: Option<&str>, tested: bool) -> String {
    let (linkage, route) = if commit.is_some() {
        ("Static", "vendored")
    } else {
        ("Dynamic", "dynamic")
    };
    format!(
        "BuildInfo {{ slang_build_tag: {build_tag:?}, slang_commit: {commit:?}, linkage: Linkage::{linkage}, tested_target: {tested}, version: {:?} }}\n",
        version(build_tag, route)
    )
}

fn version(build_tag: &str, route: &str) -> String {
    let slang = slang_identifiers(build_tag).unwrap_or_else(|| {
        build_tag
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' {
                    c
                } else {
                    '-'
                }
            })
            .collect()
    });
    format!("{CRATE_VERSION}+slang.{slang}.{route}")
}

fn slang_identifiers(build_tag: &str) -> Option<String> {
    let (release, described) = match build_tag.split_once('-') {
        Some((release, rest)) => {
            let (count, hash) = rest.split_once("-g")?;
            if !is_number(count) || hash.len() < 9 || !is_lowercase_hex(hash) {
                return None;
            }
            (release, Some((count, hash)))
        }
        None => (build_tag, None),
    };
    let numbers: Vec<&str> = release.split('.').collect();
    let (major, minor, patch) = match numbers.as_slice() {
        [major, minor] => (*major, *minor, "0"),
        [major, minor, patch] => (*major, *minor, *patch),
        _ => return None,
    };
    if ![major, minor, patch].into_iter().all(is_number) {
        return None;
    }
    Some(match described {
        Some((count, hash)) => format!("{major}.{minor}.{patch}.{count}.g{hash}"),
        None => format!("{major}.{minor}.{patch}.0"),
    })
}

fn is_number(text: &str) -> bool {
    text == "0"
        || (!text.is_empty()
            && !text.starts_with('0')
            && text.bytes().all(|byte| byte.is_ascii_digit()))
}

fn is_lowercase_hex(text: &str) -> bool {
    text.bytes()
        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn check_lib(host: &impl Host, sdk: &Path, lib: &Path) -> Result<(), BuildError> {
    let fail = |path: &Path, want: Wanted, error: KindError| {
        lib_error(sdk, lib, &kind_problem(path, want, error))
    };
    expect_kind(host, lib, Wanted::Directory)
        .map_err(|error| fail(lib, Wanted::Directory, error))?;
    let target_os = host.var(TARGET_OS);
    let Some(name) = target_os
        .as_ref()
        .and_then(|os| os.to_str())
        .filter(|os| *os == host.host_os())
        .and_then(library_file)
    else {
        return Ok(());
    };
    let file = lib.join(name);
    expect_kind(host, &file, Wanted::File).map_err(|error| fail(&file, Wanted::File, error))
}

fn expect_kind(host: &impl Host, path: &Path, want: Wanted) -> Result<(), KindError> {
    match host.kind(path) {
        Ok(kind) if kind == want.kind() => Ok(()),
        Ok(_) => Err(KindError::Wrong),
        Err(error) => Err(KindError::Inaccessible(error)),
    }
}

fn kind_problem(path: &Path, want: Wanted, error: KindError) -> String {
    match error {
        KindError::Wrong => format!("{} is not a {}", path.display(), want.noun()),
        KindError::Inaccessible(error) => format!("cannot access {}: {error}", path.display()),
    }
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

fn library_file(target_os: &str) -> Option<&'static str> {
    match target_os {
        "linux" => Some("libslang-compiler.so"),
        "macos" => Some("libslang-compiler.dylib"),
        "windows" => Some("slang-compiler.lib"),
        _ => None,
    }
}

fn sdk_dir(value: Option<OsString>) -> Result<PathBuf, BuildError> {
    let Some(value) = value else {
        return Err(unset_error("is not set"));
    };
    if value.is_empty() {
        return Err(unset_error("is set but empty"));
    }
    directive_safe_path(&value).ok_or_else(|| {
        BuildError(format!(
            "{}, so the build cannot read the Slang headers that it names and pass its library to the linker. {}.",
            path_problem(VULKAN_SDK, &value),
            sdk_advice()
        ))
    })
}

fn out_dir(value: Option<&OsStr>, purpose: &str) -> Result<PathBuf, BuildError> {
    let problem = match value {
        None => format!("{OUT_DIR} is not set"),
        Some(value) => match directive_safe_path(value) {
            Some(path) => return Ok(path),
            None => path_problem(OUT_DIR, value),
        },
    };
    Err(BuildError(format!(
        "{problem}, so the build has no directory in which to {purpose}. Cargo sets {OUT_DIR}, so build the crate through Cargo, with a target directory whose path is UTF-8."
    )))
}

fn target(value: Option<&OsStr>) -> Result<String, BuildError> {
    let problem = match value {
        None => format!("{TARGET} is not set"),
        Some(value) => match value.to_str() {
            Some(target) if !target.is_empty() => return Ok(target.to_owned()),
            _ => format!(
                "{TARGET} is {}, which is not a target triple",
                quoted(value)
            ),
        },
    };
    Err(BuildError(format!(
        "{problem}, so the build cannot generate the Slang bindings for the target of the build. Cargo sets {TARGET}, so build the crate through Cargo."
    )))
}

fn check_pointer_width(target: &str, value: Option<OsString>) -> Result<(), BuildError> {
    if value.as_deref() == Some(OsStr::new("64")) {
        return Ok(());
    }
    let width = value.map_or_else(
        || format!("no pointer width, because {POINTER_WIDTH} is not set"),
        |width| format!("a pointer width of {} bits", quoted(&width)),
    );
    Err(BuildError(format!(
        "the target {target} has {width}, but the Slang bindings support only 64-bit targets, because a 32-bit target has other struct layouts and calling conventions."
    )))
}

fn parse_tag(text: &str) -> Result<&str, TagError> {
    let mut found = None;
    for (index, line) in text.lines().enumerate() {
        let Some(rest) = tag_definition(line) else {
            continue;
        };
        let tag = rest
            .strip_prefix('"')
            .and_then(|rest| rest.strip_suffix('"'))
            .filter(|tag| !tag.is_empty() && !tag.contains(|c: char| c == '"' || c.is_control()));
        match tag {
            Some(tag) => found = Some(tag),
            None => return Err(TagError::Malformed { line: index + 1 }),
        }
    }
    found.ok_or(TagError::Missing)
}

fn tag_definition(line: &str) -> Option<&str> {
    let rest = line.trim_start().strip_prefix("#define")?;
    if !rest.starts_with(char::is_whitespace) {
        return None;
    }
    let rest = rest.trim_start().strip_prefix(TAG_MACRO)?;
    if !rest.is_empty() && !rest.starts_with(char::is_whitespace) {
        return None;
    }
    Some(rest.trim())
}

fn read_source(host: &impl Host) -> Result<Source, BuildError> {
    let root = metadata_path(host, SRC_ROOT)?;
    let commit = metadata_text(
        host,
        SRC_COMMIT,
        "a full commit hash of 40 lowercase hexadecimal digits",
        |value| value.len() == 40 && is_lowercase_hex(value),
    )?;
    let build_tag = metadata_text(
        host,
        SRC_BUILD_TAG,
        "a build tag of the form <major>.<minor>, optionally followed by .<patch> and by -<count>-g<hash> with a hash of at least nine lowercase hexadecimal digits",
        |value| slang_identifiers(value).is_some(),
    )?;
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

fn metadata_text(
    host: &impl Host,
    name: &str,
    form: &str,
    valid: impl Fn(&str) -> bool,
) -> Result<String, BuildError> {
    let value = host.var(name);
    value
        .as_deref()
        .and_then(OsStr::to_str)
        .filter(|text| valid(text))
        .map(str::to_owned)
        .ok_or_else(|| {
            metadata_error(&value.map_or_else(
                || format!("{name} is not set"),
                |value| format!("{name} is {}, which is not {form}", quoted(&value)),
            ))
        })
}

fn metadata_error(problem: &str) -> BuildError {
    BuildError(format!(
        "{problem}, so the build of Slang from source has no valid source tree. The build reads the Slang source tree and the shared folder from the metadata of slang-src, which Cargo passes only when slang-src is a direct dependency of the version that slang-compiler-sys requires, as the Cargo feature `vendored` makes it."
    ))
}

fn required(host: &impl Host, name: &str) -> Result<String, BuildError> {
    host.var(name)
        .map(|value| value.to_string_lossy().into_owned())
        .ok_or_else(|| {
            BuildError(format!(
                "{name} is not set, so the build of Slang from source cannot compare the target with the host. Cargo sets {name}, so build the crate through Cargo."
            ))
        })
}

fn platform(host: &impl Host, target: &str, host_triple: &str) -> Result<Platform, BuildError> {
    if target != host_triple {
        return Err(BuildError(format!(
            "the target {target} differs from the host {host_triple}, and the build of Slang from source cannot cross-compile, because Slang's build runs programs that it builds for the target. Build on a host of the target, or {DYNAMIC_REMEDY}."
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
            configure_options: vec![
                format!(
                    "-DCMAKE_MSVC_RUNTIME_LIBRARY={}",
                    if crt_static {
                        "MultiThreaded"
                    } else {
                        "MultiThreadedDLL"
                    }
                ),
                "-DSLANG_ENABLE_SPIRV_TOOLS_MIMALLOC=OFF".to_owned(),
            ],
        }),
        _ => Err(BuildError(format!(
            "the target {target} has no link rule for its C++ runtime library, so the build of Slang from source cannot link Slang's C++ code for it. The build has a link rule for Linux with the GNU C library, macOS, and Windows with the Microsoft toolchain. Build on a host of one of these targets, or {DYNAMIC_REMEDY}."
        ))),
    }
}

fn command(program: &str, args: &[&str]) -> Invocation {
    Invocation {
        program: program.to_owned(),
        args: args.iter().map(|arg| (*arg).to_owned()).collect(),
        dir: None,
        env: Vec::new(),
        env_remove: Vec::new(),
    }
}

fn git(dir: &Path, args: &[&str]) -> Invocation {
    Invocation {
        dir: Some(dir.to_path_buf()),
        env: vec![(OsString::from("GIT_OPTIONAL_LOCKS"), OsString::from("0"))],
        env_remove: GIT_REPOSITORY_VARIABLES.to_vec(),
        ..command("git", args)
    }
}

fn cmake(args: Vec<String>, toolchain: &Toolchain) -> Invocation {
    Invocation {
        args,
        env: toolchain.env.clone(),
        env_remove: vec!["SLANG_USE_SCCACHE"],
        ..command("cmake", &[])
    }
}

fn numeric_version(tag: &str) -> &str {
    tag.split_once('-').map_or(tag, |(numeric, _)| numeric)
}

fn configure_arguments(layout: &Layout, platform: &Platform, build_tag: &str) -> Vec<String> {
    let mut args = vec![
        format!("{ARCHIVE_OUTPUT_OPTION}{}", layout.lib.display()),
        format!("-DSLANG_VERSION_FULL={build_tag}"),
        format!("-DSLANG_VERSION_NUMERIC={}", numeric_version(build_tag)),
        "-DFETCHCONTENT_FULLY_DISCONNECTED=ON".to_owned(),
    ];
    args.extend(SLANG_OPTIONS.iter().map(|option| (*option).to_owned()));
    args.extend(platform.configure_options.iter().cloned());
    args
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
        "cannot find the Microsoft C++ compiler cl.exe for the target {target}. The build of Slang from source runs CMake with Ninja, which needs the environment of the Microsoft toolchain, so the build script looked for cl.exe first in the environment of a developer command prompt, which VCINSTALLDIR marks and which is {}, and then in every installation of Visual Studio 2015 or later that the Visual Studio setup configuration or the registry lists, limited to the toolset that VCToolsVersion names, which is {}, and found none. Install Visual Studio or its Build Tools with the workload for desktop development with C++, or {DYNAMIC_REMEDY}.",
        shown(MSVC_DEVELOPER_PROMPT),
        shown(MSVC_TOOLS_VERSION)
    ))
}

fn entry_inputs(
    host: &impl Host,
    source: &Source,
    target: &str,
    toolchain: &Toolchain,
    configure_args: &[String],
) -> Result<String, BuildError> {
    let cmake_version = first_line(host, &command("cmake", &["--version"]))?;
    let mut lines = vec![
        format!("crate={CRATE_NAME} {CRATE_VERSION}"),
        "files=".to_owned(),
        format!("commit={}", source.commit),
        format!("build_tag={}", source.build_tag),
        format!("target={target}"),
        format!("cmake={cmake_version}"),
    ];
    lines.extend(
        configure_args
            .iter()
            .filter(|arg| !arg.starts_with(ARCHIVE_OUTPUT_OPTION))
            .map(|arg| format!("configure={arg}")),
    );
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
    source: &Source,
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
    let include = entry.join("include");
    let complete = same_inputs
        && STATIC_LIBRARIES.iter().all(|library| {
            matches!(
                host.kind(&lib.join(platform.archive(library))),
                Ok(EntryKind::File)
            )
        })
        && entry_headers_complete(host, &include, source)?;
    Ok(if complete {
        EntryState::Complete
    } else {
        EntryState::Unusable
    })
}

fn entry_headers_complete(
    host: &impl Host,
    include: &Path,
    source: &Source,
) -> Result<bool, BuildError> {
    let headers = source.root.join("include");
    let files = host
        .files(&headers)
        .map_err(|error| shared_error("list the files of", &headers, &error))?;
    let copied = files
        .iter()
        .all(|file| matches!(host.kind(&include.join(file)), Ok(EntryKind::File)));
    let tagged = host
        .read(&include.join("slang-tag-version.h"))
        .is_ok_and(|text| parse_tag(&text) == Ok(source.build_tag.as_str()));
    Ok(copied && tagged)
}

struct Build<'a> {
    layout: &'a Layout,
    source: &'a Source,
    platform: &'a Platform,
    toolchain: &'a Toolchain,
    configure_args: &'a [String],
    inputs: &'a str,
}

fn build_slang(host: &impl Host, build: &Build<'_>) -> Result<(), BuildError> {
    let Build {
        layout,
        source,
        platform,
        toolchain,
        configure_args,
        inputs,
    } = *build;
    write_file(host, &layout.log, "")?;
    let stamp = stamp_text(source, configure_args, inputs);
    let fresh = host
        .read(&layout.stamp)
        .is_ok_and(|previous| previous == stamp);
    if !fresh {
        remove_if_present(host, &layout.stamp)?;
        remove_if_present(host, &layout.build.join("CMakeCache.txt"))?;
        let files = source_files(host, &source.root)?;
        sync_copy(host, &source.root, &layout.copy, &files)?;
        let mut args = vec![
            "-S".to_owned(),
            layout.copy.display().to_string(),
            "-B".to_owned(),
            layout.build.display().to_string(),
            "-G".to_owned(),
            "Ninja".to_owned(),
        ];
        args.extend(configure_args.iter().cloned());
        run_logged(host, &cmake(args, toolchain), &layout.log, "configure")?;
    }
    let mut build_args = vec![
        "--build".to_owned(),
        layout.build.display().to_string(),
        "--target".to_owned(),
        "slang".to_owned(),
    ];
    if let Some(jobs) = host
        .var(NUM_JOBS)
        .and_then(|jobs| jobs.to_str()?.parse::<u32>().ok())
    {
        build_args.extend(["--parallel".to_owned(), jobs.to_string()]);
    }
    run_logged(host, &cmake(build_args, toolchain), &layout.log, "build")?;
    verify_build(host, layout, platform, &source.build_tag)?;
    if !fresh {
        write_file(host, &layout.stamp, &stamp)?;
    }
    Ok(())
}

fn stamp_text(source: &Source, configure_args: &[String], inputs: &str) -> String {
    format!(
        "source {}\ncommit {}\nbuild_tag {}\nsubmodules {}\nconfigure {}\n{inputs}",
        source.root.display(),
        source.commit,
        source.build_tag,
        REQUIRED_SUBMODULES.join(" "),
        configure_args.join(" ")
    )
}

fn source_files(host: &impl Host, root: &Path) -> Result<Vec<PathBuf>, BuildError> {
    let mut files = tracked(host, root, Path::new(""))?;
    for submodule in REQUIRED_SUBMODULES {
        files.extend(tracked(host, &root.join(submodule), Path::new(submodule))?);
    }
    Ok(files)
}

fn tracked(host: &impl Host, dir: &Path, prefix: &Path) -> Result<Vec<PathBuf>, BuildError> {
    let list = git(dir, &["ls-files", "-z", "--stage"]);
    let finished = check_success(host, &list)?;
    let mut files = Vec::new();
    for entry in finished
        .stdout
        .split('\0')
        .filter(|entry| !entry.is_empty())
    {
        let parsed = entry.split_once('\t').and_then(|(meta, path)| {
            let normal = Path::new(path)
                .components()
                .all(|component| matches!(component, Component::Normal(_)));
            normal.then(|| (meta.split(' ').next(), path))
        });
        match parsed {
            Some((Some("160000"), _)) => {}
            Some((Some(_), path)) => files.push(prefix.join(path)),
            _ => return Err(unexpected_output_error(&list, dir, entry)),
        }
    }
    Ok(files)
}

fn sync_copy(
    host: &impl Host,
    source: &Path,
    copy: &Path,
    files: &[PathBuf],
) -> Result<(), BuildError> {
    let existing = match host.files(copy) {
        Ok(existing) => existing,
        Err(error) if error.kind() == io::ErrorKind::NotFound => Vec::new(),
        Err(error) => return Err(file_error("list the files of", copy, &error)),
    };
    let wanted: HashSet<&PathBuf> = files.iter().collect();
    for stale in existing.iter().filter(|file| !wanted.contains(file)) {
        let path = copy.join(stale);
        host.remove_file(&path)
            .map_err(|error| file_error("remove", &path, &error))?;
    }
    for file in files {
        let (from, to) = (source.join(file), copy.join(file));
        host.sync_file(&from, &to).map_err(|error| {
            BuildError(format!(
                "cannot copy {} to {}: {error}. The build copies the tracked files of the Slang source tree into {OUT_DIR} and builds the copy, because Slang's build writes into its source tree.",
                from.display(),
                to.display()
            ))
        })?;
    }
    Ok(())
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
        "`{invocation}` failed with {}, so the {step} step of the build of Slang from source failed. The full output is in {}, and it ends with these lines:\n{tail}",
        exit.status(),
        log.display()
    )))
}

fn verify_build(
    host: &impl Host,
    layout: &Layout,
    platform: &Platform,
    build_tag: &str,
) -> Result<(), BuildError> {
    for library in STATIC_LIBRARIES {
        let file = layout.lib.join(platform.archive(library));
        expect_kind(host, &file, Wanted::File)
            .map_err(|error| layout_error(&kind_problem(&file, Wanted::File, error)))?;
    }
    let header = layout.include.join("slang-tag-version.h");
    let text = host
        .read(&header)
        .map_err(|error| layout_error(&format!("cannot read {}: {error}", header.display())))?;
    let tag = parse_tag(&text).map_err(|error| layout_error(&tag_problem(&header, &error)))?;
    if tag != build_tag {
        return Err(BuildError(format!(
            "the Slang built from source reports the build tag {tag} in {}, but slang-src names the build tag {build_tag}, which the bindings and the build information name.",
            header.display()
        )));
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
    let mut copies: Vec<(PathBuf, PathBuf)> = STATIC_LIBRARIES
        .iter()
        .map(|library| {
            let name = platform.archive(library);
            (layout.lib.join(&name), temporary.join("lib").join(&name))
        })
        .collect();
    let headers = host
        .files(&layout.include)
        .map_err(|error| file_error("list the files of", &layout.include, &error))?;
    copies.extend(headers.iter().map(|file| {
        (
            layout.include.join(file),
            temporary.join("include").join(file),
        )
    }));
    for (from, to) in &copies {
        host.sync_file(from, to)
            .map_err(|error| shared_error(&format!("copy {} to", from.display()), to, &error))?;
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

fn check_success(host: &impl Host, invocation: &Invocation) -> Result<Finished, BuildError> {
    let finished = host
        .run(invocation)
        .map_err(|error| start_error(invocation, &error))?;
    if finished.exit.success() {
        return Ok(finished);
    }
    Err(BuildError(format!(
        "`{invocation}`{} failed with {}: {}",
        invocation
            .dir
            .as_ref()
            .map_or_default(|dir| format!(" in {}", dir.display())),
        finished.exit.status(),
        finished.stderr.trim()
    )))
}

fn start_error(invocation: &Invocation, error: &io::Error) -> BuildError {
    BuildError(format!(
        "cannot start {} for `{invocation}`: {error}. The build of Slang from source needs git, CMake 3.22 or later, and Ninja on PATH, and a C++ compiler. Install the missing program, or {DYNAMIC_REMEDY}.",
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

fn write_generated(host: &impl Host, path: &Path, text: &str) -> Result<(), BuildError> {
    host.write(path, text).map_err(|error| {
        BuildError(format!(
            "cannot write {}: {error}. The build writes the generated Slang bindings and the build information under {OUT_DIR}.",
            path.display()
        ))
    })
}

fn unexpected_output_error(invocation: &Invocation, dir: &Path, output: &str) -> BuildError {
    BuildError(format!(
        "`{invocation}` in {} printed \"{}\", which is not the output that the build expects from git, so the build cannot list the files of the Slang source tree to copy.",
        dir.display(),
        output.escape_debug()
    ))
}

fn file_error(action: &str, path: &Path, error: &io::Error) -> BuildError {
    BuildError(format!(
        "cannot {action} {}: {error}. The build of Slang from source keeps its copy of the source tree, its build tree, and its log under {OUT_DIR}.",
        path.display()
    ))
}

fn shared_error(action: &str, path: &Path, error: &io::Error) -> BuildError {
    BuildError(format!(
        "cannot {action} {}: {error}. The build keeps the libraries of Slang that it built in the shared folder of the Cargo target directory, and removing the entry, or running cargo clean, when no build runs is safe.",
        path.display()
    ))
}

fn layout_error(problem: &str) -> BuildError {
    BuildError(format!(
        "the build of Slang from source finished, but {problem}, so the build produced another layout than the one that this version of slang-compiler-sys expects."
    ))
}

fn sdk_advice() -> String {
    format!(
        "Install the LunarG Vulkan SDK from https://vulkan.lunarg.com, and set {VULKAN_SDK} to the absolute path of its directory that holds include/slang and lib, which is the x86_64 directory of the SDK on Linux, the macOS directory on macOS, and the installation directory on Windows"
    )
}

fn unset_error(state: &str) -> BuildError {
    BuildError(format!(
        "{VULKAN_SDK} {state}, so slang-compiler-sys cannot find the Slang library to link. {}. {VENDORED_ADVICE}.",
        sdk_advice()
    ))
}

fn unreadable_header_error(path: &Path, error: &io::Error) -> BuildError {
    BuildError(format!(
        "cannot read {}: {error}. The build reads this file to record the build tag of the Slang of the Vulkan SDK that {VULKAN_SDK} names, which the program compares with the Slang library that it loads. {}.",
        path.display(),
        sdk_advice()
    ))
}

fn tag_problem(path: &Path, error: &TagError) -> String {
    match error {
        TagError::Missing => format!("{} does not define {TAG_MACRO}", path.display()),
        TagError::Malformed { line } => format!(
            "line {line} of {} does not define {TAG_MACRO} as one non-empty string literal without quotation marks and control characters",
            path.display()
        ),
    }
}

fn tag_error(path: &Path, error: &TagError) -> BuildError {
    BuildError(format!(
        "{}, so the build cannot record the build tag of the Slang of the Vulkan SDK that {VULKAN_SDK} names. {}.",
        tag_problem(path, error),
        sdk_advice()
    ))
}

fn lib_error(sdk: &Path, lib: &Path, problem: &str) -> BuildError {
    BuildError(format!(
        "{problem}, so the build cannot link the Slang library of the Vulkan SDK at {}, which it expects in {} as libslang-compiler.so on Linux, libslang-compiler.dylib on macOS, and the import library slang-compiler.lib on Windows. {}.",
        sdk.display(),
        lib.display(),
        sdk_advice()
    ))
}

fn generation_error(
    include: &Path,
    target: &str,
    build_tag: &str,
    error: &GenerationError,
) -> BuildError {
    let advice = match error.kind {
        GenerationErrorKind::Libclang => format!(
            " slang-compiler-sys generates its bindings with libclang in every build, so install libclang or set {LIBCLANG_PATH} to the directory that holds it."
        ),
        GenerationErrorKind::Target => format!(
            " libclang does not accept the target triple {target}, so the bindings cannot be generated for it. slang-compiler-sys is tested only on {}, and a target that libclang does not know cannot be built.",
            TESTED_TARGETS.join(", ")
        ),
        GenerationErrorKind::Headers => format!(
            " The headers are those of the Slang build {build_tag}. slang-compiler-sys is written and tested against the Slang that the version of slang-src that it requires records, and another Slang is best effort, so the rules of its binding generator can refuse the headers of another Slang. {}, for the SDK that ships the recorded Slang. {VENDORED_ADVICE}.",
            sdk_advice()
        ),
        GenerationErrorKind::Other => String::new(),
    };
    BuildError(format!(
        "cannot generate the Slang bindings from {} for the target {target}: {}.{advice}",
        include.display(),
        error.message
    ))
}
