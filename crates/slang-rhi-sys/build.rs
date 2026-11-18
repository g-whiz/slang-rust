//! Builds slang-rhi with its Vulkan backend as static libraries from the Slang
//! source tree of `slang-src`, keeps them and the configured header in the
//! shared folder of the Cargo target directory, and links them.

include!("build_logic.rs");
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
