//! The `Promise` constructor.

use super::*;

pub(super) fn create_constructor(vm: &mut Vm) -> JsValue {
    vm.alloc_native_function("Promise", 1, |vm, _this, args| {
        let executor = args.first().copied().unwrap_or(JsValue::undefined());
        if !executor.is_object() {
            return Err(VmError::TypeError("Promise resolver is not a function".into()));
        }
        let promise_val = vm.alloc_promise(crate::object::PromiseState::Pending, JsValue::undefined());
        let p_for_res = promise_val;
        let resolve_fn = vm.alloc_closure_native_function("resolve", 1, Arc::new(move |vm: &mut Vm, _this: JsValue, args: &[JsValue]| -> Result<JsValue, VmError> {
            let res = args.first().copied().unwrap_or(JsValue::undefined());
            vm.resolve_promise(p_for_res, res);
            Ok(JsValue::undefined())
        }));
        let p_for_rej = promise_val;
        let reject_fn = vm.alloc_closure_native_function("reject", 1, Arc::new(move |vm: &mut Vm, _this: JsValue, args: &[JsValue]| -> Result<JsValue, VmError> {
            let rej = args.first().copied().unwrap_or(JsValue::undefined());
            vm.reject_promise(p_for_rej, rej);
            Ok(JsValue::undefined())
        }));

        match vm.call(executor, JsValue::undefined(), &[resolve_fn, reject_fn]) {
            Ok(_) => Ok(promise_val),
            Err(e) => {
                let err_str = vm.alloc_string(e.to_string());
                vm.reject_promise(promise_val, err_str);
                Ok(promise_val)
            }
        }
    })
}
