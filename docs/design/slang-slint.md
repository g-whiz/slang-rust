# `slang-slint` Design

| Field | Value |
| --- | --- |
| Status | Draft |
| Date | 2026-11-12 |
| Applies to | `crates/slang-slint` and the Slint example |

## Abstract

This document specifies milestone M5: the crate `slang-slint`, which shows what a `slango` render pipeline draws inside a Slint user interface, and the Slint example. It starts from the facts that a spike established about Slint 1.18 with wgpu 30: how Slint and `slango` share one device, which textures Slint imports as images, and how frames are driven. On those facts it defines a minimal view, one type that owns a render pipeline and a texture and hands the texture to Slint as an image, and two functions through which Slint creates a device that `slango` can use. It states the version range of Slint, the licence consequence of depending on Slint, the errors, the threading, the tests, the example, and the order of the changes that implement M5. It ends with the assumptions to confirm and the open questions.

## 1. Introduction

### 1.1. Problem

Slint draws its user interface with its own renderer and gives an application no surface of its own. A program that wants a Slang shader to draw part of a Slint window must create the GPU content on the device that Slint renders with, in a texture format that Slint accepts, hand it to Slint as an image, replace it when the element changes size, and request each frame. Slint's default device lacks what `slango` needs, and a mismatch shows only at run time.

### 1.2. Purpose

`slang-slint` lets a Rust program show what a `slango` render pipeline draws in an ordinary Slint `Image` element, with the shader parameters set by name through the draw of `slango`. This document implements Section 9 and milestone M5 (Section 15.8) of `docs/design/architecture.md`, meets ARCH-19 to ARCH-21, and answers the questions of that design that name this document, as SLS-1, SLS-4, and Section 9 record.

### 1.3. Audience

The primary readers are the coding agents that implement M5 and the reviewers who check their changes. A reader is expected to know Rust and the public API of `slango` (Sections 5.3 and 9.5 of `docs/design/slango.md`). Section 2.1 defines the Slint terms that such a reader needs.

### 1.4. Scope

This document covers the view, the sharing of the device, the dependency on Slint with its licence consequences, errors, threading, tests, continuous integration (CI), and example 4 of Section 10 of the architecture design.

### 1.5. Non-Goals

M5 covers the minimal view only. These are not part of M5: a custom Slint element, a hot-reload API, the binding of Slint properties to shader parameters from reflection, a rendering notifier installed by the library, input handling, helpers for compute kernels, multisampling, and a colour format other than `Rgba8Unorm`. The example reloads its shader as example 3 does, with the existing load (Section 10).

### 1.6. Current State

All four steps of Section 11 are done, on 2026-11-11, and M5 is complete. Step 1 added the crate `crates/slang-slint` with `wgpu_configuration` (SLS-6), `device` (SLS-7), and the error type of Section 7 with all four kinds, and their tests (Section 9). Step 2 added `View` (Section 6) and its `gpu` tests, which ran locally only (Section 9). Step 3 added example 4, `shader-playground`, with its tests (Section 10), and the licences, duplicate versions, and advisories that its Slint backend and renderer bring into `deny.toml` (Section 3.6). Step 4 completed the crate-level documentation with an example of a view in a Slint window, the guidance of Section 5.3, the threading of Section 8, and the licence statement of SLS-5, and added the statement of SLS-5 to `README.md`. The crate's API documentation is the detailed record.

The spike of M5 ran on 2026-11-11 as a scratch program that the repository does not keep, and Section 3 records what it and step 1 established. Section 11 gives the order of the changes that implement M5.

### 1.7. Defects of Third-Party Components and Trusted Input

`slang-slint` follows Hard Rule 14 of `AGENTS.md` and Section 1.7 of `docs/design/slango.md`, and applies the same stance to Slint. It does not guard against the validation message of Section 3.2 or the abort at exit of Section 3.3. Shader source and file paths are trusted input.

## 2. Requirements Language

Best Current Practice (BCP) 14 defines the key words in this section.

The key words "MUST", "MUST NOT", "REQUIRED", "SHALL", "SHALL NOT", "SHOULD", "SHOULD NOT", "RECOMMENDED", "NOT RECOMMENDED", "MAY", and "OPTIONAL" in this document are to be interpreted as described in BCP 14 [RFC2119] [RFC8174] when, and only when, they appear in all capitals, as shown here.

### 2.1. Terminology

Section 2.1 of `docs/design/architecture.md` and Section 2.1 of `docs/design/slango.md` define the terms of GPU programming that this document uses, such as adapter, device, render pipeline, texture, colour target, and depth target. This document adds these terms:

- backend selector: Slint's `BackendSelector`, through which an application chooses Slint's windowing backend and renderer and requires a graphics API before it creates a window.
- event loop: The loop in which Slint processes window events and renders frames, which `run()` starts.
- physical pixel: A pixel of the window's surface. A length in a `.slint` file is in logical pixels, and a logical pixel is the scale factor of the window times a physical pixel.
- renderer: The part of Slint that draws the user interface, such as FemtoVG on OpenGL, FemtoVG on wgpu, Skia, or the software renderer.
- rendering notifier: The callback that an application registers with `Window::set_rendering_notifier`, which Slint calls with a `RenderingState` and a `GraphicsAPI` at fixed points of rendering.
- UI thread: The thread that runs Slint's event loop.
- windowing backend: The part of Slint that creates windows and receives their events, such as the backend on winit.

## 3. Facts From the Spike

This section is non-normative. It states each fact that a later section relies on, once, and says how it is known.

### 3.1. Sources and Evidence

Each fact carries one of these marks:

