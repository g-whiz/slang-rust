use std::collections::HashSet;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::io;
use std::path::{Component, Path, PathBuf};

const DOCS_RS: &str = "DOCS_RS";
const SLANG_SOURCE_DIR: &str = "SLANG_SOURCE_DIR";
const VULKAN_SDK: &str = "VULKAN_SDK";
const TAG_MACRO: &str = "SLANG_TAG_VERSION";
const TARGET_OS: &str = "CARGO_CFG_TARGET_OS";
const TARGET_ENV: &str = "CARGO_CFG_TARGET_ENV";
const TARGET_FEATURE: &str = "CARGO_CFG_TARGET_FEATURE";
const TARGET: &str = "TARGET";
const HOST: &str = "HOST";
const OUT_DIR: &str = "OUT_DIR";
const NUM_JOBS: &str = "NUM_JOBS";

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
const STATUS_LINES: usize = 10;

trait Host {
    fn var(&self, name: &str) -> Option<OsString>;
    fn read(&self, path: &Path) -> io::Result<String>;
    fn kind(&self, path: &Path) -> io::Result<EntryKind>;
    fn host_os(&self) -> &str;
    fn run(&self, invocation: &Invocation) -> io::Result<Finished>;
    fn run_to_log(&self, invocation: &Invocation, log: &Path) -> Result<Exit, LoggedRunError>;
    fn files(&self, root: &Path) -> io::Result<Vec<PathBuf>>;
    fn sync_file(&self, from: &Path, to: &Path) -> io::Result<()>;
    fn remove_file(&self, path: &Path) -> io::Result<()>;
    fn write(&self, path: &Path, text: &str) -> io::Result<()>;
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
struct Invocation {
    program: &'static str,
    args: Vec<String>,
    dir: Option<PathBuf>,
    env: Vec<(&'static str, &'static str)>,
    env_remove: Vec<&'static str>,
}

impl fmt::Display for Invocation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.program)?;
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

struct Platform {
    msvc: bool,
    cxx_library: Option<&'static str>,
    configure_options: Vec<String>,
}

struct Layout {
    copy: PathBuf,
    build: PathBuf,
    lib: PathBuf,
    include: PathBuf,
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

fn configure(host: &impl Host, directives: &mut Vec<String>) -> Result<(), BuildError> {
    directives.push(format!("cargo::rerun-if-env-changed={DOCS_RS}"));
    if host.var(DOCS_RS).is_some() {
        return Ok(());
    }
    directives.push(format!("cargo::rerun-if-env-changed={SLANG_SOURCE_DIR}"));
    if let Some(value) = host.var(SLANG_SOURCE_DIR) {
        return static_route(host, directives, &value);
    }
    directives.push(format!("cargo::rerun-if-env-changed={VULKAN_SDK}"));
    let sdk = sdk_dir(host.var(VULKAN_SDK))?;
    let include = sdk.join("include");
    let tag_header = include.join("slang").join("slang-tag-version.h");
    directives.push(format!("cargo::rerun-if-changed={}", tag_header.display()));
    let text = host
        .read(&tag_header)
        .map_err(|error| unreadable_header_error(&tag_header, &error))?;
    let tag = parse_tag(&text).map_err(|error| tag_error(&tag_header, &error))?;
    if tag != SLANG_BUILD_TAG {
        return Err(mismatch_error(&sdk, &tag_header, tag));
    }
    let lib = sdk.join("lib");
    check_lib(host, &sdk, &lib)?;
    directives.push(format!("cargo::rustc-link-search=native={}", lib.display()));
    directives.push("cargo::rustc-link-lib=dylib=slang-compiler".to_owned());
    directives.push(format!("cargo::metadata=include={}", include.display()));
    directives.push(format!("cargo::metadata=lib={}", lib.display()));
    Ok(())
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

fn path_problem(name: &str, value: Option<&OsStr>) -> String {
    let shown = value.map_or_else(
        || "not set".to_owned(),
        |value| format!("\"{}\"", value.to_string_lossy().escape_debug()),
    );
    format!("{name} is {shown}, which is not an absolute path in UTF-8 without line breaks")
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
            "{}, so the build cannot check the Slang that it names and pass its library to the linker. {}.",
            path_problem(VULKAN_SDK, Some(&value)),
            sdk_advice()
        ))
    })
}

