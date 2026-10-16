//! Core `Object.prototype` methods and the early `Function.prototype` object.

use super::*;

pub(super) fn register_prototype_methods(vm: &mut Vm, object_proto: JsValue) {
    let is_prototype_of_fn = vm.alloc_native_function("isPrototypeOf", 1, |vm, this, args| {
        if !this.is_object() {
            return Ok(JsValue::from_boolean(false));
        }
        let target = match args.first() {
            Some(t) if t.is_object() => *t,
            _ => return Ok(JsValue::from_boolean(false)),
        };
        let mut current = vm.get_prototype_of(target);
        while let Some(proto) = current {
            if proto == this {
                return Ok(JsValue::from_boolean(true));
            }
            current = vm.get_prototype_of(proto);
        }
        Ok(JsValue::from_boolean(false))
    });
    vm.set_property(object_proto, "isPrototypeOf", is_prototype_of_fn);

    let value_of_fn = vm.alloc_native_function("valueOf", 0, |_vm, this, _args| {
        Ok(this)
    });
    vm.set_property(object_proto, "valueOf", value_of_fn);

    // Create Function.prototype early so all native functions get Function.prototype
    let native = crate::object::NativeFunction::new_fn("".to_string(), 0, |_vm, _this, _args| Ok(JsValue::undefined()));
    let fn_proto_obj = Box::new(JsObject::new_native_function(crate::shape::ShapeRegistry::ROOT_SHAPE_ID, Some(object_proto), native));
    let fn_proto_ptr = Box::into_raw(fn_proto_obj);
    vm.track_ptr(fn_proto_ptr);
    let function_proto = unsafe { JsValue::from_object_ptr(fn_proto_ptr as *mut u8) };
    vm.function_proto = Some(function_proto);
    let to_string_fn = vm.alloc_native_function("toString", 0, |vm, this, _args| {
        if let Some(ptr) = this.as_object_ptr() {
            if let Some(sym_tag) = vm.symbol_to_string_tag {
                let tag_val = vm.get_symbol_property(this, sym_tag);
                if tag_val.is_string() {
                    let s = vm.to_string_value(tag_val);
                    return Ok(vm.alloc_string(format!("[object {}]", s)));
                }
            }
            let obj = unsafe { &*(ptr as *mut JsObject) };
            let tag = match &obj.kind {
                ObjectKind::Array(_) => "Array",
                ObjectKind::Function(_) | ObjectKind::NativeFunction(_) => "Function",
                ObjectKind::Error { .. } => "Error",
                ObjectKind::RegExp { .. } => "RegExp",
                ObjectKind::Date(_) => "Date",
                ObjectKind::Map(_) => "Map",
                ObjectKind::Set(_) => "Set",
                ObjectKind::WeakMap(_) => "WeakMap",
                ObjectKind::WeakSet(_) => "WeakSet",
                ObjectKind::BigInt(_) => "BigInt",
                ObjectKind::String(_) | ObjectKind::StringObject(_) => "String",
                ObjectKind::NumberObject(_) => "Number",
                ObjectKind::BooleanObject(_) => "Boolean",
                _ => "Object",
            };
            Ok(vm.alloc_string(format!("[object {}]", tag)))
        } else if this.is_null() {
            Ok(vm.alloc_string("[object Null]".into()))
        } else if this.is_undefined() {
            Ok(vm.alloc_string("[object Undefined]".into()))
        } else if this.is_string() {
            Ok(vm.alloc_string("[object String]".into()))
        } else if this.is_number() {
            Ok(vm.alloc_string("[object Number]".into()))
        } else if this.is_boolean() {
            Ok(vm.alloc_string("[object Boolean]".into()))
        } else {
            Ok(vm.alloc_string("[object Object]".into()))
        }
    });
    vm.set_property(object_proto, "toString", to_string_fn);

    let to_locale_string_fn = vm.alloc_native_function("toLocaleString", 0, |vm, this, _args| {
        let to_str = vm.get_property(this, "toString");
        if vm.is_callable(to_str) {
            vm.call(to_str, this, &[])
        } else {
            Err(VmError::TypeError("toString is not callable".into()))
        }
    });
    vm.set_property(object_proto, "toLocaleString", to_locale_string_fn);
    vm.non_enumerable_props.insert((object_proto, "toLocaleString".to_string()));

    let proto_getter = vm.alloc_native_function("get __proto__", 0, |vm, this, _args| {
        if this.is_nullish() {
            return Err(VmError::TypeError("Cannot read property of null or undefined".into()));
        }
        let o = if this.is_object() {
            this
        } else {
            vm.to_object_value(this)?
        };
        if let Some(ptr) = o.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            Ok(obj.prototype.unwrap_or(JsValue::null()))
        } else {
            Ok(JsValue::null())
        }
    });

    let proto_setter = vm.alloc_native_function("set __proto__", 1, |vm, this, args| {
        if this.is_nullish() {
            return Err(VmError::TypeError("Cannot set property on null or undefined".into()));
        }
        let proto = args.first().copied().unwrap_or(JsValue::undefined());
        if !proto.is_object() && !proto.is_null() {
            return Ok(JsValue::undefined());
        }
        if !this.is_object() {
            return Ok(JsValue::undefined());
        }
        let ptr = this.as_object_ptr().unwrap();
        let obj = unsafe { &mut *(ptr as *mut JsObject) };
        let new_proto = if proto.is_null() { None } else { Some(proto) };
        if obj.prototype == new_proto {
            return Ok(JsValue::undefined());
        }
        if vm.non_extensible_objects.contains(&this) {
            return Err(VmError::TypeError("Cannot set prototype of non-extensible object".into()));
        }
        obj.prototype = new_proto;
        Ok(JsValue::undefined())
    });

    if let Some(ptr) = object_proto.as_object_ptr() {
        let obj = unsafe { &mut *(ptr as *mut JsObject) };
        obj.define_accessor("__proto__".to_string(), Some(proto_getter), Some(proto_setter));
    }
    vm.non_enumerable_props.insert((object_proto, "__proto__".to_string()));
}
