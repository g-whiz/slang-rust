//! The items through which the code that `slango-macros` generates, and the
//! generator in `slango-codegen`, reach the core of this crate.
//!
//! This module is not part of the API of this crate. Its items change with
//! every version of the crate, without notice, and `slango-codegen` requires
//! this crate at exactly its own version, so no other code can depend on
//! them.

use std::path::Path;

pub use crate::backend::Embedded;
use crate::device::Device;
use crate::error::Result;
use crate::module::{Module, SourceFile};
use crate::{encoding, layout};
pub use crate::{rhi_only, wgpu_only};

mod interface;

pub(crate) use interface::slang_layout_type;
pub use interface::{
    Access, ElementType, Field, FieldType, Interface, KernelInterface, LoadedModule, Parameter,
    ParameterKind, Scalar, Shape, Structure, UniformStructure, ValueType, slang_type,
    slang_value_type,
};

/// The version of the encoding of a layout snapshot that this crate writes
/// and reads.
///
/// The version changes whenever the bytes of the encoding, or the meaning of
/// any of them, change. Code generated for a build with the feature `wgpu`
/// asserts at compile time that it equals the version for which the code was
/// generated, so a mismatch fails to compile instead of failing at
/// [`load_embedded`].
pub const ENCODING_VERSION: u32 = 3;

/// The Slang source of one module that generated code embeds for a build
/// with the feature `rhi`, which [`load_embedded`] compiles.
#[expect(
    clippy::exhaustive_structs,
    reason = "the module has no stability promise, and generated code builds it with a literal"
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EmbeddedSource<'a> {
    /// The name of the module, under which the source is loaded, so that an
    /// `import` of that name finds it.
    pub name: &'a str,
    /// The path of the file of the module when the code was generated, which
    /// the messages of Slang name and from which Slang resolves an
    /// `#include`.
    pub path: &'a str,
    /// The source text of the module.
    pub text: &'a str,
}

/// The name, the offset, and the type of one field of a [`StructElement`].
#[expect(
    clippy::exhaustive_structs,
    reason = "the module has no stability promise, and generated code builds it with a literal"
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StructField {
    /// The path of the field from the struct, with the Slang name of each
    /// field along it, such as `motion.drag` for the field `drag` of the
    /// field `motion`.
    pub name: &'static str,
    /// The offset of the field from the start of the struct, in bytes.
    pub offset: usize,
    /// The type of the field.
    pub ty: StructFieldType,
}

