//! `Promise.resolve` and `Promise.reject`.

use super::*;

pub(super) fn create_basic_statics(vm: &mut Vm) -> Vec<(&'static str, JsValue)> {
    let resolve_static = vm.alloc_native_function("resolve", 1, |vm, _this, args| {
        let val = args.first().copied().unwrap_or(JsValue::undefined());
        if let Some(ptr) = val.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if matches!(obj.kind, ObjectKind::Promise(_)) {
                return Ok(val);
            }
        }
        Ok(vm.alloc_promise(crate::object::PromiseState::Fulfilled, val))
    });

    let reject_static = vm.alloc_native_function("reject", 1, |vm, _this, args| {
        let reason = args.first().copied().unwrap_or(JsValue::undefined());
        Ok(vm.alloc_promise(crate::object::PromiseState::Rejected, reason))
    });
    vec![("resolve", resolve_static), ("reject", reject_static)]
}
