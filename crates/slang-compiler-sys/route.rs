use std::ffi::OsString;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

const DOCS_RS: &str = "DOCS_RS";
const SLANG_SOURCE_DIR: &str = "SLANG_SOURCE_DIR";
const VULKAN_SDK: &str = "VULKAN_SDK";
const TAG_MACRO: &str = "SLANG_TAG_VERSION";
const TARGET_OS: &str = "CARGO_CFG_TARGET_OS";

trait Host {
    fn var(&self, name: &str) -> Option<OsString>;
    fn read(&self, path: &Path) -> io::Result<String>;
    fn kind(&self, path: &Path) -> io::Result<EntryKind>;
    fn host_os(&self) -> &str;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EntryKind {
    Directory,
    File,
    Other,
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

fn configure(host: &impl Host, directives: &mut Vec<String>) -> Result<(), BuildError> {
    directives.push(format!("cargo::rerun-if-env-changed={DOCS_RS}"));
    if host.var(DOCS_RS).is_some() {
        return Ok(());
    }
    directives.push(format!("cargo::rerun-if-env-changed={SLANG_SOURCE_DIR}"));
    if host.var(SLANG_SOURCE_DIR).is_some() {
        return Err(static_route_error());
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
    match host.kind(lib) {
        Ok(EntryKind::Directory) => {}
        Ok(_) => return Err(lib_error(sdk, lib, &format!("{} is not a directory", lib.display()))),
        Err(error) => {
            return Err(lib_error(sdk, lib, &format!("cannot access {}: {error}", lib.display())));
        }
    }
    let target_os = host.var(TARGET_OS);
    let Some(target_os) = target_os.as_ref().and_then(|os| os.to_str()) else {
        return Ok(());
    };
    if target_os != host.host_os() {
        return Ok(());
    }
    let Some(name) = library_file(target_os) else {
        return Ok(());
    };
    let file = lib.join(name);
    match host.kind(&file) {
        Ok(EntryKind::File) => Ok(()),
        Ok(_) => Err(lib_error(sdk, lib, &format!("{} is not a file", file.display()))),
        Err(error) => Err(lib_error(sdk, lib, &format!("cannot access {}: {error}", file.display()))),
    }
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
    match value.to_str() {
        Some(text) if !text.contains(['\n', '\r']) && Path::new(text).is_absolute() => {
            Ok(PathBuf::from(text))
        }
        _ => Err(BuildError(format!(
            "{VULKAN_SDK} is \"{}\", which is not an absolute path in UTF-8 without line breaks, so the build cannot check the Slang that it names and pass its library to the linker. {}.",
            value.to_string_lossy().escape_debug(),
            sdk_advice()
        ))),
    }
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

fn sdk_advice() -> String {
    format!(
        "Install the LunarG Vulkan SDK {VULKAN_SDK_VERSION} from https://vulkan.lunarg.com, and set {VULKAN_SDK} to the absolute path of its directory that holds include/slang and lib, which is the x86_64 directory of the SDK on Linux, the macOS directory on macOS, and the installation directory on Windows"
    )
}

fn unset_error(state: &str) -> BuildError {
    BuildError(format!(
        "{VULKAN_SDK} {state}, so slang-compiler-sys cannot find the Slang library to link. {}. The other way to obtain Slang, building it from the source tree that {SLANG_SOURCE_DIR} names, is not implemented in this version of slang-compiler-sys.",
        sdk_advice()
    ))
}

fn static_route_error() -> BuildError {
    BuildError(format!(
        "{SLANG_SOURCE_DIR} is set, which selects building Slang from source, and this version of slang-compiler-sys does not implement that route yet. Unset {SLANG_SOURCE_DIR} to link the Slang of the Vulkan SDK {VULKAN_SDK_VERSION} that {VULKAN_SDK} names instead."
    ))
}

fn unreadable_header_error(path: &Path, error: &io::Error) -> BuildError {
    BuildError(format!(
        "cannot read {}: {error}. The build reads this file to check that the Slang of the Vulkan SDK that {VULKAN_SDK} names is the build that slang-compiler-sys needs. {}.",
        path.display(),
        sdk_advice()
    ))
}

fn tag_error(path: &Path, error: &TagError) -> BuildError {
    let problem = match error {
        TagError::Missing => format!("{} does not define {TAG_MACRO}", path.display()),
        TagError::Malformed { line } => format!(
            "line {line} of {} does not define {TAG_MACRO} as one string literal",
            path.display()
        ),
    };
    BuildError(format!(
        "{problem}, so the build cannot check that the Slang of the Vulkan SDK that {VULKAN_SDK} names is the build {SLANG_BUILD_TAG} that slang-compiler-sys needs. {}.",
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
