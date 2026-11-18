//! The integration tests of `slang-rhi`, which use its public API on an
//! adapter of the graphics API of the platform, Vulkan on Linux and
//! Direct3D 12 on Windows.

#[cfg(test)]
mod common;
mod gpu;
