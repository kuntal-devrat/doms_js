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

pub struct doms_context_t {
    pub isolate: *mut doms_isolate_t,
    pub last_error: Option<String>,
}

pub struct doms_value_t {
    pub inner: JsValue,
    pub ref_count: AtomicUsize,
}

pub struct doms_script_t {
    pub source: String,
}
