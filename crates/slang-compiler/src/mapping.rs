use slang_compiler_sys::SlangResult;

use crate::error::{Diagnostics, Error, ErrorKind, Operation};

/// How Slang signalled the failure of a call.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Failure {
    /// A negative result code.
    Code(SlangResult),
    /// A null return.
    Null,
    /// A file system error or panic that the crate recorded during the call.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "the custom file system that records this failure arrives in a later step, and the unit tests of the mapping use it"
        )
    )]
    FileSystem,
}

/// The rows of the mapping that apply only to some operations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Class {
    TargetSupportCheck,
    ModuleLoader,
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "the lookups by name arrive in a later step, and the unit tests of the mapping use this class"
        )
    )]
    LookupByName,
    Other,
}

impl Operation {
    fn class(self) -> Class {
        match self {
            Self::CheckTargetSupport => Class::TargetSupportCheck,
            Self::CreateGlobalSession => Class::Other,
        }
    }
}

/// Returns `Ok` for a result code that is not negative, and the classified
/// error for a negative one.
pub(crate) fn check(
    operation: Operation,
    result: SlangResult,
    diagnostics: Diagnostics,
) -> Result<(), Error> {
    if result.0 >= 0 {
        Ok(())
    } else {
        Err(error(operation, Failure::Code(result), diagnostics))
    }
}

/// Returns the classified error for a null return.
pub(crate) fn null(operation: Operation, diagnostics: Diagnostics) -> Error {
    error(operation, Failure::Null, diagnostics)
}

/// Returns the error for a failure that Slang signalled during `operation`.
pub(crate) fn error(operation: Operation, failure: Failure, diagnostics: Diagnostics) -> Error {
    let kind = classify(operation.class(), failure, &diagnostics);
    let error = Error::new(kind, operation).with_diagnostics(diagnostics);
    match failure {
        Failure::Code(code) => error.with_code(code.0),
        Failure::Null | Failure::FileSystem => error,
    }
}

/// Classifies a failure by the table of SLC-11 of
/// `docs/design/slang-compiler.md`, in which the first row that matches
/// decides the kind.
#[expect(
    clippy::match_same_arms,
    reason = "each arm is one row of the table of SLC-11, whose order decides the kind, so rows with the same kind cannot merge"
)]
fn classify(class: Class, failure: Failure, diagnostics: &Diagnostics) -> ErrorKind {
    let explained = !diagnostics.is_empty();
    match failure {
        Failure::FileSystem => ErrorKind::FileSystem,
        Failure::Code(_) if class == Class::TargetSupportCheck => ErrorKind::Unsupported,
        Failure::Null if class == Class::ModuleLoader && !explained => ErrorKind::NotFound,
        Failure::Code(SlangResult::SLANG_E_INVALID_ARG) => ErrorKind::InvalidArgument,
        Failure::Code(SlangResult::SLANG_E_NOT_FOUND | SlangResult::SLANG_E_CANNOT_OPEN) => {
            ErrorKind::NotFound
        }
        Failure::Code(SlangResult::SLANG_E_NOT_AVAILABLE) if explained => ErrorKind::Compile,
        Failure::Code(
            SlangResult::SLANG_E_NOT_IMPLEMENTED
            | SlangResult::SLANG_E_NOT_AVAILABLE
            | SlangResult::SLANG_E_NO_INTERFACE,
        ) => ErrorKind::Unsupported,
        Failure::Code(SlangResult::SLANG_E_INTERNAL_FAIL) => ErrorKind::Internal,
        Failure::Code(SlangResult::SLANG_FAIL) | Failure::Null if explained => ErrorKind::Compile,
        Failure::Code(SlangResult::SLANG_FAIL) | Failure::Null => match class {
            Class::LookupByName => ErrorKind::NotFound,
            Class::TargetSupportCheck | Class::ModuleLoader | Class::Other => ErrorKind::Internal,
        },
        Failure::Code(_) => ErrorKind::Unrecognized,
    }
}

#[cfg(test)]
mod tests {
    use slang_compiler_sys::SlangResult;

    use super::{Class, Failure, check, classify, null};
    use crate::error::{Diagnostics, ErrorKind, Operation};

    const ALL_CLASSES: [Class; 4] = [
        Class::TargetSupportCheck,
        Class::ModuleLoader,
        Class::LookupByName,
        Class::Other,
    ];

    fn none() -> Diagnostics {
        Diagnostics::default()
    }

    fn some() -> Diagnostics {
        Diagnostics::from_text("error[E30015]: undefined identifier")
    }

    fn code(result: SlangResult) -> Failure {
        Failure::Code(result)
    }

    #[test]
    fn a_recorded_file_system_failure_wins_over_every_other_row() {
        for class in ALL_CLASSES {
            for diagnostics in [none(), some()] {
                assert_eq!(
                    classify(class, Failure::FileSystem, &diagnostics),
                    ErrorKind::FileSystem
                );
            }
        }
    }

    #[test]
    fn every_negative_code_of_a_target_support_check_is_unsupported() {
        for result in [
            SlangResult::SLANG_E_NOT_FOUND,
            SlangResult::SLANG_E_INVALID_ARG,
            SlangResult::SLANG_FAIL,
            SlangResult::SLANG_E_INTERNAL_FAIL,
            SlangResult(-7),
        ] {
            assert_eq!(
                classify(Class::TargetSupportCheck, code(result), &none()),
                ErrorKind::Unsupported,
                "{result:?}"
            );
        }
    }

