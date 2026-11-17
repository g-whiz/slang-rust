# slango-codegen

slango-codegen is the generator of [slango-macros](https://github.com/g-whiz/slang-rust/blob/main/crates/slango-macros/README.md), which turns the compute kernels of a [Slang](https://shader-slang.org) file into typed Rust bindings for [slango](https://github.com/g-whiz/slang-rust/blob/main/crates/slango/README.md). A program uses the macro `shader!` of slango-macros instead, and calls this crate directly only to work with the generated code outside a macro call, such as to print it.

> [!WARNING]
> This crate is an alpha release, and its API can change in any release. Read the [project status](https://github.com/g-whiz/slang-rust#status) before you depend on it.

## Installation

```toml
[dependencies]
slango-codegen = "0.1.0-alpha.2"
```

## Example

This program generates the bindings of [`examples/shaders/saxpy.slang`](https://github.com/g-whiz/slang-rust/blob/main/examples/shaders/saxpy.slang) and prints them. It resolves the relative path against the working directory, so it runs from the root of a clone of the repository. It needs Slang when it runs, so its doctest is compiled but not run:

```rust,no_run
use slango_codegen::generate;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let tokens = generate("examples/shaders/saxpy.slang")?;
    println!("{tokens}");
    Ok(())
}
```

## Requirements

Building needs Slang, from an installed Vulkan SDK or built from source with the feature `vendored`, and libclang, as the [slang-compiler-sys documentation](https://g-whiz.github.io/slang-rust/slang_compiler_sys/) describes. The [features of this crate](https://g-whiz.github.io/slang-rust/slango_codegen/#features) say what its feature `vendored` does.

The generated code calls hidden items of slango, so this crate requires slango of exactly its own version, and a crate that contains the generated code depends on the slango of that version, as the [API documentation](https://g-whiz.github.io/slang-rust/slango_codegen/) explains.

## Documentation

The [API documentation](https://g-whiz.github.io/slang-rust/slango_codegen/) describes every item, and the [slango-macros documentation](https://g-whiz.github.io/slang-rust/slango_macros/) describes the generated items. The [project README](https://github.com/g-whiz/slang-rust) lists the other crates.

## Licence

This crate is licensed under either of the [Apache License, Version 2.0](https://github.com/g-whiz/slang-rust/blob/main/LICENSE-APACHE) or the [MIT licence](https://github.com/g-whiz/slang-rust/blob/main/LICENSE-MIT), at your option.
