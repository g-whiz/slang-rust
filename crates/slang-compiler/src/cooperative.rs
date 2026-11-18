use std::fmt;
use std::marker::PhantomData;
use std::ptr::NonNull;

use slang_compiler_sys::{
    CooperativeMatrixCombination as RawMatrixCombination, CooperativeMatrixType as RawMatrixType,
    CooperativeVectorCombination as RawVectorCombination,
    CooperativeVectorTypeUsageInfo as RawVectorType, ICooperativeTypesMetadata,
    SlangCooperativeMatrixUse, SlangResult, SlangScope, SlangUInt,
};

use crate::blob::Metadata;
use crate::enums::unrecognized;
use crate::error::{Diagnostics, Operation, Result};
use crate::guard;
use crate::mapping;
use crate::reflection::ScalarType;

/// The scope of the threads that work together on one cooperative matrix,
/// such as a subgroup of a compute dispatch, which Vulkan calls a subgroup
/// and Slang a wave.
///
/// Slang calls it `SlangScope`. The enumeration is non-exhaustive, so a later
/// version can add scopes, and a `match` on it needs a wildcard arm.
///
/// # Examples
///
/// See the example of [`CooperativeTypes`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Scope {
    /// No scope, as Slang's `SLANG_SCOPE_NONE`.
    None,
    /// One thread, as Slang's `SLANG_SCOPE_THREAD`.
    Thread,
    /// The threads of one wave, which Vulkan calls a subgroup, as Slang's
    /// `SLANG_SCOPE_WAVE`.
    Wave,
    /// The threads of one thread group, which Vulkan calls a workgroup, as
    /// Slang's `SLANG_SCOPE_THREAD_GROUP`.
    ThreadGroup,
}

impl Scope {
    /// Converts a scope that Slang returned.
    fn from_raw(raw: SlangScope) -> Result<Self> {
        Ok(match raw {
            SlangScope::SLANG_SCOPE_NONE => Self::None,
            SlangScope::SLANG_SCOPE_THREAD => Self::Thread,
            SlangScope::SLANG_SCOPE_WAVE => Self::Wave,
            SlangScope::SLANG_SCOPE_THREAD_GROUP => Self::ThreadGroup,
            SlangScope(other) => return Err(unrecognized("scope", other)),
        })
    }
}

/// The operand of a multiplication of cooperative matrices that a matrix
/// type is used as, in `A * B + C`.
///
/// Slang calls it `SlangCooperativeMatrixUse`. The enumeration is
/// non-exhaustive, so a later version can add uses, and a `match` on it needs
/// a wildcard arm.
///
/// # Examples
///
/// See the example of [`CooperativeTypes`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum CooperativeMatrixUse {
    /// The left operand `A`, as Slang's `SLANG_COOPERATIVE_MATRIX_USE_A`.
    A,
    /// The right operand `B`, as Slang's `SLANG_COOPERATIVE_MATRIX_USE_B`.
    B,
    /// The accumulator `C` and the result, as Slang's
    /// `SLANG_COOPERATIVE_MATRIX_USE_ACCUMULATOR`.
    Accumulator,
}

impl CooperativeMatrixUse {
    /// Converts a matrix use that Slang returned.
    fn from_raw(raw: SlangCooperativeMatrixUse) -> Result<Self> {
        use SlangCooperativeMatrixUse as U;
        Ok(match raw {
            U::SLANG_COOPERATIVE_MATRIX_USE_A => Self::A,
            U::SLANG_COOPERATIVE_MATRIX_USE_B => Self::B,
            U::SLANG_COOPERATIVE_MATRIX_USE_ACCUMULATOR => Self::Accumulator,
            SlangCooperativeMatrixUse(other) => {
                return Err(unrecognized("cooperative matrix use", other));
            }
        })
    }
}

/// A cooperative matrix type that generated code uses, a matrix that the
/// threads of a scope hold together and multiply in one operation.
///
/// The struct is non-exhaustive, so a later version can add fields, and only
/// this crate creates it.
///
/// # Examples
///
/// See the example of [`CooperativeTypes`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct CooperativeMatrixType {
    /// The type of the elements.
    pub component_type: ScalarType,
    /// The scope of the threads that hold the matrix.
    pub scope: Scope,
    /// The number of rows.
    pub row_count: u32,
    /// The number of columns.
    pub column_count: u32,
    /// The operand that the matrix is used as, which Slang calls `use`.
    pub matrix_use: CooperativeMatrixUse,
}

