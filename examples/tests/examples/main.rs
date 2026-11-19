//! The integration tests of the examples, which run each example binary.

#[cfg(all(test, feature = "wgpu"))]
mod colour_format;
#[cfg(test)]
mod common;
#[cfg(test)]
mod ffi;
#[cfg(test)]
#[path = "../../src/shared/frames.rs"]
#[cfg_attr(
    not(feature = "wgpu"),
    expect(
        dead_code,
        reason = "only the tests of the windowed examples, which build with wgpu alone, read the usage text of the shared options"
    )
)]
mod frames;
#[cfg(test)]
mod gpu;
#[cfg(test)]
mod redraw_retry;
#[cfg(test)]
mod unattended;
#[cfg(test)]
#[path = "../../src/shared/watched.rs"]
mod watched;
