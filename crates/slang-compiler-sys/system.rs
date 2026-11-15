use std::fs;

struct System;

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

    fn write(&self, path: &Path, text: &str) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, text)
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