impl CooperativeMatrixType {
    fn from_raw(raw: RawMatrixType) -> Result<Self> {
        Ok(Self {
            component_type: ScalarType::from_raw(raw.componentType)?,
            scope: Scope::from_raw(raw.scope)?,
            row_count: raw.rowCount,
            column_count: raw.columnCount,
            matrix_use: CooperativeMatrixUse::from_raw(raw.r#use)?,
        })
    }
}

/// A multiplication of cooperative matrices that generated code performs,
/// `A * B + C` with `A` of `m` by `k`, `B` of `k` by `n`, and `C` and the
/// result of `m` by `n` elements.
///
/// The struct is non-exhaustive, so a later version can add fields, and only
/// this crate creates it.
///
/// # Examples
///
/// See the example of [`CooperativeTypes`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct CooperativeMatrixCombination {
    /// The number of rows of `A`, `C`, and the result.
    pub m: u32,
    /// The number of columns of `B`, `C`, and the result.
    pub n: u32,
    /// The number of columns of `A` and of rows of `B`.
    pub k: u32,
    /// The type of the elements of `A`.
    pub component_type_a: ScalarType,
    /// The type of the elements of `B`.
    pub component_type_b: ScalarType,
    /// The type of the elements of `C`.
    pub component_type_c: ScalarType,
    /// The type of the elements of the result.
    pub component_type_result: ScalarType,
    /// Whether the operation saturates, which clamps each result to the
    /// range of its type.
    pub saturate: bool,
    /// The scope of the threads that hold the matrices.
    pub scope: Scope,
}

impl CooperativeMatrixCombination {
    fn from_raw(raw: RawMatrixCombination) -> Result<Self> {
        Ok(Self {
            m: raw.m,
            n: raw.n,
            k: raw.k,
            component_type_a: ScalarType::from_raw(raw.componentTypeA)?,
            component_type_b: ScalarType::from_raw(raw.componentTypeB)?,
            component_type_c: ScalarType::from_raw(raw.componentTypeC)?,
            component_type_result: ScalarType::from_raw(raw.componentTypeResult)?,
            saturate: raw.saturate,
            scope: Scope::from_raw(raw.scope)?,
        })
    }
}

/// A component type of cooperative vectors that generated code uses, with
/// the largest number of elements that it uses for that type.
///
/// A cooperative vector is a vector that a matrix-vector operation multiplies
/// with a matrix in memory, such as one layer of a neural network. The struct
/// is non-exhaustive, so a later version can add fields, and only this crate
/// creates it.
///
/// # Examples
///
/// See the example of [`CooperativeTypes`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct CooperativeVectorTypeUsageInfo {
    /// The type of the elements.
    pub component_type: ScalarType,
    /// The largest number of elements that the code uses for this type.
    pub max_size: u32,
    /// Whether the code uses the type in an operation of training, such as
    /// the accumulation of an outer product.
    pub used_for_training_op: bool,
}

impl CooperativeVectorTypeUsageInfo {
    fn from_raw(raw: RawVectorType) -> Result<Self> {
        Ok(Self {
            component_type: ScalarType::from_raw(raw.componentType)?,
            max_size: raw.maxSize,
            used_for_training_op: raw.usedForTrainingOp,
        })
    }
}

/// A matrix-vector operation on cooperative vectors that generated code
/// performs, with the types in which it reads and interprets each operand.
///
/// The struct is non-exhaustive, so a later version can add fields, and only
/// this crate creates it.
///
/// # Examples
///
/// See the example of [`CooperativeTypes`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct CooperativeVectorCombination {
    /// The type of the elements of the input vector.
    pub input_type: ScalarType,
    /// The type as which the operation interprets the input elements.
    pub input_interpretation: ScalarType,
    /// The number of input elements packed in one element of the input type.
    pub input_packing_factor: u32,
    /// The type as which the operation interprets the matrix elements.
    pub matrix_interpretation: ScalarType,
    /// The type as which the operation interprets the bias elements.
    pub bias_interpretation: ScalarType,
    /// The type of the elements of the result.
    pub result_type: ScalarType,
    /// Whether the operation reads the matrix transposed.
    pub transpose: bool,
}

