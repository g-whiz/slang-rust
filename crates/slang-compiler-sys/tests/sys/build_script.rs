use slang_compiler_sys::{SLANG_BUILD_TAG, VULKAN_SDK_VERSION};

include!("../../route.rs");

struct FakeHost {
    vars: Vec<(&'static str, OsString)>,
    files: Vec<(PathBuf, String)>,
    entries: Vec<(PathBuf, Result<EntryKind, io::ErrorKind>)>,
    host_os: &'static str,
}

impl Host for FakeHost {
    fn var(&self, name: &str) -> Option<OsString> {
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
        host_os: "linux",
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
fn configure_with_slang_source_dir_stops_without_the_dynamic_route() {
    for value in ["/tmp/slang", ""] {
        let mut host = pinned_sdk_host();
        host.vars.push((SLANG_SOURCE_DIR, OsString::from(value)));
        let (directives, result) = run(&host);
        let message = result.expect_err("the static route must stop the build");
        assert_contains_all(
            &message,
            &[SLANG_SOURCE_DIR, "not implement", VULKAN_SDK_VERSION],
        );
        assert_eq!(
            directives,
            [
                "cargo::rerun-if-env-changed=DOCS_RS",
                "cargo::rerun-if-env-changed=SLANG_SOURCE_DIR",
            ]
        );
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
        assert_contains_all(&message, &[VULKAN_SDK, "absolute path", VULKAN_SDK_VERSION]);
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
