//! Extensibility and integrity statics: `preventExtensions`, `isExtensible`,
//! `freeze`, `isFrozen`, `seal` and `isSealed`.

use super::*;

pub(super) fn register_integrity(
    vm: &mut Vm,
) -> (JsValue, JsValue, JsValue, JsValue, JsValue, JsValue) {
    let prevent_extensions_fn = vm.alloc_native_function("preventExtensions", 1, |vm, _this, args| {
        let target = match args.first() {
            Some(t) if t.is_object() => *t,
            Some(t) if !t.is_nullish() => return Ok(*t),
            _ => return Err(VmError::TypeError("Object.preventExtensions called on non-object".into())),
        };
        vm.non_extensible_objects.insert(target);
        Ok(target)
    });

    let is_extensible_fn = vm.alloc_native_function("isExtensible", 1, |vm, _this, args| {
        let target = match args.first() {
            Some(t) if t.is_object() => *t,
            _ => return Ok(JsValue::from_boolean(false)),
        };
        let ext = !vm.non_extensible_objects.contains(&target);
        Ok(JsValue::from_boolean(ext))
    });

    let freeze_fn = vm.alloc_native_function("freeze", 1, |vm, _this, args| {
        let target = match args.first() {
            Some(t) if t.is_object() => *t,
            Some(t) => return Ok(*t),
            _ => return Err(VmError::TypeError("Object.freeze called on non-object".into())),
        };
        if vm.module_namespace_objects.contains(&target) {
            return Err(VmError::TypeError("Cannot freeze module namespace object".into()));
        }
        vm.non_extensible_objects.insert(target);
        if let Some(ptr) = target.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            for prop in vm.shapes.all_properties(obj.shape_id) {
                vm.non_writable_props.insert((target, prop.clone()));
                vm.non_configurable_props.insert((target, prop));
            }
            if let Some(accs) = &obj.accessors {
                for prop in accs.keys() {
                    vm.non_configurable_props.insert((target, prop.clone()));
                }
            }
        }
        Ok(target)
    });

    let is_frozen_fn = vm.alloc_native_function("isFrozen", 1, |vm, _this, args| {
        let target = match args.first() {
            Some(t) if t.is_object() => *t,
            _ => return Ok(JsValue::from_boolean(true)),
        };
        if vm.module_namespace_objects.contains(&target) {
            return Ok(JsValue::from_boolean(false));
        }
        if !vm.non_extensible_objects.contains(&target) {
            return Ok(JsValue::from_boolean(false));
        }
        if let Some(ptr) = target.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            for prop in vm.shapes.all_properties(obj.shape_id) {
                if !vm.non_configurable_props.contains(&(target, prop.clone())) {
                    return Ok(JsValue::from_boolean(false));
                }
                if !vm.non_writable_props.contains(&(target, prop.clone())) {
                    return Ok(JsValue::from_boolean(false));
                }
            }
            if let Some(accs) = &obj.accessors {
                for prop in accs.keys() {
                    if !vm.non_configurable_props.contains(&(target, prop.clone())) {
                        return Ok(JsValue::from_boolean(false));
                    }
                }
            }
        }
        Ok(JsValue::from_boolean(true))
    });

    let seal_fn = vm.alloc_native_function("seal", 1, |vm, _this, args| {
        let target = match args.first() {
            Some(t) if t.is_object() => *t,
            Some(t) => return Ok(*t),
            _ => return Err(VmError::TypeError("Object.seal called on non-object".into())),
        };
        vm.non_extensible_objects.insert(target);
        if let Some(ptr) = target.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            for prop in vm.shapes.all_properties(obj.shape_id) {
                vm.non_configurable_props.insert((target, prop));
            }
            if let Some(accs) = &obj.accessors {
                for prop in accs.keys() {
                    vm.non_configurable_props.insert((target, prop.clone()));
                }
            }
        }
        Ok(target)
    });

    let is_sealed_fn = vm.alloc_native_function("isSealed", 1, |vm, _this, args| {
        let target = match args.first() {
            Some(t) if t.is_object() => *t,
            _ => return Ok(JsValue::from_boolean(true)),
        };
        if !vm.non_extensible_objects.contains(&target) {
            return Ok(JsValue::from_boolean(false));
        }
        if let Some(ptr) = target.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            for prop in vm.shapes.all_properties(obj.shape_id) {
                if !vm.non_configurable_props.contains(&(target, prop.clone())) {
                    return Ok(JsValue::from_boolean(false));
                }
            }
            if let Some(accs) = &obj.accessors {
                for prop in accs.keys() {
                    if !vm.non_configurable_props.contains(&(target, prop.clone())) {
                        return Ok(JsValue::from_boolean(false));
                    }
                }
            }
        }
        Ok(JsValue::from_boolean(true))
    });
    (
        prevent_extensions_fn,
        is_extensible_fn,
        freeze_fn,
        is_frozen_fn,
        seal_fn,
        is_sealed_fn,
    )
}
