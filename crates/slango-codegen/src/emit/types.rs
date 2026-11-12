use proc_macro2::{Literal, TokenStream};
use quote::quote;
use slango::__private::{Scalar, Shape, ValueType};

use crate::error::{Error, ErrorKind, Result};
use crate::names::ident;

pub(super) const GENERATE: &str = "generate the bindings";

pub(super) fn scalar_type(scalar: Scalar) -> TokenStream {
    match scalar {
        Scalar::Bool => quote!(bool),
        Scalar::Int32 => quote!(i32),
        Scalar::Uint32 => quote!(u32),
        Scalar::Float32 => quote!(f32),
    }
}

pub(super) fn value_type(value: &ValueType) -> TokenStream {
    shape_type(&scalar_type(value.scalar), &value.shape)
}

fn shape_type(scalar: &TokenStream, shape: &Shape) -> TokenStream {
    match shape {
        Shape::Scalar => scalar.clone(),
        Shape::Vector { length } => {
            let length = Literal::u32_unsuffixed(*length);
            quote!([#scalar; #length])
        }
        Shape::Matrix { rows, columns } => {
            let rows = Literal::u32_unsuffixed(*rows);
            let columns = Literal::u32_unsuffixed(*columns);
            quote!([[#scalar; #columns]; #rows])
        }
        Shape::Array {
            length, element, ..
        } => {
            let element = shape_type(scalar, element);
            let length = Literal::u64_unsuffixed(*length);
            quote!([#element; #length])
        }
    }
}

/// Returns the Rust type of a generated struct, which keeps its Slang name.
pub(super) fn struct_type(name: &str) -> Result<TokenStream> {
    let ty = ident(name, &format!("the structure `{name}`"))?;
    Ok(quote!(#ty))
}

/// Returns the error for a parameter of a kernel that the generator does not
/// support, with the reason.
pub(super) fn unsupported(kernel: &str, path: &str, reason: &str) -> Error {
    Error::new(ErrorKind::Unsupported, GENERATE).with_detail(format!(
        "the parameter `{path}` of the kernel `{kernel}` is not supported, because {reason}"
    ))
}
