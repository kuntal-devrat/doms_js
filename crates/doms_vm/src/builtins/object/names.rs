//! `Object.getOwnPropertyNames` and `Object.getOwnPropertySymbols`.

use super::*;

pub(super) fn register_names_and_symbols(vm: &mut Vm) -> (JsValue, JsValue) {
    let get_own_prop_names_fn = vm.alloc_native_function("getOwnPropertyNames", 1, |vm, _this, args| {
        let target = match args.first() {
            Some(t) if t.is_object() => *t,
            _ => return Err(VmError::TypeError("Object.getOwnPropertyNames called on non-object".into())),
        };
        let mut raw_names: Vec<String> = Vec::new();
        if let Some(ptr) = target.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            match &obj.kind {
                ObjectKind::Array(elements) => {
                    for i in 0..elements.len() {
                        raw_names.push(i.to_string());
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
                    raw_names.push("length".to_string());
                    raw_names.push("byteLength".to_string());
                    raw_names.push("byteOffset".to_string());
                    raw_names.push("buffer".to_string());
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
            if let Some(accs) = &obj.accessors {
                for k in accs.keys() {
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
            for ((ns, re_name), _) in &vm.module_reexports {
                if *ns == target && !raw_names.contains(re_name) {
                    raw_names.push(re_name.clone());
                }
            }
            raw_names.retain(|p| !vm.deleted_props.contains(&(target, p.clone())));
        }
        if vm.module_namespace_objects.contains(&target) {
            raw_names.sort();
            let names = raw_names.into_iter().map(|k| vm.alloc_string(k)).collect();
            return Ok(vm.alloc_array(names));
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
        let mut names = Vec::new();
        for (_, k) in int_keys {
            names.push(vm.alloc_string(k));
        }
        for k in str_keys {
            names.push(vm.alloc_string(k));
        }
        Ok(vm.alloc_array(names))
    });

    let get_own_prop_symbols_fn = vm.alloc_native_function("getOwnPropertySymbols", 1, |vm, _this, args| {
        let target = match args.first() {
            Some(t) if t.is_object() => *t,
            _ => return Err(VmError::TypeError("Object.getOwnPropertySymbols called on non-object".into())),
        };
        let mut symbols = Vec::new();
        if let Some(ptr) = target.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let Some(order) = &obj.symbol_order {
                for k in order {
                    if !symbols.contains(k) {
                        symbols.push(*k);
                    }
                }
            } else {
                if let Some(sym_accs) = &obj.symbol_accessors {
                    for k in sym_accs.keys() {
                        if !symbols.contains(k) {
                            symbols.push(*k);
                        }
                    }
                }
                if let Some(syms) = &obj.symbol_properties {
                    for k in syms.keys() {
                        if !symbols.contains(k) {
                            symbols.push(*k);
                        }
                    }
                }
            }
        }
        Ok(vm.alloc_array(symbols))
    });
    (get_own_prop_names_fn, get_own_prop_symbols_fn)
}