- (run): Verified on 2026-11-11 by a scratch program on x86_64 Linux with Rust 1.99.1, Slint 1.18.1, wgpu 30.0.1, and `slango` of the repository at commit `e8982ca`, in debug and release builds. It ran on lavapipe under a nested Wayland compositor and on an NVIDIA GeForce RTX 4070 Laptop GPU on the development container's own Wayland socket, with the same results on both unless the fact says otherwise. A fact about dependencies means a run of `cargo deny` or `cargo tree` on a copy of the repository with a member `crates/slang-slint` added.
- (step 1): Verified on 2026-11-11 by step 1 of Section 11, which built and tested the crate `slang-slint` in the repository, in the development container on x86_64 Linux and in CI on Linux, Windows, and macOS.
- (step 2): Verified on 2026-11-11 by the `gpu` tests of step 2 of Section 11, in the development container on x86_64 Linux, on the NVIDIA GeForce RTX 4070 Laptop GPU and on lavapipe.
- (step 3): Verified on 2026-11-11 by step 3 of Section 11, which built example 4 and ran it and its tests in the development container on x86_64 Linux, as Section 10 records.
- (read): Read on 2026-11-11 in the source of the named crate of Slint 1.18.1, or in the named source.
- (inferred): Concluded from other facts and not verified. Section 12 lists the assumptions on which a requirement depends.

Nothing of the spike ran on Windows or macOS.

### 3.2. Building

Slint 1.18.1 depends on wgpu 30 with its default features off and re-exports it as `slint::wgpu_30::wgpu` (read, `slint` and `i-slint-core`). With Slint's default features off and the features `std`, `compat-1-2`, `backend-winit`, `renderer-femtovg-wgpu`, and `unstable-wgpu-30`, the build graph holds one wgpu 30.0.1, and a device and a queue from Slint go into `slango::wgpu::from_device_and_queue` with no conversion (run). Slint names the features of its wgpu API `unstable-wgpu-29` and `unstable-wgpu-30` (read). Slint's default renderer is FemtoVG on OpenGL, and with the defaults plus `unstable-wgpu-30`, `run()` fails with `WGPU 30.x rendering is not supported with an OpenGL renderer` (run). The winit backend picks Skia for wgpu when Skia is compiled in, and otherwise FemtoVG on wgpu (read, `i-slint-backend-winit`). Skia was not built.

A library that enables no windowing backend and no renderer needs the features `compat-1-18`, `std`, and `unstable-wgpu-30`, and reaches `slint::wgpu_30`, `GraphicsAPI::WGPU30`, and `BackendSelector::require_wgpu_30` with them (step 1). Every build of `slint` 1.18.1 stops with a `compile_error!` that demands `compat-1-18`, an empty feature, and `i-slint-core` 1.18.1 stops with one that demands `std` or `unsafe-single-threaded` (read, `slint` and `i-slint-core`). `compat-1-2`, which the spike used, adds only features of the linuxkms backend to `compat-1-18` (read). `unsafe-single-threaded` was not tried, because its name says that it gives up thread safety, which a library cannot impose on an application (inferred).

A Linux build links `libfontconfig` through `yeslogic-fontconfig-sys` 6.0.1 of `fontique` 0.11.1 of `i-slint-common`, found through pkg-config, so it needs the development files of fontconfig (run). This holds for the library alone, without a renderer, and the hosted Linux runner of CI lacks them (step 1). The build script of `yeslogic-fontconfig-sys` also runs when `DOCS_RS` is set, so a documentation build without fontconfig fails (step 1). The build environment of docs.rs, the repository `rust-lang/crates-build-env`, lists `libfontconfig1`, `libfontconfig1-dev`, and `pkg-config` in `linux/packages.txt` on its default branch (read, with the GitHub API), so docs.rs is expected to build the documentation of `slang-slint` (inferred, SLSA-4). The UI of the spike used the macro `slint::slint!`, so it needed no build script (run). Slint 1.18.1 declares `rust-version = "1.92"` (read).

In a debug build, the Vulkan validation layer reports `VUID-vkCmdDraw-None-07839` from draws of Slint's FemtoVG renderer on wgpu, also when `slango` draws only once (run). That it comes from Slint and not from `slango` is inferred from that and from `slango`'s stencil state.

### 3.3. Sharing the Device

An application requires wgpu 30 with `BackendSelector::require_wgpu_30`, which takes a `WGPUConfiguration`, and then calls `select()` (read, `i-slint-backend-selector`). That `slint::BackendSelector` is the public path of the selector is inferred. `WGPUConfiguration` is non-exhaustive. Its variant `Manual` takes an instance, an adapter, a device, and a queue, and its variant `Automatic` takes a non-exhaustive `WGPUSettings` with public fields, from which Slint creates the device (read, `i-slint-core`). The defaults of `WGPUSettings` are `Features::empty()` and `Limits::downlevel_webgl2_defaults()`, and Slint raises only the limits of texture size to the adapter's (read).

A device that Slint creates with these defaults has no features, a `max_immediate_size` of 0, and a `max_storage_buffers_per_shader_stage` of 0 (run). On it, a `slango` kernel with a `uniform` parameter of its entry point fails to load with `MissingFeature`, a kernel with a storage buffer fails with `Backend` and the text `Too many bindings of type StorageBuffers in Stage ShaderStages(COMPUTE), limit is 0, count was 1`, and render pipelines with a texture, a sampler, and a uniform buffer draw (run). With `Automatic`, `Features::IMMEDIATES`, and `Limits { max_immediate_size: 128, ..Limits::default() }`, the kernel with the `uniform` parameter ran and returned correct values on lavapipe (run). With `Manual` and a device created as `slango`'s `Device::new` creates one, Slint rendered with that device and the kernel ran (run). In a debug build in `Manual` mode, the process aborted after the event loop returned, with `fatal runtime error: thread local panicked on drop, aborting`, and the release build exited with status 0 (run). This design draws no conclusion from it.

