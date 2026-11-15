//! Gives the macro library an rpath to the Slang library of the Vulkan SDK
//! that `VULKAN_SDK` names, on Linux and macOS.

include!("instructions.rs");

struct System;

impl Environment for System {
    fn var(&self, name: &str) -> Option<OsString> {
        std::env::var_os(name)
    }
}

fn main() -> Result<(), BuildError> {
    for instruction in instructions(&System)? {
        println!("{instruction}");
    }
    Ok(())
}
