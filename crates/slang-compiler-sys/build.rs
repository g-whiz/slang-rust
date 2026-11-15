//! Generates the raw Slang bindings from the headers of the Vulkan SDK that
//! `VULKAN_SDK` names, records the build tag of its Slang, and links the
//! Slang shared library of that SDK.

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