With `Automatic`, Slint's selection refuses a CPU adapter unless `SLINT_WGPU_CPU=1` is set, with an error that names the variable (run). With `Manual`, Slint makes no such check (run).

`Window::set_rendering_notifier` takes a closure with no `Send` bound and fails with `AlreadySet` when the window has one, so a window has one notifier (read, `i-slint-core`). Slint calls it with a `GraphicsAPI`, a non-exhaustive enumeration whose non-exhaustive variant `WGPU30` holds the instance, the device, and the queue by value, and not the adapter (read). It calls `RenderingSetup` once before the first frame, `BeforeRendering` and `AfterRendering` around each frame, and `RenderingTeardown` once before `run()` returns, all on the main thread, which runs the event loop (run).

### 3.4. The Texture Import

`slint::Image` implements `TryFrom<wgpu::Texture>`, which takes the texture by value and returns a non-exhaustive `TextureImportError` with the variants `InvalidFormat` and `InvalidUsage` (read, `i-slint-core`). It accepts a texture of `Rgba8Unorm` or `Rgba8UnormSrgb` with both usages `TEXTURE_BINDING` and `RENDER_ATTACHMENT`, and rejects every other format and usage (read, and run for both formats of `slango` and for `Bgra8Unorm`, `Bgra8UnormSrgb`, and `Rgba16Float`). Every colour texture of `slango` has both usages (Section 8.1 of `docs/design/slango.md`). The import needs no Slint backend, window, or event loop: `Image::try_from` accepted the texture of a view in a plain test process, and `Image::to_wgpu_30_texture` returned the same wgpu texture, which wgpu compares by identity (step 2).

An image made from a clone of the wgpu texture of a `slango::Texture` shares the GPU texture with it. Draws into that texture appeared on the next frame that Slint rendered, without a new image, when a redraw was requested (run). In Slint's own snapshot of the window, an `Rgba8Unorm` texture appeared byte for byte as an image of the same bytes made on the CPU, and an `Rgba8UnormSrgb` texture was decoded when sampled (run). The pixels on the physical screen were not measured.

Nothing in wgpu or in the notifier tells the application the size of an element. The spike read it from `out` properties of the element's width and height, multiplied by the window's scale factor, and rounded, and on a change it created a new texture and set a new image, which drew on (run). The `Image` element filled a `Rectangle` that the layout sized. An `Image` element without a size from its parent takes the size of its image (inferred, not tested), which would let the texture size feed back into the layout.

### 3.5. Frames and Threads

Slint renders only when something requests a frame (run). Two ways of driving frames ran with the same result: drawing in `BeforeRendering` and calling `Window::request_redraw` in `AfterRendering`, and a repeated `slint::Timer` of 16 ms on the UI thread that draws and requests a redraw (run). `slint::Image` is neither `Send` nor `Sync`, and `slango::Device`, `slango::Texture`, and `wgpu::Texture` are `Send` (run, a compile check). Without a D-Bus session, Slint prints one line that starts `Error watching for xdg desktop settings` on standard error (run).

### 3.6. Dependencies

In the spike, with Slint's winit backend and FemtoVG on wgpu enabled and the allow-list and the skips of `deny.toml` unchanged, `cargo deny` rejected the ten crates of Slint 1.18.1, whose licence expression is `GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0`, and eight more crate versions under `BSD-2-Clause`, `BSD-3-Clause`, and `BSL-1.0`. It also reported twelve crates with duplicate versions. The lock file grew from 262 to 542 packages, and the build graph on x86_64 Linux gained 273 crates (run). The spike checked licences and bans only.

The library alone, with the features of Section 3.2, brings seven of the crates of Slint, no crate under a BSD or BSL licence, four duplicate versions, and the advisory RUSTSEC-2026-0441 for the unmaintained `pin-utils` (step 1). The rest arrives with the backend and the renderer of the examples package in step 3 of Section 11. Section 9 of `docs/design/tooling.md` records each change of `deny.toml` with its reason. The lock file grew from 262 to 508 packages, including optional dependencies of Slint that are not built, because Cargo resolves the lock file with every feature of every package. A clean debug build of the workspace compiles 236 crates instead of 128 (step 1).

The windowing backend on winit and the FemtoVG renderer on wgpu that the examples package enables for example 4 bring the rest (step 3). With them, `cargo deny` rejected the eight crate versions under `BSD-2-Clause`, `BSD-3-Clause`, and `BSL-1.0` that the spike found, and reported the eight remaining duplicate versions of the spike. It also reported four advisories, each for a crate that is unmaintained, with no upgrade available and no vulnerability reported:

- RUSTSEC-2026-0370, RUSTSEC-2026-0371, and RUSTSEC-2026-0372, for `zvariant`, `zbus_names`, and `zvariant_derive`. They enter through `zbus` 5.19, which `i-slint-backend-winit` 1.18.1 requires unconditionally on every target that is neither Windows, Apple, Android, nor WebAssembly (read), so no feature set avoids them on Linux.
- RUSTSEC-2026-0192, for `ttf-parser` 0.25. It enters through `owned_ttf_parser` and `ab_glyph` of `sctk-adwaita` 0.10, which draws the window decorations of winit 0.30 on Wayland. The feature `wayland` of `i-slint-backend-winit` turns on winit's feature `wayland-csd-adwaita`, which brings `sctk-adwaita` (read). Cargo unifies features, so the example `windowed-renderer` is now also built with it.

None of these crates enters through `slang-slint`, which enables no backend and no renderer (SLS-3). Section 9 of `docs/design/tooling.md` records each change of `deny.toml` with its reason and the owner's decisions, and Section 15.5 gives the cost.

## 4. Crate Structure and Dependencies

### 4.1. Package and Dependencies

