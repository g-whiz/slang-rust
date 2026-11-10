use slang_compiler_sys::{SLANG_BUILD_TAG, SLANG_COMMIT, VULKAN_SDK_VERSION};

fn is_number(text: &str) -> bool {
    !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit())
}

fn is_lowercase_hex(text: &str) -> bool {
    !text.is_empty()
        && text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn is_release(text: &str) -> bool {
    let mut parts = text.split('.');
    parts.next().is_some_and(is_number) && parts.next().is_some_and(is_number)
}

#[test]
fn pin_commit_is_a_full_lowercase_hexadecimal_hash() {
    assert_eq!(SLANG_COMMIT.len(), 40, "{SLANG_COMMIT}");
    assert!(is_lowercase_hex(SLANG_COMMIT), "{SLANG_COMMIT}");
}

#[test]
fn pin_build_tag_names_a_release_and_abbreviates_the_commit() {
    match SLANG_BUILD_TAG.rsplit_once("-g") {
        Some((describe, abbreviated)) => {
            let (release, count) = describe
                .rsplit_once('-')
                .unwrap_or_else(|| panic!("{SLANG_BUILD_TAG} has no commit count"));
            assert!(is_release(release), "{SLANG_BUILD_TAG}");
            assert!(is_number(count), "{SLANG_BUILD_TAG}");
            assert!(abbreviated.len() >= 7, "{SLANG_BUILD_TAG}");
            assert!(is_lowercase_hex(abbreviated), "{SLANG_BUILD_TAG}");
            assert!(
                SLANG_COMMIT.starts_with(abbreviated),
                "{SLANG_BUILD_TAG} does not abbreviate {SLANG_COMMIT}"
            );
        }
        None => assert!(is_release(SLANG_BUILD_TAG), "{SLANG_BUILD_TAG}"),
    }
}

#[test]
fn pin_vulkan_sdk_version_has_four_numeric_components() {
    let parts: Vec<&str> = VULKAN_SDK_VERSION.split('.').collect();
    assert_eq!(parts.len(), 4, "{VULKAN_SDK_VERSION}");
    assert!(
        parts.iter().all(|part| is_number(part)),
        "{VULKAN_SDK_VERSION}"
    );
}
