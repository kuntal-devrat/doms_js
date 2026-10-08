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
pub unsafe extern "C" fn doms_value_is_function(val: *mut doms_value_t) -> c_int {
    if val.is_null() {
        return 0;
    }
    let inner = unsafe { (*val).inner };
    if let Some(ptr) = inner.as_object_ptr() {
        let obj = unsafe { &*(ptr as *mut doms_vm::object::JsObject) };
        match &obj.kind {
            doms_vm::object::ObjectKind::Function(_) | doms_vm::object::ObjectKind::NativeFunction(_) => 1,
            _ => 0,
        }
    } else {
        0
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn doms_object_get(
    ctx: *mut doms_context_t,
    obj: *mut doms_value_t,
    key: *const c_char,
    key_len: usize,
    result: *mut *mut doms_value_t,
) -> doms_status_t {
    if ctx.is_null() || obj.is_null() || key.is_null() || result.is_null() {
        return doms_status_t::ErrorInvalidArg;
    }
    let slice = unsafe { std::slice::from_raw_parts(key as *const u8, key_len) };
    let key_str = match std::str::from_utf8(slice) {
        Ok(s) => s,
        Err(_) => return doms_status_t::ErrorInvalidArg,
    };
    let isolate = unsafe { &mut (*(*ctx).isolate).inner };
    let target = unsafe { (*obj).inner };
    let val = isolate.vm.get_property(target, key_str);
    unsafe {
        *result = Box::into_raw(Box::new(doms_value_t {
            inner: val,
            ref_count: AtomicUsize::new(1),
        }));
    }
    doms_status_t::Ok
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn doms_object_set(
    ctx: *mut doms_context_t,
    obj: *mut doms_value_t,
    key: *const c_char,
    key_len: usize,
    value: *mut doms_value_t,
) -> doms_status_t {
    if ctx.is_null() || obj.is_null() || key.is_null() || value.is_null() {
        return doms_status_t::ErrorInvalidArg;
    }
    let slice = unsafe { std::slice::from_raw_parts(key as *const u8, key_len) };
    let key_str = match std::str::from_utf8(slice) {
        Ok(s) => s,
        Err(_) => return doms_status_t::ErrorInvalidArg,
    };
    let isolate = unsafe { &mut (*(*ctx).isolate).inner };
    let target = unsafe { (*obj).inner };
    let val = unsafe { (*value).inner };
    isolate.vm.set_property(target, key_str, val);
    doms_status_t::Ok
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn doms_call(
    ctx: *mut doms_context_t,
    func: *mut doms_value_t,
    this_val: *mut doms_value_t,
    argc: usize,
    argv: *mut *mut doms_value_t,
    result: *mut *mut doms_value_t,
) -> doms_status_t {
    if ctx.is_null() || func.is_null() {
        return doms_status_t::ErrorInvalidArg;
    }
    let isolate = unsafe { &mut (*(*ctx).isolate).inner };
    let callee = unsafe { (*func).inner };
    let this = if this_val.is_null() {
        JsValue::undefined()
    } else {
        unsafe { (*this_val).inner }
    };

    let mut args = Vec::with_capacity(argc);
    if argc > 0 && !argv.is_null() {
        for i in 0..argc {
            let arg_ptr = unsafe { *argv.add(i) };
            if arg_ptr.is_null() {
                args.push(JsValue::undefined());
            } else {
                args.push(unsafe { (*arg_ptr).inner });
            }
        }
    }

    match isolate.vm.call(callee, this, &args) {
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
        Err(e) => {
            unsafe {
                (*ctx).last_error = std::ffi::CString::new(e.to_string()).ok();
            }
            doms_status_t::ErrorRuntime
        }
    }
}

pub type doms_native_fn = unsafe extern "C" fn(
    ctx: *mut doms_context_t,
    this_val: *mut doms_value_t,
    argc: usize,
    argv: *mut *mut doms_value_t,
    result: *mut *mut doms_value_t,
    userdata: *mut std::ffi::c_void,
) -> doms_status_t;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn doms_set_native_function(
    ctx: *mut doms_context_t,
    obj: *mut doms_value_t,
    name: *const c_char,
    name_len: usize,
    fn_ptr: doms_native_fn,
    userdata: *mut std::ffi::c_void,
) -> doms_status_t {
    if ctx.is_null() || obj.is_null() || name.is_null() {
        return doms_status_t::ErrorInvalidArg;
    }
    let slice = unsafe { std::slice::from_raw_parts(name as *const u8, name_len) };
    let fn_name = match std::str::from_utf8(slice) {
        Ok(s) => s,
        Err(_) => return doms_status_t::ErrorInvalidArg,
    };
    let isolate = unsafe { &mut (*(*ctx).isolate).inner };
    let target = unsafe { (*obj).inner };

    let ctx_raw = ctx as usize;
    let ud_raw = userdata as usize;
    let closure: doms_vm::object::ClosureNativeFn = std::sync::Arc::new(move |_vm, this_val, args| {
        let ctx_ptr = ctx_raw as *mut doms_context_t;
        let mut this_handle = doms_value_t {
            inner: this_val,
            ref_count: AtomicUsize::new(1),
        };
        let mut arg_handles: Vec<*mut doms_value_t> = args
            .iter()
            .map(|a| {
                Box::into_raw(Box::new(doms_value_t {
                    inner: *a,
                    ref_count: AtomicUsize::new(1),
                }))
            })
            .collect();

        let mut res_handle: *mut doms_value_t = ptr::null_mut();
        let status = unsafe {
            (fn_ptr)(
                ctx_ptr,
                &mut this_handle as *mut doms_value_t,
                arg_handles.len(),
                arg_handles.as_mut_ptr(),
                &mut res_handle,
                ud_raw as *mut std::ffi::c_void,
            )
        };

        for h in arg_handles {
            unsafe {
                drop(Box::from_raw(h));
            }
        }

        match status {
            doms_status_t::Ok => {
                let res_val = if res_handle.is_null() {
                    JsValue::undefined()
                } else {
                    let v = unsafe { (*res_handle).inner };
                    unsafe {
                        drop(Box::from_raw(res_handle));
                    }
                    v
                };
                Ok(res_val)
            }
            _ => Err(doms_vm::interpreter::VmError::RuntimeError("Native function error".into())),
        }
    });

    let func_val = isolate.vm.alloc_closure_native_function(fn_name, 0, closure);
    isolate.vm.set_property(target, fn_name, func_val);
    doms_status_t::Ok
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
            Some(msg) => msg.as_ptr(),
            None => ptr::null(),
        }
    }
}

pub type doms_message_handler = unsafe extern "C" fn(
    isolate: *mut doms_isolate_t,
    data: *const u8,
    len: usize,
    userdata: *mut std::ffi::c_void,
);

#[unsafe(no_mangle)]
pub unsafe extern "C" fn doms_isolate_post_message(
    target: *mut doms_isolate_t,
    data: *const u8,
    _len: usize,
) -> doms_status_t {
    if target.is_null() || data.is_null() {
        return doms_status_t::ErrorInvalidArg;
    }
    doms_status_t::Ok
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn doms_isolate_set_message_handler(
    isolate: *mut doms_isolate_t,
    _handler: doms_message_handler,
    _userdata: *mut std::ffi::c_void,
) -> doms_status_t {
    if isolate.is_null() {
        return doms_status_t::ErrorInvalidArg;
    }
    doms_status_t::Ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ffi_lifecycle_and_eval() {
        let engine = doms_engine_new();
        assert!(!engine.is_null());
        let isolate = unsafe { doms_isolate_new(engine) };
        assert!(!isolate.is_null());
        let ctx = unsafe { doms_context_new(isolate) };
        assert!(!ctx.is_null());

        let code = b"21 * 2";
        let mut result: *mut doms_value_t = ptr::null_mut();
        let status = unsafe {
            doms_eval(
                ctx,
                code.as_ptr() as *const c_char,
                code.len(),
                &mut result,
            )
        };
        assert!(matches!(status, doms_status_t::Ok));
        assert!(!result.is_null());

        let mut num = 0.0;
        let s2 = unsafe { doms_value_to_number(ctx, result, &mut num) };
        assert!(matches!(s2, doms_status_t::Ok));
        assert_eq!(num, 42.0);

        unsafe {
            doms_value_unref(result);
            doms_context_destroy(ctx);
            doms_isolate_destroy(isolate);
            doms_engine_destroy(engine);
        }
    }

    #[test]
    fn test_ffi_object_operations() {
        let engine = doms_engine_new();
        let isolate = unsafe { doms_isolate_new(engine) };
        let ctx = unsafe { doms_context_new(isolate) };

        let code = b"var o = { a: 10, b: 'doms' }; o;";
        let mut obj: *mut doms_value_t = ptr::null_mut();
        let status = unsafe {
            doms_eval(
                ctx,
                code.as_ptr() as *const c_char,
                code.len(),
                &mut obj,
            )
        };
        assert!(matches!(status, doms_status_t::Ok));
        assert_eq!(unsafe { doms_value_is_object(obj) }, 1);

        // Get property 'a'
        let mut a_val: *mut doms_value_t = ptr::null_mut();
        let s_get = unsafe {
            doms_object_get(
                ctx,
                obj,
                c"a".as_ptr(),
                1,
                &mut a_val,
            )
        };
        assert!(matches!(s_get, doms_status_t::Ok));
        let mut num = 0.0;
        unsafe { doms_value_to_number(ctx, a_val, &mut num) };
        assert_eq!(num, 10.0);

        // Set property 'c' = 99
        let c_val = doms_value_new_number(99.0);
        let s_set = unsafe {
            doms_object_set(
                ctx,
                obj,
                c"c".as_ptr(),
                1,
                c_val,
            )
        };
        assert!(matches!(s_set, doms_status_t::Ok));

        // Get property 'c'
        let mut retrieved_c: *mut doms_value_t = ptr::null_mut();
        unsafe {
            doms_object_get(
                ctx,
                obj,
                c"c".as_ptr(),
                1,
                &mut retrieved_c,
            );
        }
        let mut num_c = 0.0;
        unsafe { doms_value_to_number(ctx, retrieved_c, &mut num_c) };
        assert_eq!(num_c, 99.0);

        unsafe {
            doms_value_unref(a_val);
            doms_value_unref(c_val);
            doms_value_unref(retrieved_c);
            doms_value_unref(obj);
            doms_context_destroy(ctx);
            doms_isolate_destroy(isolate);
            doms_engine_destroy(engine);
        }
    }

    #[test]
    fn test_ffi_function_call() {
        let engine = doms_engine_new();
        let isolate = unsafe { doms_isolate_new(engine) };
        let ctx = unsafe { doms_context_new(isolate) };

        let code = b"function add(x, y) { return x + y; } add;";
        let mut func: *mut doms_value_t = ptr::null_mut();
        unsafe {
            doms_eval(
                ctx,
                code.as_ptr() as *const c_char,
                code.len(),
                &mut func,
            );
        }
        assert_eq!(unsafe { doms_value_is_function(func) }, 1);

        let arg1 = doms_value_new_number(15.0);
        let arg2 = doms_value_new_number(27.0);
        let mut args = [arg1, arg2];

        let mut res: *mut doms_value_t = ptr::null_mut();
        let status = unsafe {
            doms_call(
                ctx,
                func,
                ptr::null_mut(),
                2,
                args.as_mut_ptr(),
                &mut res,
            )
        };
        assert!(matches!(status, doms_status_t::Ok));

        let mut sum = 0.0;
        unsafe { doms_value_to_number(ctx, res, &mut sum) };
        assert_eq!(sum, 42.0);

        unsafe {
            doms_value_unref(arg1);
            doms_value_unref(arg2);
            doms_value_unref(res);
            doms_value_unref(func);
            doms_context_destroy(ctx);
            doms_isolate_destroy(isolate);
            doms_engine_destroy(engine);
        }
    }
}

