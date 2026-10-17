//! The `Boolean` global object.

use super::*;

pub(super) fn register_boolean_global(vm: &mut Vm) {
    let boolean_proto = vm.alloc_object();
    vm.boolean_proto = Some(boolean_proto);

    let bool_value_of = vm.alloc_native_function("valueOf", 0, |_vm, this, _args| {
        if this.is_boolean() {
            return Ok(this);
        }
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::BooleanObject(b) = obj.kind {
                return Ok(JsValue::from_boolean(b));
            }
        }
        Err(VmError::TypeError("Boolean.prototype.valueOf requires that 'this' be a Boolean".into()))
    });
    let bool_to_string = vm.alloc_native_function("toString", 0, |vm, this, _args| {
        let b = if this.is_boolean() {
            this.as_boolean().unwrap()
        } else if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::BooleanObject(b) = obj.kind {
                b
            } else {
                return Err(VmError::TypeError("Boolean.prototype.toString requires that 'this' be a Boolean".into()));
            }
        } else {
            return Err(VmError::TypeError("Boolean.prototype.toString requires that 'this' be a Boolean".into()));
        };
        Ok(vm.alloc_string(if b { "true".into() } else { "false".into() }))
    });
    vm.set_property(boolean_proto, "valueOf", bool_value_of);
    vm.set_property(boolean_proto, "toString", bool_to_string);

    let bool_ctor = vm.alloc_native_function("Boolean", 1, |vm, _this, args| {
        let b = args.first().map(|v| v.is_truthy()).unwrap_or(false);
        if vm.is_constructing {
            let proto = vm.boolean_proto;
            Ok(vm.alloc_boolean_object(b, proto))
        } else {
            Ok(JsValue::from_boolean(b))
        }
    });
    vm.set_property(boolean_proto, "constructor", bool_ctor);
    vm.non_enumerable_props.insert((boolean_proto, "constructor".to_string()));
    vm.set_property(bool_ctor, "prototype", boolean_proto);
    vm.non_enumerable_props.insert((bool_ctor, "prototype".to_string()));
    vm.non_writable_props.insert((bool_ctor, "prototype".to_string()));
    vm.non_configurable_props.insert((bool_ctor, "prototype".to_string()));
    vm.globals.insert("Boolean".to_string(), bool_ctor);
    if vm.global_object.is_object() {
        vm.set_property(vm.global_object, "Boolean", bool_ctor);
    }
}
