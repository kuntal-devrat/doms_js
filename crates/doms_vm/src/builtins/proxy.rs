//! The `Proxy` constructor.

use super::*;

pub(super) fn register_proxy(vm: &mut Vm) {
    let proxy_ctor = vm.alloc_native_function("Proxy", 2, |vm, _this, args| {
        if !vm.is_constructing {
            return Err(VmError::TypeError("Constructor Proxy requires 'new'".into()));
        }
        let target = args.first().copied().unwrap_or(JsValue::undefined());
        let handler = args.get(1).copied().unwrap_or(JsValue::undefined());
        if !target.is_object() || !handler.is_object() {
            return Err(VmError::TypeError("Cannot create proxy with a non-object target or handler".into()));
        }
        Ok(vm.alloc_proxy(target, handler))
    });
    let revocable_fn = vm.alloc_native_function("revocable", 2, |vm, _this, args| {
        let target = args.first().copied().unwrap_or(JsValue::undefined());
        let handler = args.get(1).copied().unwrap_or(JsValue::undefined());
        if !target.is_object() || !handler.is_object() {
            return Err(VmError::TypeError("Cannot create proxy with a non-object target or handler".into()));
        }
        let proxy = vm.alloc_proxy(target, handler);
        let revoke = vm.alloc_native_function("revoke", 0, |_vm, _this, _args| {
            Ok(JsValue::undefined())
        });
        let result = vm.alloc_object();
        vm.set_property(result, "proxy", proxy);
        vm.set_property(result, "revoke", revoke);
        Ok(result)
    });
    vm.set_property(proxy_ctor, "revocable", revocable_fn);
    vm.globals.insert("Proxy".to_string(), proxy_ctor);
    if vm.global_object.is_object() {
        vm.set_property(vm.global_object, "Proxy", proxy_ctor);
    }
}