    #[test]
    fn a_module_loader_null_without_diagnostics_is_not_found() {
        assert_eq!(
            classify(Class::ModuleLoader, Failure::Null, &none()),
            ErrorKind::NotFound
        );
        assert_eq!(
            classify(Class::ModuleLoader, Failure::Null, &some()),
            ErrorKind::Compile
        );
        assert_eq!(
            classify(Class::Other, Failure::Null, &none()),
            ErrorKind::Internal
        );
    }

    #[test]
    fn invalid_arg_is_invalid_argument_for_every_other_class() {
        for class in [Class::ModuleLoader, Class::LookupByName, Class::Other] {
            for diagnostics in [none(), some()] {
                assert_eq!(
                    classify(class, code(SlangResult::SLANG_E_INVALID_ARG), &diagnostics),
                    ErrorKind::InvalidArgument
                );
            }
        }
    }

    #[test]
    fn not_found_and_cannot_open_are_not_found() {
        for result in [
            SlangResult::SLANG_E_NOT_FOUND,
            SlangResult::SLANG_E_CANNOT_OPEN,
        ] {
            for diagnostics in [none(), some()] {
                assert_eq!(
                    classify(Class::Other, code(result), &diagnostics),
                    ErrorKind::NotFound
                );
            }
        }
    }

    #[test]
    fn not_available_is_compile_with_diagnostics_and_unsupported_without() {
        let result = code(SlangResult::SLANG_E_NOT_AVAILABLE);
        assert_eq!(classify(Class::Other, result, &some()), ErrorKind::Compile);
        assert_eq!(
            classify(Class::Other, result, &none()),
            ErrorKind::Unsupported
        );
    }

    #[test]
    fn not_implemented_and_no_interface_are_unsupported() {
        for result in [
            SlangResult::SLANG_E_NOT_IMPLEMENTED,
            SlangResult::SLANG_E_NO_INTERFACE,
        ] {
            for diagnostics in [none(), some()] {
                assert_eq!(
                    classify(Class::Other, code(result), &diagnostics),
                    ErrorKind::Unsupported
                );
            }
        }
    }

    #[test]
    fn internal_fail_is_internal_even_with_diagnostics() {
        assert_eq!(
            classify(
                Class::Other,
                code(SlangResult::SLANG_E_INTERNAL_FAIL),
                &some()
            ),
            ErrorKind::Internal
        );
    }

    #[test]
    fn fail_or_null_with_diagnostics_is_compile_for_every_other_class() {
        for class in [Class::ModuleLoader, Class::LookupByName, Class::Other] {
            for failure in [code(SlangResult::SLANG_FAIL), Failure::Null] {
                assert_eq!(classify(class, failure, &some()), ErrorKind::Compile);
            }
        }
    }

    #[test]
    fn fail_or_null_of_a_lookup_by_name_without_diagnostics_is_not_found() {
        for failure in [code(SlangResult::SLANG_FAIL), Failure::Null] {
            assert_eq!(
                classify(Class::LookupByName, failure, &none()),
                ErrorKind::NotFound
            );
        }
    }

    #[test]
    fn fail_or_null_of_any_other_operation_without_diagnostics_is_internal() {
        assert_eq!(
            classify(Class::Other, code(SlangResult::SLANG_FAIL), &none()),
            ErrorKind::Internal
        );
        assert_eq!(
            classify(Class::ModuleLoader, code(SlangResult::SLANG_FAIL), &none()),
            ErrorKind::Internal
        );
        assert_eq!(
            classify(Class::Other, Failure::Null, &none()),
            ErrorKind::Internal
        );
    }

    #[test]
    fn any_other_negative_code_is_unrecognized_whether_the_header_names_it_or_not() {
        for result in [
            SlangResult::SLANG_E_OUT_OF_MEMORY,
            SlangResult::SLANG_E_TIME_OUT,
            SlangResult::SLANG_E_ABORT,
            SlangResult(-1),
            SlangResult(i32::MIN),
        ] {
            for diagnostics in [none(), some()] {
                assert_eq!(
                    classify(Class::Other, code(result), &diagnostics),
                    ErrorKind::Unrecognized,
                    "{result:?}"
                );
            }
        }
    }

    #[test]
    fn check_accepts_every_non_negative_code_as_success() {
        for result in [SlangResult::SLANG_OK, SlangResult(1), SlangResult(i32::MAX)] {
            assert!(check(Operation::CreateGlobalSession, result, none()).is_ok());
        }
    }

    #[test]
    fn check_keeps_the_code_and_the_diagnostics_of_a_failure() {
        let error = check(
            Operation::CreateGlobalSession,
            SlangResult::SLANG_FAIL,
            some(),
        )
        .expect_err("a negative code fails");
        assert_eq!(error.kind(), ErrorKind::Compile);
        assert_eq!(error.code(), Some(SlangResult::SLANG_FAIL.0));
        assert_eq!(error.diagnostics(), &some());
    }

    #[test]
    fn check_target_support_maps_through_its_own_row() {
        let error = check(
            Operation::CheckTargetSupport,
            SlangResult::SLANG_E_NOT_FOUND,
            none(),
        )
        .expect_err("a negative code fails");
        assert_eq!(error.kind(), ErrorKind::Unsupported);
    }

    #[test]
    fn a_null_return_has_no_code() {
        let error = null(Operation::CreateGlobalSession, none());
        assert_eq!(error.kind(), ErrorKind::Internal);
        assert_eq!(error.code(), None);
    }
}
