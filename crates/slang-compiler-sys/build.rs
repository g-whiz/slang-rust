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

    fn kind(&self, path: &Path) -> io::Result<EntryKind> {
        let metadata = std::fs::metadata(path)?;
        Ok(if metadata.is_dir() {
            EntryKind::Directory
        } else if metadata.is_file() {
            EntryKind::File
        } else {
            EntryKind::Other
        })
    }

    fn host_os(&self) -> &str {
        std::env::consts::OS
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
