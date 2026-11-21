# slang-slint

slang-slint shows what a [Slang](https://shader-slang.org) render pipeline of [slango](https://github.com/g-whiz/slang-rust/blob/main/crates/slango/README.md) draws inside a [Slint](https://slint.dev) user interface, by sharing one wgpu device between Slint and slango. A program without a Slint user interface uses slango alone. The crate uses the default backend of slango, wgpu, so a program that uses the feature `rhi` of slango cannot use this crate.

> [!WARNING]
> This crate is an alpha release that is not published on crates.io, and its API can change in any release. Read the [project status](https://github.com/g-whiz/slang-rust#status) before you depend on it.

## Installation

```toml
[dependencies]
slang-slint = { git = "https://github.com/g-whiz/slang-rust", version = "0.1.0-alpha.2" }
```

The program depends on slint with a requirement that selects a 1.18 release, such as `slint = "~1.18"`, because the API of this crate names Slint's types, and a later minor version of Slint gives type errors that do not name the version.

## Example

This program makes Slint create a wgpu device that suits slango, opens a Slint window, and turns Slint's device into a `slango::Device` when Slint sets up rendering, then prints the name of its adapter. It also depends on slint with a windowing backend and a wgpu renderer enabled. It needs a GPU driver or a software driver and a display when it runs, so its doctest is compiled but not run:

```rust,no_run
use slint::{ComponentHandle, RenderingState};

slint::slint! {
    export component App inherits Window {
        width: 256px;
        height: 256px;
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    slint::BackendSelector::new()
        .require_wgpu_30(slang_slint::wgpu_configuration())
        .select()?;
    let app = App::new()?;
    app.window().set_rendering_notifier(|state, api| {
        if let RenderingState::RenderingSetup = state {
            match slang_slint::device(api) {
                Ok(device) => println!("slango runs on {}", device.adapter_name()),
                Err(error) => eprintln!("{error}"),
            }
        }
    })?;
    app.run()?;
    Ok(())
}
```

The example program [shader-playground](https://github.com/g-whiz/slang-rust/tree/main/examples/src/bin/shader-playground/) adds a `View` that draws with a render pipeline of slango in every frame and shows the result in the window.

## Requirements

Building needs Slang, from an installed Vulkan SDK or built from source with the feature `vendored`, and libclang, as the [slang-compiler-sys documentation](https://g-whiz.github.io/slang-rust/slang_compiler_sys/) describes. On Linux, building Slint also needs the development files of fontconfig. The [crate documentation](https://github.com/g-whiz/slang-rust/blob/main/crates/slang-slint/src/lib.rs) says which features of Slint a program enables and what its feature `vendored` does.

## Documentation

The crate is not on the hosted API documentation. Its [crate documentation](https://github.com/g-whiz/slang-rust/blob/main/crates/slang-slint/src/lib.rs) describes every item and how a program drives frames. The [project README](https://github.com/g-whiz/slang-rust) lists the other crates and the [examples](https://github.com/g-whiz/slang-rust#examples).

## Licence

This crate is licensed under version 3 or any later version of the [GNU General Public License](https://github.com/g-whiz/slang-rust/blob/main/LICENSE-GPL), `GPL-3.0-or-later`.

> [!IMPORTANT]
> Slint is available under several licences, and this crate takes it under version 3 of the GNU General Public License only. A program that combines this crate with Slint is therefore distributed under version 3 of the GNU General Public License only. slango and slang-compiler do not depend on Slint, so their licence does not change.
