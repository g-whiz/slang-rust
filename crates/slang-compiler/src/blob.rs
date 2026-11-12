use std::fmt;
use std::ptr::NonNull;
use std::slice;

use slang_compiler_sys::{IMetadata, ISlangBlob, SlangResult, SlangUInt};

use crate::enums::ParameterCategory;
use crate::error::{Compiled, Diagnostics, Error, ErrorKind, Operation, Result};
use crate::guard;
use crate::mapping;
use crate::owned::{ComInterface, Owned};
use crate::session::Session;
use crate::strings::invalid_utf8;

/// The bytes of a blob, read once from Slang.
#[derive(Clone, Copy)]
struct Bytes {
    data: NonNull<u8>,
    len: usize,
}

impl Bytes {
    /// Reads the pointer and the size of the blob's bytes, or returns `None`
    /// when Slang reports a null pointer with a size other than zero, or a
    /// size that no slice can have. The caller raises the Slang call guard.
    fn read(blob: &Owned<ISlangBlob>) -> Option<Self> {
        // SAFETY: `blob` owns a reference to a live blob, used only on this
        // thread, and the caller raised the Slang call guard.
        let data = unsafe {
            guard::abort_on_exception("ISlangBlob::getBufferPointer", || {
                ISlangBlob::getBufferPointer(blob.as_ptr())
            })
        };
        // SAFETY: as for `getBufferPointer`.
        let len = unsafe {
            guard::abort_on_exception("ISlangBlob::getBufferSize", || {
                ISlangBlob::getBufferSize(blob.as_ptr())
            })
        };
        if isize::try_from(len).is_err() {
            return None;
        }
        match NonNull::new(data.cast_mut().cast::<u8>()) {
            Some(data) => Some(Self { data, len }),
            None if len == 0 => Some(Self {
                data: NonNull::dangling(),
                len: 0,
            }),
            None => None,
        }
    }

    /// Returns the bytes as a slice.
    ///
    /// # Safety
    ///
    /// `self` was read from a blob that is still alive, whose bytes have not
    /// moved since. Breaking this is undefined behaviour.
    unsafe fn as_slice<'a>(self) -> &'a [u8] {
        // SAFETY: `read` checked that `data` is non-null, or dangling and
        // aligned for a length of zero, and that `len` fits `isize`. The
        // caller guarantees that the blob, which owns `len` initialized bytes
        // at `data`, is alive and that its bytes have not moved.
        unsafe { slice::from_raw_parts(self.data.as_ptr(), self.len) }
    }
}

/// Copies the text of a diagnostics blob that a Slang call returned and
/// releases the blob, or returns empty diagnostics for a null pointer.
///
/// A blob that reports a null pointer with a size other than zero yields
/// empty diagnostics, because it holds no text to read. The caller raises the
/// Slang call guard.
///
/// # Safety
///
/// `raw` is null or points to a live blob of the pinned Slang build whose one
/// reference the caller owns and transfers. Breaking this is undefined
/// behaviour.
pub(crate) unsafe fn take_diagnostics(raw: *mut ISlangBlob) -> Diagnostics {
    let Some(raw) = NonNull::new(raw) else {
        return Diagnostics::default();
    };
    // SAFETY: the caller transfers the one reference of a live blob, because
    // every diagnostics output carries a new reference.
    let blob = unsafe { Owned::adopt(raw) };
    Bytes::read(&blob).map_or_else(Diagnostics::default, |bytes| {
        // SAFETY: `bytes` was read from `blob`, which lives until the end of
        // this function, after the text is copied.
        Diagnostics::from_bytes(unsafe { bytes.as_slice() })
    })
}

