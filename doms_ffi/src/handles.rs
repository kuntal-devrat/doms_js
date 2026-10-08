//! Handle types wrapping internal engine instances for C ABI.

#![allow(non_camel_case_types)]

use doms_core::{Engine, Isolate, JsValue};
use std::sync::atomic::AtomicUsize;

pub struct doms_engine_t {
    pub inner: Engine,
}

pub struct doms_isolate_t {
    pub inner: Isolate,
}

use std::ffi::CString;

pub struct doms_context_t {
    pub isolate: *mut doms_isolate_t,
    pub last_error: Option<CString>,
}

pub struct doms_value_t {
    pub inner: JsValue,
    pub ref_count: AtomicUsize,
}

pub struct doms_script_t {
    pub source: String,
}

unsafe impl Send for doms_engine_t {}
unsafe impl Sync for doms_engine_t {}

unsafe impl Send for doms_isolate_t {}
unsafe impl Sync for doms_isolate_t {}

unsafe impl Send for doms_context_t {}
unsafe impl Sync for doms_context_t {}

unsafe impl Send for doms_value_t {}
unsafe impl Sync for doms_value_t {}

