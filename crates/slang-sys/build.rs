extern crate bindgen;

use std::convert::Into;
use std::env;
use std::path::PathBuf;
use std::process::Command;

const DEFAULT_SLANG_TAG: &str = "v2026.1.2";

#[cfg(feature = "static-slang")]
fn main() {
    // todo (clone & build statically linked libslang-compiler)

    let build_dir = env::var("OUT_DIR")
        .map(|s: String| PathBuf::from(s))
        .expect("OUT_DIR not set by cargo");
    assert!(build_dir.is_dir());

    let slang_dir = build_dir.join("slang-src");
    // Clone repo if needed.
    if !slang_dir.exists() {
        let repo_url = env::var("SLANG_REPO_URL")
            .unwrap_or("https://github.com/shader-slang/slang.git".into());
        let slang_tag = env::var("SLANG_TAG").unwrap_or(DEFAULT_SLANG_TAG.into());

        Command::new("git")
            .args([
                "--recurse-submodules",
                "--shallow-submodules",
                "--depth",
                "1",
                "--branch",
            ])
            .arg(&slang_tag)
            .arg(&repo_url)
            .arg(&slang_dir)
            .status()
            .expect("Failed to clone slang repo");
    }

    // Build slang with cmake.
    // Unimplemented: allow passing extra cmake options via env vars
    let slang_dst = cmake::Config::new(slang_dir)
        .define("SLANG_LIB_TYPE", "STATIC")
        .build();

    // todo: bindgen (maybe shared routine between both mains?)
}

#[cfg(not(feature = "static-slang"))]
fn main() {
    enum InstallationType {
        SlangSourceBuild,
        VulkanSDK,
    }

    let slang_install = [
        option_env!("SLANG_DIR").map(|e| (e, InstallationType::SlangSourceBuild)),
        option_env!("VULKAN_SDK").map(|e| (e, InstallationType::VulkanSDK)),
    ];

    let Some(Some((install_dir, install_type))) = slang_install.iter().find(|e| e.is_some()) else {
        panic!(
            "Unable to find slang installation. None of the environment variables checked \
                were set: SLANG_DIR, VULKAN_SDK."
        );
    };

    let slang_dir = Path::new(install_dir);
    let slang_lib_dir = slang_dir.join("lib");
    let slang_include_dir = match install_type {
        InstallationType::SlangSourceBuild => slang_dir.join("include"),
        InstallationType::VulkanSDK => slang_lib_dir.join("include/slang"),
    };

    if (!slang_lib_dir.exists()) || (!slang_include_dir.exists()) {
        let env = match install_type {
            InstallationType::SlangSourceBuild => "SLANG_DIR",
            InstallationType::VulkanSDK => "VULKAN_SDK",
        };
        let base = slang_dir.display();
        let lib = slang_lib_dir.display();
        let include = slang_include_dir.display();
        panic!(
            "Invalid slang install specified by env {env}={base}. \
             Expected to find include directory at {include}, and library directory at {lib}."
        );
    }

    println!()

    // TODO
}