impl CooperativeVectorCombination {
    fn from_raw(raw: RawVectorCombination) -> Result<Self> {
        Ok(Self {
            input_type: ScalarType::from_raw(raw.inputType)?,
            input_interpretation: ScalarType::from_raw(raw.inputInterpretation)?,
            input_packing_factor: raw.inputPackingFactor,
            matrix_interpretation: ScalarType::from_raw(raw.matrixInterpretation)?,
            bias_interpretation: ScalarType::from_raw(raw.biasInterpretation)?,
            result_type: ScalarType::from_raw(raw.resultType)?,
            transpose: raw.transpose,
        })
    }
}

/// The cooperative matrix and vector types that the code of [`Metadata`]
/// uses, which [`Metadata::cooperative_types`] returns.
///
/// A graphics API reports which cooperative types and operations a device
/// supports, and a program compares them with these lists before it runs the
/// code. The lists are empty for a target that lowers cooperative types to
/// plain arrays, because they hold only the types that stay native in the
/// generated code. Slang calls it `ICooperativeTypesMetadata`. It borrows the
/// metadata, so it cannot outlive it.
///
/// # Examples
///
/// This example prints the cooperative vector types of the code of an entry
/// point:
///
/// ```
/// use slang_compiler::Metadata;
///
/// fn print_vector_types(metadata: &Metadata) -> slang_compiler::Result<()> {
///     let Some(cooperative) = metadata.cooperative_types() else {
///         return Ok(());
///     };
///     for vector in cooperative.cooperative_vector_types()? {
///         println!(
///             "{:?} up to {} elements, training: {}",
///             vector.component_type, vector.max_size, vector.used_for_training_op
///         );
///     }
///     for matrix in cooperative.cooperative_matrix_types()? {
///         println!(
///             "{:?} {}x{} as {:?} in {:?}",
///             matrix.component_type, matrix.row_count, matrix.column_count, matrix.matrix_use, matrix.scope
///         );
///     }
///     Ok(())
/// }
/// # let _ = print_vector_types;
/// ```
///
/// The cooperative types cannot outlive the metadata that they came from, so
/// this example does not compile:
///
/// ```compile_fail,E0505
/// use slang_compiler::Metadata;
///
/// fn outlive(metadata: Metadata) {
///     let cooperative = metadata.cooperative_types();
///     drop(metadata);
///     let _ = cooperative.map(|c| c.cooperative_matrix_types());
/// }
/// ```
///
/// The cooperative types cannot move to another thread, so this example does
/// not compile:
///
/// ```compile_fail,E0277
/// fn assert_send<T: Send>() {}
/// assert_send::<slang_compiler::CooperativeTypes<'static>>();
/// ```
///
/// The cooperative types cannot be shared between threads either, so this
/// example does not compile:
///
/// ```compile_fail,E0277
/// fn assert_sync<T: Sync>() {}
/// assert_sync::<slang_compiler::CooperativeTypes<'static>>();
/// ```
#[derive(Clone, Copy)]
pub struct CooperativeTypes<'m> {
    raw: NonNull<ICooperativeTypesMetadata>,
    metadata: PhantomData<&'m Metadata>,
}

impl<'m> CooperativeTypes<'m> {
    /// Wraps the interface that `castAs` of the metadata returned.
    ///
    /// # Safety
    ///
    /// `raw` points to the cooperative types interface of the metadata object
    /// that `_metadata` holds, which that object returned without adding a
    /// reference and which therefore lives as long as it. Breaking this is
    /// undefined behaviour.
    pub(crate) unsafe fn new(
        raw: NonNull<ICooperativeTypesMetadata>,
        _metadata: &'m Metadata,
    ) -> Self {
        Self {
            raw,
            metadata: PhantomData,
        }
    }

