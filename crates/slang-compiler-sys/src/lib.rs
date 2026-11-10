//! Placeholder for the raw bindings to the Slang shader compiler.
//!
//! This crate has no API yet and contains no bindings. It will hold the raw
//! bindings to the native [Slang](https://shader-slang.org) library, which
//! compiles shaders, the programs that run on a graphics processing unit
//! (GPU), together with a build script that links that library. The bindings
//! will be generated from the Slang headers and will mirror the C API of
//! Slang without making it safe. Most programs will not use this crate
//! directly. The planned crate `slang-compiler` will wrap these bindings in a
//! safe API, and the planned crate `slango` will build on it to run Slang code
//! on a GPU.
//!
//! # Features
//!
//! This crate has no Cargo features.
