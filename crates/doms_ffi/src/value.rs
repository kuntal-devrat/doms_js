//! Value marshalling FFI: conversions, predicates, constructors, and refcounting.

use super::*;

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
pub unsafe extern "C" fn doms_value_to_boolean(
    _ctx: *mut doms_context_t,
    val: *mut doms_value_t,
    out: *mut c_int,
) -> doms_status_t {
    if val.is_null() || out.is_null() {
        return doms_status_t::ErrorInvalidArg;
    }
    let b = unsafe { (*val).inner.as_boolean() };
    if let Some(b) = b {
        unsafe {
            *out = b as c_int;
        }
        doms_status_t::Ok
    } else {
        doms_status_t::ErrorType
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn doms_value_to_string(
    ctx: *mut doms_context_t,
    val: *mut doms_value_t,
    out: *mut *const c_char,
    out_len: *mut usize,
) -> doms_status_t {
    if val.is_null() || out.is_null() {
        return doms_status_t::ErrorInvalidArg;
    }
    let s = if !ctx.is_null() && unsafe { !(*ctx).isolate.is_null() } {
        let isolate = unsafe { &mut (*(*ctx).isolate).inner };
        let inner = unsafe { (*val).inner };
        isolate.vm.to_string_value(inner)
    } else {
        unsafe { format!("{}", (*val).inner) }
    };
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
pub unsafe extern "C" fn doms_value_new_string(
    ctx: *mut doms_context_t,
    str_ptr: *const c_char,
    len: usize,
    result: *mut *mut doms_value_t,
) -> doms_status_t {
    if ctx.is_null() || str_ptr.is_null() || result.is_null() {
        return doms_status_t::ErrorInvalidArg;
    }
    let slice = unsafe { std::slice::from_raw_parts(str_ptr as *const u8, len) };
    let s = match std::str::from_utf8(slice) {
        Ok(v) => v,
        Err(_) => return doms_status_t::ErrorInvalidArg,
    };
    let isolate = unsafe { &mut (*(*ctx).isolate).inner };
    let str_val = isolate.vm.alloc_string(s.to_string());
    unsafe {
        *result = Box::into_raw(Box::new(doms_value_t {
            inner: str_val,
            ref_count: AtomicUsize::new(1),
        }));
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
    if val.is_null() { 0 } else { unsafe { (*val).inner.is_js_string() as c_int } }
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
            doms_vm::object::ObjectKind::Function(_)
            | doms_vm::object::ObjectKind::NativeFunction(_) => 1,
            _ => 0,
        }
    } else {
        0
    }
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
