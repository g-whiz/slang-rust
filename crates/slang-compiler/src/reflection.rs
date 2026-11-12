//! Handles to what Slang reports about compiled code, which is called
//! reflection.
//!
//! Reflection describes the parameters and types of Slang code, so that a
//! program can fill the buffers and bindings that a shader reads. A binding is
//! a numbered slot of a graphics API through which a shader reaches one
//! resource, such as a buffer or a texture. Slang separates a type from its
//! layout, because one type can be laid out differently depending on its use.
//! A layout says where a parameter sits in memory and in the bindings of one
//! target, the format of code that Slang generates, such as SPIR-V for
//! Vulkan. Each size and offset of a layout is counted in the units of a
//! [`ParameterCategory`](crate::ParameterCategory), such as bytes of uniform data or whole bindings,
//! which is why [`LayoutUnits`] carries its category.
//!
//! Start from [`ProgramLayout`], which the method `layout` of a component
//! type returns, such as [`LinkedProgram::layout`](crate::LinkedProgram::layout)
//! or [`Module::layout`](crate::Module::layout). From it, reach the
//! [`EntryPointLayout`] of each entry point, a function that a graphics API
//! calls, the [`VariableLayout`] of each parameter, and the [`TypeLayout`] and
//! [`Type`] of each. [`ProgramLayout::find_type_by_name`] finds a type by its
//! name, which [`ComponentType::specialize`](crate::ComponentType::specialize)
//! and [`Session::type_conformance`](crate::Session::type_conformance) take.
//!
//! Every handle of this module is a [`Copy`] value that borrows the object it
//! came from, so it cannot outlive it, and it is neither [`Send`] nor
//! [`Sync`]. A handle belongs to the session of that object, and a method that
//! takes a handle of another session rejects it with
//! [`ErrorKind::InvalidArgument`]. A method
//! returns `None` where Slang returns nothing, and an [`Error`]
//! where the crate converts Slang's answer and it does not fit, such as a name
//! that is not UTF-8 or a value of an enumeration that the pinned Slang
//! header does not list.

use slang_compiler_sys::{SLANG_UNBOUNDED_SIZE, SLANG_UNKNOWN_SIZE, SlangInt};

use crate::error::{Error, ErrorKind, Operation, Result};

/// Implements `Iterator`, `DoubleEndedIterator`, and `FusedIterator` for an
/// iterator of this module that holds `positions` and an `at` method, which
/// returns the item at a position or `None` to skip it.
///
/// With `exact`, the iterator never skips a position, so it also implements
/// `ExactSizeIterator` and reports the positions left as both bounds of
/// `size_hint`. Otherwise the lower bound is 0.
macro_rules! position_iterator {
    ($name:ident<$lifetime:lifetime>, $item:ty, exact) => {
        position_iterator!(@impl $name<$lifetime>, $item, true);

        impl ::std::iter::ExactSizeIterator for $name<'_> {}
    };
    ($name:ident<$lifetime:lifetime>, $item:ty) => {
        position_iterator!(@impl $name<$lifetime>, $item, false);
    };
    (@impl $name:ident<$lifetime:lifetime>, $item:ty, $exact:literal) => {
        impl<$lifetime> ::std::iter::Iterator for $name<$lifetime> {
            type Item = $item;

            fn next(&mut self) -> ::std::option::Option<$item> {
                let mut positions = self.positions;
                let item = positions.next(|position| self.at(position));
                self.positions = positions;
                item
            }

            fn size_hint(&self) -> (usize, ::std::option::Option<usize>) {
                let len = self.positions.len();
                (if $exact { len } else { 0 }, ::std::option::Option::Some(len))
            }
        }

        impl ::std::iter::DoubleEndedIterator for $name<'_> {
            fn next_back(&mut self) -> ::std::option::Option<Self::Item> {
                let mut positions = self.positions;
                let item = positions.next_back(|position| self.at(position));
                self.positions = positions;
                item
            }
        }

        impl ::std::iter::FusedIterator for $name<'_> {}
    };
}

mod binding;
mod kinds;
mod program;
mod type_layout;
mod types;
mod variable;

