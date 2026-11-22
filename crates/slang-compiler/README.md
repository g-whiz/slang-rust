# slang-compiler

slang-compiler provides safe Rust wrappers over the [Slang](https://shader-slang.org) shader compiler to compile Slang shaders and reflect their parameters without a graphics processing unit (GPU). It builds on the raw bindings of [slang-compiler-sys](https://github.com/g-whiz/slang-rust/blob/main/crates/slang-compiler-sys/README.md), and a program that runs the compiled shaders on a GPU uses [slango](https://github.com/g-whiz/slang-rust/blob/main/crates/slango/README.md) instead.

> [!WARNING]
> This crate is an alpha release, and its API can change in any release. Read the [project status](https://github.com/g-whiz/slang-rust#status) before you depend on it.

## Installation

```toml
[dependencies]
slang-compiler = "0.2.0-alpha.2"
```

## Example

This program compiles a compute shader to SPIR-V and prints the size of the code. It needs Slang when it runs, so its doctest is compiled but not run:

```rust,no_run
use slang_compiler::{Component, GlobalSession, Session, Target, TargetFormat};

const SOURCE: &str = r#"
    RWStructuredBuffer<float> data;
    [shader("compute")]
    [numthreads(64, 1, 1)]
    void scale(uint3 id : SV_DispatchThreadID) { data[id.x] *= 2.0; }
"#;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let global = GlobalSession::new()?;
    let session = Session::builder(&global)
        .target(Target::new(TargetFormat::Spirv).profile("spirv_1_5"))
        .build()?;
    let module = session.load_module_from_source("kernels", "kernels.slang", SOURCE)?.into_value();
    let entry_point = module.entry_point("scale")?;
    let program = session
        .compose(&[Component::Module(&module), Component::EntryPoint(&entry_point)])?
        .into_value()
        .link()?
        .into_value();
    let code = program.entry_point_code(0, session.target_index(0)?)?;
    println!("{} bytes of SPIR-V", code.value.as_bytes().len());
    Ok(())
}
```

## Requirements

Building needs Slang, from an installed Vulkan SDK or built from source with the feature `vendored`, and libclang, as the [slang-compiler-sys documentation](https://g-whiz.github.io/slang-rust/slang_compiler_sys/) describes. The [features of this crate](https://g-whiz.github.io/slang-rust/slang_compiler/#features) say what its feature `vendored` does.

## Documentation

The [API documentation](https://g-whiz.github.io/slang-rust/slang_compiler/) describes every item, including reflection. The example program `compile-and-reflect` in the [examples of the repository](https://github.com/g-whiz/slang-rust#examples) prints the entry points and parameters of a Slang file. The [project README](https://github.com/g-whiz/slang-rust) lists the other crates.

## Licence

This crate is licensed under either of the [Apache License, Version 2.0](https://github.com/g-whiz/slang-rust/blob/main/LICENSE-APACHE) or the [MIT licence](https://github.com/g-whiz/slang-rust/blob/main/LICENSE-MIT), at your option.
