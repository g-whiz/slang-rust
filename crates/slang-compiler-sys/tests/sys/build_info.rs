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
fn build_info_names_the_crate_version_the_slang_and_the_dynamic_route() {
    assert_ne!(BUILD_INFO.slang_build_tag, "");
    assert_eq!(
        BUILD_INFO.version,
        format!(
            "{}+slang.{}.dynamic",
            env!("CARGO_PKG_VERSION"),
            slang_identifiers(BUILD_INFO.slang_build_tag)
        )
    );
    assert_eq!(BUILD_INFO.linkage, Linkage::Dynamic);
    assert_eq!(BUILD_INFO.slang_commit, None);
    const { assert!(BUILD_INFO.tested_target) };
}
