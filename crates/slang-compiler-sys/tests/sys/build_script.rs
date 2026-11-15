use std::cell::RefCell;

include!("../../route.rs");

mod ffi;

type Generator = Box<dyn Fn(&Path, &str) -> Result<Generated, GenerationError>>;

const BUILD_TAG: &str = "2026.17-25-gc7954ebd1";
const TRIPLE: &str = "x86_64-unknown-linux-gnu";

struct FakeHost {
    vars: Vec<(&'static str, OsString)>,
    files: Vec<(PathBuf, String)>,
    entries: Vec<(PathBuf, Result<EntryKind, io::ErrorKind>)>,
    host_os: &'static str,
    generator: Generator,
    failing_write: Option<(PathBuf, io::ErrorKind)>,
    read_vars: RefCell<Vec<String>>,
    generations: RefCell<Vec<(PathBuf, String)>>,
    written: RefCell<Vec<(PathBuf, String)>>,
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

    fn write(&self, path: &Path, text: &str) -> io::Result<()> {
        if let Some((failing, kind)) = &self.failing_write
            && failing == path
        {
            return Err(io::Error::from(*kind));
        }
        self.written
            .borrow_mut()
            .push((path.to_path_buf(), text.to_owned()));
        Ok(())
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
        host_os: "linux",
        generator: Box::new(|_, _| Ok(generated())),
        failing_write: None,
        read_vars: RefCell::default(),
        generations: RefCell::default(),
        written: RefCell::default(),
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
        [VULKAN_SDK, TARGET_OS, TARGET, POINTER_WIDTH, OUT_DIR]
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
        host.failing_write = Some((path.clone(), io::ErrorKind::PermissionDenied));
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
