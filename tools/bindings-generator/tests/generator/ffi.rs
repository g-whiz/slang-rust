use std::error::Error;
use std::path::Path;

const REGENERATE: &str = "run `just generate-bindings` and commit the regenerated files together with the change that caused the difference";

fn describe(error: &dyn Error) -> String {
    let mut text = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        text.push_str("\n  caused by: ");
        text.push_str(&cause.to_string());
        source = cause.source();
    }
    text
}

#[test]
fn checked_in_bindings_equal_a_regeneration_from_the_vulkan_sdk() {
    let sdk = std::env::var_os("VULKAN_SDK")
        .filter(|sdk| !sdk.is_empty())
        .unwrap_or_else(|| {
            panic!(
                "VULKAN_SDK is not set, so the bindings cannot be regenerated. Set it to the directory of the installed Vulkan SDK {}",
                bindings_generator::pin::VULKAN_SDK_VERSION
            )
        });
    let headers = Path::new(&sdk).join("include").join("slang");
    let generated = bindings_generator::generate(&headers)
        .unwrap_or_else(|error| panic!("cannot regenerate the bindings: {}", describe(&error)));
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let differences: Vec<String> = generated
        .files()
        .into_iter()
        .filter_map(|(path, text)| {
            let path = root.join(path);
            let checked_in = std::fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
            (text != checked_in).then(|| {
                format!(
                    "{} differs from the file that the generator writes from {}, {}",
                    path.display(),
                    headers.display(),
                    difference(text, &checked_in)
                )
            })
        })
        .collect();
    assert!(
        differences.is_empty(),
        "{}\nThe generated files are never edited by hand, so {REGENERATE}",
        differences.join("\n")
    );
}

fn difference(generated: &str, checked_in: &str) -> String {
    generated
        .lines()
        .zip(checked_in.lines())
        .position(|(fresh, old)| fresh != old)
        .map_or_else(
            || {
                let (fresh, old) = (generated.lines().count(), checked_in.lines().count());
                if fresh == old {
                    "they differ only in line endings or in the final newline".to_owned()
                } else {
                    format!("the regeneration has {fresh} lines and the checked-in file has {old}")
                }
            },
            |index| {
                format!(
                    "they first differ at line {}\n  regenerated: {}\n  checked in:  {}",
                    index + 1,
                    generated.lines().nth(index).unwrap_or_default(),
                    checked_in.lines().nth(index).unwrap_or_default()
                )
            },
        )
}
