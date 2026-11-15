//! Builds glslang, SPIRV-Tools, Slang's glue code, and the loader shim as
//! static libraries from the Slang source tree of `slang-src`, keeps them in
//! the shared folder of the Cargo target directory, and links them.

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
