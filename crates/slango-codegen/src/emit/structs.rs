use proc_macro2::{Ident, Literal, TokenStream};
use quote::{format_ident, quote};
use slango::__private::{
    ElementType, FieldType, KernelInterface, ParameterKind, Scalar, Shape, Structure,
    UniformStructure, ValueType,
};

use super::types::{scalar_type, struct_type, unsupported, value_type};
use crate::error::{Error, ErrorKind, Result};
use crate::names::{NAME, Scope, snake_case};

/// The parameter of a kernel through which the generator reaches a
/// structure, which an error names.
#[derive(Clone, Copy, Debug)]
struct Use<'a> {
    kernel: &'a str,
    parameter: &'a str,
}

/// A field of a structure, with its Slang name and its Rust type.
#[derive(Debug)]
struct Member<'a> {
    name: &'a str,
    ty: TokenStream,
}

/// The layout of a structure in a structured buffer, with one entry in
/// `fields` for each member of its definition, in the same order.
#[derive(Debug)]
struct Layout<'a> {
    structure: &'a Structure,
    fields: Vec<Placed>,
    tail: u64,
}

/// A field of a structure in a structured buffer, with the padding before
/// it.
#[derive(Debug)]
struct Placed {
    padding: u64,
    offset: u64,
    content: Content,
}

/// What a field of a structure in a structured buffer holds.
#[derive(Debug)]
enum Content {
    Value { scalar: Scalar, packed: Packed },
    Structure { size: u64 },
}

/// A plain value whose elements follow each other with no gap, which a Rust
/// array of the same scalars lays out alike, with its size in bytes.
#[derive(Debug)]
struct Packed {
    shape: PackedShape,
    size: u64,
}

/// The shape of a [`Packed`] value.
#[derive(Debug)]
enum PackedShape {
    Scalar,
    Array { length: u64, element: Box<Packed> },
}

/// The size in bytes of each scalar that a generated struct supports.
const SCALAR_SIZE: u64 = 4;

/// A Slang structure for which the generator emits a struct, with the
/// layout of a structured buffer when it is the element of one or a field of
/// such an element.
#[derive(Debug)]
struct Definition<'a> {
    name: &'a str,
    first: Use<'a>,
    members: Vec<Member<'a>>,
    layout: Option<Layout<'a>>,
}

/// The structures that the kernels of a module take, each once under its
/// Slang name, in the order in which the kernels first reach them.
#[derive(Debug, Default)]
pub(super) struct Structures<'a> {
    definitions: Vec<Definition<'a>>,
}

impl<'a> Structures<'a> {
    /// Collects every structure that a buffer element or a plain value of a
    /// kernel holds, and reports the first one that the generator does not
    /// support or that differs from another structure of the same name.
    pub(super) fn collect(kernels: &'a [KernelInterface]) -> Result<Self> {
        let mut structures = Self::default();
        for kernel in kernels {
            for parameter in &kernel.parameters {
                if let ParameterKind::Buffer {
                    element: ElementType::Structure(structure),
                    ..
                } = &parameter.kind
                {
                    let at = Use {
                        kernel: &kernel.name,
                        parameter: &parameter.path,
                    };
                    if structure.size == 0 {
                        return Err(unsupported(
                            at.kernel,
                            at.parameter,
                            &format!(
                                "it is a structured buffer of the structure `{}`, whose size is 0 bytes",
                                structure.name
                            ),
                        ));
                    }
                    structures.element(structure, at)?;
                }
            }
            for structure in &kernel.structures {
                structures.uniform(kernel, structure)?;
            }
        }
        Ok(structures)
    }

    fn element(&mut self, structure: &'a Structure, at: Use<'a>) -> Result<()> {
        let field_error = |field: &str, what: &str| {
            unsupported(
                at.kernel,
                at.parameter,
                &format!(
                    "the field `{field}` of the structure `{}` is {what}, which a generated struct does not support",
                    structure.name
                ),
            )
        };
        let mut members = Vec::with_capacity(structure.fields.len());
        let mut fields = Vec::with_capacity(structure.fields.len());
        let mut end = 0_u64;
        for field in &structure.fields {
            let (ty, content, size) = match &field.ty {
                FieldType::Value(value) => {
                    let packed = packed(value).map_err(|what| field_error(&field.name, &what))?;
                    let size = packed.size;
                    let content = Content::Value {
                        scalar: value.scalar,
                        packed,
                    };
                    (value_type(value), content, size)
                }
                FieldType::Structure(inner) => {
                    self.element(inner, at)?;
                    let content = Content::Structure { size: inner.size };
                    (struct_type(&inner.name)?, content, inner.size)
                }
                FieldType::Other { name } => {
                    return Err(field_error(&field.name, &format!("of the type `{name}`")));
                }
            };
            let padding = field.offset.checked_sub(end).ok_or_else(|| {
                field_error(
                    &field.name,
                    &format!("at the offset {}, inside the field before it", field.offset),
                )
            })?;
            end = field.offset.checked_add(size).ok_or_else(|| {
                field_error(
                    &field.name,
                    &format!("at the offset {}, beyond any Rust struct", field.offset),
                )
            })?;
            members.push(Member {
                name: &field.name,
                ty,
            });
            fields.push(Placed {
                padding,
                offset: field.offset,
                content,
            });
        }
        let tail = structure.size.checked_sub(end).ok_or_else(|| {
            unsupported(
                at.kernel,
                at.parameter,
                &format!(
                    "the structure `{}` has a size of {} bytes, which ends before its last field",
                    structure.name, structure.size
                ),
            )
        })?;
        self.define(Definition {
            name: &structure.name,
            first: at,
            members,
            layout: Some(Layout {
                structure,
                fields,
                tail,
            }),
        })
    }

