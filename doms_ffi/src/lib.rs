//! C ABI implementation for DOMS JavaScript engine.

#![allow(non_camel_case_types)]
#![allow(clippy::missing_safety_doc)]

pub mod handles;

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

#[unsafe(no_mangle)]
pub extern "C" fn doms_engine_new() -> *mut doms_engine_t {
    let engine = Engine::new();
    Box::into_raw(Box::new(doms_engine_t { inner: engine }))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn doms_engine_destroy(engine: *mut doms_engine_t) {
    if !engine.is_null() {
        unsafe {
            drop(Box::from_raw(engine));
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn doms_isolate_new(engine: *mut doms_engine_t) -> *mut doms_isolate_t {
    if engine.is_null() {
        return ptr::null_mut();
    }
    let isolate = unsafe { (*engine).inner.create_isolate() };
    Box::into_raw(Box::new(doms_isolate_t { inner: isolate }))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn doms_isolate_destroy(isolate: *mut doms_isolate_t) {
    if !isolate.is_null() {
        unsafe {
            drop(Box::from_raw(isolate));
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn doms_context_new(isolate: *mut doms_isolate_t) -> *mut doms_context_t {
    if isolate.is_null() {
        return ptr::null_mut();
    }
    Box::into_raw(Box::new(doms_context_t { isolate, last_error: None }))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn doms_context_destroy(ctx: *mut doms_context_t) {
    if !ctx.is_null() {
        unsafe {
            drop(Box::from_raw(ctx));
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn doms_eval(
    ctx: *mut doms_context_t,
    source: *const c_char,
    len: usize,
    result: *mut *mut doms_value_t,
) -> doms_status_t {
    if ctx.is_null() || source.is_null() {
        return doms_status_t::ErrorInvalidArg;
    }

    let slice = unsafe { std::slice::from_raw_parts(source as *const u8, len) };
    let src_str = match std::str::from_utf8(slice) {
        Ok(s) => s,
        Err(_) => return doms_status_t::ErrorInvalidArg,
    };

    let mut rust_ctx = unsafe { (*(*ctx).isolate).inner.create_context() };
    match rust_ctx.eval(src_str) {
        Ok(val) => {
            if !result.is_null() {
                unsafe {
                    *result = Box::into_raw(Box::new(doms_value_t {
                        inner: val,
                        ref_count: AtomicUsize::new(1),
                    }));
                }
            }
            doms_status_t::Ok
        }
        Err(err) => {
            unsafe {
                (*ctx).last_error = Some(err);
            }
            doms_status_t::ErrorRuntime
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn doms_compile(
    _ctx: *mut doms_context_t,
    source: *const c_char,
    len: usize,
    script: *mut *mut doms_script_t,
) -> doms_status_t {
    if source.is_null() || script.is_null() {
        return doms_status_t::ErrorInvalidArg;
    }
    let slice = unsafe { std::slice::from_raw_parts(source as *const u8, len) };
    let src_str = match std::str::from_utf8(slice) {
        Ok(s) => s,
        Err(_) => return doms_status_t::ErrorInvalidArg,
    };

    unsafe {
        *script = Box::into_raw(Box::new(doms_script_t { source: src_str.to_string() }));
    }
    doms_status_t::Ok
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn doms_run(
    ctx: *mut doms_context_t,
    script: *mut doms_script_t,
    result: *mut *mut doms_value_t,
) -> doms_status_t {
    if ctx.is_null() || script.is_null() {
        return doms_status_t::ErrorInvalidArg;
    }
    let src = unsafe { (*script).source.as_bytes() };
    unsafe { doms_eval(ctx, src.as_ptr() as *const c_char, src.len(), result) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn doms_script_destroy(script: *mut doms_script_t) {
    if !script.is_null() {
        unsafe {
            drop(Box::from_raw(script));
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn doms_value_to_number(
    _ctx: *mut doms_context_t,
    val: *mut doms_value_t,
    out: *mut f64,
) -> doms_status_t {
    if val.is_null() || out.is_null() {
        return doms_status_t::ErrorInvalidArg;
    }
    let num = unsafe { (*val).inner.as_number() };
    if let Some(n) = num {
        unsafe {
            *out = n;
        }
        doms_status_t::Ok
    } else {
        doms_status_t::ErrorType
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn doms_value_to_string(
    _ctx: *mut doms_context_t,
    val: *mut doms_value_t,
    out: *mut *const c_char,
    out_len: *mut usize,
) -> doms_status_t {
    if val.is_null() || out.is_null() {
        return doms_status_t::ErrorInvalidArg;
    }
    let s = unsafe { format!("{}", (*val).inner) };
    let cstring = match CString::new(s) {
        Ok(cs) => cs,
        Err(_) => return doms_status_t::ErrorRuntime,
    };
    if !out_len.is_null() {
        unsafe {
            *out_len = cstring.as_bytes().len();
        }
    }
    unsafe {
        *out = cstring.into_raw();
    }
    doms_status_t::Ok
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn doms_free_string(_ctx: *mut doms_context_t, str_ptr: *const c_char) {
    if !str_ptr.is_null() {
        unsafe {
            drop(CString::from_raw(str_ptr as *mut c_char));
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn doms_value_is_null(val: *mut doms_value_t) -> c_int {
    if val.is_null() { 0 } else { unsafe { (*val).inner.is_null() as c_int } }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn doms_value_is_undefined(val: *mut doms_value_t) -> c_int {
    if val.is_null() { 0 } else { unsafe { (*val).inner.is_undefined() as c_int } }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn doms_value_is_boolean(val: *mut doms_value_t) -> c_int {
    if val.is_null() { 0 } else { unsafe { (*val).inner.is_boolean() as c_int } }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn doms_value_is_number(val: *mut doms_value_t) -> c_int {
    if val.is_null() { 0 } else { unsafe { (*val).inner.is_number() as c_int } }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn doms_value_is_string(val: *mut doms_value_t) -> c_int {
    if val.is_null() { 0 } else { unsafe { (*val).inner.is_string() as c_int } }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn doms_value_is_object(val: *mut doms_value_t) -> c_int {
    if val.is_null() { 0 } else { unsafe { (*val).inner.is_object() as c_int } }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn doms_value_is_function(_val: *mut doms_value_t) -> c_int {
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn doms_value_new_undefined() -> *mut doms_value_t {
    Box::into_raw(Box::new(doms_value_t {
        inner: JsValue::undefined(),
        ref_count: AtomicUsize::new(1),
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn doms_value_new_null() -> *mut doms_value_t {
    Box::into_raw(Box::new(doms_value_t { inner: JsValue::null(), ref_count: AtomicUsize::new(1) }))
}

#[unsafe(no_mangle)]
pub extern "C" fn doms_value_new_boolean(b: c_int) -> *mut doms_value_t {
    Box::into_raw(Box::new(doms_value_t {
        inner: JsValue::from_boolean(b != 0),
        ref_count: AtomicUsize::new(1),
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn doms_value_new_number(n: f64) -> *mut doms_value_t {
    Box::into_raw(Box::new(doms_value_t {
        inner: JsValue::from_double(n),
        ref_count: AtomicUsize::new(1),
    }))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn doms_value_ref(val: *mut doms_value_t) {
    if !val.is_null() {
        unsafe {
            (*val).ref_count.fetch_add(1, Ordering::SeqCst);
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn doms_value_unref(val: *mut doms_value_t) {
    if !val.is_null() {
        unsafe {
            if (*val).ref_count.fetch_sub(1, Ordering::SeqCst) == 1 {
                drop(Box::from_raw(val));
            }
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn doms_get_error_message(ctx: *mut doms_context_t) -> *const c_char {
    if ctx.is_null() {
        return ptr::null();
    }
    unsafe {
        match &(*ctx).last_error {
            Some(msg) => msg.as_ptr() as *const c_char,
            None => ptr::null(),
        }
    }
}
