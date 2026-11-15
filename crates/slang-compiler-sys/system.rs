use std::cell::RefCell;
use std::fs::{self, File};
use std::process::{Command, Stdio};

#[derive(Default)]
struct System {
    locks: RefCell<Vec<File>>,
}

impl Host for System {
    fn var(&self, name: &str) -> Option<OsString> {
        std::env::var_os(name)
    }

    fn read(&self, path: &Path) -> io::Result<String> {
        Ok(String::from_utf8_lossy(&fs::read(path)?).into_owned())
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

    fn run_to_log(&self, invocation: &Invocation, log: &Path) -> Result<Exit, LoggedRunError> {
        let file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log)
            .map_err(LoggedRunError::Log)?;
        let output = file.try_clone().map_err(LoggedRunError::Log)?;
        let status = system_command(invocation)
            .stdout(output)
            .stderr(file)
            .status()
            .map_err(LoggedRunError::Start)?;
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

    fn list(&self, dir: &Path) -> io::Result<Vec<OsString>> {
        fs::read_dir(dir)?
            .map(|entry| entry.map(|entry| entry.file_name()))
            .collect()
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

    fn remove_dir_all(&self, path: &Path) -> io::Result<()> {
        fs::remove_dir_all(path)
    }

    fn write(&self, path: &Path, text: &str) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, text)
    }

    fn set_modified(&self, path: &Path, time: SystemTime) -> io::Result<()> {
        File::options().write(true).open(path)?.set_modified(time)
    }

    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        fs::rename(from, to)
    }

    fn lock(&self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let file = File::options()
            .create(true)
            .truncate(false)
            .write(true)
            .open(path)?;
        file.lock()?;
        self.locks.borrow_mut().push(file);
        Ok(())
    }

    fn process_id(&self) -> u32 {
        std::process::id()
    }

    #[cfg(feature = "vendored")]
    fn msvc_compiler(&self, target: &str) -> Option<MsvcCompiler> {
        find_msvc_tools::find_tool(target, "cl.exe").map(|tool| MsvcCompiler {
            path: tool.path().to_path_buf(),
            env: tool.env().into_iter().cloned().collect(),
        })
    }

    #[cfg(not(feature = "vendored"))]
    fn msvc_compiler(&self, _target: &str) -> Option<MsvcCompiler> {
        None
    }

    fn generate(&self, header_dir: &Path, target: &str) -> Result<Generated, GenerationError> {
        let generated = slang_bindgen::generate(header_dir, target)
            .map_err(|error| generation_failure(&error))?;
        Ok(Generated {
            bindings: generated.bindings,
            symbols: generated.symbols,
        })
    }
}

fn system_command(invocation: &Invocation) -> Command {
    let mut command = Command::new(&invocation.program);
    command.args(&invocation.args).stdin(Stdio::null());
    if let Some(dir) = &invocation.dir {
        command.current_dir(dir);
    }
    for name in &invocation.env_remove {
        command.env_remove(name);
    }
    for (name, value) in &invocation.env {
        command.env(name, value);
    }
    command
}

fn generation_failure(error: &slang_bindgen::Error) -> GenerationError {
    let kind = match error.kind() {
        slang_bindgen::ErrorKind::Libclang => GenerationErrorKind::Libclang,
        slang_bindgen::ErrorKind::Target => GenerationErrorKind::Target,
        slang_bindgen::ErrorKind::UnmatchedDeclaration
        | slang_bindgen::ErrorKind::StaleRule
        | slang_bindgen::ErrorKind::Unsupported => GenerationErrorKind::Headers,
        _ => GenerationErrorKind::Other,
    };
    let mut message = error.to_string();
    let mut source = std::error::Error::source(error);
    while let Some(cause) = source {
        message.push_str(", caused by: ");
        message.push_str(&cause.to_string());
        source = cause.source();
    }
    GenerationError { kind, message }
}