    fn uniform(
        &mut self,
        kernel: &'a KernelInterface,
        structure: &'a UniformStructure,
    ) -> Result<()> {
        let prefix = format!("{}.", structure.path);
        let child = |path: &'a str| {
            path.strip_prefix(prefix.as_str())
                .filter(|rest| !rest.contains('.'))
        };
        let mut children = Vec::new();
        for (position, parameter) in kernel.parameters.iter().enumerate() {
            let Some(name) = child(&parameter.path) else {
                continue;
            };
            let ParameterKind::Value(value) = &parameter.kind else {
                return Err(unsupported(
                    &kernel.name,
                    &parameter.path,
                    &format!(
                        "it is a resource inside the structure `{}`, which a generated struct does not hold",
                        structure.name
                    ),
                ));
            };
            children.push((
                position,
                Member {
                    name,
                    ty: value_type(value),
                },
            ));
        }
        for inner in &kernel.structures {
            let Some(name) = child(&inner.path) else {
                continue;
            };
            let inner_prefix = format!("{}.", inner.path);
            let position = kernel
                .parameters
                .iter()
                .position(|parameter| parameter.path.starts_with(&inner_prefix))
                .unwrap_or(usize::MAX);
            children.push((
                position,
                Member {
                    name,
                    ty: struct_type(&inner.name)?,
                },
            ));
        }
        children.sort_by_key(|(position, _)| *position);
        self.define(Definition {
            name: &structure.name,
            first: Use {
                kernel: &kernel.name,
                parameter: &structure.path,
            },
            members: children.into_iter().map(|(_, member)| member).collect(),
            layout: None,
        })
    }

    /// Records a structure, or checks it against the structure of the same
    /// name that is already recorded and adds its layout when the recorded
    /// one has none.
    fn define(&mut self, definition: Definition<'a>) -> Result<()> {
        let Some(existing) = self
            .definitions
            .iter_mut()
            .find(|existing| existing.name == definition.name)
        else {
            self.definitions.push(definition);
            return Ok(());
        };
        let same_members = existing.members.len() == definition.members.len()
            && existing
                .members
                .iter()
                .zip(&definition.members)
                .all(|(left, right)| {
                    left.name == right.name && left.ty.to_string() == right.ty.to_string()
                });
        let same_layout = match (&existing.layout, &definition.layout) {
            (Some(left), Some(right)) => left.structure == right.structure,
            _ => true,
        };
        if !(same_members && same_layout) {
            let name = definition.name;
            let Use { kernel, parameter } = existing.first;
            let Use {
                kernel: other_kernel,
                parameter: other_parameter,
            } = definition.first;
            return Err(Error::new(ErrorKind::NameConflict, NAME).with_detail(format!(
                "the structure `{name}` of the parameter `{parameter}` of the kernel `{kernel}` and the structure `{name}` of the parameter `{other_parameter}` of the kernel `{other_kernel}` are different structures that both become the Rust name `{name}`"
            )));
        }
        if existing.layout.is_none() {
            existing.layout = definition.layout;
        }
        Ok(())
    }

    /// Returns the items of every structure, each named after its Slang name
    /// in `types`.
    pub(super) fn items(&self, types: &mut Scope) -> Result<TokenStream> {
        let items = self
            .definitions
            .iter()
            .map(|definition| {
                let name = types.take(
                    definition.name.to_owned(),
                    format!("the structure `{}`", definition.name),
                )?;
                definition.items(&name)
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(quote!(#(#items)*))
    }
}

impl Definition<'_> {
    fn items(&self, name: &Ident) -> Result<TokenStream> {
        let mut scope = Scope::default();
        let fields = self
            .members
            .iter()
            .map(|member| {
                scope.take(
                    snake_case(member.name),
                    format!(
                        "the field `{}` of the structure `{}`",
                        member.name, self.name
                    ),
                )
            })
            .collect::<Result<Vec<_>>>()?;
        match &self.layout {
            None => Ok(self.uniform_items(name, &fields)),
            Some(layout) => self.element_items(name, &fields, layout, &mut scope),
        }
    }

    fn field_declarations(&self, fields: &[Ident]) -> Vec<TokenStream> {
        fields
            .iter()
            .zip(&self.members)
            .map(|(field, member)| {
                let doc = format!(" The field `{}` of the Slang structure.", member.name);
                let ty = &member.ty;
                quote! {
                    #[doc = #doc]
                    pub #field: #ty,
                }
            })
            .collect()
    }

    fn uniform_items(&self, name: &Ident, fields: &[Ident]) -> TokenStream {
        let declarations = self.field_declarations(fields);
        let types = self.members.iter().map(|member| &member.ty);
        let expect = many_arguments(fields.len());
        let doc = format!(
            " The Slang structure `{}`, which a kernel takes as a plain value.",
            self.name
        );
        quote! {
            #[doc = #doc]
            ///
            /// A dispatch sets each field of the value by its path, so the
            /// struct has no padding.
            #[derive(Clone, Copy, Debug, PartialEq)]
            pub struct #name {
                #(#declarations)*
            }

            impl #name {
                /// Returns the structure with the given fields.
                #[must_use]
                #expect
                pub const fn new(#(#fields: #types),*) -> Self {
                    Self { #(#fields),* }
                }
            }
        }
    }

    fn element_items(
        &self,
        name: &Ident,
        fields: &[Ident],
        layout: &Layout<'_>,
        scope: &mut Scope,
    ) -> Result<TokenStream> {
        let mut parts = Parts::default();
        let declarations = self.field_declarations(fields);
        let placed_fields = fields
            .iter()
            .zip(declarations)
            .zip(&self.members)
            .zip(&layout.fields);
        for (((field, declaration), member), placed) in placed_fields {
            parts.padding(placed.padding, self.name, scope)?;
            parts.field(name, field, declaration, placed, &member.ty);
        }
        parts.padding(layout.tail, self.name, scope)?;
        let Parts {
            declarations,
            zeroes,
            assertions,
            writes,
            reads,
            ..
        } = parts;
        let types = self.members.iter().map(|member| &member.ty);
        let expect = many_arguments(fields.len());
        let slang_name = self.name;
        let size = Literal::u64_unsuffixed(layout.structure.size);
        let doc = format!(
            " The Slang structure `{slang_name}`, with the layout that Slang gives it in a structured buffer."
        );
        let size_message = format!(
            "the size of the struct `{slang_name}` differs from the stride that Slang gives the structure"
        );
        Ok(quote! {
            #[doc = #doc]
            ///
            /// Each public field holds the field of the same name at the
            /// offset that Slang gives it, and private fields hold the
            /// padding, so a value is created with `new`. A `slango::Buffer`
            /// of the struct binds to a structured buffer of the structure.
            #[repr(C)]
            #[derive(Clone, Copy, Debug, PartialEq)]
            pub struct #name {
                #(#declarations)*
            }

            impl #name {
                /// Returns the structure with the given fields and zero in
                /// every byte of padding.
                #[must_use]
                #expect
                pub const fn new(#(#fields: #types),*) -> Self {
                    Self {
                        #(#fields,)*
                        #(#zeroes)*
                    }
                }
            }

            const _: () = {
                ::core::assert!(::core::mem::size_of::<#name>() == #size, #size_message);
                #(#assertions)*
            };

            impl ::slango::__private::StructElement for #name {
                const SLANG_NAME: &'static str = #slang_name;
                const STRIDE: usize = #size;

                fn write_bytes(&self, out: &mut ::std::vec::Vec<u8>) {
                    #(#writes)*
                }

                fn read_bytes(bytes: &[u8]) -> ::core::option::Option<Self> {
                    if bytes.len() != #size {
                        return ::core::option::Option::None;
                    }
                    ::core::option::Option::Some(Self {
                        #(#reads)*
                    })
                }
            }
        })
    }
}

