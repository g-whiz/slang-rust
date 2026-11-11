use std::error::Error as _;
use std::path::{Path, PathBuf};

use slango::__private::precompile_file;
use slango_codegen::{ErrorKind, generate, generate_precompiled};

pub(crate) fn shaders() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/codegen/shaders")
}

pub(crate) fn shader(name: &str) -> PathBuf {
    shaders().join(name)
}

/// Returns the generated bindings of a test shader, formatted, with the
/// directory of the test shaders and the encoded snapshot, whose bytes
/// depend on the machine and on the Slang of the pin, replaced by
/// placeholders before the formatting.
pub(crate) fn bindings(name: &str) -> String {
    let mut precompiled = precompile_file(&shader(name)).expect("the module compiles");
    for path in &mut precompiled.interface.dependencies {
        *path = redacted(path);
    }
    precompiled.encoded = b"<encoded snapshot>".to_vec();
    let tokens = generate_precompiled(&precompiled).expect("the bindings of the shader");
    let file = syn::parse2::<syn::File>(tokens).expect("the bindings parse as a Rust file");
    prettyplease::unparse(&file)
}

/// Returns `<shaders>/` followed by the file name of a path, with a slash on
/// every platform, so that the generated bindings are the same text on
/// Windows, whose paths separate their components with backslashes.
fn redacted(path: &Path) -> PathBuf {
    let path = path.to_str().expect("a UTF-8 path");
    let file = path.rsplit(['/', '\\']).next().unwrap_or(path);
    PathBuf::from(format!("<shaders>/{file}"))
}

pub(crate) fn failure(name: &str) -> (ErrorKind, String, Vec<String>) {
    let error = generate(shader(name)).expect_err("the generator rejects the shader");
    let mut causes = Vec::new();
    let mut cause = error.source();
    while let Some(error) = cause {
        causes.push(error.to_string());
        cause = error.source();
    }
    (error.kind(), error.to_string(), causes)
}

pub(crate) fn same_file(left: &Path, right: &Path) -> bool {
    std::fs::canonicalize(left).expect("an existing file")
        == std::fs::canonicalize(right).expect("an existing file")
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::redacted;

    #[test]
    fn a_path_with_slashes_becomes_the_placeholder_and_its_file_name() {
        assert_eq!(
            redacted(Path::new("/tmp/shaders/saxpy.slang")),
            PathBuf::from("<shaders>/saxpy.slang")
        );
    }

    #[test]
    fn a_windows_path_with_backslashes_becomes_the_same_text_as_on_linux() {
        for path in [
            "D:\\a\\slang-rust\\tests\\codegen\\shaders\\saxpy.slang",
            "\\\\?\\D:\\a\\slang-rust\\tests\\codegen\\shaders\\saxpy.slang",
            "D:\\a\\slang-rust\\tests/codegen/shaders\\saxpy.slang",
        ] {
            let redacted = redacted(Path::new(path));
            assert_eq!(redacted.to_str(), Some("<shaders>/saxpy.slang"), "{path}");
        }
    }
}
