# slango-macros

slango-macros provides the procedural macro `shader!`, which compiles a [Slang](https://shader-slang.org) file while a crate builds and generates typed Rust bindings to its compute kernels for [slango](https://github.com/g-whiz/slang-rust/blob/main/crates/slango/README.md), on whichever backend of slango the program uses. Use it when the Slang file is part of the crate, and the runtime API of slango when a program chooses its shaders while it runs.

> [!WARNING]
> This crate is an alpha release, and its API can change in any release. Read the [project status](https://github.com/g-whiz/slang-rust#status) before you depend on it.

## Installation

```toml
[dependencies]
slango-macros = "0.2.0-alpha.2"
```

## Example

This program calls the macro on the file `saxpy.slang` beside it, a copy of [`examples/shaders/saxpy.slang`](https://github.com/g-whiz/slang-rust/blob/main/examples/shaders/saxpy.slang), and runs its kernel `saxpy`, which computes `result = a * x + y`. The macro resolves the path against the directory of the Rust file that calls it, which a doctest does not have, so the example is a `text` block. The example program `typed-kernel` calls the macro in the same way:

```text
use slango::{Buffer, Device, DeviceOptions};

mod saxpy {
    slango_macros::shader!("saxpy.slang");
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let device = Device::new(&DeviceOptions::default())?;
    let shader = saxpy::Shader::load(&device)?;
    let x = Buffer::from_slice(&device, &[1.0_f32, 2.0, 3.0, 4.0])?;
    let y = Buffer::from_slice(&device, &[1.0_f32; 4])?;
    let result = Buffer::<f32>::zeroed(&device, 4)?;
    let args = saxpy::SaxpyArgs { a: 2.0, count: 4, x: &x, y: &y, result: &result };
    shader.saxpy.dispatch(&args, [4, 1, 1])?;
    println!("{:?}", result.read()?);
    Ok(())
}
```

The program prints this line:

```text
[3.0, 5.0, 7.0, 9.0]
```

## Requirements

Building a crate that calls the macro needs Slang, from an installed Vulkan SDK or built from source with the feature `vendored`, and libclang, as the [slang-compiler-sys documentation](https://g-whiz.github.io/slang-rust/slang_compiler_sys/) describes. A build from source enables `vendored` on both slango-macros and slango, as the [features of this crate](https://g-whiz.github.io/slang-rust/slango_macros/#features) explain. A program that uses the feature `rhi` of slango enables `vendored` on slango-macros to build without a Vulkan SDK, as [finding the Slang library](https://g-whiz.github.io/slang-rust/slango_macros/#finding-the-slang-library) explains.

The generated code calls slango, so a crate that calls the macro also depends on the slango of the same version as this crate, as the [versions](https://g-whiz.github.io/slang-rust/slango_macros/#versions) of the API documentation explain.

## Documentation

The [API documentation](https://g-whiz.github.io/slang-rust/slango_macros/) describes the macro, the items it generates, and its compile errors. The [project README](https://github.com/g-whiz/slang-rust) lists the other crates and the [examples](https://github.com/g-whiz/slang-rust#examples).

## Licence

This crate is licensed under either of the [Apache License, Version 2.0](https://github.com/g-whiz/slang-rust/blob/main/LICENSE-APACHE) or the [MIT licence](https://github.com/g-whiz/slang-rust/blob/main/LICENSE-MIT), at your option.
