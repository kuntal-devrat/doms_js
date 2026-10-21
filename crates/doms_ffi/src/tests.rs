//! FFI integration tests covering lifecycle, values, objects, and calls.

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
    let status = unsafe { doms_eval(ctx, code.as_ptr() as *const c_char, code.len(), &mut result) };
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
    let status = unsafe { doms_eval(ctx, code.as_ptr() as *const c_char, code.len(), &mut obj) };
    assert!(matches!(status, doms_status_t::Ok));
    assert_eq!(unsafe { doms_value_is_object(obj) }, 1);

    // Get property 'a'
    let mut a_val: *mut doms_value_t = ptr::null_mut();
    let s_get = unsafe { doms_object_get(ctx, obj, c"a".as_ptr(), 1, &mut a_val) };
    assert!(matches!(s_get, doms_status_t::Ok));
    let mut num = 0.0;
    unsafe { doms_value_to_number(ctx, a_val, &mut num) };
    assert_eq!(num, 10.0);

    // Set property 'c' = 99
    let c_val = doms_value_new_number(99.0);
    let s_set = unsafe { doms_object_set(ctx, obj, c"c".as_ptr(), 1, c_val) };
    assert!(matches!(s_set, doms_status_t::Ok));

    // Get property 'c'
    let mut retrieved_c: *mut doms_value_t = ptr::null_mut();
    unsafe {
        doms_object_get(ctx, obj, c"c".as_ptr(), 1, &mut retrieved_c);
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
        doms_eval(ctx, code.as_ptr() as *const c_char, code.len(), &mut func);
    }
    assert_eq!(unsafe { doms_value_is_function(func) }, 1);

    let arg1 = doms_value_new_number(15.0);
    let arg2 = doms_value_new_number(27.0);
    let mut args = [arg1, arg2];

    let mut res: *mut doms_value_t = ptr::null_mut();
    let status = unsafe { doms_call(ctx, func, ptr::null_mut(), 2, args.as_mut_ptr(), &mut res) };
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

unsafe extern "C" fn native_multiply(
    _ctx: *mut doms_context_t,
    _this_val: *mut doms_value_t,
    argc: usize,
    argv: *mut *mut doms_value_t,
    result: *mut *mut doms_value_t,
    _userdata: *mut std::ffi::c_void,
) -> doms_status_t {
    if argc < 2 {
        return doms_status_t::ErrorInvalidArg;
    }
    let a_ptr = unsafe { *argv };
    let b_ptr = unsafe { *argv.add(1) };
    let a = unsafe { (*a_ptr).inner.as_number().unwrap_or(0.0) };
    let b = unsafe { (*b_ptr).inner.as_number().unwrap_or(0.0) };
    unsafe {
        *result = doms_value_new_number(a * b);
    }
    doms_status_t::Ok
}

#[test]
fn test_ffi_string_operations() {
    let engine = doms_engine_new();
    let isolate = unsafe { doms_isolate_new(engine) };
    let ctx = unsafe { doms_context_new(isolate) };

    let input = "hello doms!";
    let mut str_val: *mut doms_value_t = ptr::null_mut();
    let s = unsafe {
        doms_value_new_string(ctx, input.as_ptr() as *const c_char, input.len(), &mut str_val)
    };
    assert!(matches!(s, doms_status_t::Ok));
    assert_eq!(unsafe { doms_value_is_string(str_val) }, 1);

    let mut out_str: *const c_char = ptr::null();
    let mut out_len: usize = 0;
    let s2 = unsafe { doms_value_to_string(ctx, str_val, &mut out_str, &mut out_len) };
    assert!(matches!(s2, doms_status_t::Ok));
    assert_eq!(out_len, input.len());
    let cstr = unsafe { std::ffi::CStr::from_ptr(out_str) };
    assert_eq!(cstr.to_str().unwrap(), input);

    unsafe {
        doms_free_string(ctx, out_str);
        doms_value_unref(str_val);
        doms_context_destroy(ctx);
        doms_isolate_destroy(isolate);
        doms_engine_destroy(engine);
    }
}

#[test]
fn test_ffi_global_and_native_callback() {
    let engine = doms_engine_new();
    let isolate = unsafe { doms_isolate_new(engine) };
    let ctx = unsafe { doms_context_new(isolate) };

    // Register native multiply function globally
    let s_reg = unsafe {
        doms_set_global_function(ctx, c"multiply".as_ptr(), 8, native_multiply, ptr::null_mut())
    };
    assert!(matches!(s_reg, doms_status_t::Ok));

    // Also set a global variable `factor` = 3
    let num_val = doms_value_new_number(3.0);
    let s_set = unsafe { doms_set_global(ctx, c"factor".as_ptr(), 6, num_val) };
    assert!(matches!(s_set, doms_status_t::Ok));

    // Call from JS
    let code = b"multiply(factor, 14)";
    let mut result: *mut doms_value_t = ptr::null_mut();
    let s_eval = unsafe { doms_eval(ctx, code.as_ptr() as *const c_char, code.len(), &mut result) };
    assert!(matches!(s_eval, doms_status_t::Ok));

    let mut answer = 0.0;
    unsafe { doms_value_to_number(ctx, result, &mut answer) };
    assert_eq!(answer, 42.0);

    unsafe {
        doms_value_unref(num_val);
        doms_value_unref(result);
        doms_context_destroy(ctx);
        doms_isolate_destroy(isolate);
        doms_engine_destroy(engine);
    }
}

#[test]
fn test_ffi_array_operations() {
    let engine = doms_engine_new();
    let isolate = unsafe { doms_isolate_new(engine) };
    let ctx = unsafe { doms_context_new(isolate) };

    let mut arr: *mut doms_value_t = ptr::null_mut();
    let s_arr = unsafe { doms_array_new(ctx, &mut arr) };
    assert!(matches!(s_arr, doms_status_t::Ok));

    let v1 = doms_value_new_number(100.0);
    let v2 = doms_value_new_number(200.0);
    unsafe {
        doms_array_push(ctx, arr, v1);
        doms_array_push(ctx, arr, v2);
    }

    let mut len: usize = 0;
    unsafe { doms_array_length(ctx, arr, &mut len) };
    assert_eq!(len, 2);

    let mut elem0: *mut doms_value_t = ptr::null_mut();
    let mut elem1: *mut doms_value_t = ptr::null_mut();
    unsafe {
        doms_array_get_at(ctx, arr, 0, &mut elem0);
        doms_array_get_at(ctx, arr, 1, &mut elem1);
    }

    let mut n0 = 0.0;
    let mut n1 = 0.0;
    unsafe {
        doms_value_to_number(ctx, elem0, &mut n0);
        doms_value_to_number(ctx, elem1, &mut n1);
    }
    assert_eq!(n0, 100.0);
    assert_eq!(n1, 200.0);

    unsafe {
        doms_value_unref(v1);
        doms_value_unref(v2);
        doms_value_unref(elem0);
        doms_value_unref(elem1);
        doms_value_unref(arr);
        doms_context_destroy(ctx);
        doms_isolate_destroy(isolate);
        doms_engine_destroy(engine);
    }
}
