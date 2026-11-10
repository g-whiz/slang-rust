//! Links the Slang library that the pin names: the shared library of the
//! Vulkan SDK that `VULKAN_SDK` names, after checking its build tag, or, when
//! `SLANG_SOURCE_DIR` is present, a static library that it builds from the
//! Slang source tree that the variable names, after checking its commit.

include!("pin.rs");
include!("route.rs");
include!("system.rs");

fn main() -> Result<(), BuildError> {
    let mut directives = Vec::new();
    let result = configure(&System, &mut directives);
    for directive in &directives {
        println!("{directive}");
    }
    result
}
