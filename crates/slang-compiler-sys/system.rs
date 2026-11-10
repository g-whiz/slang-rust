use std::fs;
use std::process::{Command, Stdio};

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
        let output = system_command(invocation).output()?;
        Ok(Finished {
            exit: Exit {
                code: output.status.code(),
            },
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }

    fn run_to_log(&self, invocation: &Invocation, log: &Path) -> io::Result<Exit> {
        let file = fs::OpenOptions::new().create(true).append(true).open(log)?;
        let status = system_command(invocation)
            .stdout(file.try_clone()?)
            .stderr(file)
            .status()?;
        Ok(Exit {
            code: status.code(),
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

fn system_command(invocation: &Invocation) -> Command {
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
    command
}