fn parse_tag(text: &str) -> Result<&str, TagError> {
    let mut found = None;
    for (index, line) in text.lines().enumerate() {
        let mut words = line.split_whitespace();
        if words.next() != Some("#define") || words.next() != Some(TAG_MACRO) {
            continue;
        }
        let value = words.next().and_then(|word| {
            word.strip_prefix('"')?
                .strip_suffix('"')
                .filter(|tag| !tag.contains('"'))
        });
        match value {
            Some(tag) if words.next().is_none() => found = Some(tag),
            _ => return Err(TagError::Malformed { line: index + 1 }),
        }
    }
    found.ok_or(TagError::Missing)
}

fn static_route(
    host: &impl Host,
    directives: &mut Vec<String>,
    value: &OsStr,
) -> Result<(), BuildError> {
    let source = source_dir(value)?;
    expect_kind(host, &source, Wanted::Directory).map_err(|error| match error {
        KindError::Wrong => source_error(&source, "which is not a directory"),
        KindError::Inaccessible(error) => {
            source_error(&source, &format!("which cannot be accessed: {error}"))
        }
    })?;
    let out = out_dir(host.var(OUT_DIR).as_deref())?;
    let platform = static_platform(host)?;
    let commit = verify_checkout(host, directives, &source)?;
    verify_unmodified(host, &source)?;
    let layout = Layout::new(&out);
    let configure_args = configure_arguments(&layout, &platform);
    let stamp = stamp_text(&source, &commit, &configure_args);
    let ninja = Invocation {
        program: "ninja",
        args: vec!["--version".to_owned()],
        ..invocation()
    };
    check_success(host, &ninja, &source)?;
    write_file(host, &layout.log, "")?;
    let fresh = host
        .read(&layout.stamp)
        .is_ok_and(|previous| previous == stamp);
    if !fresh {
        remove_if_present(host, &layout.stamp)?;
        let files = source_files(host, &source)?;
        sync_copy(host, &source, &layout.copy, &files)?;
        let configure = Invocation {
            program: "cmake",
            args: configure_args,
            env_remove: vec!["SLANG_USE_SCCACHE"],
            ..invocation()
        };
        run_logged(host, &configure, &layout.log, "configure")?;
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
    let build = Invocation {
        program: "cmake",
        args: build_args,
        env_remove: vec!["SLANG_USE_SCCACHE"],
        ..invocation()
    };
    run_logged(host, &build, &layout.log, "build")?;
    verify_build(host, &layout, &platform)?;
    if !fresh {
        write_file(host, &layout.stamp, &stamp)?;
    }
    directives.push(format!(
        "cargo::rustc-link-search=native={}",
        layout.lib.display()
    ));
    for library in STATIC_LIBRARIES {
        directives.push(format!("cargo::rustc-link-lib=static={library}"));
    }
    if let Some(library) = platform.cxx_library {
        directives.push(format!("cargo::rustc-link-lib=dylib={library}"));
    }
    directives.push(format!(
        "cargo::metadata=include={}",
        layout.include.display()
    ));
    directives.push(format!("cargo::metadata=lib={}", layout.lib.display()));
    Ok(())
}

fn invocation() -> Invocation {
    Invocation {
        program: "",
        args: Vec::new(),
        dir: None,
        env: Vec::new(),
        env_remove: Vec::new(),
    }
}

fn git(dir: &Path, args: &[&str]) -> Invocation {
    Invocation {
        program: "git",
        args: args.iter().map(|arg| (*arg).to_owned()).collect(),
        dir: Some(dir.to_path_buf()),
        env: vec![("GIT_OPTIONAL_LOCKS", "0")],
        env_remove: GIT_REPOSITORY_VARIABLES.to_vec(),
    }
}

fn source_dir(value: &OsStr) -> Result<PathBuf, BuildError> {
    if value.is_empty() {
        return Err(BuildError(format!(
            "{SLANG_SOURCE_DIR} is set but empty, which selects building Slang from source without naming a source tree. {}.",
            source_advice()
        )));
    }
    directive_safe_path(value).ok_or_else(|| {
        BuildError(format!(
            "{}, so the build cannot check the Slang source tree that it names and build it. {}.",
            path_problem(SLANG_SOURCE_DIR, Some(value)),
            source_advice()
        ))
    })
}

fn out_dir(value: Option<&OsStr>) -> Result<PathBuf, BuildError> {
    value.and_then(directive_safe_path).ok_or_else(|| {
        BuildError(format!(
            "{}, so the build has no directory in which to build Slang from source. Cargo sets {OUT_DIR} for every build script, so build slang-compiler-sys through Cargo, with a target directory whose path is UTF-8.",
            path_problem(OUT_DIR, value)
        ))
    })
}

fn static_platform(host: &impl Host) -> Result<Platform, BuildError> {
    let text = |name: &str| {
        host.var(name)
            .map(|value| value.to_string_lossy().into_owned())
            .unwrap_or_default()
    };
    let required = |name: &str| {
        host.var(name)
            .map(|value| value.to_string_lossy().into_owned())
            .ok_or_else(|| {
                BuildError(format!(
                    "{name} is not set, so the build of Slang from source that {SLANG_SOURCE_DIR} selects cannot compare the target with the host. Cargo sets {name} for every build script, so build slang-compiler-sys through Cargo."
                ))
            })
    };
    let target = required(TARGET)?;
    let host_triple = required(HOST)?;
    if target != host_triple {
        return Err(BuildError(format!(
            "the target {target} differs from the host {host_triple}, and the build of Slang from source that {SLANG_SOURCE_DIR} selects cannot cross-compile, because Slang's build runs programs that it builds for the target. Build on a host of the target, or {}.",
            dynamic_fallback()
        )));
    }
    let crt_static = text(TARGET_FEATURE)
        .split(',')
        .any(|feature| feature == "crt-static");
    match (text(TARGET_OS).as_str(), text(TARGET_ENV).as_str()) {
        ("linux", "gnu") => Ok(Platform {
            msvc: false,
            cxx_library: Some("stdc++"),
            configure_options: Vec::new(),
        }),
        ("macos", _) => Ok(Platform {
            msvc: false,
            cxx_library: Some("c++"),
            configure_options: Vec::new(),
        }),
        ("windows", "msvc") => Ok(Platform {
            msvc: true,
            cxx_library: None,
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
            "the build of Slang from source that {SLANG_SOURCE_DIR} selects supports Linux with the GNU C library, macOS, and Windows with the Microsoft toolchain, and the target {target} is none of these. Build for one of them, or {}.",
            dynamic_fallback()
        ))),
    }
}

