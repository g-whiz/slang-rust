use slang_compiler_sys::{SLANG_BUILD_TAG, VULKAN_SDK_VERSION};

include!("../../route.rs");

struct FakeHost {
    vars: Vec<(&'static str, OsString)>,
    files: Vec<(PathBuf, String)>,
    dirs: Vec<PathBuf>,
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

    fn is_dir(&self, path: &Path) -> bool {
        self.dirs.iter().any(|dir| dir == path)
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

fn sdk_host(header: &str) -> FakeHost {
    FakeHost {
        vars: vec![(VULKAN_SDK, sdk_root().into_os_string())],
        files: vec![(tag_header(), header.to_owned())],
        dirs: vec![sdk_root().join("lib")],
    }
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

fn assert_no_link_directives(directives: &[String]) {
    assert!(
        directives
            .iter()
            .all(|directive| !directive.starts_with("cargo::rustc-link")
                && !directive.starts_with("cargo::metadata")),
        "{directives:?}"
    );
}

#[test]
fn configure_links_the_sdk_library_when_the_build_tag_matches_the_pin() {
    let root = sdk_root();
    let (directives, result) = run(&pinned_sdk_host());
    assert_eq!(result, Ok(()));
    assert_eq!(
        directives,
        [
            "cargo::rerun-if-env-changed=DOCS_RS".to_owned(),
            "cargo::rerun-if-env-changed=SLANG_SOURCE_DIR".to_owned(),
            "cargo::rerun-if-env-changed=VULKAN_SDK".to_owned(),
            format!("cargo::rerun-if-changed={}", tag_header().display()),
            format!(
                "cargo::rustc-link-search=native={}",
                root.join("lib").display()
            ),
            "cargo::rustc-link-lib=dylib=slang-compiler".to_owned(),
            format!("cargo::metadata=include={}", root.join("include").display()),
            format!("cargo::metadata=lib={}", root.join("lib").display()),
        ]
    );
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
            files: Vec::new(),
            dirs: Vec::new(),
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
        files: Vec::new(),
        dirs: Vec::new(),
    };
    let absent = FakeHost {
        vars: Vec::new(),
        files: Vec::new(),
        dirs: Vec::new(),
    };
    for (host, state) in [(absent, "is not set"), (empty, "is set but empty")] {
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

#[test]
fn configure_rejects_a_vulkan_sdk_that_is_relative_or_has_line_breaks() {
    let multiline = sdk_root().join("x\ncargo::rustc-link-lib=other");
    for value in [OsString::from("vulkan-sdk"), multiline.into_os_string()] {
        let host = FakeHost {
            vars: vec![(VULKAN_SDK, value)],
            files: Vec::new(),
            dirs: Vec::new(),
        };
        let (directives, result) = run(&host);
        let message = result.expect_err("the path must be rejected");
        assert_contains_all(&message, &[VULKAN_SDK, "absolute path", VULKAN_SDK_VERSION]);
        assert!(!message.contains('\n'), "{message:?}");
        assert_no_link_directives(&directives);
    }
}

#[test]
fn configure_without_the_tag_header_names_its_path_and_the_sdk_version() {
    let mut host = pinned_sdk_host();
    host.files.clear();
    let message = run(&host).1.expect_err("the build must stop");
    assert_contains_all(
        &message,
        &[&tag_header().display().to_string(), VULKAN_SDK_VERSION],
    );
}

#[test]
fn configure_without_the_tag_definition_names_the_path_and_the_pinned_tag() {
    let message = run(&sdk_host("#pragma once\n"))
        .1
        .expect_err("the build must stop");
    assert_contains_all(
        &message,
        &[
            &tag_header().display().to_string(),
            &format!("does not define {TAG_MACRO}"),
            SLANG_BUILD_TAG,
        ],
    );
}

#[test]
fn configure_with_a_malformed_tag_names_the_line_the_path_and_the_pinned_tag() {
    for (text, line) in [
        ("#define SLANG_TAG_VERSION 2026\n", 1),
        ("\n#define SLANG_TAG_VERSION \"a\" \"b\"\n", 2),
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

#[test]
fn configure_without_the_library_directory_names_it_and_the_sdk_version() {
    let mut host = pinned_sdk_host();
    host.dirs.clear();
    let (directives, result) = run(&host);
    let message = result.expect_err("a missing library directory must stop the build");
    assert_contains_all(
        &message,
        &[
            &sdk_root().join("lib").display().to_string(),
            VULKAN_SDK_VERSION,
        ],
    );
    assert_no_link_directives(&directives);
}