/// Copies the text of a blob that a Slang call returned, such as a type name
/// or the JSON text of a layout.
///
/// The caller drops `blob` afterwards, which releases it. One trailing NUL
/// byte that Slang counted in the size is removed. A blob whose bytes cannot
/// be read fails with kind `Internal`, and bytes that are not UTF-8 fail with
/// kind `InvalidUtf8`, each of `operation`, with a detail that names the text
/// as `what`. The caller raises the Slang call guard.
pub(crate) fn take_text(
    blob: &Owned<ISlangBlob>,
    operation: Operation,
    what: &str,
) -> Result<String> {
    let bytes = Bytes::read(blob).ok_or_else(|| {
        Error::new(ErrorKind::Internal, operation).with_detail(format!(
            "Slang returned {what} in a blob with a null pointer or an impossible size"
        ))
    })?;
    // SAFETY: `bytes` was read from `blob`, which lives until the end of this
    // function, after the text is copied.
    let bytes = unsafe { bytes.as_slice() };
    let bytes = bytes.strip_suffix(&[0]).unwrap_or(bytes);
    String::from_utf8(bytes.to_vec())
        .map_err(|error| invalid_utf8(operation, what, error.utf8_error()))
}

/// Finishes a Slang call that writes an output object and a diagnostics blob:
/// takes the diagnostics, adopts the output, then checks the result code and
/// the output for null.
///
/// The output is adopted before the result code is checked, so that an
/// output that Slang wrote during a failed call is released. The caller
/// raises the Slang call guard.
///
/// # Safety
///
/// `out` is null or points to a live object of the pinned Slang build that
/// implements `T`, and `diagnostics` is null or points to a live blob. Each
/// non-null pointer carries one reference that the caller owns and transfers,
/// which is the new reference that the call returned in the output. Breaking
/// any of them is undefined behaviour.
pub(crate) unsafe fn take_output<T: ComInterface>(
    operation: Operation,
    result: SlangResult,
    out: *mut T,
    diagnostics: *mut ISlangBlob,
) -> Result<Compiled<Owned<T>>> {
    // SAFETY: the caller transfers the reference of `diagnostics`, or passes
    // null.
    let diagnostics = unsafe { take_diagnostics(diagnostics) };
    // SAFETY: the caller transfers the reference of `out`, or passes null.
    let out = unsafe { Owned::adopt_output(out) };
    let diagnostics = mapping::finish(operation, result, diagnostics)?;
    let value = out.ok_or_else(|| mapping::null(operation, diagnostics.clone()))?;
    Ok(Compiled { value, diagnostics })
}

/// Bytes that Slang produced, such as generated code or a serialized module.
///
/// Slang returns its output as a blob, a reference-counted buffer of bytes.
/// Read the bytes with [`Blob::as_bytes`], which borrows them from the blob,
/// or copy them with [`Blob::to_vec`]. A blob holds the [`Session`] of the
/// call that returned it.
///
/// A blob is neither [`Send`] nor [`Sync`], because it holds a session.
/// [`Clone`] returns another handle to the same bytes.
///
/// # Examples
///
/// This example generates SPIR-V, the binary shader format of Vulkan, and
/// writes it to a file. It needs the Slang shared library at run time, so it
/// is compiled but not run:
///
/// ```no_run
/// use slang_compiler::{Component, GlobalSession, Session, Target, TargetFormat};
///
/// let global = GlobalSession::new()?;
/// let session = Session::builder(&global)
///     .target(Target::new(TargetFormat::Spirv))
///     .build()?;
/// let source = "[shader(\"compute\")] [numthreads(1, 1, 1)] void main() {}";
/// let module = session
///     .load_module_from_source("kernel", "kernel.slang", source)?
///     .into_value();
/// let entry_point = module.entry_point("main")?;
/// let program = session
///     .compose(&[Component::Module(&module), Component::EntryPoint(&entry_point)])?
///     .into_value()
///     .link()?
///     .into_value();
/// let code = program.entry_point_code(0, session.target_index(0)?)?.into_value();
/// std::fs::write("/tmp/kernel.spv", code.as_bytes())?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// A blob cannot move to another thread, so this example does not compile:
///
/// ```compile_fail,E0277
/// fn assert_send<T: Send>() {}
/// assert_send::<slang_compiler::Blob>();
/// ```
///
/// A blob cannot be shared between threads either, so this example does not
/// compile:
///
/// ```compile_fail,E0277
/// fn assert_sync<T: Sync>() {}
/// assert_sync::<slang_compiler::Blob>();
/// ```
#[derive(Clone)]
pub struct Blob {
    raw: Owned<ISlangBlob>,
    bytes: Bytes,
    session: Session,
}

