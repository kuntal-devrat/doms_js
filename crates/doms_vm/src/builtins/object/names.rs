//! `Object.getOwnPropertyNames` and `Object.getOwnPropertySymbols`.

use super::*;

/// Returns all own property keys of `target` in ECMAScript specified order:
/// 1. Array index strings in ascending numeric order
/// 2. Other string properties in insertion order
/// 3. Symbols in insertion order
/// If `target` is a Proxy, the `ownKeys` trap is invoked.
pub fn get_own_property_keys(vm: &mut Vm, target: JsValue) -> Result<Vec<PropertyKeyVal>, VmError> {
    if let Some(ptr) = target.as_object_ptr() {
        let obj = unsafe { &*(ptr as *mut JsObject) };
        if let ObjectKind::Proxy { target: inner_target, handler } = &obj.kind {
            let inner_target = *inner_target;
            let handler = *handler;
            let trap = vm.get_property(handler, "ownKeys");
            if vm.is_callable(trap) {
                let trap_res = vm.call(trap, handler, &[inner_target])?;
                if !trap_res.is_object() {
                    return Err(VmError::TypeError("Proxy ownKeys trap must return an object".into()));
                }
                let len = vm.get_property(trap_res, "length").to_number().unwrap_or(0.0) as usize;
                let mut keys = Vec::with_capacity(len);
                for i in 0..len {
                    let elem = vm.get_element(trap_res, i);
                    let key = vm.to_property_key(elem)?;
                    keys.push(key);
                }
                for (t, prop) in &vm.non_configurable_props {
                    if *t == inner_target && !prop.starts_with("[Symbol:") {
                        let prop_key = PropertyKeyVal::String(prop.clone());
                        if !keys.contains(&prop_key) {
                            return Err(VmError::TypeError("Proxy ownKeys must include all non-configurable properties of the target".into()));
                        }
                    }
                }
                if vm.non_extensible_objects.contains(&inner_target) {
                    let target_keys = get_own_property_keys(vm, inner_target)?;
                    for tk in &target_keys {
                        if !keys.contains(tk) {
                            return Err(VmError::TypeError("Proxy ownKeys on non-extensible target must report all properties".into()));
                        }
                    }
                    if keys.len() != target_keys.len() {
                        return Err(VmError::TypeError("Proxy ownKeys on non-extensible target cannot report extra properties".into()));
                    }
                }
                return Ok(keys);
            } else {
                return get_own_property_keys(vm, inner_target);
            }
        }
    }

    let mut raw_names: Vec<String> = Vec::new();
    if let Some(ptr) = target.as_object_ptr() {
        let obj = unsafe { &*(ptr as *mut JsObject) };
        match &obj.kind {
            ObjectKind::Array(elements) => {
                for i in 0..elements.len() {
                    if !elements[i].is_hole() {
                        raw_names.push(i.to_string());
                    }
                }
                raw_names.push("length".to_string());
            }
            ObjectKind::String(s) | ObjectKind::StringObject(s) => {
                for i in 0..s.chars().count() {
                    raw_names.push(i.to_string());
                }
                raw_names.push("length".to_string());
            }
            ObjectKind::TypedArray(ta) => {
                for i in 0..ta.length {
                    raw_names.push(i.to_string());
                }
            }
            ObjectKind::Function(_) | ObjectKind::NativeFunction(_) => {
                raw_names.push("length".to_string());
                raw_names.push("name".to_string());
                if matches!(&obj.kind, ObjectKind::Function(_)) {
                    raw_names.push("prototype".to_string());
                }
            }
            _ => {}
        }
        if let Some(order) = &obj.prop_order {
            for k in order {
                if !raw_names.contains(k) {
                    raw_names.push(k.clone());
                }
            }
        }
        for prop in vm.shapes.all_properties(obj.shape_id) {
            if !raw_names.contains(&prop) {
                raw_names.push(prop);
            }
        }
        if let Some(accs) = &obj.accessors {
            for k in accs.keys() {
                if !raw_names.contains(k) {
                    raw_names.push(k.clone());
                }
            }
        }
        for ((ns, re_name), _) in &vm.module_reexports {
            if *ns == target && !raw_names.contains(re_name) {
                raw_names.push(re_name.clone());
            }
        }
        raw_names.retain(|p| !vm.deleted_props.contains(&(target, p.clone())));
    }
    if vm.module_namespace_objects.contains(&target) {
        raw_names.sort();
        let out = raw_names.into_iter().map(PropertyKeyVal::String).collect::<Vec<_>>();
        return Ok(out);
    }
    let mut int_keys: Vec<(u32, String)> = Vec::new();
    let mut str_keys: Vec<String> = Vec::new();
    for k in raw_names {
        if let Ok(idx) = k.parse::<u32>() {
            if idx < u32::MAX && idx.to_string() == k {
                int_keys.push((idx, k));
                continue;
            }
        }
        str_keys.push(k);
    }
    int_keys.sort_by_key(|(idx, _)| *idx);
    let mut out_keys = Vec::new();
    for (_, k) in int_keys {
        out_keys.push(PropertyKeyVal::String(k));
    }
    for k in str_keys {
        out_keys.push(PropertyKeyVal::String(k));
    }

    if let Some(ptr) = target.as_object_ptr() {
        let obj = unsafe { &*(ptr as *mut JsObject) };
        if let Some(order) = &obj.symbol_order {
            for k in order {
                let pkv = PropertyKeyVal::Symbol(*k);
                if !out_keys.contains(&pkv) {
                    out_keys.push(pkv);
                }
            }
        } else {
            if let Some(sym_accs) = &obj.symbol_accessors {
                for k in sym_accs.keys() {
                    let pkv = PropertyKeyVal::Symbol(*k);
                    if !out_keys.contains(&pkv) {
                        out_keys.push(pkv);
                    }
                }
            }
            if let Some(syms) = &obj.symbol_properties {
                for k in syms.keys() {
                    let pkv = PropertyKeyVal::Symbol(*k);
                    if !out_keys.contains(&pkv) {
                        out_keys.push(pkv);
                    }
                }
            }
        }
    }
    Ok(out_keys)
}

pub(super) fn register_names_and_symbols(vm: &mut Vm) -> (JsValue, JsValue) {
    let get_own_prop_names_fn = vm.alloc_native_function("getOwnPropertyNames", 1, |vm, _this, args| {
        let target = match args.first() {
            Some(t) if !t.is_nullish() => *t,
            _ => return Err(VmError::TypeError("Cannot convert undefined or null to object".into())),
        };
        let target_obj = if target.is_object() {
            target
        } else {
            vm.to_object_value(target)?
        };
        let keys = get_own_property_keys(vm, target_obj)?;
        let mut names = Vec::new();
        for key in keys {
            if let PropertyKeyVal::String(s) = key {
                names.push(vm.alloc_string(s));
            }
        }
        Ok(vm.alloc_array(names))
    });

    let get_own_prop_symbols_fn = vm.alloc_native_function("getOwnPropertySymbols", 1, |vm, _this, args| {
        let target = match args.first() {
            Some(t) if !t.is_nullish() => *t,
            _ => return Err(VmError::TypeError("Cannot convert undefined or null to object".into())),
        };
        let target_obj = if target.is_object() {
            target
        } else {
            vm.to_object_value(target)?
        };
        let keys = get_own_property_keys(vm, target_obj)?;
        let mut symbols = Vec::new();
        for key in keys {
            if let PropertyKeyVal::Symbol(sym) = key {
                symbols.push(sym);
            }
        }
        Ok(vm.alloc_array(symbols))
    });

    (get_own_prop_names_fn, get_own_prop_symbols_fn)
}

