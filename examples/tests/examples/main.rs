//! The integration tests of the examples, which run each example binary.

#[cfg(test)]
mod colour_format;
#[cfg(test)]
mod ffi;
#[cfg(test)]
mod gpu;
#[cfg(test)]
#[path = "../../src/shared/watched.rs"]
mod watched;