pub use binding::{
    BindingRange, BindingRanges, BindingType, DescriptorRange, DescriptorRanges, DescriptorSet,
    DescriptorSets,
};
pub use kinds::{
    LayoutSize, LayoutUnits, ResourceAccess, ResourceBaseShape, ResourceShape, ScalarType, TypeKind,
};
pub use program::{EntryPointLayout, EntryPointLayouts, ParameterLayouts, ProgramLayout};
pub use type_layout::{Categories, FieldLayouts, TypeLayout};
pub use types::{Fields, Function, Parameters, Type};
pub use variable::{Attribute, UserAttributes, Variable, VariableLayout};

/// The positions that an iterator of this module has not yet yielded, from
/// the front and from the back, below the count that it read once.
#[derive(Clone, Copy, Debug)]
struct Positions {
    front: usize,
    back: usize,
}

impl Positions {
    fn new(count: usize) -> Self {
        Self {
            front: 0,
            back: count,
        }
    }

    /// Returns the item at the next position from the front at which `at`
    /// returns one, and skips each position at which it returns `None`.
    fn next<T>(&mut self, mut at: impl FnMut(usize) -> Option<T>) -> Option<T> {
        while self.front < self.back {
            let position = self.front;
            self.front += 1;
            if let Some(item) = at(position) {
                return Some(item);
            }
        }
        None
    }

    /// Returns the item at the next position from the back at which `at`
    /// returns one, and skips each position at which it returns `None`.
    fn next_back<T>(&mut self, mut at: impl FnMut(usize) -> Option<T>) -> Option<T> {
        while self.front < self.back {
            self.back -= 1;
            if let Some(item) = at(self.back) {
                return Some(item);
            }
        }
        None
    }

    /// Returns the number of positions left.
    fn len(self) -> usize {
        self.back - self.front
    }
}

/// Converts a count that Slang returned as an unsigned integer of 32 bits.
///
/// Every supported platform has a `usize` of 64 bits, because
/// `slang-compiler-sys` compiles for no other, so the conversion never fails.
/// Were it to fail, the count would be 0, so that no position past the real
/// end reaches Slang.
fn count(value: u32) -> usize {
    usize::try_from(value).unwrap_or(0)
}

/// Converts a position below a count that [`count`] converted back to
/// Slang's unsigned integer of 32 bits, or returns `None` when it does not
/// fit, which no position below such a count does.
fn index(position: usize) -> Option<u32> {
    u32::try_from(position).ok()
}

/// Converts a position below a signed count back to Slang's signed integer,
/// or returns `None` when it does not fit, which no position below such a
/// count does.
fn signed_index(position: usize) -> Option<SlangInt> {
    SlangInt::try_from(position).ok()
}

/// Converts a count or a position that Slang returned as a signed integer,
/// and reports a negative value as an error of kind `Internal` that names
/// `what` (SLC-12).
fn signed(value: SlangInt, what: &str) -> Result<usize> {
    usize::try_from(value).map_err(|error| {
        Error::new(ErrorKind::Internal, Operation::Reflect)
            .with_detail(format!("Slang reported {what} {value}: {error}"))
    })
}

/// Converts a binding number or a binding space that Slang returned as
/// `size_t`, or returns `None` for `SLANG_UNKNOWN_SIZE`, which Slang returns
/// when the number is not known, and for `SLANG_UNBOUNDED_SIZE`, which the
/// pinned Slang does not return for these numbers (SLC-12, SLC-13).
fn binding_number(raw: usize) -> Option<u64> {
    match raw {
        SLANG_UNBOUNDED_SIZE | SLANG_UNKNOWN_SIZE => None,
        number => u64::try_from(number).ok(),
    }
}

/// Converts a binding number that Slang returned as a signed integer, which
/// holds the sentinels of `size_t` as -1 and -2, as [`binding_number`] does,
/// and reports any other negative value as an error of kind `Internal` that
/// names `what`.
fn signed_binding_number(raw: SlangInt, what: &str) -> Result<Option<u64>> {
    if raw >= 0 {
        return Ok(binding_number(raw.cast_unsigned()));
    }
    match raw.cast_unsigned() {
        SLANG_UNBOUNDED_SIZE | SLANG_UNKNOWN_SIZE => Ok(None),
        _ => Err(Error::new(ErrorKind::Internal, Operation::Reflect)
            .with_detail(format!("Slang reported {what} {raw}"))),
    }
}