fn verify_checkout(
    host: &impl Host,
    directives: &mut Vec<String>,
    source: &Path,
) -> Result<String, BuildError> {
    let query = git(
        source,
        &[
            "rev-parse",
            "--is-inside-work-tree",
            "--show-prefix",
            "--git-path",
            "HEAD",
            "--git-path",
            "index",
            "--verify",
            "HEAD^{commit}",
        ],
    );
    let finished = start(host, &query, Some(source))?;
    if !finished.exit.success() {
        return Err(BuildError(format!(
            "`{query}` in {} failed with {}: {}. {SLANG_SOURCE_DIR} has to name the top level of a git checkout of Slang, because the build verifies the commit of the tree through git, and a tree without git metadata, such as a source archive, cannot be verified. {}.",
            source.display(),
            finished.exit.status(),
            finished.stderr.trim(),
            source_advice()
        )));
    }
    let lines: Vec<&str> = finished.stdout.lines().collect();
    let [inside, prefix, head_file, index_file, head] = lines[..] else {
        return Err(unexpected_output_error(&query, source, &finished.stdout));
    };
    for file in [head_file, index_file] {
        directives.push(format!(
            "cargo::rerun-if-changed={}",
            source.join(file).display()
        ));
    }
    if inside != "true" || !prefix.is_empty() {
        let place = if inside == "true" {
            format!("lies at {prefix} inside a git work tree whose top level is another directory")
        } else {
            "is not inside a git work tree".to_owned()
        };
        return Err(BuildError(format!(
            "{SLANG_SOURCE_DIR} is {}, which {place}, so it is not the top level of a git checkout of Slang, and the build cannot verify its commit. {}.",
            source.display(),
            source_advice()
        )));
    }
    if head != SLANG_COMMIT {
        return Err(BuildError(format!(
            "the Slang source tree at {} is at the commit {head}, but this version of slang-compiler-sys needs the commit {SLANG_COMMIT}, which is the Slang build {SLANG_BUILD_TAG} of the Vulkan SDK {VULKAN_SDK_VERSION}, because its bindings match only that commit. Check out the pinned commit with `git -C {} checkout {SLANG_COMMIT}` followed by `git -C {} submodule update --init --recursive`, or use the version of slang-compiler-sys that matches the tree.",
            source.display(),
            source.display(),
            source.display()
        )));
    }
    Ok(head.to_owned())
}