/// The parts of the items of a structure in a structured buffer, which the
/// generator fills field by field in the order of their offsets.
#[derive(Debug, Default)]
struct Parts {
    paddings: usize,
    declarations: Vec<TokenStream>,
    zeroes: Vec<TokenStream>,
    assertions: Vec<TokenStream>,
    writes: Vec<TokenStream>,
    reads: Vec<TokenStream>,
}

impl Parts {
    fn padding(&mut self, bytes: u64, structure: &str, scope: &mut Scope) -> Result<()> {
        if bytes == 0 {
            return Ok(());
        }
        let field = scope.take(
            format!("_padding{}", self.paddings),
            format!("the padding of the structure `{structure}`"),
        )?;
        self.paddings += 1;
        let bytes = Literal::u64_unsuffixed(bytes);
        self.declarations.push(quote!(#field: [u8; #bytes],));
        let zero = quote!(#field: [0; #bytes],);
        self.zeroes.push(zero.clone());
        self.reads.push(zero);
        self.writes
            .push(quote!(out.extend_from_slice(&[0_u8; #bytes]);));
        Ok(())
    }

    fn field(
        &mut self,
        name: &Ident,
        field: &Ident,
        declaration: TokenStream,
        placed: &Placed,
        ty: &TokenStream,
    ) {
        let offset = Literal::u64_unsuffixed(placed.offset);
        let message = format!(
            "the offset of the field `{field}` of the struct `{name}` differs from the offset that Slang gives it"
        );
        self.declarations.push(declaration);
        self.assertions.push(quote! {
            ::core::assert!(::core::mem::offset_of!(#name, #field) == #offset, #message);
        });
        match &placed.content {
            Content::Value { scalar, packed } => {
                let scalar = scalar_type(*scalar);
                self.writes
                    .push(write_value(&quote!(self.#field), packed, 0));
                let read = read_value(&scalar, packed, placed.offset);
                self.reads.push(quote!(#field: #read,));
            }
            Content::Structure { size } => {
                let end = Literal::u64_unsuffixed(placed.offset + size);
                self.writes.push(quote! {
                    ::slango::__private::StructElement::write_bytes(&self.#field, out);
                });
                self.reads.push(quote! {
                    #field: <#ty as ::slango::__private::StructElement>::read_bytes(bytes.get(#offset..#end)?)?,
                });
            }
        }
    }
}

/// Returns the statements that append the scalars of `value`, a plain value
/// of the shape `packed`, to `out` in little-endian order.
fn write_value(value: &TokenStream, packed: &Packed, depth: usize) -> TokenStream {
    match &packed.shape {
        PackedShape::Scalar => quote!(out.extend_from_slice(&#value.to_le_bytes());),
        PackedShape::Array { element, .. } => {
            let item = format_ident!("item{depth}");
            let inner = write_value(&quote!(#item), element, depth + 1);
            quote! {
                for #item in #value {
                    #inner
                }
            }
        }
    }
}

/// Returns the expression that reads a plain value of the shape `packed`
/// from `bytes` at `offset`.
fn read_value(scalar: &TokenStream, packed: &Packed, offset: u64) -> TokenStream {
    match &packed.shape {
        PackedShape::Scalar => {
            let offset = Literal::u64_unsuffixed(offset);
            quote!(#scalar::from_le_bytes(*bytes.get(#offset..)?.first_chunk()?))
        }
        PackedShape::Array { length, element } => {
            let items = (0..*length)
                .map(|index| read_value(scalar, element, offset + index * element.size));
            quote!([#(#items),*])
        }
    }
}

/// Returns the shape of a plain value in a structured buffer with its size,
/// or what the value is when a Rust array does not lay it out as Slang does.
fn packed(value: &ValueType) -> std::result::Result<Packed, String> {
    if value.scalar == Scalar::Bool {
        return Err(match value.shape {
            Shape::Scalar => "a `bool`".to_owned(),
            _ => "made of `bool` values".to_owned(),
        });
    }
    packed_shape(&value.shape)
}

fn packed_shape(shape: &Shape) -> std::result::Result<Packed, String> {
    let scalar = || Packed {
        shape: PackedShape::Scalar,
        size: SCALAR_SIZE,
    };
    match shape {
        Shape::Scalar => Ok(scalar()),
        Shape::Vector { length } => Ok(Packed {
            shape: PackedShape::Array {
                length: u64::from(*length),
                element: Box::new(scalar()),
            },
            size: u64::from(*length) * SCALAR_SIZE,
        }),
        Shape::Matrix { .. } => Err("a matrix".to_owned()),
        Shape::Array {
            length,
            stride,
            element,
        } => {
            let element = packed_shape(element)?;
            let element_size = element.size;
            if *stride != element_size {
                return Err(format!(
                    "an array whose stride of {stride} bytes differs from the {element_size} bytes of its element"
                ));
            }
            let size = length.checked_mul(element_size).ok_or_else(|| {
                format!(
                    "an array of {length} elements of {element_size} bytes, beyond any Rust struct"
                )
            })?;
            Ok(Packed {
                shape: PackedShape::Array {
                    length: *length,
                    element: Box::new(element),
                },
                size,
            })
        }
    }
}

/// Returns the lint exception of a `new` with one argument for each of
/// `count` fields, when Clippy's default limit of 7 arguments lints it.
fn many_arguments(count: usize) -> Option<TokenStream> {
    (count > 7).then(|| {
        quote! {
            #[expect(
                clippy::too_many_arguments,
                reason = "`new` takes one argument for each field of the Slang structure"
            )]
        }
    })
}

#[cfg(test)]
mod tests;