impl Blob {
    /// Wraps a blob that a call of `operation` returned, reading its bytes
    /// once. The caller raises the Slang call guard.
    pub(crate) fn new(
        raw: Owned<ISlangBlob>,
        session: Session,
        operation: Operation,
    ) -> Result<Self> {
        let bytes = Bytes::read(&raw).ok_or_else(|| {
            Error::new(ErrorKind::Internal, operation).with_detail(
                "Slang returned a blob whose bytes cannot be read, with a null pointer and a size other than zero or a size larger than any slice".to_owned(),
            )
        })?;
        Ok(Self {
            raw,
            bytes,
            session,
        })
    }

    /// Returns the bytes of the blob.
    ///
    /// # Examples
    ///
    /// See the example of [`Blob`].
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        // SAFETY: `self.bytes` was read from `self.raw`, which `self` keeps
        // alive while the returned slice borrows `self`, and a blob's bytes do
        // not move while it lives.
        unsafe { self.bytes.as_slice() }
    }

    /// Copies the bytes of the blob into a new vector.
    ///
    /// # Examples
    ///
    /// This example keeps generated code after the blob is dropped:
    ///
    /// ```
    /// use slang_compiler::Blob;
    ///
    /// fn keep(code: &Blob) -> Vec<u8> {
    ///     code.to_vec()
    /// }
    /// # let _ = keep;
    /// ```
    #[must_use]
    pub fn to_vec(&self) -> Vec<u8> {
        self.as_bytes().to_vec()
    }

    /// Returns the bytes of the blob as text, for code in a text format such
    /// as HLSL, the High-Level Shading Language of Direct3D.
    ///
    /// # Examples
    ///
    /// This example prints generated source code:
    ///
    /// ```
    /// use slang_compiler::Blob;
    ///
    /// fn print(code: &Blob) -> slang_compiler::Result<()> {
    ///     println!("{}", code.to_str()?);
    ///     Ok(())
    /// }
    /// # let _ = print;
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error of kind [`ErrorKind::InvalidUtf8`] when the bytes are
    /// not valid UTF-8, as for binary code such as SPIR-V.
    pub fn to_str(&self) -> Result<&str> {
        std::str::from_utf8(self.as_bytes()).map_err(|error| {
            Error::new(ErrorKind::InvalidUtf8, Operation::ReadBlob).with_detail(error.to_string())
        })
    }

    /// Returns the bytes of the blob as 32-bit words, the unit of SPIR-V, the
    /// binary shader format of Vulkan.
    ///
    /// Returns `None` when the length is not a multiple of four or the bytes
    /// are not aligned for `u32`. Copy the bytes and convert them when the
    /// alignment does not fit.
    ///
    /// # Examples
    ///
    /// This example checks the first word of SPIR-V, its magic number:
    ///
    /// ```
    /// use slang_compiler::Blob;
    ///
    /// fn is_spirv(code: &Blob) -> bool {
    ///     code.as_spirv_words()
    ///         .and_then(|words| words.first())
    ///         .is_some_and(|&magic| magic == 0x0723_0203)
    /// }
    /// # let _ = is_spirv;
    /// ```
    #[must_use]
    pub fn as_spirv_words(&self) -> Option<&[u32]> {
        // SAFETY: every bit pattern of four bytes is a valid `u32`, so
        // reading the aligned middle part of the bytes as words is sound, and
        // the words borrow `self`, which keeps the bytes alive and unmoved.
        let (prefix, words, suffix) = unsafe { self.as_bytes().align_to::<u32>() };
        (prefix.is_empty() && suffix.is_empty()).then_some(words)
    }

    /// Adopts a blob that the caller obtained through the raw bindings of
    /// `slang-compiler-sys`, and reads its bytes once.
    ///
    /// # Examples
    ///
    /// This example moves a blob out of the crate and back. It needs the
    /// Slang shared library at run time, so it is compiled but not run:
    ///
    /// ```no_run
    /// use slang_compiler::Blob;
    ///
    /// fn round_trip(code: Blob) -> slang_compiler::Result<Blob> {
    ///     let (raw, session) = code.into_raw();
    ///     // SAFETY: `raw` carries the one reference that `into_raw` gave up,
    ///     // of a blob that a call on `session` returned, and only this thread
    ///     // uses it.
    ///     unsafe { Blob::from_raw(raw, session) }
    /// }
    /// # let _ = round_trip;
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error of kind [`ErrorKind::Internal`] when the blob reports
    /// a null pointer with a size other than zero, or a size larger than any
    /// slice can have, and releases the blob.
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    ///
    /// # Safety
    ///
    /// The caller guarantees all of these, and breaking any of them is
    /// undefined behaviour:
    ///
    /// - `raw` points to a live blob that the Slang library of the build tag
    ///   [`slang_compiler_sys::SLANG_BUILD_TAG`] created.
    /// - The caller owns one reference to it and transfers that reference.
    /// - `session` is the session of the call that returned the blob.
    /// - The blob's bytes do not change or move while the blob lives.
    /// - The blob is not used on another thread while the returned value or a
    ///   clone of it exists.
    pub unsafe fn from_raw(raw: NonNull<ISlangBlob>, session: Session) -> Result<Self> {
        // SAFETY: the caller guarantees that `raw` points to a live blob of the
        // pinned build and transfers one reference to it.
        let raw = unsafe { Owned::adopt(raw) };
        guard::call("Blob::from_raw", || {
            Self::new(raw, session, Operation::ReadBlob)
        })
    }

    /// Gives up this handle's reference to the blob without releasing it, and
    /// returns the pointer for use with the raw bindings of
    /// `slang-compiler-sys`, together with this handle's handle to the session
    /// of the call that returned the blob.
    ///
    /// The caller then owns the reference to the blob and releases it. Keep
    /// the returned [`Session`] until after that release, or pass both back
    /// to [`Blob::from_raw`].
    ///
    /// # Examples
    ///
    /// See the example of [`Blob::from_raw`].
    #[must_use]
    pub fn into_raw(self) -> (NonNull<ISlangBlob>, Session) {
        let Self { raw, session, .. } = self;
        (raw.into_raw(), session)
    }

    /// Returns the pointer to the blob without changing its reference count,
    /// for use with the raw bindings of `slang-compiler-sys`.
    ///
    /// The pointer is valid only while this handle lives. Every use of it is
    /// unsafe and follows the safety contract of `slang-compiler-sys`.
    ///
    /// # Examples
    ///
    /// This example reads the size of a blob through the raw bindings:
    ///
    /// ```
    /// use slang_compiler::Blob;
    /// use slang_compiler_sys::ISlangBlob;
    ///
    /// fn size(code: &Blob) -> usize {
    ///     // SAFETY: `code` keeps the blob alive during the call, and only this
    ///     // thread uses it.
    ///     unsafe { ISlangBlob::getBufferSize(code.as_raw().as_ptr()) }
    /// }
    /// # let _ = size;
    /// ```
    #[must_use]
    pub fn as_raw(&self) -> NonNull<ISlangBlob> {
        self.raw.as_non_null()
    }
}