    /// Returns the cooperative matrix types that the code uses.
    ///
    /// # Examples
    ///
    /// See the example of [`CooperativeTypes`].
    ///
    /// # Errors
    ///
    /// Returns the kind that Slang's result code maps to when Slang fails to
    /// return a record, and an error of kind
    /// [`ErrorKind::Unrecognized`](crate::ErrorKind::Unrecognized) when a
    /// record holds a value that the Slang header does not list.
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    pub fn cooperative_matrix_types(self) -> Result<Vec<CooperativeMatrixType>> {
        let raw = self.raw.as_ptr();
        guard::call("CooperativeTypes::cooperative_matrix_types", || {
            read_all(
                || {
                    // SAFETY: `raw` is the live interface of the metadata that
                    // `'m` borrows, used only on this thread, and the call runs
                    // with the Slang call guard raised.
                    unsafe {
                        guard::abort_on_exception(
                            "ICooperativeTypesMetadata::getCooperativeMatrixTypeCount",
                            || ICooperativeTypesMetadata::getCooperativeMatrixTypeCount(raw),
                        )
                    }
                },
                |index, out: &mut RawMatrixType| {
                    // SAFETY: as for the count, `index` is below the count that
                    // Slang reported, and `out` is a live out-pointer.
                    unsafe {
                        guard::abort_on_exception(
                            "ICooperativeTypesMetadata::getCooperativeMatrixTypeByIndex",
                            || {
                                ICooperativeTypesMetadata::getCooperativeMatrixTypeByIndex(
                                    raw, index, out,
                                )
                            },
                        )
                    }
                },
                CooperativeMatrixType::from_raw,
            )
        })
    }

    /// Returns the multiplications of cooperative matrices that the code
    /// performs.
    ///
    /// # Examples
    ///
    /// This example collects the sizes of the multiplications:
    ///
    /// ```
    /// use slang_compiler::CooperativeTypes;
    ///
    /// fn sizes(cooperative: CooperativeTypes<'_>) -> slang_compiler::Result<Vec<[u32; 3]>> {
    ///     Ok(cooperative
    ///         .cooperative_matrix_combinations()?
    ///         .into_iter()
    ///         .map(|combination| [combination.m, combination.n, combination.k])
    ///         .collect())
    /// }
    /// # let _ = sizes;
    /// ```
    ///
    /// # Errors
    ///
    /// Returns the kind that Slang's result code maps to when Slang fails to
    /// return a record, and an error of kind
    /// [`ErrorKind::Unrecognized`](crate::ErrorKind::Unrecognized) when a
    /// record holds a value that the Slang header does not list.
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    pub fn cooperative_matrix_combinations(self) -> Result<Vec<CooperativeMatrixCombination>> {
        let raw = self.raw.as_ptr();
        guard::call("CooperativeTypes::cooperative_matrix_combinations", || {
            read_all(
                || {
                    // SAFETY: as for `cooperative_matrix_types`.
                    unsafe {
                        guard::abort_on_exception(
                            "ICooperativeTypesMetadata::getCooperativeMatrixCombinationCount",
                            || ICooperativeTypesMetadata::getCooperativeMatrixCombinationCount(raw),
                        )
                    }
                },
                |index, out: &mut RawMatrixCombination| {
                    // SAFETY: as for `cooperative_matrix_types`.
                    unsafe {
                        guard::abort_on_exception(
                            "ICooperativeTypesMetadata::getCooperativeMatrixCombinationByIndex",
                            || {
                                ICooperativeTypesMetadata::getCooperativeMatrixCombinationByIndex(
                                    raw, index, out,
                                )
                            },
                        )
                    }
                },
                CooperativeMatrixCombination::from_raw,
            )
        })
    }

