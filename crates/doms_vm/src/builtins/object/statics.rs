//! Prototype- and object-level statics: `getPrototypeOf`, `setPrototypeOf`,
//! `hasOwn`, `fromEntries` and `is`.

use super::*;

pub(super) fn register_statics(vm: &mut Vm) -> (JsValue, JsValue, JsValue, JsValue, JsValue, JsValue, JsValue) {
    let get_prototype_of_fn = vm.alloc_native_function("getPrototypeOf", 1, |vm, _this, args| {
        let target = match args.first() {
            Some(t) if !t.is_nullish() => *t,
            _ => return Err(VmError::TypeError("Cannot convert undefined or null to object".into())),
        };
        if let Some(ptr) = target.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            Ok(obj.prototype.unwrap_or(JsValue::null()))
        } else if target.is_string() {
            let string_ctor = vm.globals.get("String").copied().unwrap_or(JsValue::undefined());
            Ok(vm.get_property(string_ctor, "prototype"))
        } else if target.is_number() {
            let num_ctor = vm.globals.get("Number").copied().unwrap_or(JsValue::undefined());
            Ok(vm.get_property(num_ctor, "prototype"))
        } else if target.is_boolean() {
            let bool_ctor = vm.globals.get("Boolean").copied().unwrap_or(JsValue::undefined());
            Ok(vm.get_property(bool_ctor, "prototype"))
        } else {
            Ok(JsValue::null())
        }
    });

    let set_prototype_of_fn = vm.alloc_native_function("setPrototypeOf", 2, |vm, _this, args| {
        let target = match args.first() {
            Some(t) if !t.is_nullish() => *t,
            _ => return Err(VmError::TypeError("Cannot convert undefined or null to object".into())),
        };
        let proto = args.get(1).copied().unwrap_or(JsValue::undefined());
        if !proto.is_object() && !proto.is_null() {
            return Err(VmError::TypeError("Object prototype may only be an Object or null".into()));
        }
        if let Some(ptr) = target.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if vm.module_namespace_objects.contains(&target) {
                if proto.is_null() {
                    return Ok(target);
                }
                return Err(VmError::TypeError("Cannot set prototype of module namespace object".into()));
            }
            if vm.non_extensible_objects.contains(&target) {
                return Err(VmError::TypeError("Cannot set prototype of non-extensible object".into()));
            }
            obj.prototype = if proto.is_null() { None } else { Some(proto) };
        }
        Ok(target)
    });

    let has_own_fn = vm.alloc_native_function("hasOwn", 2, |vm, _this, args| {
        let target = match args.first() {
            Some(t) if !t.is_nullish() => *t,
            _ => return Err(VmError::TypeError("Object.hasOwn called on null or undefined".into())),
        };
        let prop = args.get(1).copied().unwrap_or(JsValue::undefined());
        let prop_key = vm.to_property_key(prop)?;
        Ok(JsValue::from_boolean(vm.has_own_property_key(target, &prop_key)))
    });

    let from_entries_fn = vm.alloc_native_function("fromEntries", 1, |vm, _this, args| {
        let iterable = args.first().copied().unwrap_or(JsValue::undefined());
        if iterable.is_nullish() {
            return Err(VmError::TypeError("Object.fromEntries requires an iterable".into()));
        }
        let items = iterate_to_vec(vm, iterable)?;
        let obj = vm.alloc_object();
        for item in items {
            if !item.is_object() {
                return Err(VmError::TypeError("Iterator value is not an entry object".into()));
            }
            let k = vm.get_property(item, "0");
            let v = vm.get_property(item, "1");
            let key = vm.to_property_key(k)?;
            match key {
                PropertyKeyVal::String(s) => vm.set_property(obj, &s, v),
                PropertyKeyVal::Symbol(sym) => vm.set_symbol_property(obj, sym, v),
            }
        }
        Ok(obj)
    });

    let is_fn = vm.alloc_native_function("is", 2, |vm, _this, args| {
        let a = args.first().copied().unwrap_or(JsValue::undefined());
        let b = args.get(1).copied().unwrap_or(JsValue::undefined());
        if a.is_number() && b.is_number() {
            let na = a.to_number().unwrap();
            let nb = b.to_number().unwrap();
            if na.is_nan() && nb.is_nan() {
                return Ok(JsValue::from_boolean(true));
            }
            if na == 0.0 && nb == 0.0 {
                let a_neg = na.to_bits() == (-0.0f64).to_bits();
                let b_neg = nb.to_bits() == (-0.0f64).to_bits();
                return Ok(JsValue::from_boolean(a_neg == b_neg));
            }
            return Ok(JsValue::from_boolean(na == nb));
        }
        Ok(JsValue::from_boolean(vm.strict_equal(a, b)))
    });

    let define_properties_fn = vm.alloc_native_function("defineProperties", 2, |vm, _this, args| {
        let target = match args.first() {
            Some(t) if t.is_object() => *t,
            _ => return Err(VmError::TypeError("Object.defineProperties called on non-object".into())),
        };
        let props = match args.get(1) {
            Some(p) if !p.is_nullish() => *p,
            _ => return Err(VmError::TypeError("Cannot convert undefined or null to object".into())),
        };
        if !props.is_object() {
            return Err(VmError::TypeError("Property description must be an object".into()));
        }
        let define_prop_fn = vm.get_property(vm.globals.get("Object").copied().unwrap_or(JsValue::undefined()), "defineProperty");
        let keys = vm.get_for_in_keys(props);
        for key in keys {
            let desc = vm.get_property(props, &key);
            let key_val = vm.alloc_string(key);
            vm.call(define_prop_fn, JsValue::undefined(), &[target, key_val, desc])?;
        }
        Ok(target)
    });

    let create_fn = vm.alloc_native_function("create", 2, |vm, _this, args| {
        let proto = args.first().copied().unwrap_or(JsValue::undefined());
        if !proto.is_object() && !proto.is_null() {
            return Err(VmError::TypeError("Object prototype may only be an Object or null".into()));
        }
        let obj = vm.alloc_object();
        if let Some(ptr) = obj.as_object_ptr() {
            let jsobj = unsafe { &mut *(ptr as *mut JsObject) };
            jsobj.prototype = if proto.is_null() { None } else { Some(proto) };
        }
        if let Some(props) = args.get(1) {
            if !props.is_undefined() {
                if !props.is_object() {
                    return Err(VmError::TypeError("Property description must be an object".into()));
                }
                let define_props_fn = vm.get_property(vm.globals.get("Object").copied().unwrap_or(JsValue::undefined()), "defineProperties");
                if vm.is_callable(define_props_fn) {
                    vm.call(define_props_fn, JsValue::undefined(), &[obj, *props])?;
                }
            }
        }
        Ok(obj)
    });

    (get_prototype_of_fn, set_prototype_of_fn, has_own_fn, from_entries_fn, is_fn, create_fn, define_properties_fn)
}
