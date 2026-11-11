use proc_macro2::{Ident, Literal, TokenStream};
use quote::{format_ident, quote};
use slango::__private::{
    Access, ElementType, Interface, KernelInterface, Parameter, ParameterKind, Precompiled, Scalar,
    Shape, ValueType,
};

use crate::error::{Error, ErrorKind, Result};
use crate::names::{Scope, snake_case, upper_camel_case};

const GENERATE: &str = "generate the bindings";

/// The items of `Shader` whose names no kernel field may take.
const SHADER_ITEMS: [&str; 2] = ["load", "module"];

/// Returns the tokens of the bindings of a precompiled module.
pub(crate) fn bindings(precompiled: &Precompiled) -> Result<TokenStream> {
    let Precompiled { interface, encoded } = precompiled;
    let Interface {
        kernels,
        dependencies,
    } = interface;
    let tracking = dependencies
        .iter()
        .map(|path| {
            let path = path.to_str().ok_or_else(|| {
                Error::new(ErrorKind::Unsupported, GENERATE).with_detail(format!(
                    "the path `{}` of a file that Slang read is not valid UTF-8",
                    path.display()
                ))
            })?;
            Ok(quote! { const _: &[u8] = ::core::include_bytes!(#path); })
        })
        .collect::<Result<Vec<_>>>()?;
    let version = Literal::u32_unsuffixed(slango::__private::ENCODING_VERSION);
    let encoded = Literal::byte_string(encoded);

    let mut types = Scope::reserving(&["Shader"], "the struct `Shader`");
    let mut shader_fields = Scope::reserving(&SHADER_ITEMS, "an item of `Shader`");
    let mut items = Vec::with_capacity(kernels.len());
    let mut fields = Vec::with_capacity(kernels.len());
    let mut lookups = Vec::with_capacity(kernels.len());
    for kernel in kernels {
        let what = format!("the kernel `{}`", kernel.name);
        let name = upper_camel_case(&kernel.name);
        let type_name = types.take(name.clone(), what.clone())?;
        let args_name = types.take(
            format!("{name}Args"),
            format!("the arguments of the kernel `{}`", kernel.name),
        )?;
        let field = shader_fields.take(snake_case(&kernel.name), what)?;
        items.push(kernel_items(kernel, &type_name, &args_name)?);
        let slang_name = &kernel.name;
        let doc = format!(" The kernel `{slang_name}`.");
        fields.push(quote! {
            #[doc = #doc]
            pub #field: #type_name,
        });
        lookups.push(quote! {
            #field: #type_name { kernel: module.kernel(#slang_name)?.clone() },
        });
    }

    Ok(quote! {
        #(#tracking)*

        const _: () = ::core::assert!(
            ::slango::__private::ENCODING_VERSION == #version,
            "the bindings were generated for another version of slango, so the versions of slango and slango-macros differ"
        );

        #(#items)*

        /// The Slang module of the bindings, compiled when the crate was
        /// built, with a typed kernel for each of its compute entry points.
        ///
        /// A kernel is a function of the module that runs on many threads of
        /// the GPU at once. [`Shader::load`] creates the module on a device,
        /// and each field of the module runs one kernel with typed arguments.
        #[derive(Clone, Debug)]
        pub struct Shader {
            #(#fields)*
            module: ::slango::Module,
        }

        impl Shader {
            /// Loads the module on `device` from the code that was compiled
            /// when the crate was built, with no call into Slang, and looks up
            /// each of its kernels.
            ///
            /// # Errors
            ///
            /// Returns a [`slango::Error`] when `slango` cannot load the
            /// module on the device, for the reasons that
            /// [`slango::Device::load_module`] gives, or when the version of
            /// `slango` differs from the one that generated the bindings.
            pub fn load(device: &::slango::Device) -> ::slango::Result<Self> {
                let module = ::slango::__private::load_precompiled(device, #encoded)?;
                ::core::result::Result::Ok(Self {
                    #(#lookups)*
                    module,
                })
            }

            /// Returns the module, for what the typed kernels do not offer,
            /// such as its render pipelines.
            #[must_use]
            pub fn module(&self) -> &::slango::Module {
                &self.module
            }
        }
    })
}

/// A field of the arguments of a kernel, with the call that gives it.
struct Argument {
    name: Ident,
    ty: TokenStream,
    doc: String,
    method: Ident,
    path: String,
    borrows: bool,
}

fn kernel_items(
    kernel: &KernelInterface,
    type_name: &Ident,
    args_name: &Ident,
) -> Result<TokenStream> {
    let KernelInterface {
        name,
        thread_group_size,
        parameters,
        structures,
    } = kernel;
    if let Some(structure) = structures.first() {
        return Err(unsupported(
            name,
            &structure.path,
            &format!(
                "it is a value of the structure `{}`, for which this version generates no struct",
                structure.name
            ),
        ));
    }
    let mut scope = Scope::default();
    let arguments = parameters
        .iter()
        .map(|parameter| argument(name, parameter, &mut scope))
        .collect::<Result<Vec<_>>>()?;

    let lifetime = arguments
        .iter()
        .any(|argument| argument.borrows)
        .then(|| quote!(<'a>));
    let elided = lifetime.as_ref().map(|_| quote!(<'_>));
    let declarations = arguments.iter().map(|argument| {
        let Argument { name, ty, doc, .. } = argument;
        quote! {
            #[doc = #doc]
            pub #name: #ty,
        }
    });
    let calls = arguments.iter().map(|argument| {
        let Argument {
            name, method, path, ..
        } = argument;
        quote!(.#method(#path, args.#name))
    });
    let calls = quote!(#(#calls)*);
    let use_args = arguments
        .is_empty()
        .then(|| quote!(let #args_name {} = args;));
    let [x, y, z] = thread_group_size.map(Literal::u32_unsuffixed);
    let args_doc = format!(" The arguments of the kernel `{name}`, one field for each parameter.");
    let kernel_doc =
        format!(" The kernel `{name}` of the module, which [`{args_name}`] gives its arguments.");

    Ok(quote! {
        #[doc = #args_doc]
        ///
        /// Each field holds the value or the resource that the call gives the
        /// parameter of the same name. A parameter that the compiled kernel
        /// does not use has a field too, so the fields do not change when the
        /// compiler drops a parameter.
        #[derive(Clone, Copy, Debug)]
        pub struct #args_name #lifetime {
            #(#declarations)*
        }

        #[doc = #kernel_doc]
        ///
        /// A kernel is a function of the module that runs on many threads of
        /// the GPU at once, in groups of [`Self::THREAD_GROUP_SIZE`] threads.
        #[derive(Clone, Debug)]
        pub struct #type_name {
            kernel: ::slango::Kernel,
        }

        impl #type_name {
            /// The number of threads in each thread group, along x, y, and
            /// z, which the `[numthreads]` attribute of the kernel sets.
            pub const THREAD_GROUP_SIZE: [u32; 3] = [#x, #y, #z];

            /// Runs the kernel with `args` on at least `elements` threads
            /// along x, y, and z, in as many thread groups as that needs.
            ///
            /// # Errors
            ///
            /// Returns a [`slango::Error`] for the reasons that
            /// [`slango::Call::dispatch`] gives, such as a buffer of another
            /// device.
            pub fn dispatch(&self, args: &#args_name #elided, elements: [u32; 3]) -> ::slango::Result<()> {
                #use_args
                self.kernel.call() #calls .dispatch(elements)
            }

            /// Runs the kernel with `args` on `groups` thread groups along x,
            /// y, and z.
            ///
            /// # Errors
            ///
            /// Returns a [`slango::Error`] for the reasons that
            /// [`slango::Call::dispatch_groups`] gives, such as a buffer of
            /// another device.
            pub fn dispatch_groups(&self, args: &#args_name #elided, groups: [u32; 3]) -> ::slango::Result<()> {
                #use_args
                self.kernel.call() #calls .dispatch_groups(groups)
            }

            /// Returns the kernel, for a call that sets its parameters by
            /// name.
            #[must_use]
            pub fn kernel(&self) -> &::slango::Kernel {
                &self.kernel
            }
        }
    })
}

fn argument(kernel: &str, parameter: &Parameter, scope: &mut Scope) -> Result<Argument> {
    let Parameter { path, kind } = parameter;
    let (ty, doc, method, borrows) = match kind {
        ParameterKind::Value(value) => (
            value_type(value),
            format!(" The value of the parameter `{path}`."),
            "set",
            false,
        ),
        ParameterKind::Buffer { access, element } => {
            let element =
                element_type(element).map_err(|reason| unsupported(kernel, path, &reason))?;
            let access = match access {
                Access::ReadOnly => "reads",
                Access::ReadWrite => "reads and writes",
            };
            (
                quote!(&'a ::slango::Buffer<#element>),
                format!(" The buffer bound to the parameter `{path}`, which the kernel {access}."),
                "bind",
                true,
            )
        }
        ParameterKind::Texture => (
            quote!(&'a ::slango::Texture),
            format!(" The texture bound to the parameter `{path}`, which the kernel samples."),
            "texture",
            true,
        ),
        ParameterKind::Sampler => (
            quote!(&'a ::slango::Sampler),
            format!(
                " The sampler bound to the parameter `{path}`, which states how the kernel reads a texture."
            ),
            "sampler",
            true,
        ),
    };
    Ok(Argument {
        name: scope.take(
            snake_case(path),
            format!("the parameter `{path}` of the kernel `{kernel}`"),
        )?,
        ty,
        doc,
        method: format_ident!("{method}"),
        path: path.clone(),
        borrows,
    })
}

fn scalar_type(scalar: Scalar) -> TokenStream {
    match scalar {
        Scalar::Bool => quote!(bool),
        Scalar::Int32 => quote!(i32),
        Scalar::Uint32 => quote!(u32),
        Scalar::Float32 => quote!(f32),
    }
}

fn scalar_name(scalar: Scalar) -> &'static str {
    match scalar {
        Scalar::Bool => "bool",
        Scalar::Int32 => "int",
        Scalar::Uint32 => "uint",
        Scalar::Float32 => "float",
    }
}

fn value_type(value: &ValueType) -> TokenStream {
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

/// Returns the Rust type of the elements of a structured buffer, or the
/// reason why no built-in element type of `slango` matches them.
fn element_type(element: &ElementType) -> std::result::Result<TokenStream, String> {
    match element {
        ElementType::Plain { scalar, components } => {
            let rust = match scalar {
                Scalar::Int32 | Scalar::Uint32 | Scalar::Float32 => Some(scalar_type(*scalar)),
                Scalar::Bool => None,
            };
            match (rust, components) {
                (Some(rust), 1) => Ok(rust),
                (Some(rust), 4) => Ok(quote!([#rust; 4])),
                _ => {
                    let name = scalar_name(*scalar);
                    let name = if *components == 1 {
                        name.to_owned()
                    } else {
                        format!("{name}{components}")
                    };
                    Err(no_element(&name))
                }
            }
        }
        ElementType::OtherPlain { name } => Err(no_element(name)),
        ElementType::Structure(structure) => Err(format!(
            "it is a structured buffer of the structure `{}`, for which this version generates no struct",
            structure.name
        )),
        ElementType::Composite => Err(
            "it is a structured buffer of a matrix, an array, or another composite type, which has no built-in element type in slango"
                .to_owned(),
        ),
    }
}

fn no_element(name: &str) -> String {
    format!("it is a structured buffer of `{name}`, which has no built-in element type in slango")
}

fn unsupported(kernel: &str, path: &str, reason: &str) -> Error {
    Error::new(ErrorKind::Unsupported, GENERATE).with_detail(format!(
        "the parameter `{path}` of the kernel `{kernel}` is not supported, because {reason}"
    ))
}

#[cfg(test)]
mod tests;