    /// Returns the component types of cooperative vectors that the code
    /// uses.
    ///
    /// # Examples
    ///
    /// See the example of [`CooperativeTypes`].
    ///
    /// # Errors
    ///
    /// Returns the kind that Slang's result code maps to when Slang fails to
    /// return a record, and an error of kind
    /// [`ErrorKind::Unrecognized`](crate::ErrorKind::Unrecognized) when a
    /// record holds a value that the Slang header does not list.
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    pub fn cooperative_vector_types(self) -> Result<Vec<CooperativeVectorTypeUsageInfo>> {
        let raw = self.raw.as_ptr();
        guard::call("CooperativeTypes::cooperative_vector_types", || {
            read_all(
                || {
                    // SAFETY: as for `cooperative_matrix_types`.
                    unsafe {
                        guard::abort_on_exception(
                            "ICooperativeTypesMetadata::getCooperativeVectorTypeCount",
                            || ICooperativeTypesMetadata::getCooperativeVectorTypeCount(raw),
                        )
                    }
                },
                |index, out: &mut RawVectorType| {
                    // SAFETY: as for `cooperative_matrix_types`.
                    unsafe {
                        guard::abort_on_exception(
                            "ICooperativeTypesMetadata::getCooperativeVectorTypeByIndex",
                            || {
                                ICooperativeTypesMetadata::getCooperativeVectorTypeByIndex(
                                    raw, index, out,
                                )
                            },
                        )
                    }
                },
                CooperativeVectorTypeUsageInfo::from_raw,
            )
        })
    }

    /// Returns the matrix-vector operations on cooperative vectors that the
    /// code performs.
    ///
    /// # Examples
    ///
    /// This example tells whether any operation reads its matrix transposed:
    ///
    /// ```
    /// use slang_compiler::CooperativeTypes;
    ///
    /// fn any_transposed(cooperative: CooperativeTypes<'_>) -> slang_compiler::Result<bool> {
    ///     Ok(cooperative
    ///         .cooperative_vector_combinations()?
    ///         .iter()
    ///         .any(|combination| combination.transpose))
    /// }
    /// # let _ = any_transposed;
    /// ```
    ///
    /// # Errors
    ///
    /// Returns the kind that Slang's result code maps to when Slang fails to
    /// return a record, and an error of kind
    /// [`ErrorKind::Unrecognized`](crate::ErrorKind::Unrecognized) when a
    /// record holds a value that the Slang header does not list.
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    pub fn cooperative_vector_combinations(self) -> Result<Vec<CooperativeVectorCombination>> {
        let raw = self.raw.as_ptr();
        guard::call("CooperativeTypes::cooperative_vector_combinations", || {
            read_all(
                || {
                    // SAFETY: as for `cooperative_matrix_types`.
                    unsafe {
                        guard::abort_on_exception(
                            "ICooperativeTypesMetadata::getCooperativeVectorCombinationCount",
                            || ICooperativeTypesMetadata::getCooperativeVectorCombinationCount(raw),
                        )
                    }
                },
                |index, out: &mut RawVectorCombination| {
                    // SAFETY: as for `cooperative_matrix_types`.
                    unsafe {
                        guard::abort_on_exception(
                            "ICooperativeTypesMetadata::getCooperativeVectorCombinationByIndex",
                            || {
                                ICooperativeTypesMetadata::getCooperativeVectorCombinationByIndex(
                                    raw, index, out,
                                )
                            },
                        )
                    }
                },
                CooperativeVectorCombination::from_raw,
            )
        })
    }

    /// Returns the pointer to the interface without changing a reference
    /// count, for use with the raw bindings of `slang-compiler-sys`.
    ///
    /// The pointer is valid only while the metadata that this borrows lives.
    /// Every use of it is unsafe and follows the safety contract of
    /// `slang-compiler-sys`.
    ///
    /// # Examples
    ///
    /// This example passes the pointer on to code that uses the raw
    /// bindings:
    ///
    /// ```
    /// use slang_compiler::CooperativeTypes;
    /// use slang_compiler_sys::ICooperativeTypesMetadata;
    ///
    /// fn raw_pointer(cooperative: CooperativeTypes<'_>) -> *mut ICooperativeTypesMetadata {
    ///     cooperative.as_raw().as_ptr()
    /// }
    /// # let _ = raw_pointer;
    /// ```
    #[must_use]
    #[expect(
        clippy::trivially_copy_pass_by_ref,
        reason = "every as_raw method of the crate borrows its value, so this one does too for a uniform public API"
    )]
    pub fn as_raw(&self) -> NonNull<ICooperativeTypesMetadata> {
        self.raw
    }
}

