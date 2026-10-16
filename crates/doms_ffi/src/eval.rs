//! Source evaluation and script compilation FFI.

use super::*;

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
                (*ctx).last_error = std::ffi::CString::new(err.to_string()).ok();
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
