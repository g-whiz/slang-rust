use crate::__private::ENCODING_VERSION;
use crate::error::{Error, ErrorKind, Result};
use crate::layout::{
    Access, Attribute, Binding, Compiled, CompiledKernel, CompiledRender, ElementType, Field,
    FieldType, KernelLayout, PlainValue, Rejection, RenderLayout, RenderStage, Resource, Scalar,
    Shape, Structure, UniformBlock, UniformStructure, UsableRender, Usage,
};

/// The bytes with which every encoded snapshot starts, before its version.
const MAGIC: [u8; 8] = *b"slango\0S";

/// The deepest nesting of arrays in a shape, or of structures in a
/// structure, that the encoding holds, so that a decode of any input
/// recurses a bounded number of times.
const MAX_DEPTH: u32 = 256;

const ENCODE: &str = "encode a layout snapshot";
const DECODE: &str = "load a precompiled module";

/// Encodes a snapshot as the private, versioned byte string of SLM-7 of
/// `docs/design/slango-macros.md`: [`MAGIC`], [`ENCODING_VERSION`] as a
/// little-endian `u32`, and the records of the snapshot, each integer
/// little-endian, each count and byte length a `u32`, and each enumeration a
/// tag byte.
pub(crate) fn encode(compiled: &Compiled) -> Result<Vec<u8>> {
    let mut writer = Writer {
        bytes: MAGIC.to_vec(),
        overflow: false,
    };
    writer.u32(ENCODING_VERSION);
    let Compiled { kernels, render } = compiled;
    writer.list(kernels, Writer::kernel);
    writer.list(render, Writer::render);
    if writer.overflow {
        return Err(Error::new(ErrorKind::UnsupportedParameter, ENCODE).with_detail(format!(
            "the snapshot holds a list longer than {} items or nests deeper than {MAX_DEPTH} levels",
            u32::MAX
        )));
    }
    Ok(writer.bytes)
}

/// Decodes a byte string that [`encode`] wrote, and fails with
/// [`ErrorKind::InvalidArgument`] on a byte string of another version, a
/// truncated or malformed one, or one with trailing bytes.
pub(crate) fn decode(bytes: &[u8]) -> Result<Compiled> {
    let mut input = Reader { bytes };
    if input.take(MAGIC.len()) != Some(MAGIC.as_slice()) {
        return Err(invalid(
            "the bytes are not an encoded snapshot of slango".to_owned(),
        ));
    }
    let version = input.u32().ok_or_else(malformed)?;
    if version != ENCODING_VERSION {
        return Err(invalid(format!(
            "the encoded snapshot has version {version}, and this version of slango reads version {ENCODING_VERSION}"
        )));
    }
    let kernels = input.list(Reader::kernel).ok_or_else(malformed)?;
    let render = input.list(Reader::render).ok_or_else(malformed)?;
    if !input.bytes.is_empty() {
        return Err(invalid(format!(
            "the encoded snapshot has {} trailing bytes",
            input.bytes.len()
        )));
    }
    Ok(Compiled { kernels, render })
}

fn invalid(detail: String) -> Error {
    Error::new(ErrorKind::InvalidArgument, DECODE).with_detail(detail)
}

fn malformed() -> Error {
    invalid("the encoded snapshot is truncated or malformed".to_owned())
}

struct Writer {
    bytes: Vec<u8>,
    /// Whether a count, a length, or a nesting did not fit the encoding.
    overflow: bool,
}

impl Writer {
    fn u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    fn u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn count(&mut self, count: usize) {
        if let Ok(count) = u32::try_from(count) {
            self.u32(count);
        } else {
            self.overflow = true;
        }
    }

    fn list<T>(&mut self, items: &[T], mut write: impl FnMut(&mut Self, &T)) {
        self.count(items.len());
        for item in items {
            write(self, item);
        }
    }

    fn str(&mut self, text: &str) {
        self.count(text.len());
        self.bytes.extend_from_slice(text.as_bytes());
    }

    fn words(&mut self, words: &[u32]) {
        self.list(words, |writer, word| writer.u32(*word));
    }