impl fmt::Debug for CooperativeTypes<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CooperativeTypes")
            .field("raw", &self.raw)
            .finish_non_exhaustive()
    }
}

/// Reads every record of one list of the cooperative types: the count that
/// `count` returns, then the record at each position below it through `at`,
/// whose result code goes through the one mapping of result codes, and then
/// the conversion of each record.
fn read_all<R: Default, T>(
    count: impl FnOnce() -> SlangUInt,
    mut at: impl FnMut(SlangUInt, &mut R) -> SlangResult,
    convert: impl Fn(R) -> Result<T>,
) -> Result<Vec<T>> {
    let count = count();
    let mut records = Vec::new();
    for index in 0..count {
        let mut record = R::default();
        let result = at(index, &mut record);
        mapping::check(Operation::QueryMetadata, result, Diagnostics::default())?;
        records.push(convert(record)?);
    }
    Ok(records)
}

#[cfg(test)]
mod tests {
    use std::panic::{AssertUnwindSafe, catch_unwind};

    use slang_compiler_sys::{
        CooperativeMatrixCombination as RawMatrixCombination,
        CooperativeMatrixType as RawMatrixType,
        CooperativeVectorCombination as RawVectorCombination,
        CooperativeVectorTypeUsageInfo as RawVectorType, SlangCooperativeMatrixUse, SlangResult,
        SlangScalarType, SlangScope,
    };

    use super::{
        CooperativeMatrixCombination, CooperativeMatrixType, CooperativeMatrixUse,
        CooperativeVectorCombination, CooperativeVectorTypeUsageInfo, Scope,
    };
    use crate::fake::{self, CooperativeRecords, Event, FakeMetadata, Log, panic_message};
    use crate::reflection::ScalarType;
    use crate::{ErrorKind, Metadata, Operation, with_slang_call_guard};

    fn assert_send_sync<T: Send + Sync>() {}

    #[test]
    fn the_enumerations_and_records_are_send_and_sync() {
        assert_send_sync::<Scope>();
        assert_send_sync::<CooperativeMatrixUse>();
        assert_send_sync::<CooperativeMatrixType>();
        assert_send_sync::<CooperativeMatrixCombination>();
        assert_send_sync::<CooperativeVectorTypeUsageInfo>();
        assert_send_sync::<CooperativeVectorCombination>();
    }

    #[test]
    fn every_scope_and_matrix_use_converts_to_its_variant() {
        let scopes = [
            (SlangScope::SLANG_SCOPE_NONE, Scope::None),
            (SlangScope::SLANG_SCOPE_THREAD, Scope::Thread),
            (SlangScope::SLANG_SCOPE_WAVE, Scope::Wave),
            (SlangScope::SLANG_SCOPE_THREAD_GROUP, Scope::ThreadGroup),
        ];
        for (raw, scope) in scopes {
            assert_eq!(Scope::from_raw(raw).ok(), Some(scope), "{raw:?}");
        }
        assert!(
            (0..=SlangScope::SLANG_SCOPE_THREAD_GROUP.0).eq(scopes.iter().map(|(raw, _)| raw.0))
        );
        let error = Scope::from_raw(SlangScope(4)).expect_err("not a scope");
        assert_eq!(error.kind(), ErrorKind::Unrecognized);
        let uses = [
            (
                SlangCooperativeMatrixUse::SLANG_COOPERATIVE_MATRIX_USE_A,
                CooperativeMatrixUse::A,
            ),
            (
                SlangCooperativeMatrixUse::SLANG_COOPERATIVE_MATRIX_USE_B,
                CooperativeMatrixUse::B,
            ),
            (
                SlangCooperativeMatrixUse::SLANG_COOPERATIVE_MATRIX_USE_ACCUMULATOR,
                CooperativeMatrixUse::Accumulator,
            ),
        ];
        for (raw, matrix_use) in uses {
            assert_eq!(
                CooperativeMatrixUse::from_raw(raw).ok(),
                Some(matrix_use),
                "{raw:?}"
            );
        }
        assert!(
            (0..=SlangCooperativeMatrixUse::SLANG_COOPERATIVE_MATRIX_USE_ACCUMULATOR.0)
                .eq(uses.iter().map(|(raw, _)| raw.0))
        );
        let error =
            CooperativeMatrixUse::from_raw(SlangCooperativeMatrixUse(3)).expect_err("not a use");
        assert_eq!(error.kind(), ErrorKind::Unrecognized);
    }