/// Returns the error of a position past the end of a list of `count` items
/// that `what` names (SLC-20).
fn out_of_range(what: &str, position: usize, count: usize) -> Error {
    Error::new(ErrorKind::InvalidArgument, Operation::Reflect).with_detail(format!(
        "{what} {position} is past the end of the {count} that Slang reports"
    ))
}

#[cfg(test)]
mod tests {
    use slang_compiler_sys::{SLANG_UNBOUNDED_SIZE, SLANG_UNKNOWN_SIZE};

    use super::{
        Positions, binding_number, count, index, out_of_range, signed, signed_binding_number,
        signed_index,
    };
    use crate::{ErrorKind, Operation};

    #[test]
    fn counts_and_positions_convert_without_loss() {
        assert_eq!(count(u32::MAX), 4_294_967_295);
        assert_eq!(index(4_294_967_295), Some(u32::MAX));
        assert_eq!(index(4_294_967_296), None);
        assert_eq!(signed_index(7), Some(7));
        assert_eq!(signed_index(usize::MAX), None);
        assert_eq!(signed(3, "the count").ok(), Some(3));
    }

    #[test]
    fn a_negative_number_from_slang_is_internal() {
        let error = signed(-1, "the binding range count").expect_err("a negative count fails");
        assert_eq!(error.kind(), ErrorKind::Internal);
        assert_eq!(error.operation(), Operation::Reflect);
        assert!(
            error
                .to_string()
                .starts_with("cannot read reflection: Slang reported the binding range count -1"),
            "{error}"
        );
    }

    #[test]
    fn binding_numbers_turn_the_sentinels_into_none() {
        assert_eq!(binding_number(0), Some(0));
        assert_eq!(binding_number(7), Some(7));
        assert_eq!(binding_number(SLANG_UNKNOWN_SIZE - 1), Some(u64::MAX - 2));
        assert_eq!(binding_number(SLANG_UNKNOWN_SIZE), None);
        assert_eq!(binding_number(SLANG_UNBOUNDED_SIZE), None);
        assert_eq!(signed_binding_number(3, "the offset").ok(), Some(Some(3)));
        assert_eq!(signed_binding_number(-1, "the offset").ok(), Some(None));
        assert_eq!(signed_binding_number(-2, "the offset").ok(), Some(None));
        let error = signed_binding_number(-3, "the index offset of a descriptor range")
            .expect_err("-3 is no sentinel");
        assert_eq!(error.kind(), ErrorKind::Internal);
        assert_eq!(
            error.to_string(),
            "cannot read reflection: Slang reported the index offset of a descriptor range -3"
        );
    }

    #[test]
    fn a_position_past_the_end_names_the_count() {
        let error = out_of_range("field", 4, 2);
        assert_eq!(error.kind(), ErrorKind::InvalidArgument);
        assert_eq!(
            error.to_string(),
            "cannot read reflection: field 4 is past the end of the 2 that Slang reports"
        );
    }

    #[test]
    fn positions_skip_none_from_both_ends_and_stop_at_the_count() {
        let mut positions = Positions::new(5);
        assert_eq!(positions.len(), 5);
        let odd = |position: usize| (position % 2 == 1).then_some(position);
        assert_eq!(positions.next(odd), Some(1));
        assert_eq!(positions.next_back(odd), Some(3));
        assert_eq!(positions.len(), 1);
        assert_eq!(positions.next(odd), None);
        assert_eq!(positions.len(), 0);
        assert_eq!(positions.next_back(odd), None);
        let mut seen = Vec::new();
        let mut all = Positions::new(3);
        while let Some(position) = all.next(|position| {
            seen.push(position);
            Some(position)
        }) {
            assert!(position < 3);
        }
        assert_eq!(seen, [0, 1, 2]);
    }
}
