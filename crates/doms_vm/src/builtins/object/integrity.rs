//! Extensibility and integrity statics: `preventExtensions`, `isExtensible`,
//! `freeze`, `isFrozen`, `seal` and `isSealed`.

use super::*;

pub fn prevent_extensions_internal(vm: &mut Vm, target: JsValue) -> Result<bool, VmError> {
    if let Some(ptr) = target.as_object_ptr() {
        let obj = unsafe { &*(ptr as *mut JsObject) };
        if let ObjectKind::Proxy { target: inner_target, handler } = &obj.kind {
            let inner_target = *inner_target;
            let handler = *handler;
            let trap = vm.get_property(handler, "preventExtensions");
            if vm.is_callable(trap) {
                let trap_res = vm.call(trap, handler, &[inner_target])?;
                let boolean_res = trap_res.is_truthy();
                if boolean_res {
                    let inner_ext = is_extensible_internal(vm, inner_target)?;
                    if inner_ext {
                        return Err(VmError::TypeError("Proxy preventExtensions trap cannot return true if target is extensible".into()));
                    }
                }
                return Ok(boolean_res);
            } else {
                return prevent_extensions_internal(vm, inner_target);
            }
        }
    }
    if vm.module_namespace_objects.contains(&target) {
        return Err(VmError::TypeError("Cannot prevent extensions on module namespace object".into()));
    }
    vm.non_extensible_objects.insert(target);
    Ok(true)
}

pub fn is_extensible_internal(vm: &mut Vm, target: JsValue) -> Result<bool, VmError> {
    if !target.is_js_object() {
        return Ok(false);
    }
    if let Some(ptr) = target.as_object_ptr() {
        let obj = unsafe { &*(ptr as *mut JsObject) };
        if let ObjectKind::Proxy { target: inner_target, handler } = &obj.kind {
            let inner_target = *inner_target;
            let handler = *handler;
            let trap = vm.get_property(handler, "isExtensible");
            if vm.is_callable(trap) {
                let trap_res = vm.call(trap, handler, &[inner_target])?;
                let boolean_res = trap_res.is_truthy();
                let inner_ext = is_extensible_internal(vm, inner_target)?;
                if boolean_res != inner_ext {
                    return Err(VmError::TypeError("Proxy isExtensible trap returned inconsistent result".into()));
                }
                return Ok(boolean_res);
            } else {
                return is_extensible_internal(vm, inner_target);
            }
        }
    }
    Ok(!vm.non_extensible_objects.contains(&target))
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum IntegrityLevel {
    Sealed,
    Frozen,
}

pub fn set_integrity_level(vm: &mut Vm, target: JsValue, level: IntegrityLevel) -> Result<bool, VmError> {
    let status = prevent_extensions_internal(vm, target)?;
    if !status {
        return Ok(false);
    }
    let keys = super::names::get_own_property_keys(vm, target)?;
    for key in keys {
        match level {
            IntegrityLevel::Sealed => {
                let desc = vm.alloc_object();
                vm.set_property(desc, "configurable", JsValue::from_boolean(false));
                super::define_property::define_property_internal(vm, target, key, desc)?;
            }
            IntegrityLevel::Frozen => {
                let current = super::descriptor::get_own_property_descriptor_internal(vm, target, &key)?;
                if !current.is_undefined() {
                    let desc = vm.alloc_object();
                    vm.set_property(desc, "configurable", JsValue::from_boolean(false));
                    let is_accessor = !vm.get_property(current, "get").is_undefined()
                        || !vm.get_property(current, "set").is_undefined();
                    if !is_accessor {
                        vm.set_property(desc, "writable", JsValue::from_boolean(false));
                    }
                    super::define_property::define_property_internal(vm, target, key, desc)?;
                }
            }
        }
    }
    Ok(true)
}

pub fn test_integrity_level(vm: &mut Vm, target: JsValue, level: IntegrityLevel) -> Result<bool, VmError> {
    if !target.is_js_object() {
        return Ok(true);
    }
    let ext = is_extensible_internal(vm, target)?;
    if ext {
        return Ok(false);
    }
    let keys = super::names::get_own_property_keys(vm, target)?;
    for key in keys {
        let current = super::descriptor::get_own_property_descriptor_internal(vm, target, &key)?;
        if !current.is_undefined() {
            if vm.get_property(current, "configurable").is_truthy() {
                return Ok(false);
            }
            if level == IntegrityLevel::Frozen {
                let is_accessor = !vm.get_property(current, "get").is_undefined()
                    || !vm.get_property(current, "set").is_undefined();
                if !is_accessor && vm.get_property(current, "writable").is_truthy() {
                    return Ok(false);
                }
            }
        }
    }
    Ok(true)
}

pub(super) fn register_integrity(
    vm: &mut Vm,
) -> (JsValue, JsValue, JsValue, JsValue, JsValue, JsValue) {
    let prevent_extensions_fn = vm.alloc_native_function("preventExtensions", 1, |vm, _this, args| {
        let target = match args.first() {
            Some(t) if t.is_js_object() => *t,
            Some(t) => return Ok(*t),
            _ => return Err(VmError::TypeError("Object.preventExtensions called on undefined or null".into())),
        };
        let status = prevent_extensions_internal(vm, target)?;
        if !status {
            return Err(VmError::TypeError("Cannot prevent extensions".into()));
        }
        Ok(target)
    });

    let is_extensible_fn = vm.alloc_native_function("isExtensible", 1, |vm, _this, args| {
        let target = match args.first() {
            Some(t) if t.is_js_object() => *t,
            _ => return Ok(JsValue::from_boolean(false)),
        };
        let ext = is_extensible_internal(vm, target)?;
        Ok(JsValue::from_boolean(ext))
    });

    let freeze_fn = vm.alloc_native_function("freeze", 1, |vm, _this, args| {
        let target = match args.first() {
            Some(t) if t.is_js_object() => *t,
            Some(t) => return Ok(*t),
            _ => return Err(VmError::TypeError("Object.freeze called on undefined or null".into())),
        };
        let status = set_integrity_level(vm, target, IntegrityLevel::Frozen)?;
        if !status {
            return Err(VmError::TypeError("Cannot freeze object".into()));
        }
        Ok(target)
    });

    let is_frozen_fn = vm.alloc_native_function("isFrozen", 1, |vm, _this, args| {
        let target = match args.first() {
            Some(t) if t.is_js_object() => *t,
            _ => return Ok(JsValue::from_boolean(true)),
        };
        let res = test_integrity_level(vm, target, IntegrityLevel::Frozen)?;
        Ok(JsValue::from_boolean(res))
    });

    let seal_fn = vm.alloc_native_function("seal", 1, |vm, _this, args| {
        let target = match args.first() {
            Some(t) if t.is_js_object() => *t,
            Some(t) => return Ok(*t),
            _ => return Err(VmError::TypeError("Object.seal called on undefined or null".into())),
        };
        let status = set_integrity_level(vm, target, IntegrityLevel::Sealed)?;
        if !status {
            return Err(VmError::TypeError("Cannot seal object".into()));
        }
        Ok(target)
    });

    let is_sealed_fn = vm.alloc_native_function("isSealed", 1, |vm, _this, args| {
        let target = match args.first() {
            Some(t) if t.is_js_object() => *t,
            _ => return Ok(JsValue::from_boolean(true)),
        };
        let res = test_integrity_level(vm, target, IntegrityLevel::Sealed)?;
        Ok(JsValue::from_boolean(res))
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
