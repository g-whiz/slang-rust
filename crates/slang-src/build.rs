//! Clones the recorded Slang commit with its required submodules into the
//! shared folder of the Cargo target directory, once for every build of a
//! project, and exports the path, the commit, the build tag, and the shared
//! folder to the build scripts of the crates that depend on this crate.

include!("recorded.rs");
include!("source.rs");
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
