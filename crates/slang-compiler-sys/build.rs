//! Generates the raw Slang bindings and links Slang: by default the Slang
//! shared library of the Vulkan SDK that `VULKAN_SDK` names, and with the
//! feature `vendored` a static Slang that it builds from the source tree of
//! `slang-src`. It records which Slang the build uses in the build
//! information.

include!("route.rs");
include!("system.rs");

fn main() -> Result<(), BuildError> {
    let system = System::default();
    let mut directives = Vec::new();
    let result = configure(&system, &mut directives);
    for directive in &directives {
        println!("{directive}");
    }
    result
}
