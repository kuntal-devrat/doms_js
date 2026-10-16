//! Object property access FFI.

use super::*;

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