impl fmt::Debug for Blob {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Blob")
            .field("raw", &self.raw.as_non_null())
            .field("len", &self.bytes.len)
            .finish_non_exhaustive()
    }
}

/// The number of a binding space, a numbered group of bindings, which Vulkan
/// calls a descriptor set and Direct3D a register space.
///
/// # Examples
///
/// See the example of [`Metadata`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BindingSpace(u64);

impl BindingSpace {
    /// Returns the binding space with the number `space`.
    #[must_use]
    pub const fn new(space: u64) -> Self {
        Self(space)
    }

    /// Returns the number of the binding space.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// The number of a binding in its binding space, a numbered slot through
/// which a graphics API passes a resource to a shader.
///
/// # Examples
///
/// See the example of [`Metadata`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BindingIndex(u64);

impl BindingIndex {
    /// Returns the binding with the number `index`.
    #[must_use]
    pub const fn new(index: u64) -> Self {
        Self(index)
    }

    /// Returns the number of the binding.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Facts about generated code that Slang records when it generates it, such
/// as which parameters the code uses.
///
/// Get it from
/// [`LinkedProgram::entry_point_metadata`](crate::LinkedProgram::entry_point_metadata)
/// or [`LinkedProgram::target_metadata`](crate::LinkedProgram::target_metadata).
/// Use it to skip binding a resource that the code never reads. Metadata
/// holds the [`Session`] of the call that returned it.
///
/// Metadata is neither [`Send`] nor [`Sync`], because it holds a session.
/// [`Clone`] returns another handle to the same metadata.
///
/// # Examples
///
/// This example checks whether a compute shader uses the binding 0 of the
/// descriptor set 0, a numbered slot through which Vulkan passes a resource
/// to a shader. It needs the Slang shared library at run time, so it is
/// compiled but not run:
///
/// ```no_run
/// use slang_compiler::{
///     BindingIndex, BindingSpace, Component, GlobalSession, ParameterCategory, Session, Target,
///     TargetFormat,
/// };
///
/// let global = GlobalSession::new()?;
/// let session = Session::builder(&global)
///     .target(Target::new(TargetFormat::Spirv))
///     .build()?;
/// let source = r#"
///     RWStructuredBuffer<float> data;
///     [shader("compute")] [numthreads(1, 1, 1)]
///     void main() { data[0] = 1.0; }
/// "#;
/// let module = session
///     .load_module_from_source("kernel", "kernel.slang", source)?
///     .into_value();
/// let entry_point = module.entry_point("main")?;
/// let program = session
///     .compose(&[Component::Module(&module), Component::EntryPoint(&entry_point)])?
///     .into_value()
///     .link()?
///     .into_value();
/// let metadata = program
///     .entry_point_metadata(0, session.target_index(0)?)?
///     .into_value();
/// let used = metadata.is_parameter_location_used(
///     ParameterCategory::DescriptorTableSlot,
///     BindingSpace::new(0),
///     BindingIndex::new(0),
/// )?;
/// println!("binding 0 is used: {used}");
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// Metadata cannot move to another thread, so this example does not compile:
///
/// ```compile_fail,E0277
/// fn assert_send<T: Send>() {}
/// assert_send::<slang_compiler::Metadata>();
/// ```
///
/// Metadata cannot be shared between threads either, so this example does not
/// compile:
///
/// ```compile_fail,E0277
/// fn assert_sync<T: Sync>() {}
/// assert_sync::<slang_compiler::Metadata>();
/// ```
#[derive(Clone)]
pub struct Metadata {
    raw: Owned<IMetadata>,
    session: Session,
}

impl Metadata {
    pub(crate) fn new(raw: Owned<IMetadata>, session: Session) -> Self {
        Self { raw, session }
    }

