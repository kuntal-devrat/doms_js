//! The `Promise` constructor.

use super::*;

pub(super) fn create_constructor(vm: &mut Vm) -> JsValue {
    vm.alloc_native_function("Promise", 1, |vm, _this, args| {
        if !vm.is_constructing {
            return Err(VmError::TypeError("Promise constructor cannot be invoked without 'new'".into()));
        }
        let executor = args.first().copied().unwrap_or(JsValue::undefined());
        if !vm.is_callable(executor) {
            return Err(VmError::TypeError("Promise resolver is not a function".into()));
        }
        let promise_val = vm.alloc_promise(crate::object::PromiseState::Pending, JsValue::undefined());
        let p_for_res = promise_val;
        let resolve_fn = vm.alloc_closure_native_function("", 1, Arc::new(move |vm: &mut Vm, _this: JsValue, args: &[JsValue]| -> Result<JsValue, VmError> {
            let res = args.first().copied().unwrap_or(JsValue::undefined());
            vm.resolve_promise(p_for_res, res);
            Ok(JsValue::undefined())
        }));
        let p_for_rej = promise_val;
        let reject_fn = vm.alloc_closure_native_function("", 1, Arc::new(move |vm: &mut Vm, _this: JsValue, args: &[JsValue]| -> Result<JsValue, VmError> {
            let rej = args.first().copied().unwrap_or(JsValue::undefined());
            vm.reject_promise(p_for_rej, rej);
            Ok(JsValue::undefined())
        }));

        match vm.call(executor, JsValue::undefined(), &[resolve_fn, reject_fn]) {
            Ok(_) => Ok(promise_val),
            Err(e) => {
                let err_val = vm.vm_error_to_js_value(&e);
                let _ = vm.call(reject_fn, JsValue::undefined(), &[err_val]);
                Ok(promise_val)
            }
        }
    })
}
