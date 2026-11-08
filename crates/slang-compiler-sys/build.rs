extern crate bindgen;

use bindgen::EnumVariation;
use std::convert::Into;
use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

const DEFAULT_SLANG_TAG: &str = "v2026.1.2";

// IDEA: - rename this method to build_from_source, have cargo feature called vendored
//       - when vendored is enabled, skips checks for VULKAN_SDK, etc. and just builds from source
//         and static links
//       - when vendored is not enabled, checks for VULKAN_SDK, etc. and dynamic links, if possible.
//         otherwise, builds from source and static links

/*
Build Env Vars todo: (add this to main docs)
- SLANG_DIR: path to slang cmake build install directory
- SLANG_BUILD_REPO_URL: url of slang repo to clone
                        (default: https://github.com/shader-slang/slang.git)
- SLANG_BUILD_TAG: tag of slang repo to clone
                   (default: v2026.1.2)
- SLANG_BUILD_NUM_JOBS: number of parallel jobs to use when building slang
                        (default: number of available logical cores in the system)
*/

// #[cfg(feature = "static-slang")]
// return include and lib dir paths
fn build_slang_from_source(build_dir: &Path) -> (PathBuf, PathBuf) {
    let slang_dir = build_dir.join("../../../slang-src");
    // Clone repo if needed.
    if !slang_dir.exists() {
        let repo_url = env::var("SLANG_BUILD_REPO_URL")
            .unwrap_or("https://github.com/shader-slang/slang.git".into());
        let slang_tag = env::var("SLANG_BUILD_TAG").unwrap_or(DEFAULT_SLANG_TAG.into());

        // clone slang and dependencies
        // - shallow clone, since we only need tagged sources
        // - speeds up this build step by an order of magnitude
        Command::new("git")
            .args([
                "clone",
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

    let num_jobs: usize = match env::var("SLANG_BUILD_NUM_JOBS").map(|v| v.parse()) {
        Ok(Ok(n)) => n,
        // default to number of logical cores if num_jobs unspecified
        _ => std::thread::available_parallelism()
            .map(|n| n.into())
            .unwrap_or(1),
    };

    // Build libslang-compiler.a with cmake.
    // Unimplemented: allow passing extra cmake options via env vars
    let mut config = cmake::Config::new(&slang_dir);
    // should generator be an env option? probably...
    config
        .generator("Ninja")
        .build_target("slang")
        .build_arg("-j")
        .build_arg(format!("{}", num_jobs))
        .define("SLANG_LIB_TYPE", "STATIC");
    let profile = String::from(config.get_profile());
    let slang_dst = config.build();

    let slang_lib_dir = slang_dst.join(format!("build/{profile}/lib"));
    let slang_include_dir = slang_dst.join(format!("build/{profile}/include"));

    (slang_include_dir, slang_lib_dir)
}

#[cfg(feature = "dynamic")]
fn find_dynamic_slang() -> Option<(PathBuf, PathBuf)> {
    enum InstallationType {
        SlangSourceBuild,
        VulkanSDK,
    }

    let slang_install = [
        option_env!("SLANG_DIR").map(|e| (e, InstallationType::SlangSourceBuild)),
        option_env!("VULKAN_SDK").map(|e| (e, InstallationType::VulkanSDK)),
    ];

    let Some(Some((install_dir, install_type))) = slang_install.iter().find(|e| e.is_some()) else {
        println!(
            "cargo:warning=\
            Unable to find slang installation. None of the environment variables checked were set: \
            SLANG_DIR, VULKAN_SDK. Defaulting to vendored build."
        );
        return None;
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
        println!(
            "cargo:warning=\
            Invalid slang install specified by env {env}={base}. \
            Expected to find include directory at {include}, and library directory at {lib}."
        );
        return None;
    }
    let slang_dir = slang_dir.display();
    println!("Detected Slang installation at {slang_dir}.");
    Some((slang_include_dir, slang_lib_dir))
}

// To Document in README:
// - feature "dynamic": try to dynamic link to slang-compiler shared library, if possible

// #[cfg(not(feature = "static-slang"))]
fn main() {
    let build_dir = env::var("OUT_DIR")
        .map(|s: String| PathBuf::from(s))
        .expect("OUT_DIR not set by cargo");
    assert!(build_dir.is_dir());

    let slang_lib_dir: PathBuf;
    let slang_include_dir: PathBuf;

    #[cfg(not(feature = "dynamic"))]
    {
        (slang_include_dir, slang_lib_dir) = build_slang_from_source(&build_dir);
        let lib_dir = slang_lib_dir.display();
        // Tell cargo to look for shared libraries in the specified directory
        println!("cargo:rustc-link-search=native={lib_dir}");
        println!("cargo:rustc-link-lib=static=slang-compiler");
    }

    #[cfg(feature = "dynamic")]
    {
        let is_dynamic: bool;
        (is_dynamic, (slang_include_dir, slang_lib_dir)) = match find_dynamic_slang() {
            Some((include_dir, lib_dir)) => (true, (include_dir, lib_dir)),
            None => (true, build_slang_from_source(&build_dir)),
        };
        let lib_dir = slang_lib_dir.display();
        println!("cargo:rustc-link-search=native={lib_dir}");
        if is_dynamic {
            println!("cargo:rustc-link-lib=dylib=slang-compiler");
        } else {
            println!("cargo:rustc-link-lib=static=slang-compiler");
        }
    }

    let slang_h = slang_include_dir
        .join("slang.h")
        .into_os_string()
        .into_string()
        .expect("Could not convert slang.h path to String");

    let slang_deprecated_h = slang_include_dir
        .join("slang-deprecated.h")
        .into_os_string()
        .into_string()
        .expect("Could not convert slang.h path to String");

    // todo: add dynamic library info?

    bindgen::Builder::default()
        .header(slang_h)
        .clang_arg("-v")
        .clang_arg("-xc++")
        .clang_arg("-std=c++17")
        .enable_cxx_namespaces()
        .allowlist_item("slang.*")
        .allowlist_item("spReflection.*")
        .allowlist_item("Slang.*")
        .allowlist_item("ISlang.*")
        .allowlist_item("OSPathKind")
        .allowlist_item("PathKind")
        .default_enum_style(EnumVariation::ModuleConsts)
        .generate()
        .expect("Unable to generate bindings")
        .write_to_file(build_dir.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}
