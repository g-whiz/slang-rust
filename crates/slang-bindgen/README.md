# slang-bindgen

slang-bindgen generates the raw Rust bindings of slang-compiler-sys from the headers of the [Slang](https://shader-slang.org) shader compiler with libclang. The build script of [slang-compiler-sys](https://github.com/g-whiz/slang-rust/blob/main/crates/slang-compiler-sys/README.md) runs it in every build, so a program that uses Slang depends on slang-compiler-sys or the safe crate [slang-compiler](https://github.com/g-whiz/slang-rust/blob/main/crates/slang-compiler/README.md) instead.

> [!WARNING]
> This crate is an alpha release, and its API can change in any release. Read the [project status](https://github.com/g-whiz/slang-rust#status) before you depend on it.

## Installation

```toml
[build-dependencies]
slang-bindgen = "0.1.0-alpha.2"
```

## Example

This build script generates the bindings from the Slang headers of the Vulkan SDK that `VULKAN_SDK` names, for the target of the build, and writes both texts into `OUT_DIR`. It needs libclang, the headers, and the environment that Cargo gives a build script, so it is compiled but not run:

```rust,no_run
use std::env;
use std::fs;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let headers = PathBuf::from(env::var("VULKAN_SDK")?).join("include/slang");
    let target = env::var("TARGET")?;
    let generated = slang_bindgen::generate(&headers, &target)?;

    let out = PathBuf::from(env::var("OUT_DIR")?);
    fs::write(out.join("bindings.rs"), generated.bindings)?;
    fs::write(out.join("symbols.rs"), generated.symbols)?;
    Ok(())
}
```

## Requirements

Running the generator needs libclang, which it loads when it runs, as the [slang-compiler-sys documentation](https://g-whiz.github.io/slang-rust/slang_compiler_sys/#libclang) describes.

## Documentation

The [API documentation](https://g-whiz.github.io/slang-rust/slang_bindgen/) describes `generate`, its errors, and the binary `slang-bindgen`. The [project README](https://github.com/g-whiz/slang-rust) lists the other crates.

## Licence

This crate is licensed under either of the [Apache License, Version 2.0](https://github.com/g-whiz/slang-rust/blob/main/LICENSE-APACHE) or the [MIT licence](https://github.com/g-whiz/slang-rust/blob/main/LICENSE-MIT), at your option.
