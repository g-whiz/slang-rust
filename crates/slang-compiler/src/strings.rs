use std::ffi::CString;

use crate::error::{Error, ErrorKind, Operation, Result};

/// Converts text that the crate passes to Slang into a C string, and reports
/// an interior NUL byte as an error of kind `InvalidArgument` (SLC-15).
///
/// `what` names the text in the error, such as `the profile`.
pub(crate) fn c_string(text: &str, operation: Operation, what: &str) -> Result<CString> {
    CString::new(text).map_err(|error| {
        Error::new(ErrorKind::InvalidArgument, operation).with_detail(format!(
            "{what} {text:?} contains a NUL byte at byte {}, which Slang cannot take",
            error.nul_position()
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::c_string;
    use crate::{ErrorKind, Operation};

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
            "cannot create a session: the profile \"a\\0b\" contains a NUL byte at byte 1, which Slang cannot take"
        );
    }
}