SLS-1. `slang-slint` MUST depend on `slango` and on `slint` with the version requirement `~1.18`, default features off, and the features `compat-1-18`, `std`, and `unstable-wgpu-30`, and on no other crate. Slint marks this API unstable (Section 3.2), so a wider range lets a minor release of Slint break the builds of users (ARCH-20). Slint 1.18 does not compile without the first two features (Section 3.2). This answers Q-25 of the architecture design.

The crate reaches wgpu's types through `slint::wgpu_30::wgpu`, which is the wgpu 30 that `slango` uses (Section 3.2), so it needs no direct dependency on wgpu. `slint` joins `[workspace.dependencies]` with its default features off, and each member adds the features it needs (DEP-2).

SLS-2. An agent that admits a newer minor version of Slint MUST run every test of `slang-slint` and of example 4. It MUST also check ARCH-19 for the wgpu version of `slango`, because the unstable API and the wgpu versions that Slint supports can change in that release.

SLS-3. `slang-slint` SHOULD NOT enable a windowing backend or a renderer of Slint. Cargo features are additive, so an application could not remove a backend or renderer that the library enables. The features of SLS-1 are the smallest set with which the crate compiles, and they enable neither (Section 3.2).

The application chooses the windowing backend and a renderer on wgpu, as example 4 does (Section 10). A renderer on OpenGL fails at `run()` (Section 3.2).

SLS-4. `slang-slint` MUST contain no unsafe code and MUST emit no logs. The device, the queue, and the texture pass between Slint and `slango` as safe wgpu 30 values (Sections 3.2 and 3.4), and every failure is returned as an error (Section 7), so neither is needed. This answers Q-19 and Q-16 of the architecture design for `slang-slint`.

The crate inherits the workspace lints and has no Cargo features.

### 4.2. Licences

This design decides on 2026-11-11 that `slang-slint` takes Slint's crates, whose licence expression Section 3.6 gives, under `GPL-3.0-only`. The source of `slang-slint` stays under `GPL-3.0-or-later`. A program that combines it with Slint under this option is distributed under version 3 of the GNU General Public License only. `slang-compiler` and `slango` do not depend on Slint and are not affected.

SLS-5. The crate-level documentation of `slang-slint` and `README.md` MUST state this consequence, because a user who distributes a program built with the crate must know the licence of the result first.

Step 1 of Section 11 admitted `GPL-3.0-only` under TOOL-24, with the evidence of compatibility that TOOL-24 requires, and added a `skip` entry under TOOL-26 for each duplicate version that the library brings. `BSD-2-Clause`, `BSD-3-Clause`, and `BSL-1.0`, which this design had planned for step 1, are needed only with a backend and a renderer, so step 3 admitted all three, which the examples package needs (Section 3.6). Section 9 of `docs/design/tooling.md` records the changes, the evidence, and the owner's exceptions for five advisories.

## 5. The Device

### 5.1. Configuration for Slint

Slint creates the device, and the application tells Slint what `slango` needs. A device that Slint creates with its defaults has no immediates and no storage buffers, so no `slango` kernel with a buffer loads on it (Section 3.3).

SLS-6. `slang-slint` MUST provide a function `wgpu_configuration` that returns `WGPUConfiguration::Automatic` with `Features::IMMEDIATES`, a `max_immediate_size` of 128 bytes, and `Limits::default()` for every other limit. The application passes it to `BackendSelector::require_wgpu_30` before Slint chooses an adapter, so the device that Slint creates suits `slango` (Section 3.3).

The function cannot ask the adapter for its maximum immediate size, as `slango`'s `Device::new` does (SLO-18), because Slint chooses the adapter after it receives the configuration. Every adapter tested offers at least 128 bytes (Section 3.8 of `docs/design/slango.md`). Every other field of `WGPUSettings` keeps Slint's default. On an adapter without immediates, Slint's device request fails (SLSA-3), and the application then passes a configuration of its own.

An application can also create the wgpu device itself, pass it to Slint with `WGPUConfiguration::Manual`, and wrap it with `slango::wgpu::from_device_and_queue`, which needs nothing from `slang-slint`. This design recommends `wgpu_configuration`, because the application then needs no device code. Section 3.3 records one observation of the other path.

### 5.2. The Device From Slint

SLS-7. `slang-slint` MUST provide a function `device` that takes a `&slint::GraphicsAPI` and returns a `slango::Device` of the device and the queue of its variant `WGPU30`. It MUST return `ErrorKind::UnsupportedGraphicsApi` for every other variant, because only wgpu 30 objects can be shared with `slango`.

The application calls it in `RenderingSetup`. It clones the device and the queue, which wgpu counts by reference (Section 3.2 of `docs/design/slango.md`).

SLS-8. `slang-slint` MUST NOT install a rendering notifier, because a window has one notifier (Section 3.3), so a notifier that the library installed would take it from the application.

### 5.3. Driving Frames

This section is non-normative. It is guidance for applications and defines no API.

Slint renders only on request (Section 3.5). An application draws in `BeforeRendering` and calls `Window::request_redraw` in `AfterRendering`, or draws and requests a redraw from a repeated `slint::Timer` on the UI thread. Before each draw, it reads the element's size in physical pixels from properties of the element, as Section 3.4 describes, and calls `View::resize` (Section 6.2). The `Image` element fills a parent that the layout sizes, so that the texture size does not feed back into the layout. An application that draws only on a change, such as of a slider, requests a redraw only then.

## 6. The View

### 6.1. The Type

`View` is the one public type of the crate. It owns a `slango::Device`, a `slango::RenderPipeline`, and a `slango::Texture`, and holds nothing of Slint.

SLS-9. `View::new` MUST take a `slango::Device`, a `slango::RenderPipeline`, and a size `[u32; 2]` in physical pixels, and create a texture of `TextureFormat::Rgba8Unorm` of that size. Slint imports that format and shows it byte for byte as a CPU image (Section 3.4), so the colours that a shader writes appear unchanged.