fn verify_unmodified(host: &impl Host, source: &Path) -> Result<(), BuildError> {
    let status = git(
        source,
        &[
            "status",
            "--porcelain=v1",
            "--untracked-files=no",
            "--ignore-submodules=none",
        ],
    );
    let finished = check_success(host, &status, source)?;
    let changes: Vec<&str> = finished
        .stdout
        .lines()
        .filter(|line| !line.is_empty())
        .collect();
    if changes.is_empty() {
        return Ok(());
    }
    let listed = changes
        .iter()
        .take(STATUS_LINES)
        .map(|line| format!("\"{}\"", line.escape_debug()))
        .collect::<Vec<_>>()
        .join(", ");
    let more = match changes.len().checked_sub(STATUS_LINES) {
        Some(more) if more > 0 => format!(", and {more} more"),
        _ => String::new(),
    };
    Err(BuildError(format!(
        "the Slang source tree at {} has tracked files or submodules that differ from the commit {SLANG_COMMIT}, as `{status}` reports: {listed}{more}. The build copies the tracked files of the tree and builds the copy, and a changed file would build a library that differs from the one that the bindings match. Commit, stash, or discard the changes, or point {SLANG_SOURCE_DIR} at an unchanged checkout of the commit {SLANG_COMMIT}.",
        source.display()
    )))
}

fn source_files(host: &impl Host, source: &Path) -> Result<Vec<PathBuf>, BuildError> {
    let (mut files, submodules) = tracked(host, source, Path::new(""))?;
    for submodule in REQUIRED_SUBMODULES {
        let directory = source.join(submodule);
        let git_entry = directory.join(".git");
        let present = submodules.iter().any(|path| path == submodule)
            && match host.kind(&git_entry) {
                Ok(EntryKind::Directory | EntryKind::File) => true,
                Ok(EntryKind::Other) => false,
                Err(error) if error.kind() == io::ErrorKind::NotFound => false,
                Err(error) => {
                    return Err(BuildError(format!(
                        "cannot access {}: {error}, so the build cannot check that the submodule {submodule} of the Slang source tree at {} is checked out.",
                        git_entry.display(),
                        source.display()
                    )));
                }
            };
        if !present {
            return Err(BuildError(format!(
                "the submodule {submodule} of the Slang source tree at {} is missing or empty, and the build of Slang from source needs it. Fetch it with `git -C {} submodule update --init {submodule}`, which needs network access.",
                source.display(),
                source.display()
            )));
        }
        let (inner, _) = tracked(host, &directory, Path::new(submodule))?;
        files.extend(inner);
    }
    Ok(files)
}

