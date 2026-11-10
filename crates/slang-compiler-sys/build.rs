//! Links the Slang library of the Vulkan SDK that `VULKAN_SDK` names, after
//! checking that its build tag is the build tag of the pin.

include!("pin.rs");
include!("route.rs");

struct System;

impl Host for System {
    fn var(&self, name: &str) -> Option<OsString> {
        std::env::var_os(name)
    }

    fn read(&self, path: &Path) -> io::Result<String> {
        std::fs::read_to_string(path)
    }

    fn is_dir(&self, path: &Path) -> bool {
        path.is_dir()
    }
}

fn main() -> Result<(), BuildError> {
    let mut directives = Vec::new();
    let result = configure(&System, &mut directives);
    for directive in &directives {
        println!("{directive}");
    }
    result
}
