use std::cell::RefCell;
use std::fs::{self, File};
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime};

const STAMP_TIME: Duration = Duration::from_hours(262_968);

#[derive(Default)]
struct System {
    locks: RefCell<Vec<File>>,
}

impl Host for System {
    fn var(&self, name: &str) -> Option<OsString> {
        std::env::var_os(name)
    }

    fn exists(&self, path: &Path) -> io::Result<bool> {
        fs::exists(path)
    }

    fn create_dir_all(&self, path: &Path) -> io::Result<()> {
        fs::create_dir_all(path)
    }

    fn lock(&self, path: &Path) -> io::Result<()> {
        let file = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(path)?;
        file.lock()?;
        self.locks.borrow_mut().push(file);
        Ok(())
    }

    fn list(&self, dir: &Path) -> io::Result<Vec<OsString>> {
        fs::read_dir(dir)?
            .map(|entry| entry.map(|entry| entry.file_name()))
            .collect()
    }

    fn remove_dir_all(&self, path: &Path) -> io::Result<()> {
        fs::remove_dir_all(path)
    }

    fn write(&self, path: &Path, text: &str) -> io::Result<()> {
        fs::write(path, text)
    }

    fn write_stamp(&self, path: &Path) -> io::Result<()> {
        File::create(path)?.set_modified(SystemTime::UNIX_EPOCH + STAMP_TIME)
    }

    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        fs::rename(from, to)
    }

    fn process_id(&self) -> u32 {
        std::process::id()
    }

    fn run(&self, invocation: &Invocation) -> io::Result<Finished> {
        let output = system_command(invocation).output()?;
        Ok(Finished {
            code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

fn system_command(invocation: &Invocation) -> Command {
    let mut command = Command::new("git");
    command
        .args(&invocation.args)
        .current_dir(&invocation.dir)
        .stdin(Stdio::null());
    for name in &invocation.env_remove {
        command.env_remove(name);
    }
    for (name, value) in &invocation.env {
        command.env(name, value);
    }
    command
}