fn tracked(
    host: &impl Host,
    dir: &Path,
    prefix: &Path,
) -> Result<(Vec<PathBuf>, Vec<String>), BuildError> {
    let list = git(dir, &["ls-files", "-z", "--stage"]);
    let finished = check_success(host, &list, dir)?;
    let mut files = Vec::new();
    let mut submodules = Vec::new();
    for entry in finished
        .stdout
        .split('\0')
        .filter(|entry| !entry.is_empty())
    {
        let parsed = entry.split_once('\t').and_then(|(meta, path)| {
            let relative = Path::new(path);
            let normal = relative
                .components()
                .all(|component| matches!(component, Component::Normal(_)));
            normal.then(|| (meta.split(' ').next(), path))
        });
        match parsed {
            Some((Some("160000"), path)) => submodules.push(path.to_owned()),
            Some((Some(_), path)) => files.push(prefix.join(path)),
            _ => return Err(unexpected_output_error(&list, dir, entry)),
        }
    }
    Ok((files, submodules))
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

fn numeric_version(tag: &str) -> &str {
    tag.split_once('-').map_or(tag, |(numeric, _)| numeric)
}

fn configure_arguments(layout: &Layout, platform: &Platform) -> Vec<String> {
    let numeric = numeric_version(SLANG_BUILD_TAG);
    let mut args = vec![
        "-S".to_owned(),
        layout.copy.display().to_string(),
        "-B".to_owned(),
        layout.build.display().to_string(),
        "-G".to_owned(),
        "Ninja".to_owned(),
        format!("-DCMAKE_ARCHIVE_OUTPUT_DIRECTORY={}", layout.lib.display()),
        format!("-DSLANG_VERSION_FULL={SLANG_BUILD_TAG}"),
        format!("-DSLANG_VERSION_NUMERIC={numeric}"),
        "-DFETCHCONTENT_FULLY_DISCONNECTED=ON".to_owned(),
    ];
    args.extend(SLANG_OPTIONS.iter().map(|option| (*option).to_owned()));
    args.extend(platform.configure_options.iter().cloned());
    args
}

fn stamp_text(source: &Path, commit: &str, configure_args: &[String]) -> String {
    format!(
        "source {}\ncommit {commit}\nsubmodules {}\nconfigure {}\n",
        source.display(),
        REQUIRED_SUBMODULES.join(" "),
        configure_args.join(" ")
    )
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
            LoggedRunError::Start(error) => start_error(invocation, None, &error),
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

fn verify_build(host: &impl Host, layout: &Layout, platform: &Platform) -> Result<(), BuildError> {
    for library in STATIC_LIBRARIES {
        let file = layout.lib.join(if platform.msvc {
            format!("{library}.lib")
        } else {
            format!("lib{library}.a")
        });
        expect_kind(host, &file, Wanted::File)
            .map_err(|error| layout_error(&kind_problem(&file, Wanted::File, error)))?;
    }
    let header = layout.include.join("slang-tag-version.h");
    let text = host
        .read(&header)
        .map_err(|error| layout_error(&format!("cannot read {}: {error}", header.display())))?;
    let tag = parse_tag(&text).map_err(|error| layout_error(&tag_problem(&header, &error)))?;
    if tag != SLANG_BUILD_TAG {
        return Err(BuildError(format!(
            "the Slang built from source reports the build tag {tag} in {}, but this version of slang-compiler-sys needs the build {SLANG_BUILD_TAG}, because its bindings match only that build.",
            header.display()
        )));
    }
    Ok(())
}

fn start(
    host: &impl Host,
    invocation: &Invocation,
    source: Option<&Path>,
) -> Result<Finished, BuildError> {
    host.run(invocation)
        .map_err(|error| start_error(invocation, source, &error))
}

fn start_error(invocation: &Invocation, source: Option<&Path>, error: &io::Error) -> BuildError {
    let place = source.map_or_else(String::new, |source| {
        format!(" from the source tree at {}", source.display())
    });
    BuildError(format!(
        "cannot start {} for `{invocation}`: {error}. The build of Slang from source that {SLANG_SOURCE_DIR} selects needs git, CMake 3.22 or later, and Ninja on PATH to build Slang{place}. Install the missing program, or {}.",
        invocation.program,
        dynamic_fallback()
    ))
}

fn check_success(
    host: &impl Host,
    invocation: &Invocation,
    source: &Path,
) -> Result<Finished, BuildError> {
    let finished = start(host, invocation, Some(source))?;
    if finished.exit.success() {
        return Ok(finished);
    }
    Err(BuildError(format!(
        "`{invocation}`{} failed with {}: {}",
        invocation
            .dir
            .as_ref()
            .map(|dir| format!(" in {}", dir.display()))
            .unwrap_or_default(),
        finished.exit.status(),
        finished.stderr.trim()
    )))
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

fn source_advice() -> String {
    format!(
        "Set {SLANG_SOURCE_DIR} to the absolute path of the top-level directory of a git clone of Slang at the commit {SLANG_COMMIT} with its submodules, such as one made with `git clone --recursive https://github.com/shader-slang/slang`, `git checkout {SLANG_COMMIT}`, and `git submodule update --init --recursive`, or {}",
        dynamic_fallback()
    )
}

fn dynamic_fallback() -> String {
    format!(
        "unset {SLANG_SOURCE_DIR} to link the Slang of the Vulkan SDK {VULKAN_SDK_VERSION} that {VULKAN_SDK} names"
    )
}

fn source_error(source: &Path, problem: &str) -> BuildError {
    BuildError(format!(
        "{SLANG_SOURCE_DIR} is {}, {problem}, so the build cannot build Slang from it. {}.",
        source.display(),
        source_advice()
    ))
}

fn unexpected_output_error(invocation: &Invocation, dir: &Path, output: &str) -> BuildError {
    BuildError(format!(
        "`{invocation}` in {} printed \"{}\", which is not the output that the build expects from git, so the build cannot check the Slang source tree.",
        dir.display(),
        output.escape_debug()
    ))
}

fn file_error(action: &str, path: &Path, error: &io::Error) -> BuildError {
    BuildError(format!(
        "cannot {action} {}: {error}. The build of Slang from source keeps its copy of the source tree, its build, and its log under {OUT_DIR}.",
        path.display()
    ))
}

fn layout_error(problem: &str) -> BuildError {
    BuildError(format!(
        "the build of Slang from source finished, but {problem}, so the build produced another layout than the one that this version of slang-compiler-sys expects from the Slang commit {SLANG_COMMIT}."
    ))
}

fn sdk_advice() -> String {
    format!(
        "Install the LunarG Vulkan SDK {VULKAN_SDK_VERSION} from https://vulkan.lunarg.com, and set {VULKAN_SDK} to the absolute path of its directory that holds include/slang and lib, which is the x86_64 directory of the SDK on Linux, the macOS directory on macOS, and the installation directory on Windows"
    )
}

fn unset_error(state: &str) -> BuildError {
    BuildError(format!(
        "{VULKAN_SDK} {state}, so slang-compiler-sys cannot find the Slang library to link. {}. The other way to obtain Slang is to build it from source: set {SLANG_SOURCE_DIR} to the absolute path of a git checkout of Slang at the commit {SLANG_COMMIT}.",
        sdk_advice()
    ))
}

fn unreadable_header_error(path: &Path, error: &io::Error) -> BuildError {
    BuildError(format!(
        "cannot read {}: {error}. The build reads this file to check that the Slang of the Vulkan SDK that {VULKAN_SDK} names is the build that slang-compiler-sys needs. {}.",
        path.display(),
        sdk_advice()
    ))
}

fn tag_problem(path: &Path, error: &TagError) -> String {
    match error {
        TagError::Missing => format!("{} does not define {TAG_MACRO}", path.display()),
        TagError::Malformed { line } => format!(
            "line {line} of {} does not define {TAG_MACRO} as one string literal",
            path.display()
        ),
    }
}

fn tag_error(path: &Path, error: &TagError) -> BuildError {
    BuildError(format!(
        "{}, so the build cannot check that the Slang of the Vulkan SDK that {VULKAN_SDK} names is the build {SLANG_BUILD_TAG} that slang-compiler-sys needs. {}.",
        tag_problem(path, error),
        sdk_advice()
    ))
}

fn mismatch_error(sdk: &Path, path: &Path, tag: &str) -> BuildError {
    BuildError(format!(
        "the Vulkan SDK at {} ships the Slang build {tag}, as {} records, but this version of slang-compiler-sys needs the Slang build {SLANG_BUILD_TAG} of the Vulkan SDK {VULKAN_SDK_VERSION}, because its bindings match only that build. Install the Vulkan SDK {VULKAN_SDK_VERSION} and set {VULKAN_SDK} to it, or use the version of slang-compiler-sys that matches the installed SDK.",
        sdk.display(),
        path.display()
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
