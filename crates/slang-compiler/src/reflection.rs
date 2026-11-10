//! Handles to what Slang reports about compiled code, which is called
//! reflection.
//!
//! Reflection describes the parameters and types of Slang code. Each handle
//! borrows the object it came from, so it cannot outlive it, and it is
//! neither [`Send`] nor [`Sync`]. This version offers only [`Type`], a type of
//! Slang code, which
//! [`ComponentType::specialize`](crate::ComponentType::specialize) and
//! [`Session::type_conformance`](crate::Session::type_conformance) take, and
//! which a program gets from the raw bindings of `slang-compiler-sys` through
//! [`Type::from_raw`].

use std::fmt;
use std::ptr::NonNull;

use slang_compiler_sys::TypeReflection;

use crate::session::Session;

/// A type of Slang code, such as a structure or a Slang interface, an
/// `interface` declaration that Slang types implement.
///
/// This version has no safe way to find a type: a program finds one through
/// the reflection functions of `slang-compiler-sys`, such as
/// `spReflection_FindTypeByName`, and wraps it with [`Type::from_raw`]. A
/// type borrows its [`Session`], so it cannot outlive it, and it belongs to
/// that session: a method of another session rejects it.
///
/// # Examples
///
/// See the example of
/// [`ComponentType::specialize`](crate::ComponentType::specialize).
///
/// A type handle cannot move to another thread, so this example does not
/// compile:
///
/// ```compile_fail,E0277
/// fn assert_send<T: Send>() {}
/// assert_send::<slang_compiler::reflection::Type<'static>>();
/// ```
///
/// A type handle cannot be shared between threads either, so this example
/// does not compile:
///
/// ```compile_fail,E0277
/// fn assert_sync<T: Sync>() {}
/// assert_sync::<slang_compiler::reflection::Type<'static>>();
/// ```
#[derive(Clone, Copy)]
pub struct Type<'session> {
    raw: NonNull<TypeReflection>,
    session: &'session Session,
}

impl<'session> Type<'session> {
    /// Wraps a type that the caller found through the raw bindings of
    /// `slang-compiler-sys`.
    ///
    /// # Examples
    ///
    /// This example finds a type by its name in the layout of a module, which
    /// the caller got from `getLayout` through the raw bindings:
    ///
    /// ```
    /// use std::ffi::CStr;
    /// use std::ptr::NonNull;
    ///
    /// use slang_compiler::Session;
    /// use slang_compiler::reflection::Type;
    /// use slang_compiler_sys::{SlangReflection, TypeReflection, spReflection_FindTypeByName};
    ///
    /// /// # Safety
    /// ///
    /// /// `layout` is a live layout of a module of `session`, which stays loaded
    /// /// while `session` lives. Breaking this is undefined behaviour.
    /// unsafe fn find<'session>(
    ///     session: &'session Session,
    ///     layout: NonNull<SlangReflection>,
    ///     name: &CStr,
    /// ) -> Option<Type<'session>> {
    ///     // SAFETY: the caller guarantees that `layout` is live, and `name` is a
    ///     // C string.
    ///     let raw = unsafe { spReflection_FindTypeByName(layout.as_ptr(), name.as_ptr()) };
    ///     let raw = NonNull::new(raw.cast::<TypeReflection>())?;
    ///     // SAFETY: Slang returned a non-null type of the session of `layout`,
    ///     // which `session` keeps alive.
    ///     Some(unsafe { Type::from_raw(raw, session) })
    /// }
    /// # let _ = find;
    /// ```
    ///
    /// # Safety
    ///
    /// The caller guarantees all of these, and breaking any of them is
    /// undefined behaviour:
    ///
    /// - `raw` points to a type that a reflection function of the Slang
    ///   library of the build tag [`slang_compiler_sys::SLANG_BUILD_TAG`]
    ///   returned for an object of `session`, such as the layout of one of its
    ///   modules.
    /// - The type stays alive while `session` lives, which holds for the types
    ///   of the modules that the session loaded and of the core module.
    /// - The type is not used on another thread while the returned value
    ///   exists.
    ///
    /// Section 11.3 of the design of this crate states the conditions of a
    /// reflection handle.
    #[must_use]
    pub unsafe fn from_raw(raw: NonNull<TypeReflection>, session: &'session Session) -> Self {
        Self { raw, session }
    }

    /// Returns the session that the type belongs to, for the check of SLC-7.
    pub(crate) fn session(&self) -> &'session Session {
        self.session
    }

    /// Returns the pointer to the type, for use with the reflection functions
    /// of `slang-compiler-sys` that this version does not wrap.
    ///
    /// The pointer is valid only while the session that the type borrows
    /// lives. Every use of it is unsafe and follows the safety contract of
    /// `slang-compiler-sys`.
    ///
    /// # Examples
    ///
    /// This example reads the kind of a type through the raw bindings:
    ///
    /// ```
    /// use slang_compiler::reflection::Type;
    /// use slang_compiler_sys::{SlangReflectionType, SlangTypeKind, spReflectionType_GetKind};
    ///
    /// fn kind(ty: Type<'_>) -> SlangTypeKind {
    ///     // SAFETY: `ty` borrows its session, which keeps the type alive during
    ///     // the call, and only this thread uses it.
    ///     unsafe { spReflectionType_GetKind(ty.as_raw().as_ptr().cast::<SlangReflectionType>()) }
    /// }
    /// # let _ = kind;
    /// ```
    #[must_use]
    pub fn as_raw(&self) -> NonNull<TypeReflection> {
        self.raw
    }
}

impl fmt::Debug for Type<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Type")
            .field("raw", &self.raw)
            .finish_non_exhaustive()
    }
}
