# slango

slango runs [Slang](https://shader-slang.org) compute kernels and draws with Slang render pipelines on the graphics processing unit (GPU) from Rust, through [wgpu](https://wgpu.rs) by default or through [slang-rhi](https://github.com/shader-slang/slang-rhi), the render hardware interface of Slang, with its feature `rhi`, with the same API on both. It builds on [slang-compiler](https://github.com/g-whiz/slang-rust/blob/main/crates/slang-compiler/README.md), which a program uses instead to compile or reflect shaders without a GPU, and [slango-macros](https://github.com/g-whiz/slang-rust/blob/main/crates/slango-macros/README.md) generates typed bindings to its kernels while a crate builds.

> [!WARNING]
> This crate is an alpha release, and its API can change in any release. Read the [project status](https://github.com/g-whiz/slang-rust#status) before you depend on it.

## Installation

```toml
[dependencies]
slango = "0.2.0-alpha.2"
```

## Example

This program loads a Slang module from source, runs its kernel `scale` over a buffer of four numbers, and prints the result. It needs a GPU driver or a software driver when it runs, so its doctest is compiled but not run:

```rust,no_run
use slango::{Buffer, Device, DeviceOptions};

const SOURCE: &str = r#"
    RWStructuredBuffer<float> data;
    uniform float factor;
    uniform uint count;

    [shader("compute")]
    [numthreads(64, 1, 1)]
    void scale(uint3 id : SV_DispatchThreadID)
    {
        if (id.x < count) { data[id.x] *= factor; }
    }
"#;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let device = Device::new(&DeviceOptions::default())?;
    let module = device.load_module_from_source("scale", SOURCE)?;
    let data = Buffer::from_slice(&device, &[1.0_f32, 2.0, 3.0, 4.0])?;
    module
        .kernel("scale")?
        .call()
        .set("factor", 2.0_f32)
        .set("count", 4_u32)
        .bind("data", &data)
        .dispatch([4, 1, 1])?;
    let result: Vec<f32> = data.read()?;
    println!("{result:?}");
    Ok(())
}
```

The program prints this line:

```text
[2.0, 4.0, 6.0, 8.0]
```

## Requirements

Building needs Slang, from an installed Vulkan SDK or built from source with the feature `vendored`, and libclang, as the [slang-compiler-sys documentation](https://g-whiz.github.io/slang-rust/slang_compiler_sys/) describes. The [features of this crate](https://g-whiz.github.io/slang-rust/slango/#features) say what its features `vendored` and `rhi` do.

The feature `rhi` builds Slang and slang-rhi from source, on fewer platforms than the default backend, as [choosing a backend](https://g-whiz.github.io/slang-rust/slango/#choosing-a-backend) in the API documentation describes.

Running a kernel or a draw needs a GPU adapter, which a GPU or a software driver provides, as the [platform support](https://g-whiz.github.io/slang-rust/slango/#platform-support) of the API documentation describes.

## Documentation

The [API documentation](https://g-whiz.github.io/slang-rust/slango/) describes every item, including the parameters that a kernel or a render pipeline can take, the differences between the two backends, and the changes from the published release `0.1.0-alpha.1`. The [examples of the repository](https://github.com/g-whiz/slang-rust#examples) include a compute kernel and a windowed renderer. The [project README](https://github.com/g-whiz/slang-rust) lists the other crates.

## Licence

This crate is licensed under either of the [Apache License, Version 2.0](https://github.com/g-whiz/slang-rust/blob/main/LICENSE-APACHE) or the [MIT licence](https://github.com/g-whiz/slang-rust/blob/main/LICENSE-MIT), at your option.