    fn kernel(&mut self, kernel: &CompiledKernel) {
        let CompiledKernel { layout, spirv } = kernel;
        let KernelLayout {
            name,
            thread_group_size,
            bindings,
            immediates,
            structures,
        } = layout;
        self.str(name);
        for size in thread_group_size {
            self.u32(*size);
        }
        self.list(bindings, Self::binding);
        self.immediates(immediates.as_ref());
        self.list(structures, |writer, structure| {
            let UniformStructure { path, name } = structure;
            writer.str(path);
            writer.str(name);
        });
        self.words(spirv);
    }

    fn render(&mut self, record: &CompiledRender) {
        let CompiledRender {
            name,
            stage,
            usable,
        } = record;
        self.str(name);
        self.u8(match stage {
            RenderStage::Vertex => 0,
            RenderStage::Fragment => 1,
        });
        match usable {
            Ok(UsableRender { layout, spirv }) => {
                let RenderLayout {
                    bindings,
                    immediates,
                    attributes,
                } = layout;
                self.u8(0);
                self.list(bindings, Self::binding);
                self.immediates(immediates.as_ref());
                self.list(attributes, Self::attribute);
                self.words(spirv);
            }
            Err(Rejection { kind, detail }) => {
                self.u8(1);
                self.u8(kind_tag(*kind));
                self.str(detail);
            }
        }
    }

    fn attribute(&mut self, attribute: &Attribute) {
        let Attribute {
            path,
            location,
            scalar,
            components,
            usage,
        } = attribute;
        self.str(path);
        self.u32(*location);
        self.scalar(*scalar);
        self.u32(*components);
        self.usage(*usage);
    }

    fn binding(&mut self, binding: &Binding) {
        let Binding {
            path,
            set,
            index,
            usage,
            resource,
        } = binding;
        self.str(path);
        self.u32(*set);
        self.u32(*index);
        self.usage(*usage);
        match resource {
            Resource::StorageBuffer {
                access,
                stride,
                element,
            } => {
                self.u8(0);
                self.u8(match access {
                    Access::ReadOnly => 0,
                    Access::ReadWrite => 1,
                });
                self.u64(*stride);
                self.element(element);
            }
            Resource::UniformBuffer(block) => {
                self.u8(1);
                self.block(block);
            }
            Resource::SampledTexture => self.u8(2),
            Resource::Sampler => self.u8(3),
        }
    }

    fn element(&mut self, element: &ElementType) {
        match element {
            ElementType::Plain { scalar, components } => {
                self.u8(0);
                self.scalar(*scalar);
                self.u32(*components);
            }
            ElementType::OtherPlain { name } => {
                self.u8(1);
                self.str(name);
            }
            ElementType::Structure(structure) => {
                self.u8(2);
                self.structure(structure, 0);
            }
            ElementType::Composite => self.u8(3),
        }
    }

    fn structure(&mut self, structure: &Structure, depth: u32) {
        if depth >= MAX_DEPTH {
            self.overflow = true;
            return;
        }
        let Structure { name, size, fields } = structure;
        self.str(name);
        self.u64(*size);
        self.list(fields, |writer, field| {
            let Field { name, offset, ty } = field;
            writer.str(name);
            writer.u64(*offset);
            match ty {
                FieldType::Plain { scalar, shape } => {
                    writer.u8(0);
                    writer.scalar(*scalar);
                    writer.shape(shape, 0);
                }
                FieldType::Structure(inner) => {
                    writer.u8(1);
                    writer.structure(inner, depth.saturating_add(1));
                }
                FieldType::Other { name } => {
                    writer.u8(2);
                    writer.str(name);
                }
            }
        });
    }

    fn immediates(&mut self, immediates: Option<&UniformBlock>) {
        match immediates {
            None => self.u8(0),
            Some(block) => {
                self.u8(1);
                self.block(block);
            }
        }
    }

    fn block(&mut self, block: &UniformBlock) {
        let UniformBlock { size, values } = block;
        self.u64(*size);
        self.list(values, |writer, value| {
            let PlainValue {
                path,
                offset,
                size,
                scalar,
                shape,
            } = value;
            writer.str(path);
            writer.u64(*offset);
            writer.u64(*size);
            writer.scalar(*scalar);
            writer.shape(shape, 0);
        });
    }

    fn shape(&mut self, shape: &Shape, depth: u32) {
        if depth >= MAX_DEPTH {
            self.overflow = true;
            return;
        }
        match shape {
            Shape::Scalar => self.u8(0),
            Shape::Vector { length } => {
                self.u8(1);
                self.u32(*length);
            }
            Shape::Matrix { rows, columns } => {
                self.u8(2);
                self.u32(*rows);
                self.u32(*columns);
            }
            Shape::Array {
                length,
                stride,
                element,
            } => {
                self.u8(3);
                self.u64(*length);
                self.u64(*stride);
                self.shape(element, depth.saturating_add(1));
            }
        }
    }

