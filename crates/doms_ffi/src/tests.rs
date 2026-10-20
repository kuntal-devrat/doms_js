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
