//! C ABI implementation for DOMS JavaScript engine.

#![allow(non_camel_case_types)]
#![allow(clippy::missing_safety_doc)]

pub mod handles;

mod call;
mod engine;
mod eval;
mod message;
mod object;
mod value;

#[cfg(test)]
mod tests;

pub use call::*;
pub use engine::*;
pub use eval::*;
pub use message::*;
pub use object::*;
pub use value::*;

use doms_core::{Engine, JsValue};
use handles::*;
use std::ffi::CString;
use std::os::raw::{c_char, c_int};
use std::ptr;
use std::sync::atomic::{AtomicUsize, Ordering};

pub const DOMS_ABI_VERSION: u32 = 1;

#[repr(C)]
pub enum doms_status_t {
    Ok = 0,
    ErrorCompile = 1,
    ErrorRuntime = 2,
    ErrorOom = 3,
    ErrorType = 4,
    ErrorInvalidArg = 5,
}

#[unsafe(no_mangle)]
pub extern "C" fn doms_version() -> u32 {
    DOMS_ABI_VERSION
}
