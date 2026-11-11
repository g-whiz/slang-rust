use std::collections::HashMap;

include!("../../instructions.rs");

struct FakeEnvironment {
    vars: HashMap<&'static str, OsString>,
}

impl FakeEnvironment {
    fn new(target_os: &str) -> Self {
        Self {
            vars: HashMap::from([(TARGET_OS, OsString::from(target_os))]),
        }
    }

    fn with(mut self, name: &'static str, value: impl Into<OsString>) -> Self {
        self.vars.insert(name, value.into());
        self
    }
}

impl Environment for FakeEnvironment {
    fn var(&self, name: &str) -> Option<OsString> {
        self.vars.get(name).cloned()
    }
}

const RERUN: [&str; 3] = [
    "cargo::rerun-if-env-changed=VULKAN_SDK",
    "cargo::rerun-if-env-changed=DOCS_RS",
    "cargo::rustc-check-cfg=cfg(slango_docs_rs)",
];

fn expected(extra: &[&str]) -> Vec<String> {
    RERUN
        .iter()
        .chain(extra)
        .map(|line| (*line).to_owned())
        .collect()
}

#[test]
fn linux_and_macos_get_an_rpath_to_the_library_directory_of_the_sdk() {
    for os in ["linux", "macos"] {
        let environment = FakeEnvironment::new(os).with(VULKAN_SDK, "/opt/vulkan-sdk/x86_64");
        assert_eq!(
            instructions(&environment),
            Ok(expected(&[
                "cargo::rustc-link-arg=-Wl,-rpath,/opt/vulkan-sdk/x86_64/lib"
            ])),
            "{os}"
        );
    }
}

#[test]
fn windows_gets_no_link_argument() {
    let environment = FakeEnvironment::new("windows").with(VULKAN_SDK, "C:\\VulkanSDK\\1.4.363.1");
    assert_eq!(instructions(&environment), Ok(expected(&[])));
}

#[test]
fn an_unset_or_empty_sdk_gives_no_rpath() {
    assert_eq!(
        instructions(&FakeEnvironment::new("linux")),
        Ok(expected(&[]))
    );
    let environment = FakeEnvironment::new("macos").with(VULKAN_SDK, "");
    assert_eq!(instructions(&environment), Ok(expected(&[])));
}

#[test]
fn docs_rs_sets_the_cfg_of_the_stub() {
    let environment = FakeEnvironment::new("linux").with(DOCS_RS, "1");
    assert_eq!(
        instructions(&environment),
        Ok(expected(&["cargo::rustc-cfg=slango_docs_rs"]))
    );
}

#[test]
fn an_sdk_path_with_a_comma_stops_the_build() {
    let environment = FakeEnvironment::new("linux").with(VULKAN_SDK, "/opt/sdk,v1");
    assert_eq!(
        instructions(&environment),
        Err(BuildError(
            "VULKAN_SDK contains a comma, which the linker would read as the end of the rpath of the macro library to the Slang library of the Vulkan SDK. Install the SDK in a directory whose path has no comma.".to_owned()
        ))
    );
}

#[cfg(unix)]
#[test]
fn an_sdk_path_that_is_not_utf8_stops_the_build() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    let environment =
        FakeEnvironment::new("linux").with(VULKAN_SDK, OsStr::from_bytes(b"/opt/\xff"));
    assert_eq!(
        instructions(&environment),
        Err(BuildError(
            "VULKAN_SDK is not valid UTF-8, so the build script of slango-macros cannot give the macro library an rpath to the Slang library of the Vulkan SDK. Install the SDK in a directory whose path is valid UTF-8.".to_owned()
        ))
    );
}