    /// Returns whether the generated code uses the binding `index` of the
    /// binding space `space` in the parameter category `category`.
    ///
    /// For Vulkan, pass [`ParameterCategory::DescriptorTableSlot`] with the
    /// descriptor set as the [`BindingSpace`] and the binding as the
    /// [`BindingIndex`].
    ///
    /// # Examples
    ///
    /// See the example of [`Metadata`].
    ///
    /// # Errors
    ///
    /// Returns an error of kind [`ErrorKind::InvalidArgument`] when `space`
    /// or `index` does not fit Slang's integer type, or the kind that Slang's
    /// result code maps to when Slang fails.
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    pub fn is_parameter_location_used(
        &self,
        category: ParameterCategory,
        space: BindingSpace,
        index: BindingIndex,
    ) -> Result<bool> {
        let operation = Operation::QueryMetadata;
        let convert = |value: u64, what: &str| {
            SlangUInt::try_from(value).map_err(|error| {
                Error::new(ErrorKind::InvalidArgument, operation).with_detail(format!(
                    "the {what} {value} does not fit Slang's integer type: {error}"
                ))
            })
        };
        let space = convert(space.get(), "binding space")?;
        let index = convert(index.get(), "binding")?;
        guard::call("Metadata::is_parameter_location_used", || {
            let mut used = false;
            // SAFETY: `self.raw` points to a live metadata object, used only on
            // this thread, `used` is a live out-pointer, and the call runs with
            // the Slang call guard raised.
            let result = unsafe {
                guard::abort_on_exception("IMetadata::isParameterLocationUsed", || {
                    IMetadata::isParameterLocationUsed(
                        self.raw.as_ptr(),
                        category.to_raw(),
                        space,
                        index,
                        &raw mut used,
                    )
                })
            };
            mapping::check(operation, result, Diagnostics::default())?;
            Ok(used)
        })
    }

