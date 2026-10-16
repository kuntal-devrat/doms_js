//! Calling JS functions and registering native callbacks FFI.

use super::*;

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
