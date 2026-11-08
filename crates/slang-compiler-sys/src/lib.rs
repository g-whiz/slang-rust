//! FFI bindings for the Slang shader language compiler

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(non_camel_case_types, non_snake_case, non_upper_case_globals)]

pub mod reflection;

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

use crate::root::*;