    /// Adopts a metadata object that the caller obtained through the raw
    /// bindings of `slang-compiler-sys`.
    ///
    /// # Examples
    ///
    /// This example moves metadata out of the crate and back:
    ///
    /// ```
    /// use slang_compiler::Metadata;
    ///
    /// fn round_trip(metadata: Metadata) -> Metadata {
    ///     let (raw, session) = metadata.into_raw();
    ///     // SAFETY: `raw` carries the one reference that `into_raw` gave up,
    ///     // of metadata that a call on `session` returned, and only this
    ///     // thread uses it.
    ///     unsafe { Metadata::from_raw(raw, session) }
    /// }
    /// # let _ = round_trip;
    /// ```
    ///
    /// # Safety
    ///
    /// The caller guarantees all of these, and breaking any of them is
    /// undefined behaviour:
    ///
    /// - `raw` points to a live metadata object that the Slang library of the
    ///   build tag [`slang_compiler_sys::SLANG_BUILD_TAG`] created.
    /// - The caller owns one reference to it and transfers that reference.
    /// - `session` is the session of the call that returned the metadata.
    /// - The metadata is not used on another thread while the returned value
    ///   or a clone of it exists.
    #[must_use]
    pub unsafe fn from_raw(raw: NonNull<IMetadata>, session: Session) -> Self {
        // SAFETY: the caller guarantees that `raw` points to a live metadata
        // object of the pinned build and transfers one reference to it.
        let raw = unsafe { Owned::adopt(raw) };
        Self { raw, session }
    }

    /// Gives up this handle's reference to the metadata without releasing
    /// it, and returns the pointer for use with the raw bindings of
    /// `slang-compiler-sys`, together with this handle's handle to the session
    /// of the call that returned the metadata.
    ///
    /// The caller then owns the reference to the metadata and releases it.
    /// Keep the returned [`Session`] until after that release, or pass both
    /// back to [`Metadata::from_raw`].
    ///
    /// # Examples
    ///
    /// See the example of [`Metadata::from_raw`].
    #[must_use]
    pub fn into_raw(self) -> (NonNull<IMetadata>, Session) {
        let Self { raw, session } = self;
        (raw.into_raw(), session)
    }

    /// Returns the pointer to the metadata without changing its reference
    /// count, for use with the raw bindings of `slang-compiler-sys`.
    ///
    /// The pointer is valid only while this handle lives. Every use of it is
    /// unsafe and follows the safety contract of `slang-compiler-sys`.
    ///
    /// # Examples
    ///
    /// This example passes the pointer on to code that uses the raw
    /// bindings:
    ///
    /// ```
    /// use slang_compiler::Metadata;
    ///
    /// fn raw_pointer(metadata: &Metadata) -> *mut slang_compiler_sys::IMetadata {
    ///     metadata.as_raw().as_ptr()
    /// }
    /// # let _ = raw_pointer;
    /// ```
    #[must_use]
    pub fn as_raw(&self) -> NonNull<IMetadata> {
        self.raw.as_non_null()
    }
}

impl fmt::Debug for Metadata {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Metadata")
            .field("raw", &self.raw.as_non_null())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use std::ptr;

