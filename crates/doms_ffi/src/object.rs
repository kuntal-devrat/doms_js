//! Object and Array operations FFI.

use super::*;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn doms_get_global(
    ctx: *mut doms_context_t,
    result: *mut *mut doms_value_t,
) -> doms_status_t {
    if ctx.is_null() || result.is_null() {
        return doms_status_t::ErrorInvalidArg;
    }
    let isolate = unsafe { &mut (*(*ctx).isolate).inner };
    let g = isolate.vm.global_object;
    unsafe {
        *result =
            Box::into_raw(Box::new(doms_value_t { inner: g, ref_count: AtomicUsize::new(1) }));
    }
    doms_status_t::Ok
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn doms_set_global(
    ctx: *mut doms_context_t,
    name: *const c_char,
    name_len: usize,
    val: *mut doms_value_t,
) -> doms_status_t {
    if ctx.is_null() || name.is_null() || val.is_null() {
        return doms_status_t::ErrorInvalidArg;
    }
    let slice = unsafe { std::slice::from_raw_parts(name as *const u8, name_len) };
    let key_str = match std::str::from_utf8(slice) {
        Ok(s) => s,
        Err(_) => return doms_status_t::ErrorInvalidArg,
    };
    let isolate = unsafe { &mut (*(*ctx).isolate).inner };
    let g = isolate.vm.global_object;
    let v = unsafe { (*val).inner };
    isolate.vm.set_property(g, key_str, v);
    doms_status_t::Ok
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn doms_object_new(
    ctx: *mut doms_context_t,
    result: *mut *mut doms_value_t,
) -> doms_status_t {
    if ctx.is_null() || result.is_null() {
        return doms_status_t::ErrorInvalidArg;
    }
    let isolate = unsafe { &mut (*(*ctx).isolate).inner };
    let obj = isolate.vm.alloc_object();
    unsafe {
        *result =
            Box::into_raw(Box::new(doms_value_t { inner: obj, ref_count: AtomicUsize::new(1) }));
    }
    doms_status_t::Ok
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
        *result =
            Box::into_raw(Box::new(doms_value_t { inner: val, ref_count: AtomicUsize::new(1) }));
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
pub unsafe extern "C" fn doms_array_new(
    ctx: *mut doms_context_t,
    result: *mut *mut doms_value_t,
) -> doms_status_t {
    if ctx.is_null() || result.is_null() {
        return doms_status_t::ErrorInvalidArg;
    }
    let isolate = unsafe { &mut (*(*ctx).isolate).inner };
    let arr = isolate.vm.alloc_array(Vec::new());
    unsafe {
        *result =
            Box::into_raw(Box::new(doms_value_t { inner: arr, ref_count: AtomicUsize::new(1) }));
    }
    doms_status_t::Ok
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn doms_array_push(
    ctx: *mut doms_context_t,
    arr: *mut doms_value_t,
    val: *mut doms_value_t,
) -> doms_status_t {
    if ctx.is_null() || arr.is_null() || val.is_null() {
        return doms_status_t::ErrorInvalidArg;
    }
    let isolate = unsafe { &mut (*(*ctx).isolate).inner };
    let target = unsafe { (*arr).inner };
    let elem = unsafe { (*val).inner };
    let len_val = isolate.vm.get_property(target, "length");
    let len = len_val
        .as_int32()
        .map(|n| n.max(0) as usize)
        .or_else(|| len_val.as_double().map(|n| n.max(0.0) as usize))
        .unwrap_or(0);
    isolate.vm.set_element(target, len, elem);
    doms_status_t::Ok
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn doms_array_length(
    ctx: *mut doms_context_t,
    arr: *mut doms_value_t,
    out_len: *mut usize,
) -> doms_status_t {
    if ctx.is_null() || arr.is_null() || out_len.is_null() {
        return doms_status_t::ErrorInvalidArg;
    }
    let isolate = unsafe { &mut (*(*ctx).isolate).inner };
    let target = unsafe { (*arr).inner };
    let len_val = isolate.vm.get_property(target, "length");
    let len = len_val
        .as_int32()
        .map(|n| n.max(0) as usize)
        .or_else(|| len_val.as_double().map(|n| n.max(0.0) as usize))
        .unwrap_or(0);
    unsafe {
        *out_len = len;
    }
    doms_status_t::Ok
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn doms_array_get_at(
    ctx: *mut doms_context_t,
    arr: *mut doms_value_t,
    index: usize,
    result: *mut *mut doms_value_t,
) -> doms_status_t {
    if ctx.is_null() || arr.is_null() || result.is_null() {
        return doms_status_t::ErrorInvalidArg;
    }
    let isolate = unsafe { &mut (*(*ctx).isolate).inner };
    let target = unsafe { (*arr).inner };
    let val = isolate.vm.get_element(target, index);
    unsafe {
        *result =
            Box::into_raw(Box::new(doms_value_t { inner: val, ref_count: AtomicUsize::new(1) }));
    }
    doms_status_t::Ok
}
