//! Prototype- and object-level statics: `getPrototypeOf`, `setPrototypeOf`,
//! `hasOwn`, `fromEntries` and `is`.

use super::*;

pub(super) fn register_statics(
    vm: &mut Vm,
) -> (JsValue, JsValue, JsValue, JsValue, JsValue, JsValue, JsValue) {
    let get_prototype_of_fn = vm.alloc_native_function("getPrototypeOf", 1, |vm, _this, args| {
        let target = match args.first() {
            Some(t) if !t.is_nullish() => *t,
            _ => {
                return Err(VmError::TypeError(
                    "Cannot convert undefined or null to object".into(),
                ));
            }
        };
        vm.get_prototype_of_val(target)
    });

    let set_prototype_of_fn = vm.alloc_native_function("setPrototypeOf", 2, |vm, _this, args| {
        let target = match args.first() {
            Some(t) if !t.is_nullish() => *t,
            _ => {
                return Err(VmError::TypeError(
                    "Cannot convert undefined or null to object".into(),
                ));
            }
        };
        let proto = args.get(1).copied().unwrap_or(JsValue::undefined());
        if !proto.is_object() && !proto.is_null() {
            return Err(VmError::TypeError(
                "Object prototype may only be an Object or null".into(),
            ));
        }
        if !target.is_object() {
            return Ok(target);
        }
        if vm.module_namespace_objects.contains(&target) {
            if proto.is_null() {
                return Ok(target);
            }
            return Err(VmError::TypeError(
                "Cannot set prototype of module namespace object".into(),
            ));
        }
        let success = vm.set_prototype_of_val(target, proto)?;
        if !success {
            return Err(VmError::TypeError("Cannot set prototype of object".into()));
        }
        Ok(target)
    });

    let has_own_fn = vm.alloc_native_function("hasOwn", 2, |vm, _this, args| {
        let target = match args.first() {
            Some(t) if !t.is_nullish() => *t,
            _ => {
                return Err(VmError::TypeError("Object.hasOwn called on null or undefined".into()));
            }
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
        let prev_temp = vm.temp_roots.len();
        vm.temp_roots.push(iterable);
        let items = iterate_to_vec(vm, vm.temp_roots[prev_temp])?;
        let obj = vm.alloc_object();
        vm.temp_roots[prev_temp] = obj;
        let items_start = vm.temp_roots.len();
        for item in items {
            vm.temp_roots.push(item);
        }
        let num_items = vm.temp_roots.len() - items_start;
        for i in 0..num_items {
            let cur_item = vm.temp_roots[items_start + i];
            if !cur_item.is_object() {
                vm.temp_roots.truncate(prev_temp);
                return Err(VmError::TypeError("Iterator value is not an entry object".into()));
            }
            let k = vm.get_property(cur_item, "0");
            vm.temp_roots.push(k);
            let cur_item = vm.temp_roots[items_start + i];
            let v = vm.get_property(cur_item, "1");
            vm.temp_roots.push(v);
            let k_val = vm.temp_roots[vm.temp_roots.len() - 2];
            let v_val = vm.temp_roots[vm.temp_roots.len() - 1];
            let key = vm.to_property_key(k_val)?;
            let cur_obj = vm.temp_roots[prev_temp];
            match key {
                PropertyKeyVal::String(s) => vm.set_property(cur_obj, &s, v_val),
                PropertyKeyVal::Symbol(sym) => vm.set_symbol_property(cur_obj, sym, v_val),
            }
            vm.temp_roots.truncate(items_start + num_items);
        }
        let res = vm.temp_roots[prev_temp];
        vm.temp_roots.truncate(prev_temp);
        Ok(res)
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

    let define_properties_fn =
        vm.alloc_native_function("defineProperties", 2, |vm, _this, args| {
            let target = match args.first() {
                Some(t) if t.is_js_object() => *t,
                _ => {
                    return Err(VmError::TypeError(
                        "Object.defineProperties called on non-object".into(),
                    ));
                }
            };
            let props = match args.get(1) {
                Some(p) if !p.is_nullish() => *p,
                _ => {
                    return Err(VmError::TypeError(
                        "Cannot convert undefined or null to object".into(),
                    ));
                }
            };
            let props_obj = if props.is_js_object() { props } else { vm.to_object_value(props)? };

            // Pass 1: Collect own enumerable properties and descriptors
            let keys = super::names::get_own_property_keys(vm, props_obj)?;
            let mut descriptors = Vec::new();
            for key in keys {
                let desc =
                    super::descriptor::get_own_property_descriptor_internal(vm, props_obj, &key)?;
                if !desc.is_undefined() && vm.get_property(desc, "enumerable").is_truthy() {
                    let desc_val = match &key {
                        PropertyKeyVal::String(s) => vm.get_property(props_obj, s),
                        PropertyKeyVal::Symbol(sym) => vm.get_symbol_property(props_obj, *sym),
                    };
                    if !desc_val.is_js_object() {
                        return Err(VmError::TypeError(
                            "Property description must be an object".into(),
                        ));
                    }
                    descriptors.push((key, desc_val));
                }
            }

            // Pass 2: Define each property
            for (key, desc) in descriptors {
                super::define_property::define_property_internal(vm, target, key, desc)?;
            }

            Ok(target)
        });

    let create_fn = vm.alloc_native_function("create", 2, |vm, _this, args| {
        let proto = args.first().copied().unwrap_or(JsValue::undefined());
        if !proto.is_js_object() && !proto.is_null() {
            return Err(VmError::TypeError(
                "Object prototype may only be an Object or null".into(),
            ));
        }
        let obj = vm.alloc_object();
        if let Some(ptr) = obj.as_object_ptr() {
            let jsobj = unsafe { &mut *(ptr as *mut JsObject) };
            jsobj.prototype = if proto.is_null() { None } else { Some(proto) };
        }
        if let Some(props) = args.get(1) {
            if !props.is_undefined() {
                let define_props_fn = vm.get_property(
                    vm.globals.get("Object").copied().unwrap_or(JsValue::undefined()),
                    "defineProperties",
                );
                if vm.is_callable(define_props_fn) {
                    vm.call(define_props_fn, JsValue::undefined(), &[obj, *props])?;
                }
            }
        }
        Ok(obj)
    });

    (
        get_prototype_of_fn,
        set_prototype_of_fn,
        has_own_fn,
        from_entries_fn,
        is_fn,
        create_fn,
        define_properties_fn,
    )
}
