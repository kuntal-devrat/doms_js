//! Engine, isolate, and context lifecycle FFI.

use super::*;

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
pub unsafe extern "C" fn doms_get_error_message(ctx: *mut doms_context_t) -> *const c_char {
    if ctx.is_null() {
        return ptr::null();
    }
    unsafe {
        match &(*ctx).last_error {
            Some(msg) => msg.as_ptr(),
            None => ptr::null(),
        }
    }
}
