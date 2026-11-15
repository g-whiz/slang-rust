use slang_compiler_sys::{BUILD_INFO, Linkage};

fn slang_identifiers(build_tag: &str) -> String {
    let (release, described) = build_tag.split_once('-').unwrap_or((build_tag, ""));
    let mut numbers: Vec<&str> = release.split('.').collect();
    if numbers.len() == 2 {
        numbers.push("0");
    }
    match described.split_once("-g") {
        Some((count, hash)) => format!("{}.{count}.g{hash}", numbers.join(".")),
        None => format!("{}.0", numbers.join(".")),
    }
}

#[test]
fn build_info_names_the_crate_version_the_slang_and_the_route_of_the_feature() {
    let (route, linkage) = if cfg!(feature = "vendored") {
        ("vendored", Linkage::Static)
    } else {
        ("dynamic", Linkage::Dynamic)
    };
    assert_ne!(BUILD_INFO.slang_build_tag, "");
    assert_eq!(
        BUILD_INFO.version,
        format!(
            "{}+slang.{}.{route}",
            env!("CARGO_PKG_VERSION"),
            slang_identifiers(BUILD_INFO.slang_build_tag)
        )
    );
    assert_eq!(BUILD_INFO.linkage, linkage);
    if cfg!(feature = "vendored") {
        let commit = BUILD_INFO.slang_commit.unwrap_or_default();
        assert_eq!(commit.len(), 40, "{commit:?}");
        assert!(
            commit
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
            "{commit:?}"
        );
        let Some((_, hash)) = BUILD_INFO.slang_build_tag.rsplit_once("-g") else {
            panic!(
                "the build tag {} names no commit hash to compare with the commit {commit}",
                BUILD_INFO.slang_build_tag
            );
        };
        assert!(hash.len() >= 9, "{hash}");
        assert!(commit.starts_with(hash), "{commit} {hash}");
    } else {
        assert_eq!(BUILD_INFO.slang_commit, None);
    }
    const { assert!(BUILD_INFO.tested_target) };
}