`View::new` takes both values by value, because it stores them (OWN-1). A width or a height of zero gives `ErrorKind::InvalidSize` before anything is created. A size over the device's limit reaches `slango::Texture::new`, whose error the view returns as `ErrorKind::Slango` (Section 7). `View::size` returns the size, and `View::texture` returns the texture, such as for a read back.

SLS-10. `View::draw` MUST return the `slango::Draw` of the view's pipeline with the view's texture set as its colour target, and MUST set nothing else. The application then sets values, buffers, textures, samplers, vertex buffers, and a clear colour by name as with `RenderPipeline::draw`, so the view imposes no scene.

`slango`'s draw checks the target against the pipeline, so a pipeline whose colour format is not `Rgba8Unorm`, or that belongs to another device, fails at the draw with `slango::ErrorKind::InvalidArgument` (SLO-26), and `View::new` does not check it. The view offers no depth target. An application with a pipeline that has a depth format passes a `Depth32Float` texture of the view's size to `Draw::depth`, and replaces it whenever `View::resize` returns `true`. This works with no code of the view.

SLS-11. `View::to_image` MUST return a `slint::Image` that shares the view's current texture, from a clone of its wgpu texture, because an image of a copy would not show later draws (Section 3.4).

A `TextureImportError` gives `ErrorKind::TextureImport` with Slint's error as the source. The application sets the image on its `Image` element after `View::new` and after each resize that returns `true`. Every later draw into the same texture shows on the next frame that Slint renders.

### 6.2. Resize

SLS-12. `View::resize` MUST create a new texture and return `true` only when the given size differs from the current size. It MUST keep the current texture when it returns an error, because the image that Slint holds shows that texture until the application sets a new one (Section 3.4).

An equal size returns `false` and creates nothing. A width or a height of zero gives `ErrorKind::InvalidSize`, so an application skips resizing and drawing while its element has no area.

### 6.3. Usage Illustration

This illustration shows the form of the API of M5 and has not been compiled. It unwraps every error for brevity, and a line marked `// ...` is elided. The program passes the configuration to Slint, creates the view in `RenderingSetup`, and resizes and draws it before each frame with one value set by name:

```rust
use std::{cell::RefCell, rc::Rc};

use slang_slint::View;
use slango::RenderPipelineOptions;
use slint::{ComponentHandle, RenderingState};

slint::slint! {
    export component App inherits Window {
        in property <image> view;
        in-out property <float> tint: 1.0;
        out property <length> view-width: area.width;
        out property <length> view-height: area.height;
        area := Rectangle { Image { source: view; width: parent.width; height: parent.height; } }
    }
}

fn physical_size(app: &App) -> [u32; 2] {
    // ... view-width and view-height times the scale factor, rounded
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    slint::BackendSelector::new()
        .require_wgpu_30(slang_slint::wgpu_configuration())
        .select()?;
    let app = App::new()?;
    let weak = app.as_weak();
    let view: Rc<RefCell<Option<View>>> = Rc::default();
    app.window().set_rendering_notifier(move |state, api| {
        let Some(app) = weak.upgrade() else { return };
        match state {
            RenderingState::RenderingSetup => {
                let device = slang_slint::device(api).unwrap();
                let module = device.load_module("gradient.slang").unwrap();
                let options = RenderPipelineOptions::default();
                let pipeline = module.render_pipeline("vsMain", "fsMain", &options).unwrap();
                let new = View::new(device, pipeline, physical_size(&app)).unwrap();
                app.set_view(new.to_image().unwrap());
                *view.borrow_mut() = Some(new);
            }
            RenderingState::BeforeRendering => {
                if let Some(view) = view.borrow_mut().as_mut() {
                    if view.resize(physical_size(&app)).unwrap() {
                        app.set_view(view.to_image().unwrap());
                    }
                    view.draw().set("tint", app.get_tint()).submit(3, 1).unwrap();
                }
            }
            RenderingState::AfterRendering => app.window().request_redraw(),
            _ => {}
        }
    })?;
    app.run()?;
    Ok(())
}
```

## 7. Errors

SLS-13. `slang-slint` MUST have one public error type `Error` and the alias `Result`, as ERR-1 defines, in the form of `slango`'s, with these kinds of `ErrorKind`, because callers match on the kind:

| Kind | Meaning |
| --- | --- |
| `UnsupportedGraphicsApi` | The `GraphicsAPI` that `device` received is not wgpu 30, such as when the application chose a renderer on OpenGL |
| `TextureImport` | Slint rejected the texture of `View::to_image`. The source error is Slint's `TextureImportError` |
| `InvalidSize` | A size given to `View::new` or `View::resize` has a width or a height of zero |
| `Slango` | `slango` failed, such as when it created a texture. The source error is the error of `slango` |

The error of a dependency is returned only through `source()`, never as a public type of a field or a variant (ERR-2). The `Display` text names the failed operation and does not repeat the source's text (ERR-4). An error of a draw comes from `slango` directly, because `View::draw` returns `slango`'s builder.

## 8. Threading

SLS-14. `View`, `Error`, and `ErrorKind` MUST be `Send` and `Sync`, asserted in a test (CONC-1), and the crate MUST implement neither trait by hand, because the view holds only `slango` types, which are both (SLO-17).

The `slint::Image` that `View::to_image` returns is neither `Send` nor `Sync` (Section 3.5), so the application creates and sets it on the UI thread. In M5, the application draws on the UI thread, in its rendering notifier or in a timer (Section 5.3). `slang-slint` starts no thread (CONC-4).

## 9. Tests

