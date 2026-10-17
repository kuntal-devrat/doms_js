//! The `Proxy` constructor.

use super::*;

pub(super) fn register_proxy(vm: &mut Vm) {
    let proxy_ctor = vm.alloc_native_function("Proxy", 2, |vm, _this, args| {
        if !vm.is_constructing {
            return Err(VmError::TypeError("Constructor Proxy requires 'new'".into()));
        }
        let target = args.first().copied().unwrap_or(JsValue::undefined());
        let handler = args.get(1).copied().unwrap_or(JsValue::undefined());
        if !target.is_js_object() || !handler.is_js_object() {
            return Err(VmError::TypeError("Cannot create proxy with a non-object target or handler".into()));
        }
        Ok(vm.alloc_proxy(target, handler))
    });
    let revocable_fn = vm.alloc_native_function("revocable", 2, |vm, _this, args| {
        let target = args.first().copied().unwrap_or(JsValue::undefined());
        let handler = args.get(1).copied().unwrap_or(JsValue::undefined());
        if !target.is_js_object() || !handler.is_js_object() {
            return Err(VmError::TypeError("Cannot create proxy with a non-object target or handler".into()));
        }
        let proxy = vm.alloc_proxy(target, handler);
        let proxy_cell = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(proxy.raw_bits()));
        let revoke = vm.alloc_closure_native_function("", 0, std::sync::Arc::new(move |_vm, _this, _args| {
            let raw = proxy_cell.swap(0, std::sync::atomic::Ordering::SeqCst);
            if raw != 0 {
                let p = JsValue(raw);
                if let Some(ptr) = p.as_object_ptr() {
                    let obj = unsafe { &mut *(ptr as *mut JsObject) };
                    if let ObjectKind::Proxy { target, .. } = obj.kind {
                        obj.kind = ObjectKind::Proxy { target, handler: JsValue::null() };
                    }
                }
            }
            Ok(JsValue::undefined())
        }));
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