    fn matrix_type(scope: SlangScope) -> RawMatrixType {
        RawMatrixType {
            componentType: SlangScalarType::SLANG_SCALAR_TYPE_FLOAT16,
            scope,
            rowCount: 16,
            columnCount: 8,
            r#use: SlangCooperativeMatrixUse::SLANG_COOPERATIVE_MATRIX_USE_B,
        }
    }

    fn records() -> CooperativeRecords {
        CooperativeRecords {
            matrix_types: vec![matrix_type(SlangScope::SLANG_SCOPE_WAVE)],
            matrix_combinations: vec![RawMatrixCombination {
                m: 16,
                n: 8,
                k: 4,
                componentTypeA: SlangScalarType::SLANG_SCALAR_TYPE_INT8,
                componentTypeB: SlangScalarType::SLANG_SCALAR_TYPE_INT8,
                componentTypeC: SlangScalarType::SLANG_SCALAR_TYPE_INT32,
                componentTypeResult: SlangScalarType::SLANG_SCALAR_TYPE_INT32,
                saturate: true,
                scope: SlangScope::SLANG_SCOPE_THREAD_GROUP,
            }],
            vector_types: vec![
                RawVectorType {
                    componentType: SlangScalarType::SLANG_SCALAR_TYPE_INT8,
                    maxSize: 8,
                    usedForTrainingOp: false,
                },
                RawVectorType {
                    componentType: SlangScalarType::SLANG_SCALAR_TYPE_FLOAT32,
                    maxSize: 4,
                    usedForTrainingOp: true,
                },
            ],
            vector_combinations: vec![RawVectorCombination {
                inputType: SlangScalarType::SLANG_SCALAR_TYPE_UINT32,
                inputInterpretation: SlangScalarType::SLANG_SCALAR_TYPE_INT8,
                inputPackingFactor: 4,
                matrixInterpretation: SlangScalarType::SLANG_SCALAR_TYPE_INT8,
                biasInterpretation: SlangScalarType::SLANG_SCALAR_TYPE_NONE,
                resultType: SlangScalarType::SLANG_SCALAR_TYPE_INT32,
                transpose: true,
            }],
            failure: None,
        }
    }

    fn metadata(log: &Log, records: CooperativeRecords) -> Metadata {
        let global = fake::global_session(log);
        let session = fake::session(&global, 1);
        // SAFETY: the new fake metadata carries one reference, which
        // `from_raw` takes, and it belongs to no other session.
        unsafe { Metadata::from_raw(FakeMetadata::create_cooperative(log, records), session) }
    }

