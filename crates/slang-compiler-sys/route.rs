use std::ffi::{OsStr, OsString};
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

const VULKAN_SDK: &str = "VULKAN_SDK";
const LIBCLANG_PATH: &str = "LIBCLANG_PATH";
const TAG_MACRO: &str = "SLANG_TAG_VERSION";
const TARGET_OS: &str = "CARGO_CFG_TARGET_OS";
const POINTER_WIDTH: &str = "CARGO_CFG_TARGET_POINTER_WIDTH";
const TARGET: &str = "TARGET";
const OUT_DIR: &str = "OUT_DIR";
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

trait Host {
    fn var(&self, name: &str) -> Option<OsString>;
    fn read(&self, path: &Path) -> io::Result<String>;
    fn kind(&self, path: &Path) -> io::Result<EntryKind>;
    fn host_os(&self) -> &str;
    fn write(&self, path: &Path, text: &str) -> io::Result<()>;
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

struct Slang {
    include: PathBuf,
    lib: PathBuf,
    build_tag: String,
}

fn configure(host: &impl Host, directives: &mut Vec<String>) -> Result<(), BuildError> {
    directives.push(format!("cargo::rerun-if-env-changed={LIBCLANG_PATH}"));
    let slang = dynamic_route(host, directives)?;
    generate(host, directives, &slang)?;
    directives.push(format!(
        "cargo::rustc-link-search=native={}",
        slang.lib.display()
    ));
    directives.push("cargo::rustc-link-lib=dylib=slang-compiler".to_owned());
    directives.push(format!(
        "cargo::metadata=include={}",
        slang.include.display()
    ));
    directives.push(format!("cargo::metadata=lib={}", slang.lib.display()));
    directives.push("cargo::metadata=linkage=dynamic".to_owned());
    directives.push(format!("cargo::metadata=build_tag={}", slang.build_tag));
    Ok(())
}

fn dynamic_route(host: &impl Host, directives: &mut Vec<String>) -> Result<Slang, BuildError> {
    directives.push(format!("cargo::rerun-if-env-changed={VULKAN_SDK}"));
    let sdk = sdk_dir(host.var(VULKAN_SDK))?;
    let include = sdk.join("include").join("slang");
    directives.push(format!("cargo::rerun-if-changed={}", include.display()));
    let tag_header = include.join("slang-tag-version.h");
    let text = host
        .read(&tag_header)
        .map_err(|error| unreadable_header_error(&tag_header, &error))?;
    let build_tag = parse_tag(&text).map_err(|error| tag_error(&tag_header, &error))?;
    let lib = sdk.join("lib");
    check_lib(host, &sdk, &lib)?;
    Ok(Slang {
        include,
        lib,
        build_tag: build_tag.to_owned(),
    })
}

fn generate(
    host: &impl Host,
    directives: &mut Vec<String>,
    slang: &Slang,
) -> Result<(), BuildError> {
    let target = target(host.var(TARGET).as_deref())?;
    check_pointer_width(&target, host.var(POINTER_WIDTH))?;
    let tested = TESTED_TARGETS.contains(&target.as_str());
    if !tested {
        directives.push(format!(
            "cargo::warning=slang-compiler-sys is untested and unsupported on the target {target}. It is tested on {}, and a build for another 64-bit target goes on but may fail or misbehave.",
            TESTED_TARGETS.join(", ")
        ));
    }
    let out = out_dir(host.var(OUT_DIR).as_deref())?;
    let generated = host
        .generate(&slang.include, &target)
        .map_err(|error| generation_error(&slang.include, &target, &slang.build_tag, &error))?;
    write_file(host, &out.join(BINDINGS_FILE), &generated.bindings)?;
    write_file(host, &out.join(SYMBOLS_FILE), &generated.symbols)?;
    write_file(host, &out.join(BUILD_INFO_FILE), &build_info(slang, tested))
}

fn build_info(slang: &Slang, tested: bool) -> String {
    format!(
        "BuildInfo {{ slang_build_tag: {:?}, slang_commit: None, linkage: Linkage::Dynamic, tested_target: {tested}, version: {:?} }}\n",
        slang.build_tag,
        version(&slang.build_tag, "dynamic")
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

fn out_dir(value: Option<&OsStr>) -> Result<PathBuf, BuildError> {
    let problem = match value {
        None => format!("{OUT_DIR} is not set"),
        Some(value) => match directive_safe_path(value) {
            Some(path) => return Ok(path),
            None => path_problem(OUT_DIR, value),
        },
    };
    Err(BuildError(format!(
        "{problem}, so the build has no directory in which to write the generated Slang bindings. Cargo sets {OUT_DIR}, so build the crate through Cargo, with a target directory whose path is UTF-8."
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

fn write_file(host: &impl Host, path: &Path, text: &str) -> Result<(), BuildError> {
    host.write(path, text).map_err(|error| {
        BuildError(format!(
            "cannot write {}: {error}. The build writes the generated Slang bindings and the build information under {OUT_DIR}.",
            path.display()
        ))
    })
}

fn sdk_advice() -> String {
    format!(
        "Install the LunarG Vulkan SDK from https://vulkan.lunarg.com, and set {VULKAN_SDK} to the absolute path of its directory that holds include/slang and lib, which is the x86_64 directory of the SDK on Linux, the macOS directory on macOS, and the installation directory on Windows"
    )
}

fn unset_error(state: &str) -> BuildError {
    BuildError(format!(
        "{VULKAN_SDK} {state}, so slang-compiler-sys cannot find the Slang library to link. {}.",
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
            " The headers are those of the Slang build {build_tag}. slang-compiler-sys is written and tested against the Slang that the version of slang-src that it requires records, and another Slang is best effort, so the rules of its binding generator can refuse the headers of another Slang. {}, for the SDK that ships the recorded Slang.",
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
