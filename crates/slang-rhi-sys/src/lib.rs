//! slang-rhi, the render hardware interface of Slang, built from source and
//! linked statically.
//!
//! slang-rhi is the C++ library of the Slang source tree that gives one API
//! over several graphics and compute APIs. This crate builds it with its
//! Vulkan backend from the Slang source tree that `slang-src` clones, and
//! links it into the program, together with the Slang that
//! `slang-compiler-sys` builds from the same tree with its feature
//! `vendored`. Slang and slang-rhi therefore always come from one Slang
//! commit, and the crate never uses the Slang of a Vulkan SDK.
//!
//! The crate has no Rust API yet, so a program can call nothing of slang-rhi
//! through it.
//!
//! # What the build does
//!
//! The build script runs `cmake` with the Ninja generator on a wrapper
//! project of the crate, which adds slang-rhi from the source tree and builds
//! three static libraries outside the tree:
//!
//! - `slang-rhi`, the library with its Vulkan backend.
//! - `slang-rhi-vma`, the Vulkan Memory Allocator.
//! - `slang-rhi-resources`, the files that slang-rhi embeds.
//!
//! It links them in that order, followed by the C++ standard library
//! `stdc++` and the system libraries `dl` and `pthread`. slang-rhi loads the
//! Vulkan loader `libvulkan.so.1` when it creates a device, so the build
//! links no graphics library.
//!
//! The build reaches no network. It turns off every download that the build
//! files of slang-rhi make by default, such as that of a prebuilt Slang, and
//! takes the Vulkan headers from the directory `external/vulkan` of the
//! source tree. slang-rhi treats every compiler warning of its own sources as
//! an error, and the wrapper project turns that off, so that a compiler that
//! slang-rhi was never tested with does not stop the build. The warnings stay
//! in the log of the build.
//!
//! A build needs everything that a vendored build of `slang-compiler-sys`
//! needs, which the
//! [documentation of slang-src](https://g-whiz.github.io/slang-rust/slang_src/#what-a-vendored-build-needs)
//! lists, and the development headers of Xlib, the C library of the X Window
//! System, such as those of the Debian package `libx11-dev`. The Vulkan
//! backend of slang-rhi includes `X11/Xlib.h` on Linux. The first build in a
//! Cargo target directory clones Slang and builds Slang and slang-rhi from
//! source, which takes minutes.
//!
//! # The shared folder
//!
//! The build keeps the three libraries and the header `slang-rhi-config.h`
//! that the configure step writes in the directory `slang-rhi-sys` of the
//! shared folder, which is the directory `slang-src` in the Cargo target
//! directory that the
//! [documentation of slang-src](https://g-whiz.github.io/slang-rust/slang_src/#the-shared-folder)
//! describes. A later build with the same Slang commit, target, compiler,
//! version of `cmake`, and compiler settings uses them without building
//! again. Removing the folder, or any directory in it, is safe whenever no
//! build runs.
//!
//! # Failures
//!
//! When a program of the build cannot start, the build stops with an error
//! that starts with "cannot start" and says that the build needs `cmake`
//! 3.22 or later, Ninja, a C++20 compiler, and the development headers of
//! Xlib. When the configure or build step fails, the error names the command
//! and the log `build.log` in the output directory of the build script, and
//! ends with the last 40 lines of that log.
//!
//! Without the development headers of Xlib, the configure step fails before
//! it adds slang-rhi, and the last lines of the log hold the message "The C++
//! compiler cannot find the header X11/Xlib.h", which names the Debian package
//! `libx11-dev` as an example.
//!
//! # Third-party code in a program
//!
//! A program built with this crate contains, besides slang-rhi and Slang,
//! the Vulkan Memory Allocator, under the MIT licence, and the runtime of
//! the `cmake` module `CMakeRC`, which embeds files in slang-rhi. A program
//! that ships a binary carries their notices.
//!
//! # Features
//!
//! This crate has no Cargo features.
//!
//! # Platform support
//!
//! The crate builds only for Linux with the GNU C library, on a host of the
//! target. On every other target, the crate has no dependencies, and its
//! build stops at once with an error that names the target, before anything
//! is cloned or built. Its builds for Windows and macOS are not designed yet.
//!
//! A cross-compilation from one Linux target with the GNU C library to
//! another keeps the dependencies, so the build of `slang-src` clones Slang
//! first, and the build of `slang-compiler-sys` then stops with its own error
//! that it cannot cross-compile. When it gets that far, the build of this
//! crate stops with an error that names both triples. The crate is tested on `x86_64` Linux, and
//! another architecture of Linux with the GNU C library is untested.
//!
//! A program that creates a device needs the Vulkan loader `libvulkan.so.1`
//! and a Vulkan driver at run time, such as lavapipe, the software driver of
//! Mesa.

#[cfg(all(target_os = "linux", target_env = "gnu"))]
use slang_compiler_sys as _;
#[cfg(all(target_os = "linux", target_env = "gnu"))]
use slang_src as _;