`slang-slint` has one integration-test binary, `tests/slang_slint/` (TEST-10). Its module `configuration` checks the fields of the configuration of SLS-6, its module `device` checks the error path of `device`, and its module `view` asserts SLS-14 for `View`. The unit tests in `src/error.rs` check the `Display` text of each error kind, `source`, and the assertions of SLS-14 for `Error` and `ErrorKind`, because only the crate can construct an error of each kind, as in `slango`. These ten tests are pure, and Miri runs them all.

The nine tests of the module `gpu`, with their helpers in the module `common`, create the device with `slango::Device::new`, so they need no Slint window, no event loop, and no `SLINT_WGPU_CPU`. Their shader draws one triangle that covers the texture, with no vertex buffer. They check:

- The size and the format of a new view (SLS-9).
- A draw that fills the view's texture, read back through `slango` (SLS-10).
- A resize that draws into a new texture of the new size, and a resize to the current size that keeps the texture and its pixels (SLS-12).
- `InvalidSize` for a zero size, for `View::new` and `View::resize`, with the texture and its pixels kept on the error (TEST-6).
- `Slango` with `slango`'s `InvalidArgument` as the source for a size over the device's limit, for `View::new` and `View::resize`, with the texture and its pixels kept on the error.
- A pipeline of another colour format that fails at the draw.
- An image of `View::to_image` that has the view's size and shares its texture, before and after a resize (SLS-11).

On 2026-11-11, these tests passed in the development container on the NVIDIA GeForce RTX 4070 Laptop GPU and on lavapipe.

SLS-15. A test of `slang-slint` or of example 4 MUST NOT open a window or need a display, because the Linux CI runner has none, so such a test fails in CI (ARCH-21).

The error path of `device` is tested with `GraphicsAPI::NativeOpenGL` and `GraphicsAPI::WebGL`, which are not non-exhaustive variants, so a test can build them. Its success path has no automated test, because only Slint creates a value of the non-exhaustive variant `GraphicsAPI::WGPU30` (Section 3.3), and a struct expression for it outside Slint fails to compile with E0639 (step 1). Example 4 calls it whenever it runs.

These tests answer Q-24 of the architecture design for `slang-slint`: the view is checked headless. The window of example 4, like that of example 3, is not checked headless.

## 10. The Slint Example

Example 4 of the architecture design is the binary target `shader-playground` of the package `slang-rust-examples`. Step 3 of Section 11 added it. Its source is the directory `examples/src/bin/shader-playground/`, with `main.rs`, which holds the arguments and the rendering notifier, `ui.rs`, which declares the Slint window, and `scene.rs`, which holds the view, the reload, and the draw, and which the examples' test binary also compiles (TEST-1). Its shader is `examples/shaders/playground.slang`, which draws a full-screen triangle with no vertex buffer and colours it from the global uniforms `time`, `brightness`, and `scale`. The vertex entry point numbers the vertices with `SV_VulkanVertexID`, because naga rejects `SV_VertexID` (Section 3.9 of `docs/design/slango.md`), and the parameters are global uniforms, because `slango` rejects `uniform` parameters of a render entry point (SLO-23). The shader of the `gpu` tests of Section 9 draws its triangle in the same way.

The window has an `Image` element that fills a `Rectangle` that the layout sizes, and two sliders, "Brightness" from 0 to 1 and "Scale" from 0 to 4, both at 1 by default. In each draw, the example sets `brightness` and `scale` by name from the sliders, and `time` to the seconds since it started. It selects the backend with `slang_slint::wgpu_configuration`. In `RenderingSetup`, it gets the device with `slang_slint::device`, prints the adapter's name on standard error, and creates the view. It drives frames from the notifier as Section 5.3 describes. It computes the element's size in physical pixels with at least one pixel on each side, so a view always exists and `InvalidSize` does not arise, instead of skipping draws while the element has no area (Section 6.2). It declares its UI with `slint::slint!`, so the examples package needs no build script. The examples package enables the Slint features `std`, `compat-1-18`, `backend-winit`, `renderer-femtovg-wgpu`, and `unstable-wgpu-30`. That is the set of the spike (Section 3.2) with `compat-1-18` in place of `compat-1-2`, which adds only features of the linuxkms backend.

The example reloads its shader as example 3 does (Section 13.5 of `docs/design/slango.md`). Before each frame, it compares the modification time of the file with that of the last load. When the time changed, it loads the module, creates the pipeline, and creates a new view of the current size, whose image it sets, and prints `reloaded the shader` on standard error. When the load fails, it prints the error and its causes on standard error, keeps drawing with the old view, and does not load the file again until its time changes once more.

SLS-16. Example 4 SHOULD take the option `--frames N`, which closes the window after `N` drawn frames and exits. It SHOULD then fail when it draws no frame for 10 seconds, because otherwise a run needs a person or can wait forever.

With `--frames N`, the example prints `drew N frames` on standard output when it closes the window. A repeated `slint::Timer` of one second checks the bound of 10 seconds.

The five `gpu` tests in the module `gpu::shader_playground` of the examples' test binary draw the scene offscreen into a view of 64 by 32 pixels on a device that `slango` creates, with the example's own shader, and read the pixels back through `View::texture`. They check every pixel against the shader's formula for two values of each slider and for a later time. They check that a failed reload keeps the view, its wgpu texture, and its pixels, and is not tried again until the file changes. They check that a successful reload after a resize gives a new wgpu texture of the resized size with the new shader's pixels. The two tests in the module `ffi::shader_playground` start the binary to check `--help` and each argument error, including a frame count of zero, which need no window. On 2026-11-11, all of them passed in the development container on the NVIDIA GeForce RTX 4070 Laptop GPU and on lavapipe (step 3). The example runs locally only (ARCH-21). On lavapipe, a run needs `SLINT_WGPU_CPU=1` (Section 3.3), and a user runs it from the workspace root with the Slang library on the library path and a display:

```sh
SLINT_WGPU_CPU=1 cargo run -p slang-rust-examples --bin shader-playground -- examples/shaders/playground.slang
```

