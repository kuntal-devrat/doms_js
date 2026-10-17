//! `Promise.resolve` and `Promise.reject`.

use super::*;

pub(super) fn promise_resolve(vm: &mut Vm, ctor: JsValue, x: JsValue) -> Result<JsValue, VmError> {
    if let Some(ptr) = x.as_object_ptr() {
        let obj = unsafe { &*(ptr as *mut JsObject) };
        if matches!(obj.kind, ObjectKind::Promise(_)) {
            let x_ctor = vm.get_property(x, "constructor");
            if x_ctor == ctor {
                return Ok(x);
            }
        }
    }
    let capability = super::new_promise_capability(vm, ctor)?;
    vm.call(capability.resolve, JsValue::undefined(), &[x])?;
    Ok(capability.promise)
}

pub(super) fn create_basic_statics(vm: &mut Vm) -> Vec<(&'static str, JsValue)> {
    let resolve_static = vm.alloc_native_function("resolve", 1, |vm, this, args| {
        if !this.is_object() {
            return Err(VmError::TypeError("Promise.resolve called on non-object".into()));
        }
        let x = args.first().copied().unwrap_or(JsValue::undefined());
        promise_resolve(vm, this, x)
    });

    let reject_static = vm.alloc_native_function("reject", 1, |vm, this, args| {
        if !this.is_object() {
            return Err(VmError::TypeError("Promise.reject called on non-object".into()));
        }
        let r = args.first().copied().unwrap_or(JsValue::undefined());
        let capability = super::new_promise_capability(vm, this)?;
        vm.call(capability.reject, JsValue::undefined(), &[r])?;
        Ok(capability.promise)
    });

    let try_static = vm.alloc_native_function("try", 1, |vm, this, args| {
        if !this.is_object() {
            return Err(VmError::TypeError("Promise.try called on non-object".into()));
        }
        let callback = args.first().copied().unwrap_or(JsValue::undefined());
        let rest_args = if args.len() > 1 { &args[1..] } else { &[] };
        let call_res = vm.call(callback, JsValue::undefined(), rest_args);
        match call_res {
            Ok(val) => promise_resolve(vm, this, val),
            Err(e) => {
                let capability = super::new_promise_capability(vm, this)?;
                let err_val = vm.vm_error_to_js_value(&e);
                vm.call(capability.reject, JsValue::undefined(), &[err_val])?;
                Ok(capability.promise)
            }
        }
    });

    vec![("resolve", resolve_static), ("reject", reject_static), ("try", try_static)]
}