    use std::ptr::NonNull;

    use slang_compiler_sys::ISlangBlob;

    use super::{Blob, take_diagnostics};
    use crate::fake::{self, Event, FakeBlob, Log, Reported};
    use crate::owned::Owned;
    use crate::{ErrorKind, Operation};

    fn blob(log: &Log, bytes: &[u8]) -> Blob {
        let global = fake::global_session(log);
        let session = fake::session(&global, 1);
        // SAFETY: the new fake blob carries one reference, which `adopt` takes.
        let raw = unsafe { Owned::adopt(FakeBlob::create(log, "blob", bytes)) };
        Blob::new(raw, session, Operation::GenerateCode).expect("the fake blob has bytes")
    }

    #[test]
    fn take_diagnostics_of_null_is_empty() {
        // SAFETY: a null pointer carries no reference.
        assert!(unsafe { take_diagnostics(ptr::null_mut()) }.is_empty());
    }

    #[test]
    fn take_diagnostics_copies_the_text_drops_a_final_nul_and_releases_the_blob() {
        let log = Log::new();
        for (bytes, text) in [
            (&b"warning\0"[..], "warning"),
            (&b"warning"[..], "warning"),
            (&b"bad \xFF byte"[..], "bad \u{FFFD} byte"),
            (&b""[..], ""),
        ] {
            let raw = FakeBlob::create(&log, "diagnostics", bytes);
            // SAFETY: the new fake blob carries one reference, which the call
            // takes over.
            let diagnostics = unsafe { take_diagnostics(raw.as_ptr()) };
            assert_eq!(diagnostics.as_str(), text);
        }
        let freed = log
            .events()
            .into_iter()
            .filter(|event| *event == Event::freed("diagnostics"))
            .count();
        assert_eq!(freed, 4);
    }

    #[test]
    fn a_blob_reads_its_bytes_as_text_and_as_words() {
        let log = Log::new();
        let code = blob(&log, &[3, 2, 0x23, 7, 1, 0, 0, 0]);
        assert_eq!(code.as_bytes(), [3, 2, 0x23, 7, 1, 0, 0, 0]);
        assert_eq!(code.to_str().ok(), Some("\u{3}\u{2}#\u{7}\u{1}\0\0\0"));
        assert_eq!(
            code.as_spirv_words(),
            Some(
                &[
                    u32::from_ne_bytes([3, 2, 0x23, 7]),
                    u32::from_ne_bytes([1, 0, 0, 0])
                ][..]
            )
        );
        let odd = blob(&log, &[1, 2, 3]);
        assert_eq!(odd.as_spirv_words(), None);
        let empty = blob(&log, &[]);
        assert_eq!(empty.as_bytes(), []);
        assert_eq!(empty.as_spirv_words(), Some(&[][..]));
    }

    #[test]
    fn a_misaligned_blob_has_no_words_but_its_bytes() {
        let log = Log::new();
        let global = fake::global_session(&log);
        let session = fake::session(&global, 1);
        let raw = FakeBlob::create_misaligned(&log, "blob", &[3, 2, 0x23, 7]);
        // SAFETY: the new fake blob carries one reference, which `adopt` takes.
        let raw = unsafe { Owned::adopt(raw) };
        let code =
            Blob::new(raw, session, Operation::GenerateCode).expect("the bytes are readable");
        assert_ne!(code.as_bytes().as_ptr().align_offset(align_of::<u32>()), 0);
        assert_eq!(code.as_bytes(), [3, 2, 0x23, 7]);
        assert_eq!(code.as_spirv_words(), None);
    }