    #[test]
    fn the_records_are_read_and_converted_through_the_interface_that_cast_as_returns() {
        let log = Log::new();
        let metadata = metadata(&log, records());
        let cooperative = metadata.cooperative_types().expect("the fake answers");
        assert_eq!(
            cooperative.cooperative_matrix_types().ok(),
            Some(vec![CooperativeMatrixType {
                component_type: ScalarType::Float16,
                scope: Scope::Wave,
                row_count: 16,
                column_count: 8,
                matrix_use: CooperativeMatrixUse::B,
            }])
        );
        assert_eq!(
            cooperative.cooperative_matrix_combinations().ok(),
            Some(vec![CooperativeMatrixCombination {
                m: 16,
                n: 8,
                k: 4,
                component_type_a: ScalarType::Int8,
                component_type_b: ScalarType::Int8,
                component_type_c: ScalarType::Int32,
                component_type_result: ScalarType::Int32,
                saturate: true,
                scope: Scope::ThreadGroup,
            }])
        );
        assert_eq!(
            cooperative.cooperative_vector_types().ok(),
            Some(vec![
                CooperativeVectorTypeUsageInfo {
                    component_type: ScalarType::Int8,
                    max_size: 8,
                    used_for_training_op: false,
                },
                CooperativeVectorTypeUsageInfo {
                    component_type: ScalarType::Float32,
                    max_size: 4,
                    used_for_training_op: true,
                },
            ])
        );
        assert_eq!(
            cooperative.cooperative_vector_combinations().ok(),
            Some(vec![CooperativeVectorCombination {
                input_type: ScalarType::Uint32,
                input_interpretation: ScalarType::Int8,
                input_packing_factor: 4,
                matrix_interpretation: ScalarType::Int8,
                bias_interpretation: ScalarType::None,
                result_type: ScalarType::Int32,
                transpose: true,
            }])
        );
        assert_eq!(
            fake::calls(&log),
            [
                Event::call("metadata", "castAs"),
                Event::call("cooperative", "getCooperativeMatrixTypeCount"),
                Event::call("cooperative", "getCooperativeMatrixTypeByIndex"),
                Event::call("cooperative", "getCooperativeMatrixCombinationCount"),
                Event::call("cooperative", "getCooperativeMatrixCombinationByIndex"),
                Event::call("cooperative", "getCooperativeVectorTypeCount"),
                Event::call("cooperative", "getCooperativeVectorTypeByIndex"),
                Event::call("cooperative", "getCooperativeVectorTypeByIndex"),
                Event::call("cooperative", "getCooperativeVectorCombinationCount"),
                Event::call("cooperative", "getCooperativeVectorCombinationByIndex"),
            ]
        );
        drop(metadata);
        assert!(log.events().contains(&Event::freed("metadata")));
    }

    #[test]
    fn metadata_without_the_interface_has_no_cooperative_types() {
        let log = Log::new();
        let global = fake::global_session(&log);
        let session = fake::session(&global, 1);
        // SAFETY: the new fake metadata carries one reference, which
        // `from_raw` takes, and it belongs to no other session.
        let metadata = unsafe { Metadata::from_raw(FakeMetadata::create(&log), session) };
        assert!(metadata.cooperative_types().is_none());
        assert!(fake::calls(&log).contains(&Event::call("metadata", "castAs")));
    }

    #[test]
    fn a_failing_record_maps_its_result_code_and_an_unlisted_value_is_unrecognized() {
        let log = Log::new();
        let failing = metadata(
            &log,
            CooperativeRecords {
                failure: Some(SlangResult::SLANG_FAIL),
                ..records()
            },
        );
        let cooperative = failing.cooperative_types().expect("the fake answers");
        let error = cooperative
            .cooperative_vector_types()
            .expect_err("the record fails");
        assert_eq!(error.kind(), ErrorKind::Internal);
        assert_eq!(error.operation(), Operation::QueryMetadata);
        assert_eq!(error.code(), Some(SlangResult::SLANG_FAIL.0));
        let empty = metadata(
            &log,
            CooperativeRecords {
                failure: Some(SlangResult::SLANG_FAIL),
                ..CooperativeRecords::default()
            },
        );
        assert_eq!(
            empty
                .cooperative_types()
                .expect("the fake answers")
                .cooperative_matrix_combinations()
                .ok(),
            Some(Vec::new())
        );

        let unlisted = metadata(
            &log,
            CooperativeRecords {
                matrix_types: vec![matrix_type(SlangScope(9))],
                ..CooperativeRecords::default()
            },
        );
        let error = unlisted
            .cooperative_types()
            .expect("the fake answers")
            .cooperative_matrix_types()
            .expect_err("the scope is not listed");
        assert_eq!(error.kind(), ErrorKind::Unrecognized);
    }

    #[test]
    fn a_read_while_the_guard_is_raised_panics_before_it_calls_the_fake() {
        let log = Log::new();
        let metadata = metadata(&log, records());
        let cooperative = metadata.cooperative_types().expect("the fake answers");
        let before = log.events();
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            with_slang_call_guard(|| cooperative.cooperative_vector_types())
        }));
        let payload = outcome.expect_err("the read panics");
        let message = panic_message(payload.as_ref());
        assert!(
            message.starts_with("reentrant call: CooperativeTypes::cooperative_vector_types "),
            "{message}"
        );
        assert_eq!(log.events(), before);
    }
}