/// The type of a [`StructField`].
#[expect(
    clippy::exhaustive_enums,
    reason = "the module has no stability promise, and generated code builds it with a literal"
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StructFieldType {
    /// A plain value, with the name of its Slang type as [`slang_value_type`]
    /// gives it, such as `float3` or `int[4]`, which names its scalar type,
    /// its number of components, and the lengths of its arrays.
    Plain(&'static str),
    /// A structure, with its Slang name, whose fields follow this field in
    /// [`StructElement::FIELDS`] under its path.
    ///
    /// The check of a bound buffer compares the fields of the structure and
    /// not its name, as it compares a scalar or a vector by its type and not
    /// by a name, so a structure of another name with the same fields
    /// matches.
    Structure(&'static str),
}

/// A Rust struct that `slango-macros` generates for a Slang structure that is
/// the element of a structured buffer, which makes it an
/// [`Element`](crate::Element) and a [`Value`](crate::Value).
///
/// A structured buffer is a GPU buffer that holds an array of one element
/// type. A [`Buffer`](crate::Buffer) of a type that implements this trait
/// binds to a structured buffer of a structure with the same stride, the
/// distance in bytes from one element to the next, and the same fields at
/// the same offsets with the same types, as [`Call::bind`](crate::Call::bind)
/// checks. The name of the structure is not compared, so a structure of the
/// same fields under another name, or in another module, matches.
/// [`Call::set`](crate::Call::set)
/// and [`Draw::set`](crate::Draw::set) only record such a value, and the
/// dispatch or the draw rejects it, because a structure among the plain
/// values is set field by field. A draw also rejects a buffer of it given to
/// [`Draw::vertex`](crate::Draw::vertex), because no vertex attribute holds a
/// structure.
///
/// An implementation that does not hold to this contract cannot cause
/// undefined behaviour, because this crate has no unsafe code, but its
/// buffers hold wrong bytes, and a creation or a read of them can fail. The
/// trait is used only as a generic bound, not as a trait object.
pub trait StructElement: Copy + Send + Sync + 'static {
    /// The name of the structure in the Slang source.
    const SLANG_NAME: &'static str;

    /// The stride of the structure in a structured buffer, in bytes, which
    /// is not zero.
    const STRIDE: usize;

    /// Every field of the structure, in the order of their offsets, with
    /// each field of a nested structure after the field that holds it, and
    /// the type of each.
    const FIELDS: &'static [StructField];

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
/// the generator, and the encoded snapshot that the generated code embeds for
/// a build with the feature `wgpu`.
#[expect(
    clippy::exhaustive_structs,
    reason = "the module has no stability promise, and its users read every field"
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Precompiled {
    /// The description of the kernels of the module and of the files that
    /// Slang read for it.
    pub interface: Interface,
    /// The layout snapshot of the module, with the SPIR-V of each entry
    /// point, encoded for [`load_embedded`] in a build with the feature
    /// `wgpu`.
    pub encoded: Vec<u8>,
}

/// Compiles the Slang module in a file exactly as
/// [`Device::load_module`](crate::Device::load_module) does with the feature
/// `wgpu`, and returns the description of its kernels and its encoded layout
/// snapshot.
///
/// The generator of `slango-codegen` calls this while a crate that uses the
/// macro of `slango-macros` compiles. The compilation uses the same session,
/// search path, and walk of the parameters as a load, so a module that
/// [`load_embedded`] loads from the encoded snapshot has the same kernels as
/// one that `Device::load_module` loads from the file.
///
/// It also compiles the file a second time for SPIR-V with the scalar layout
/// of buffers, as the feature `rhi` compiles for Vulkan, and adds the layout
/// that structures have there to the description. That second compilation
/// reads the layouts of types only and generates no code. The description
/// also names each module that the file imports, with the files that Slang
/// read for it.
///
/// # Examples
///
/// This example compiles a file ahead of time and prints the name of each
/// of its kernels and the files that Slang read for it. It needs the Slang
/// library at run time, so it is compiled but not run:
///
/// ```no_run
/// use std::path::Path;
///
/// let precompiled = slango::__private::precompile_file(Path::new("/tmp/shaders/scale.slang"))?;
/// for kernel in &precompiled.interface.kernels {
///     println!("{}", kernel.name);
/// }
/// for file in &precompiled.interface.dependencies {
///     println!("{}", file.display());
/// }
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
    let file = SourceFile::read(path)?;
    let source = file.source();
    let compilation = layout::compile(&source)?;
    let packed = layout::packed_structures(&source)?;
    let encoded = encoding::encode(&compilation.compiled)?;
    Ok(Precompiled {
        interface: interface::describe(&compilation, &packed),
        encoded,
    })
}

/// Loads on `device` a module that generated code embeds, and creates on the
/// device everything that a dispatch of each of its kernels needs.
///
/// The generated code calls this from its `Shader::load`. With the feature
/// `wgpu`, the module is the encoded snapshot that [`precompile_file`]
/// returned, which the load decodes with no call into Slang. With the
/// feature `rhi`, it is the Slang source of the module and of every module
/// that it imports, each after the modules that it imports, with the module
/// itself last. The load then compiles each in one session of its own under
/// its name, so that each `import` finds the module that the session holds,
/// and loads the last as
/// [`Device::load_module_from_source`](crate::Device::load_module_from_source)
/// does.
///
/// # Examples
///
/// The code that the macro `shader!` of `slango-macros` generates defines a
/// constant `EMBEDDED` of the type [`Embedded`] for the backend of the build,
/// and its `Shader::load` passes that constant to this function.
///
/// # Errors
///
/// Returns an [`Error`](crate::Error) of the kinds that
/// [`Device::load_module`](crate::Device::load_module) returns. With the
/// feature `wgpu`, it also returns
/// [`ErrorKind::InvalidArgument`](crate::ErrorKind::InvalidArgument) when
/// the encoded snapshot is not one of [`ENCODING_VERSION`], is truncated or
/// malformed, or has trailing bytes. With the feature `rhi`, it returns
/// [`ErrorKind::InvalidArgument`](crate::ErrorKind::InvalidArgument) when
/// the embedded module holds no source.
pub fn load_embedded(device: &Device, embedded: &Embedded<'_>) -> Result<Module> {
    device.load_embedded(embedded)
}

#[cfg(test)]
mod tests;
