use std::ffi::{CStr, CString, c_char};
use std::str::Utf8Error;

use crate::error::{Error, ErrorKind, Operation, Result};

/// Converts text that the crate passes to Slang into a C string, and reports
/// an interior NUL byte as an error of kind `InvalidArgument`.
///
/// `what` names the text in the error, such as `the profile`. The error
/// gives the position of the NUL byte and not the text, which can be a whole
/// shader source.
pub(crate) fn c_string(text: &str, operation: Operation, what: &str) -> Result<CString> {
    CString::new(text).map_err(|error| {
        Error::new(ErrorKind::InvalidArgument, operation).with_detail(format!(
            "{what} contains a NUL byte at byte {}, which Slang cannot take",
            error.nul_position()
        ))
    })
}

/// Reads a string that Slang returned, as text borrowed for `'a`.
///
/// A null pointer is `None`, and bytes that are not UTF-8 are an error of
/// kind `InvalidUtf8` whose detail names the string as `what`.
///
/// # Safety
///
/// `raw` is null or points to a NUL-terminated string that stays alive and
/// unchanged for `'a`. Breaking this is undefined behaviour.
pub(crate) unsafe fn borrowed_str<'a>(
    raw: *const c_char,
    operation: Operation,
    what: &str,
) -> Result<Option<&'a str>> {
    if raw.is_null() {
        return Ok(None);
    }
    // SAFETY: `raw` is non-null, and the caller guarantees that it points to
    // a NUL-terminated string that lives for `'a`.
    let text = unsafe { CStr::from_ptr(raw) };
    text.to_str()
        .map(Some)
        .map_err(|error| invalid_utf8(operation, what, error))
}

/// Returns the error of kind `InvalidUtf8` for text from Slang that is not
/// UTF-8, whose detail names the text as `what`.
pub(crate) fn invalid_utf8(operation: Operation, what: &str, error: Utf8Error) -> Error {
    Error::new(ErrorKind::InvalidUtf8, operation)
        .with_detail(format!("{what} is not UTF-8: {error}"))
}

#[cfg(test)]
mod tests {
    use std::ffi::c_char;
    use std::ptr;

    use super::{borrowed_str, c_string};
    use crate::{ErrorKind, Operation};

    #[test]
    fn borrowed_str_reads_null_as_none_and_checks_utf8() {
        // SAFETY: a null pointer is allowed.
        let read = unsafe { borrowed_str(ptr::null(), Operation::Reflect, "the name") };
        assert_eq!(read.ok(), Some(None));
        let name = c"value";
        // SAFETY: `name` is a static NUL-terminated string.
        let read = unsafe { borrowed_str(name.as_ptr(), Operation::Reflect, "the name") };
        assert_eq!(read.ok(), Some(Some("value")));
        let invalid = [0xFF_u8, 0];
        // SAFETY: `invalid` is NUL-terminated and outlives the call.
        let error = unsafe {
            borrowed_str(
                invalid.as_ptr().cast::<c_char>(),
                Operation::ReadModuleName,
                "the name",
            )
        }
        .expect_err("0xFF is not UTF-8");
        assert_eq!(error.kind(), ErrorKind::InvalidUtf8);
        assert_eq!(error.operation(), Operation::ReadModuleName);
        assert!(
            error.to_string().ends_with(
                ": the name is not UTF-8: invalid utf-8 sequence of 1 bytes from index 0"
            ),
            "{error}"
        );
    }

    #[test]
    fn c_string_keeps_text_without_a_nul_byte() {
        let text = c_string("spirv_1_5", Operation::CreateSession, "the profile")
            .expect("the text has no NUL byte");
        assert_eq!(text.as_bytes(), b"spirv_1_5");
    }

    #[test]
    fn c_string_rejects_an_interior_nul_byte_and_names_the_text() {
        let error = c_string("a\0b", Operation::CreateSession, "the profile")
            .expect_err("a NUL byte fails");
        assert_eq!(error.kind(), ErrorKind::InvalidArgument);
        assert_eq!(error.operation(), Operation::CreateSession);
        assert_eq!(
            error.to_string(),
            "cannot create a session: the profile contains a NUL byte at byte 1, which Slang cannot take"
        );
    }

    #[test]
    fn c_string_leaves_a_long_text_out_of_the_error() {
        let source = format!("{}\0{}", "x".repeat(10_000), "y".repeat(10_000));
        let error =
            c_string(&source, Operation::LoadModule, "the source").expect_err("a NUL byte fails");
        assert_eq!(
            error.to_string(),
            "cannot load module: the source contains a NUL byte at byte 10000, which Slang cannot take"
        );
    }
}