With a window, the example ran on 2026-11-11 on the development machine only (step 3):

- Recorded for 29 seconds in a nested sway compositor with the software adapter llvmpipe in a debug build, the window opened and drew at once, at about 7 updates per second. Both sliders changed the picture. They were clicked, not dragged. A shader edited to tint the picture changed it on reload, and a shader that did not compile printed the error while the old picture kept animating. Restoring the file restored the picture. Each reload paused the picture for about 0.3 seconds.
- On the container's own Wayland socket with the NVIDIA GeForce RTX 4070 Laptop GPU, `--frames 120` drew 120 frames and exited with status 0.

It has not run with a window on Windows, macOS, or X11.

## 11. Implementation Order for M5

These steps implement M5. Each step leaves `just check` passing and adds its tests.

1. Done on 2026-11-11. Add `crates/slang-slint` with its manifest and dependencies (Section 4.1), `slint` in `[workspace.dependencies]`, the changes of `deny.toml` and of the CI workflow that Sections 8.1 and 9 of `docs/design/tooling.md` record, the error type (Section 7), `wgpu_configuration`, and `device` (Section 5). Settle the smallest feature set, which was assumption SLSA-1, and record it in Section 4.1. Tests: the pure tests of Section 9. Check: `check-deny` passes, and all CI jobs pass on the three platforms.
2. Done on 2026-11-11. Add `View` (Section 6). Settle SLSA-2, which held (Section 3.4). Tests: the `gpu` tests of Section 9.
3. Done on 2026-11-11. Add example 4 (Section 10) with its tests. Check: `check-clippy` builds it, and it runs locally with `--frames` on lavapipe and on a hardware adapter.
4. Done on 2026-11-11. Complete the crate-level documentation, including the licence statement of SLS-5 and the guidance of Section 5.3, add the statement of SLS-5 to `README.md`, and update the documents that M5 makes false. With this step, M5 is complete.

## 12. Assumptions to Confirm

Each row is a statement that this design relies on and that the spike did not establish. A section that relies on one cites its identifier. The change in the last column confirms or corrects it, states the result in Section 3 and in the section that relies on it, and removes the row. An identifier is never reused.

| Identifier | Assumption | What depends on it | Where it is confirmed |
| --- | --- | --- | --- |
| SLSA-3 | With the configuration of SLS-6, Slint's device request fails on an adapter without immediates or with a smaller immediate size | Section 5.1 | Deferred until such an adapter is used. Every adapter tested offers 128 bytes or more |
| SLSA-4 | docs.rs builds the documentation of `slang-slint`, because its build environment provides fontconfig and pkg-config | Section 3.2 | The first release, in M7 |

Step 2 of Section 11 confirmed SLSA-2, that Slint imports a wgpu texture with no backend, window, or event loop, and Section 3.4 records the result.

Section 3.3 confirms A-14 of the architecture design in part: external GPU content enters Slint through its rendering notifier and its import of wgpu textures, and Slint names the features of that API unstable. The spike read no evidence about a public Vulkan hook in Slint, and none about whether the API changes in a minor release. SLS-1 does not depend on either.

## 13. Open Questions

No question is open. SLSQ-1 asked whether docs.rs builds the documentation of `slang-slint`, whose dependency `yeslogic-fontconfig-sys` looks for fontconfig through pkg-config. Step 1 of Section 11 answered it with yes, by inference from the build environment of docs.rs (Section 3.2), and SLSA-4 tracks its confirmation.

## 14. Security Considerations

The threat model treats the shader source, the file paths, and the sizes that a program passes as trusted input from the program itself, as Hard Rule 14 of `AGENTS.md` states. `slang-slint` has no unsafe code (SLS-4) and no build script. Its memory safety rests on the safe APIs of Slint, wgpu, and `slango`, and the safety of the GPU's accesses rests on `slango` (Section 17 of `docs/design/slango.md`). Slint adds many crates to the build graph (Section 3.6) and links fontconfig and the libraries that it uses. `Cargo.lock` fixes their versions, and `cargo-deny` checks their advisories, licences, and sources (Section 4.2). By the owner's decisions, it ignores five advisories, each for an unmaintained crate with no reported vulnerability: one for `pin-utils`, which the library brings, and four that only the Slint backend of the unpublished examples package brings (Section 3.6).

These attacks are out of scope, because a program that runs them has chosen to: a malicious shader, and a shader or a file that reaches a defect of Slang, naga, wgpu, Slint, or a driver (Section 1.7).

The residual risk is a defect in Slint, in its native libraries, which read the system's font files, or in wgpu or a driver, that corrupts memory in the application's process.

## 15. Operational Considerations

### 15.1. Cargo Features and Environment Variables

`slang-slint` has no Cargo features and reads no environment variable. Slint reads `SLINT_WGPU_CPU`. When it is unset, Slint's selection with `Automatic` fails on a machine whose only adapter runs on the CPU, such as lavapipe (Section 3.3). The tests of Section 9 do not need it. Building needs `VULKAN_SDK`, as `slango` does, and on Linux the development files of fontconfig (Section 3.2).

### 15.2. Platforms and Minimum Supported Rust Version

The spike ran on x86_64 Linux with Vulkan only (Section 3.1). Step 1 of Section 11 built the crate on Windows and macOS in CI. The `gpu` tests of step 2 ran locally on x86_64 Linux only (Section 9). Example 4 ran with a window on x86_64 Linux on Wayland only (Section 10). The `rust-version` of Slint (Section 3.2) is below the workspace's minimum supported Rust version (MSRV) of 1.99.

### 15.3. Failure Modes

