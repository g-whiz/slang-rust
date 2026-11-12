//! The report of an error that every example prints on standard error, which
//! each binary includes with a `#[path]` attribute.

use std::error::Error;

/// Prints `error` on standard error after the name of the example, and then
/// each error that caused it on a line that starts with `  caused by: `.
pub(crate) fn report(error: &dyn Error) {
    eprintln!("{}: {error}", env!("CARGO_BIN_NAME"));
    let mut source = error.source();
    while let Some(cause) = source {
        eprintln!("  caused by: {cause}");
        source = cause.source();
    }
}
