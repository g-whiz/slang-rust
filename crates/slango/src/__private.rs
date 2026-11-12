//! The items through which the code that `slango-macros` generates, and the
//! generator in `slango-codegen`, reach the core of this crate.
//!
//! This module is not part of the API of this crate. Its items change with
//! every version of the crate, without notice, and `slango-codegen` requires
//! this crate at exactly its own version, so no other code can depend on
//! them.

use std::path::Path;

use crate::device::Device;
use crate::encoding;
use crate::error::Result;
use crate::module::{self, Module};

mod interface;

pub use interface::{
    Access, ElementType, Field, FieldType, Interface, KernelInterface, Parameter, ParameterKind,
    Scalar, Shape, Structure, UniformStructure, ValueType, slang_type,
};

/// The version of the encoding of a layout snapshot that this crate writes
/// and reads.
///
/// The version changes whenever the bytes of the encoding, or the meaning of
/// any of them, change. Generated code asserts at compile time that it
/// equals the version for which the code was generated, so a mismatch fails
/// to compile instead of failing at [`load_precompiled`].
pub const ENCODING_VERSION: u32 = 1;

/// A Rust struct that `slango-macros` generates for a Slang structure that is
/// the element of a structured buffer, which makes it an
/// [`Element`](crate::Element) and a [`Value`](crate::Value).
///
/// A structured buffer is a GPU buffer that holds an array of one element
/// type. A [`Buffer`](crate::Buffer) of a type that implements this trait
/// binds to a structured buffer whose element has the same stride, the
/// distance in bytes from one element to the next, as
/// [`Call::bind`](crate::Call::bind) checks. [`Call::set`](crate::Call::set)
/// and [`Draw::set`](crate::Draw::set) only record such a value, and the
/// dispatch or the draw rejects it, because a structure among the plain
/// values is set field by field. A draw also rejects a buffer of it given to
/// [`Draw::vertex`](crate::Draw::vertex), because no vertex attribute holds a
/// structure.
///
/// An implementation that does not hold to this contract cannot cause
/// undefined behaviour, because this crate has no unsafe code, but its
/// buffers hold wrong bytes, and a creation or a read of them can fail. The
/// trait serves as a generic bound only, not as a trait object.
pub trait StructElement: Copy + Send + Sync + 'static {
    /// The name of the structure in the Slang source.
    const SLANG_NAME: &'static str;

    /// The stride of the structure in a structured buffer, in bytes, which
    /// is not zero.
    const STRIDE: usize;

    /// Appends exactly [`StructElement::STRIDE`] bytes to `out`: the scalars
    /// of each field in little-endian order at the offset that Slang gives
    /// the field, and zero bytes in every gap.
    fn write_bytes(&self, out: &mut Vec<u8>);

    /// Reads a value from the [`StructElement::STRIDE`] bytes that
    /// [`StructElement::write_bytes`] writes, and returns `None` when `bytes`
    /// has another length.
    fn read_bytes(bytes: &[u8]) -> Option<Self>;
}

/// A Slang module compiled ahead of time: the description of its kernels for
/// the generator, and the encoded snapshot that the generated code embeds.
#[expect(
    clippy::exhaustive_structs,
    reason = "the module has no stability promise (SLM-6), and its users read every field"
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Precompiled {
    /// The description of the kernels of the module and of the files that
    /// Slang read for it.
    pub interface: Interface,
    /// The layout snapshot of the module, with the SPIR-V of each entry
    /// point, encoded for [`load_precompiled`].
    pub encoded: Vec<u8>,
}

/// Compiles the Slang module in a file exactly as
/// [`Device::load_module`](crate::Device::load_module) does, and returns the
/// description of its kernels and its encoded layout snapshot.
///
/// The generator of `slango-codegen` calls this while a crate that uses the
/// macro of `slango-macros` compiles. The compilation uses the same session,
/// search path, and walk of the parameters as a load, so a module that
/// [`load_precompiled`] loads from the encoded snapshot has the same kernels
/// as one that `Device::load_module` loads from the file.
///
/// # Examples
///
/// This example compiles a file and loads it from its encoded snapshot. It
/// needs the Slang library and a GPU driver or a software driver at run
/// time, so it is compiled but not run:
///
/// ```no_run
/// use std::path::Path;
///
/// use slango::{Device, DeviceOptions};
///
/// let precompiled = slango::__private::precompile_file(Path::new("/tmp/shaders/scale.slang"))?;
/// let device = Device::new(&DeviceOptions::default())?;
/// let module = slango::__private::load_precompiled(&device, &precompiled.encoded)?;
/// # drop(module);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Errors
///
/// Returns an [`Error`](crate::Error) of these kinds:
///
/// - [`ErrorKind::InvalidArgument`](crate::ErrorKind::InvalidArgument) when
///   the file cannot be read as text, or its path is not valid UTF-8.
/// - [`ErrorKind::Compile`](crate::ErrorKind::Compile) when Slang cannot
///   compile the module.
/// - [`ErrorKind::UnsupportedParameter`](crate::ErrorKind::UnsupportedParameter)
///   when a kernel has a parameter of a kind that this crate does not
///   support, or the snapshot is too large for the encoding.
pub fn precompile_file(path: &Path) -> Result<Precompiled> {
    let compilation = module::compile_file(path)?;
    let encoded = encoding::encode(&compilation.compiled)?;
    Ok(Precompiled {
        interface: interface::describe(&compilation),
        encoded,
    })
}

/// Loads a module on `device` from an encoded snapshot that
/// [`precompile_file`] returned, with no call into Slang.
///
/// The generated code calls this from its `Shader::load`. The load then
/// creates on the device everything that a dispatch of each kernel needs, as
/// [`Device::load_module`](crate::Device::load_module) does after it has
/// compiled the module.
///
/// # Examples
///
/// See the example of [`precompile_file`].
///
/// # Errors
///
/// Returns an [`Error`](crate::Error) of kind
/// [`ErrorKind::InvalidArgument`](crate::ErrorKind::InvalidArgument) when
/// `encoded` is not an encoded snapshot of [`ENCODING_VERSION`], is
/// truncated or malformed, or has trailing bytes, and of the kinds
/// [`ErrorKind::MissingFeature`](crate::ErrorKind::MissingFeature),
/// [`ErrorKind::UnsupportedShader`](crate::ErrorKind::UnsupportedShader), and
/// [`ErrorKind::Backend`](crate::ErrorKind::Backend) for the reasons that
/// `Device::load_module` returns them.
pub fn load_precompiled(device: &Device, encoded: &[u8]) -> Result<Module> {
    device.load(encoding::decode(encoded)?)
}

#[cfg(test)]
mod tests;