| Failure | Reported as | Recovery |
| --- | --- | --- |
| The application chose no renderer on wgpu | `run()` of Slint fails with the text of Section 3.2, or `device` returns `UnsupportedGraphicsApi` | Enable a renderer on wgpu, such as `renderer-femtovg-wgpu` |
| Only a CPU adapter exists and `SLINT_WGPU_CPU` is unset | Slint's `select()` fails with an error that names the variable | Set `SLINT_WGPU_CPU=1` |
| A kernel fails to load on a device that Slint created with its defaults | `slango`'s `MissingFeature` or `Backend` (Section 3.3) | Pass `wgpu_configuration()` to the backend selector |
| A size has a width or a height of zero | `InvalidSize` | Skip resizing and drawing until the element has an area |
| A texture larger than the device allows | `Slango`, with `slango`'s `InvalidArgument` as the source | Use a smaller element or another adapter |
| The pipeline's colour format is not `Rgba8Unorm` | `slango`'s `InvalidArgument` at the draw | Create the pipeline with the default options |
| Slint rejects the texture | `TextureImport`, with Slint's error as the source | Report it as a defect of `slang-slint` |
| A build on Linux without the development files of fontconfig | The build script of `yeslogic-fontconfig-sys` fails | Install them, such as `libfontconfig-dev` |

### 15.4. Logs

`slang-slint` emits no logs (SLS-4). Slint itself prints the line of Section 3.5 without a D-Bus session, and wgpu's validation layer prints the message of Section 3.2 in a debug build.

### 15.5. Costs

Slint grows the lock file and the build graph (Section 3.6). A clean debug build of the spike, with `slang-compiler` and `slango` and with Slint's winit backend and FemtoVG on wgpu, took 42 s on 16 cores (run). A clean debug build of the whole workspace with all targets took 29 s on 16 cores with the library of step 1, and 22 s before it (step 1). The backend and the renderer that the examples package enables in step 3 of Section 11 add to both. With them, the lock file grew from 508 to 524 packages, and a clean debug build of the workspace with all targets in a fresh target directory took 50.5 s on 16 cores and compiled 354 crates, against 30.0 s and 236 crates without them in the same measurement (step 3).

## 16. References

### 16.1. Normative References

- [RFC2119] Bradner, S., "Key words for use in RFCs to Indicate Requirement Levels", BCP 14, RFC 2119, March 1997, <https://www.rfc-editor.org/info/rfc2119>.
- [RFC8174] Leiba, B., "Ambiguity of Uppercase vs Lowercase in RFC 2119 Key Words", BCP 14, RFC 8174, May 2017, <https://www.rfc-editor.org/info/rfc8174>.

## Appendix A. Requirement Summary

This table lists every labelled requirement of this document with a one-line summary.

| Label | Section | Summary |
| --- | --- | --- |
| SLS-1 | 4.1 | Depend on `slango` and on `slint` `~1.18` with `compat-1-18`, `std`, and `unstable-wgpu-30` and default features off, and on nothing else. |
| SLS-2 | 4.1 | A new minor version of Slint is admitted only after all tests run again and ARCH-19 is checked. |
| SLS-3 | 4.1 | The library enables no windowing backend or renderer of Slint. |
| SLS-4 | 4.1 | No unsafe code and no logs. |
| SLS-5 | 4.2 | The crate documentation and `README.md` state that a program with Slint is under GPL-3.0-only. |
| SLS-6 | 5.1 | `wgpu_configuration` returns `Automatic` with immediates of 128 bytes and wgpu's default limits. |
| SLS-7 | 5.2 | `device` wraps the wgpu 30 device and queue of a `GraphicsAPI`, and rejects every other variant. |
| SLS-8 | 5.2 | The library installs no rendering notifier. |
| SLS-9 | 6.1 | `View::new` takes a device, a pipeline, and a size, and creates an `Rgba8Unorm` texture. |
| SLS-10 | 6.1 | `View::draw` returns `slango`'s draw with the view's texture as its colour target and nothing else set. |
| SLS-11 | 6.1 | `View::to_image` returns a `slint::Image` that shares the current texture. |
| SLS-12 | 6.2 | `View::resize` replaces the texture only on a new size, reports it, and keeps the texture on an error. |
| SLS-13 | 7 | One `Error` type with the four kinds of Section 7. |
| SLS-14 | 8 | `View`, `Error`, and `ErrorKind` are `Send` and `Sync` without a manual implementation, asserted in a test. |
| SLS-15 | 9 | No test opens a window or needs a display. |
| SLS-16 | 10 | Example 4 offers `--frames N` and fails after 10 seconds without a frame. |

## Change Log

- 2026-11-12: Corrected the number of tests in `ffi::shader_playground` in Section 10 from six to two.
- 2026-11-11: Recorded step 4 of Section 11, the completed crate-level documentation and the statement of SLS-5 in `README.md`, as done in Sections 1.6 and 11, and M5 as complete.
- 2026-11-11: Recorded step 3 of Section 11, example 4, as done in Sections 1.6, 3.1, 11, and 15.2, described the example as built with its tests and its runs with a window in Section 10, including its Slint features, recorded the licences, duplicates, and four advisories of its Slint backend in Sections 3.6, 4.2, and 14, and the cost in Section 15.5.
- 2026-11-11: Recorded step 2 of Section 11, the view, as done in Sections 1.6, 6.3, 9, 11, and 15.2, recorded the confirmation of SLSA-2 in Sections 3.4 and 12 and removed its row, and cited the shader constraints of example 4 in Section 10.
- 2026-11-11: Recorded step 1 of Section 11 as done. Corrected the feature set of SLS-1, removed SLSA-1, and recorded what the library alone brings and costs (Sections 3.2, 3.6, 4.2, and 15.5), what Section 9 tests, and the advisory exception (Section 14). Answered SLSQ-1 by inference and added SLSA-4 to confirm it.
- 2026-11-11: First draft, written after the spike of M5 as the first task of M5.
