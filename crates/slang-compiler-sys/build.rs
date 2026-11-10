//! Links the Slang library that the pin names: the shared library of the
//! Vulkan SDK that `VULKAN_SDK` names, after checking its build tag, or, when
//! `SLANG_SOURCE_DIR` is present, a static library that it builds from the
//! Slang source tree that the variable names, after checking its commit.

use std::fs;
use std::process::{Command, Stdio};

include!("pin.rs");
include!("route.rs");

struct System;

impl Host for System {
    fn var(&self, name: &str) -> Option<OsString> {
        std::env::var_os(name)
    }

    fn read(&self, path: &Path) -> io::Result<String> {
        fs::read_to_string(path)
    }

    fn kind(&self, path: &Path) -> io::Result<EntryKind> {
        let metadata = fs::metadata(path)?;
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

    fn run(&self, invocation: &Invocation) -> io::Result<Finished> {
        let mut command = Command::new(invocation.program);
        command.args(&invocation.args).stdin(Stdio::null());
        if let Some(dir) = &invocation.dir {
            command.current_dir(dir);
        }
        for (name, value) in &invocation.env {
            command.env(name, value);
        }
        for name in &invocation.env_remove {
            command.env_remove(name);
        }
        if let Some(log) = &invocation.log {
            let file = fs::OpenOptions::new().create(true).append(true).open(log)?;
            command.stdout(file.try_clone()?).stderr(file);
            let status = command.status()?;
            return Ok(Finished {
                code: status.code(),
                stdout: String::new(),
                stderr: String::new(),
            });
        }
        let output = command.output()?;
        Ok(Finished {
            code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }

    fn files(&self, root: &Path) -> io::Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        let mut pending = vec![PathBuf::new()];
        while let Some(relative) = pending.pop() {
            for entry in fs::read_dir(root.join(&relative))? {
                let entry = entry?;
                let path = relative.join(entry.file_name());
                if entry.file_type()?.is_dir() {
                    pending.push(path);
                } else {
                    files.push(path);
                }
            }
        }
        Ok(files)
    }

    fn sync_file(&self, from: &Path, to: &Path) -> io::Result<()> {
        let data = fs::read(from)?;
        if fs::read(to).is_ok_and(|existing| existing == data) {
            return Ok(());
        }
        if let Some(parent) = to.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(to, data)
    }

    fn remove_file(&self, path: &Path) -> io::Result<()> {
        fs::remove_file(path)
    }

    fn write(&self, path: &Path, text: &str) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, text)
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
