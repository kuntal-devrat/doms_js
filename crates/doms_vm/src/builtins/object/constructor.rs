//! The `Object` constructor, its core statics and the creation of
//! `Object.prototype`.

use super::*;

pub(super) fn register_constructor(
    vm: &mut Vm,
) -> (JsValue, JsValue, JsValue, JsValue, JsValue, JsValue) {
    let object_constructor = vm.alloc_native_function("Object", 1, |vm, _this, args| {
        if let Some(arg) = args.first() {
            if !arg.is_nullish() {
                return vm.to_object_value(*arg);
            }
        }
        Ok(vm.alloc_object())
    });

    let keys_fn = vm.alloc_native_function("keys", 1, |vm, _this, args| {
        let arg = match args.first() {
            Some(a) if a.is_object() => *a,
            _ => return Err(VmError::TypeError("Object.keys called on non-object".into())),
        };
        let keys_to_alloc = get_enumerable_keys(vm, arg);
        if vm.module_namespace_objects.contains(&arg) {
            for k in &keys_to_alloc {
                if vm.get_property(arg, k) == JsValue::uninitialized() {
                    return Err(VmError::ReferenceError(format!("Cannot access uninitialized export '{}'", k)));
                }
            }
        }
        let keys_vec: Vec<JsValue> = keys_to_alloc.into_iter().map(|k| vm.alloc_string(k)).collect();
        Ok(vm.alloc_array(keys_vec))
    });

    let values_fn = vm.alloc_native_function("values", 1, |vm, _this, args| {
        let arg = match args.first() {
            Some(a) if a.is_object() => *a,
            _ => return Err(VmError::TypeError("Object.values called on non-object".into())),
        };
        let keys_to_alloc = get_enumerable_keys(vm, arg);
        if vm.module_namespace_objects.contains(&arg) {
            for k in &keys_to_alloc {
                if vm.get_property(arg, k) == JsValue::uninitialized() {
                    return Err(VmError::ReferenceError(format!("Cannot access uninitialized export '{}'", k)));
                }
            }
        }
        let vals_vec: Vec<JsValue> = keys_to_alloc.into_iter().map(|k| vm.get_property(arg, &k)).collect();
        Ok(vm.alloc_array(vals_vec))
    });

    let entries_fn = vm.alloc_native_function("entries", 1, |vm, _this, args| {
        let arg = match args.first() {
            Some(a) if a.is_object() => *a,
            _ => return Err(VmError::TypeError("Object.entries called on non-object".into())),
        };
        let keys_to_alloc = get_enumerable_keys(vm, arg);
        if vm.module_namespace_objects.contains(&arg) {
            for k in &keys_to_alloc {
                if vm.get_property(arg, k) == JsValue::uninitialized() {
                    return Err(VmError::ReferenceError(format!("Cannot access uninitialized export '{}'", k)));
                }
            }
        }
        let entries_vec: Vec<JsValue> = keys_to_alloc
            .into_iter()
            .map(|k| {
                let v = vm.get_property(arg, &k);
                let k_val = vm.alloc_string(k);
                vm.alloc_array(vec![k_val, v])
            })
            .collect();
        Ok(vm.alloc_array(entries_vec))
    });

    let assign_fn = vm.alloc_native_function("assign", 2, |vm, _this, args| {
        let target = if let Some(t) = args.first() {
            *t
        } else {
            return Err(VmError::TypeError("Cannot convert undefined or null to object".into()));
        };

        for source in args.iter().skip(1) {
            if let Some(src_ptr) = source.as_object_ptr() {
                let src_obj = unsafe { &*(src_ptr as *mut JsObject) };
                let props: Vec<(String, JsValue)> = vm
                    .shapes
                    .all_property_entries(src_obj.shape_id)
                    .into_iter()
                    .filter_map(|(k, offset)| src_obj.get_slot(offset).map(|v| (k, v)))
                    .collect();

                for (k, v) in props {
                    vm.set_property(target, &k, v);
                }
            }
        }
        Ok(target)
    });

    let object_proto = vm.alloc_object();
    vm.object_proto = Some(object_proto);
    if let Some(ptr) = vm.global_object.as_object_ptr() {
        let obj = unsafe { &mut *(ptr as *mut JsObject) };
        obj.prototype = Some(object_proto);
    }

    vm.set_property(object_proto, "constructor", object_constructor);
    vm.non_enumerable_props.insert((object_proto, "constructor".to_string()));
    (object_proto, object_constructor, keys_fn, values_fn, entries_fn, assign_fn)
}
