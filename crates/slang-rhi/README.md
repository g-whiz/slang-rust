# slang-rhi

The crate slang-rhi provides safe Rust wrappers over the C++ library [slang-rhi](https://github.com/shader-slang/slang-rhi), the render hardware interface of the [Slang](https://shader-slang.org) shader compiler, to run compute kernels and draw with render pipelines on the graphics processing unit (GPU) with Vulkan on Linux and Direct3D 12 on Windows.

It builds on the raw bindings of [slang-rhi-sys](https://github.com/g-whiz/slang-rust/blob/main/crates/slang-rhi-sys/README.md) and compiles through [slang-compiler](https://github.com/g-whiz/slang-rust/blob/main/crates/slang-compiler/README.md), and most programs use [slango](https://github.com/g-whiz/slang-rust/blob/main/crates/slango/README.md) instead, which runs kernels and render pipelines through wgpu by default and through this crate with its feature `rhi`, behind one API that sets parameters by name.

> [!WARNING]
> This crate is an alpha release that is not published on crates.io, and its API can change in any release. Read the [project status](https://github.com/g-whiz/slang-rust#status) before you depend on it.

## Installation

A program also depends on slang-compiler from the same repository, because a device takes a global session of slang-compiler:

```toml
[dependencies]
slang-rhi = { git = "https://github.com/g-whiz/slang-rust", version = "0.1.0-alpha.2" }
slang-compiler = { git = "https://github.com/g-whiz/slang-rust", version = "0.1.0-alpha.2" }
```

## Example

This program opens a device with the graphics API of the platform and prints the API and the name of its adapter. It needs a Vulkan driver on Linux or a Direct3D 12 adapter on Windows when it runs, so its doctest is compiled but not run:

```rust,no_run
use slang_compiler::GlobalSession;
use slang_rhi::{Device, DeviceOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let options = DeviceOptions::new(GlobalSession::new()?);
    let device = Device::new(&options)?;
    let info = device.info();
    println!("{} on {}", info.api_name(), info.adapter_name());
    Ok(())
}
```

The [crate documentation](https://github.com/g-whiz/slang-rust/blob/main/crates/slang-rhi/src/lib.rs) shows a whole compute dispatch: it compiles a kernel, binds a buffer and a value by name, runs the kernel, and reads the result back.

## Requirements

The crate always builds Slang and slang-rhi from source and never uses the Slang of a Vulkan SDK, so building it needs the tools that the [slang-src documentation](https://g-whiz.github.io/slang-rust/slang_src/#what-a-vendored-build-needs) lists, and on Linux also the development headers of Xlib, as the [slang-rhi-sys documentation](https://github.com/g-whiz/slang-rust/blob/main/crates/slang-rhi-sys/src/lib.rs) describes.

The crate builds on Linux with the GNU C library and on x86_64 Windows with the Microsoft toolchain, and not on macOS. A program needs a Vulkan driver on Linux, and `dxcompiler.dll` on Windows when it creates a pipeline, as the [crate documentation](https://github.com/g-whiz/slang-rust/blob/main/crates/slang-rhi/src/lib.rs) describes together with what the crate does not cover.

## Documentation

The crate is not on the hosted API documentation. Its [crate documentation](https://github.com/g-whiz/slang-rust/blob/main/crates/slang-rhi/src/lib.rs) describes the compute path, the textures, render pipelines, and render passes of a draw, ownership and threads, debug messages, and the limits of the crate, and each item is documented in the [source of the crate](https://github.com/g-whiz/slang-rust/tree/main/crates/slang-rhi/src). The [project README](https://github.com/g-whiz/slang-rust) lists the other crates.

## Licence

This crate is licensed under either of the [Apache License, Version 2.0](https://github.com/g-whiz/slang-rust/blob/main/LICENSE-APACHE) or the [MIT licence](https://github.com/g-whiz/slang-rust/blob/main/LICENSE-MIT), at your option.