    fn scalar(&mut self, scalar: Scalar) {
        self.u8(match scalar {
            Scalar::Bool => 0,
            Scalar::Int32 => 1,
            Scalar::Uint32 => 2,
            Scalar::Float32 => 3,
        });
    }

    fn usage(&mut self, usage: Usage) {
        self.u8(match usage {
            Usage::Used => 0,
            Usage::Unused => 1,
        });
    }
}

fn kind_tag(kind: ErrorKind) -> u8 {
    match kind {
        ErrorKind::Compile => 0,
        ErrorKind::NoAdapter => 1,
        ErrorKind::DeviceRequest => 2,
        ErrorKind::UnsupportedShader => 3,
        ErrorKind::UnsupportedParameter => 4,
        ErrorKind::MissingFeature => 5,
        ErrorKind::InvalidArgument => 6,
        ErrorKind::Backend => 7,
    }
}

/// The reader of an encoded snapshot, whose every method returns `None` on
/// bytes that [`Writer`] cannot have written.
struct Reader<'a> {
    bytes: &'a [u8],
}

impl<'a> Reader<'a> {
    fn take(&mut self, len: usize) -> Option<&'a [u8]> {
        let (head, tail) = self.bytes.split_at_checked(len)?;
        self.bytes = tail;
        Some(head)
    }

    fn array<const N: usize>(&mut self) -> Option<[u8; N]> {
        self.take(N)?.try_into().ok()
    }

    fn u8(&mut self) -> Option<u8> {
        self.array::<1>().map(u8::from_le_bytes)
    }

    fn u32(&mut self) -> Option<u32> {
        self.array().map(u32::from_le_bytes)
    }

    fn u64(&mut self) -> Option<u64> {
        self.array().map(u64::from_le_bytes)
    }

    /// Reads a count of items, each of which takes at least one byte, so a
    /// count above the number of bytes left is malformed.
    fn count(&mut self) -> Option<usize> {
        let count = usize::try_from(self.u32()?).ok()?;
        (count <= self.bytes.len()).then_some(count)
    }

    fn list<T>(&mut self, mut read: impl FnMut(&mut Self) -> Option<T>) -> Option<Vec<T>> {
        let count = self.count()?;
        let mut items = Vec::with_capacity(count);
        for _ in 0..count {
            items.push(read(self)?);
        }
        Some(items)
    }

    fn string(&mut self) -> Option<String> {
        let len = self.count()?;
        let bytes = self.take(len)?;
        String::from_utf8(bytes.to_vec()).ok()
    }

    fn words(&mut self) -> Option<Vec<u32>> {
        self.list(Self::u32)
    }

    fn kernel(&mut self) -> Option<CompiledKernel> {
        let name = self.string()?;
        let thread_group_size = [self.u32()?, self.u32()?, self.u32()?];
        let bindings = self.list(Self::binding)?;
        let immediates = match self.u8()? {
            0 => None,
            1 => Some(self.block()?),
            _ => return None,
        };
        let structures = self.list(|reader| {
            Some(UniformStructure {
                path: reader.string()?,
                name: reader.string()?,
            })
        })?;
        let spirv = self.words()?;
        Some(CompiledKernel {
            layout: KernelLayout {
                name,
                thread_group_size,
                bindings,
                immediates,
                structures,
            },
            spirv,
        })
    }

    fn render(&mut self) -> Option<CompiledRender> {
        let name = self.string()?;
        let stage = match self.u8()? {
            0 => RenderStage::Vertex,
            1 => RenderStage::Fragment,
            _ => return None,
        };
        let usable = match self.u8()? {
            0 => {
                let bindings = self.list(Self::binding)?;
                let immediates = match self.u8()? {
                    0 => None,
                    1 => Some(self.block()?),
                    _ => return None,
                };
                let attributes = self.list(Self::attribute)?;
                let spirv = self.words()?;
                Ok(UsableRender {
                    layout: RenderLayout {
                        bindings,
                        immediates,
                        attributes,
                    },
                    spirv,
                })
            }
            1 => {
                let kind = kind_of_tag(self.u8()?)?;
                let detail = self.string()?;
                Err(Rejection { kind, detail })
            }
            _ => return None,
        };
        Some(CompiledRender {
            name,
            stage,
            usable,
        })
    }

    fn attribute(&mut self) -> Option<Attribute> {
        Some(Attribute {
            path: self.string()?,
            location: self.u32()?,
            scalar: self.scalar()?,
            components: self.u32()?,
            usage: self.usage()?,
        })
    }

    fn binding(&mut self) -> Option<Binding> {
        let path = self.string()?;
        let set = self.u32()?;
        let index = self.u32()?;
        let usage = self.usage()?;
        let resource = match self.u8()? {
            0 => {
                let access = match self.u8()? {
                    0 => Access::ReadOnly,
                    1 => Access::ReadWrite,
                    _ => return None,
                };
                let stride = self.u64()?;
                let element = self.element()?;
                Resource::StorageBuffer {
                    access,
                    stride,
                    element,
                }
            }
            1 => Resource::UniformBuffer(self.block()?),
            2 => Resource::SampledTexture,
            3 => Resource::Sampler,
            _ => return None,
        };
        Some(Binding {
            path,
            set,
            index,
            usage,
            resource,
        })
    }

    fn element(&mut self) -> Option<ElementType> {
        Some(match self.u8()? {
            0 => ElementType::Plain {
                scalar: self.scalar()?,
                components: self.u32()?,
            },
            1 => ElementType::OtherPlain {
                name: self.string()?,
            },
            2 => ElementType::Structure(self.structure(0)?),
            3 => ElementType::Composite,
            _ => return None,
        })
    }

    fn structure(&mut self, depth: u32) -> Option<Structure> {
        if depth >= MAX_DEPTH {
            return None;
        }
        let name = self.string()?;
        let size = self.u64()?;
        let fields = self.list(|reader| {
            let name = reader.string()?;
            let offset = reader.u64()?;
            let ty = match reader.u8()? {
                0 => FieldType::Plain {
                    scalar: reader.scalar()?,
                    shape: reader.shape(0)?,
                },
                1 => FieldType::Structure(reader.structure(depth.saturating_add(1))?),
                2 => FieldType::Other {
                    name: reader.string()?,
                },
                _ => return None,
            };
            Some(Field { name, offset, ty })
        })?;
        Some(Structure { name, size, fields })
    }

    fn block(&mut self) -> Option<UniformBlock> {
        let size = self.u64()?;
        let values = self.list(|reader| {
            Some(PlainValue {
                path: reader.string()?,
                offset: reader.u64()?,
                size: reader.u64()?,
                scalar: reader.scalar()?,
                shape: reader.shape(0)?,
            })
        })?;
        Some(UniformBlock { size, values })
    }

    fn shape(&mut self, depth: u32) -> Option<Shape> {
        if depth >= MAX_DEPTH {
            return None;
        }
        Some(match self.u8()? {
            0 => Shape::Scalar,
            1 => Shape::Vector {
                length: self.u32()?,
            },
            2 => Shape::Matrix {
                rows: self.u32()?,
                columns: self.u32()?,
            },
            3 => Shape::Array {
                length: self.u64()?,
                stride: self.u64()?,
                element: Box::new(self.shape(depth.saturating_add(1))?),
            },
            _ => return None,
        })
    }

    fn scalar(&mut self) -> Option<Scalar> {
        Some(match self.u8()? {
            0 => Scalar::Bool,
            1 => Scalar::Int32,
            2 => Scalar::Uint32,
            3 => Scalar::Float32,
            _ => return None,
        })
    }

    fn usage(&mut self) -> Option<Usage> {
        Some(match self.u8()? {
            0 => Usage::Used,
            1 => Usage::Unused,
            _ => return None,
        })
    }
}

fn kind_of_tag(tag: u8) -> Option<ErrorKind> {
    Some(match tag {
        0 => ErrorKind::Compile,
        1 => ErrorKind::NoAdapter,
        2 => ErrorKind::DeviceRequest,
        3 => ErrorKind::UnsupportedShader,
        4 => ErrorKind::UnsupportedParameter,
        5 => ErrorKind::MissingFeature,
        6 => ErrorKind::InvalidArgument,
        7 => ErrorKind::Backend,
        _ => return None,
    })
}

#[cfg(test)]
mod tests;