    /// A fake blob that reports `reported` instead of its four bytes.
    fn reporting(log: &Log, name: &'static str, reported: Reported) -> NonNull<ISlangBlob> {
        let raw = FakeBlob::create(log, name, b"text");
        FakeBlob::report(raw, reported);
        raw
    }

    const UNREADABLE: [Reported; 2] = [
        Reported::Null(4),
        Reported::Size(isize::MAX.unsigned_abs() + 1),
    ];

    #[test]
    fn new_fails_with_internal_for_an_unreadable_blob_and_releases_it() {
        let log = Log::new();
        let global = fake::global_session(&log);
        let session = fake::session(&global, 1);
        for reported in UNREADABLE {
            // SAFETY: the new fake blob carries one reference, which `adopt`
            // takes.
            let raw = unsafe { Owned::adopt(reporting(&log, "blob", reported)) };
            let error = Blob::new(raw, session.clone(), Operation::GenerateCode)
                .expect_err("the bytes cannot be read");
            assert_eq!(error.kind(), ErrorKind::Internal, "{reported:?}");
            assert_eq!(error.operation(), Operation::GenerateCode);
        }
        let freed = log
            .events()
            .into_iter()
            .filter(|event| *event == Event::freed("blob"))
            .count();
        assert_eq!(freed, UNREADABLE.len());
    }

    #[test]
    fn new_reads_a_null_pointer_with_size_zero_as_empty() {
        let log = Log::new();
        let global = fake::global_session(&log);
        let session = fake::session(&global, 1);
        // SAFETY: the new fake blob carries one reference, which `adopt` takes.
        let raw = unsafe { Owned::adopt(reporting(&log, "blob", Reported::Null(0))) };
        let code = Blob::new(raw, session, Operation::GenerateCode).expect("an empty blob reads");
        assert_eq!(code.as_bytes(), []);
    }

    #[test]
    fn from_raw_fails_with_internal_for_an_unreadable_blob_and_releases_it() {
        let log = Log::new();
        let global = fake::global_session(&log);
        let session = fake::session(&global, 1);
        let raw = reporting(&log, "blob", Reported::Null(4));
        // SAFETY: the new fake blob carries one reference, which `from_raw`
        // takes, and it belongs to no other session.
        let error = unsafe { Blob::from_raw(raw, session) }.expect_err("the bytes cannot be read");
        assert_eq!(error.kind(), ErrorKind::Internal);
        assert_eq!(error.operation(), Operation::ReadBlob);
        assert!(log.events().contains(&Event::freed("blob")));
    }

    #[test]
    fn take_diagnostics_of_an_unreadable_blob_is_empty_and_releases_it() {
        let log = Log::new();
        for reported in UNREADABLE {
            let raw = reporting(&log, "diagnostics", reported);
            // SAFETY: the new fake blob carries one reference, which the call
            // takes over.
            let diagnostics = unsafe { take_diagnostics(raw.as_ptr()) };
            assert!(diagnostics.is_empty(), "{reported:?}");
        }
        let freed = log
            .events()
            .into_iter()
            .filter(|event| *event == Event::freed("diagnostics"))
            .count();
        assert_eq!(freed, UNREADABLE.len());
    }

    #[test]
    fn to_str_of_bytes_that_are_not_utf8_fails_with_invalid_utf8() {
        let log = Log::new();
        let error = blob(&log, &[0xFF, 0xFE]).to_str().expect_err("not UTF-8");
        assert_eq!(error.kind(), ErrorKind::InvalidUtf8);
        assert_eq!(error.operation(), Operation::ReadBlob);
    }

    #[test]
    fn a_blob_is_released_before_its_session() {
        let log = Log::new();
        let code = blob(&log, b"code");
        let clone = code.clone();
        drop(code);
        drop(clone);
        let freed = log
            .events()
            .into_iter()
            .filter(|event| matches!(event, Event::Freed { .. }))
            .collect::<Vec<_>>();
        assert_eq!(
            freed,
            [
                Event::freed("blob"),
                Event::freed("session"),
                Event::freed("global")
            ]
        );
    }
}
