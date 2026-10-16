//! `Object.prototype.hasOwnProperty` and `propertyIsEnumerable`.

use super::*;

pub(super) fn register_predicates(vm: &mut Vm, object_proto: JsValue) {
    let has_own_prop = vm.alloc_native_function("hasOwnProperty", 1, |vm, this, args| {
        if this.is_nullish() {
            return Err(VmError::TypeError("Object.prototype.hasOwnProperty called on null or undefined".into()));
        }
        let prop = args.first().copied().unwrap_or(JsValue::undefined());
        let key = vm.to_property_key(prop)?;
        if vm.module_namespace_objects.contains(&this) {
            if let PropertyKeyVal::String(s) = &key {
                if vm.has_own_property_key(this, &key) && vm.get_property(this, s) == JsValue::uninitialized() {
                    return Err(VmError::ReferenceError(format!("Cannot access uninitialized export '{}'", s)));
                }
            }
        }
        Ok(JsValue::from_boolean(vm.has_own_property_key(this, &key)))
    });
    vm.set_property(object_proto, "hasOwnProperty", has_own_prop);

    let prop_is_enumerable = vm.alloc_native_function("propertyIsEnumerable", 1, |vm, this, args| {
        let key = args.first().map(|v| vm.to_string_value(*v)).unwrap_or_default();
        if vm.module_namespace_objects.contains(&this) {
            if vm.has_property(this, &key)? && vm.get_property(this, &key) == JsValue::uninitialized() {
                return Err(VmError::ReferenceError(format!("Cannot access uninitialized export '{}'", key)));
            }
        }
        if vm.deleted_props.contains(&(this, key.clone())) {
            return Ok(JsValue::from_boolean(false));
        }
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if match &obj.kind {
                ObjectKind::Array(elements) => {
                    if key == "length" {
                        return Ok(JsValue::from_boolean(false));
                    }
                    if key.parse::<usize>().map(|idx| idx < elements.len()).unwrap_or(false) {
                        return Ok(JsValue::from_boolean(true));
                    }
                    false
                }
                ObjectKind::String(s) | ObjectKind::StringObject(s) => {
                    if key == "length" {
                        return Ok(JsValue::from_boolean(false));
                    }
                    if key.parse::<usize>().map(|idx| idx < s.chars().count()).unwrap_or(false) {
                        return Ok(JsValue::from_boolean(true));
                    }
                    false
                }
                ObjectKind::TypedArray(ta) => {
                    if matches!(key.as_str(), "length" | "byteLength" | "byteOffset" | "buffer") {
                        return Ok(JsValue::from_boolean(false));
                    }
                    if key.parse::<usize>().map(|idx| idx < ta.length).unwrap_or(false) {
                        return Ok(JsValue::from_boolean(true));
                    }
                    false
                }
                _ => false,
            } {
                return Ok(JsValue::from_boolean(true));
            }
            let has = vm.shapes.has_property(obj.shape_id, &key)
                || obj.accessors.as_ref().map(|a| a.contains_key(&key)).unwrap_or(false);
            if has {
                let is_enum = !vm.non_enumerable_props.contains(&(this, key));
                Ok(JsValue::from_boolean(is_enum))
            } else {
                Ok(JsValue::from_boolean(false))
            }
        } else {
            Ok(JsValue::from_boolean(false))
        }
    });
    vm.set_property(object_proto, "propertyIsEnumerable", prop_is_enumerable);
}
