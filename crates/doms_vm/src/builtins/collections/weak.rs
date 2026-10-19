//! `WeakRef`, `FinalizationRegistry`, `WeakMap` and `WeakSet`.

use super::*;

pub(super) fn register_weak_builtins(vm: &mut Vm) {
    // WeakRef & FinalizationRegistry
    let weakref_ctor = vm.alloc_native_function("WeakRef", 1, |vm, _this, args| {
        let target = args.first().copied().unwrap_or(JsValue::undefined());
        Ok(vm.alloc_weak_ref(target))
    });
    vm.globals.insert("WeakRef".to_string(), weakref_ctor);

    let fr_ctor = vm.alloc_native_function("FinalizationRegistry", 1, |vm, _this, args| {
        let cb = args.first().copied().unwrap_or(JsValue::undefined());
        Ok(vm.alloc_finalization_registry(cb))
    });
    vm.globals.insert("FinalizationRegistry".to_string(), fr_ctor);

    // WeakMap
    let wm_proto = vm.alloc_object();

    let wm_get = vm.alloc_native_function("get", 1, |_vm, this, args| {
        let key = args.first().copied().unwrap_or(JsValue::undefined());
        if !key.is_object() {
            return Ok(JsValue::undefined());
        }
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::WeakMap(entries) = &obj.kind {
                for (k, v) in entries {
                    if *k == key {
                        return Ok(*v);
                    }
                }
                return Ok(JsValue::undefined());
            }
        }
        Err(VmError::TypeError("Method WeakMap.prototype.get called on incompatible receiver".into()))
    });

    let wm_set = vm.alloc_native_function("set", 2, |_vm, this, args| {
        let key = args.first().copied().unwrap_or(JsValue::undefined());
        if !key.is_object() {
            return Err(VmError::TypeError("Invalid value used as weak map key".into()));
        }
        let val = args.get(1).copied().unwrap_or(JsValue::undefined());
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::WeakMap(entries) = &mut obj.kind {
                for entry in entries.iter_mut() {
                    if entry.0 == key {
                        entry.1 = val;
                        return Ok(this);
                    }
                }
                entries.push((key, val));
                return Ok(this);
            }
        }
        Err(VmError::TypeError("Method WeakMap.prototype.set called on incompatible receiver".into()))
    });

    let wm_has = vm.alloc_native_function("has", 1, |_vm, this, args| {
        let key = args.first().copied().unwrap_or(JsValue::undefined());
        if !key.is_object() {
            return Ok(JsValue::from_boolean(false));
        }
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::WeakMap(entries) = &obj.kind {
                for (k, _) in entries {
                    if *k == key {
                        return Ok(JsValue::from_boolean(true));
                    }
                }
                return Ok(JsValue::from_boolean(false));
            }
        }
        Err(VmError::TypeError("Method WeakMap.prototype.has called on incompatible receiver".into()))
    });

    let wm_delete = vm.alloc_native_function("delete", 1, |_vm, this, args| {
        let key = args.first().copied().unwrap_or(JsValue::undefined());
        if !key.is_object() {
            return Ok(JsValue::from_boolean(false));
        }
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::WeakMap(entries) = &mut obj.kind {
                if let Some(idx) = entries.iter().position(|(k, _)| *k == key) {
                    entries.remove(idx);
                    return Ok(JsValue::from_boolean(true));
                }
                return Ok(JsValue::from_boolean(false));
            }
        }
        Err(VmError::TypeError("Method WeakMap.prototype.delete called on incompatible receiver".into()))
    });

    vm.set_property(wm_proto, "get", wm_get);
    vm.set_property_non_enumerable(wm_proto, "get".to_string());
    vm.set_property(wm_proto, "set", wm_set);
    vm.set_property_non_enumerable(wm_proto, "set".to_string());
    vm.set_property(wm_proto, "has", wm_has);
    vm.set_property_non_enumerable(wm_proto, "has".to_string());
    vm.set_property(wm_proto, "delete", wm_delete);
    vm.set_property_non_enumerable(wm_proto, "delete".to_string());
    if let Some(sym_tag) = vm.symbol_to_string_tag {
        let tag_str = vm.alloc_string("WeakMap".into());
        vm.set_symbol_property(wm_proto, sym_tag, tag_str);
    }

    let weakmap_ctor = vm.alloc_native_function("WeakMap", 0, |vm, this, args| {
        if !vm.is_constructing {
            return Err(VmError::TypeError("Constructor WeakMap requires 'new'".into()));
        }
        let wm_val = if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if matches!(obj.kind, ObjectKind::Ordinary) {
                obj.kind = ObjectKind::WeakMap(Vec::new());
                this
            } else {
                vm.alloc_weak_map()
            }
        } else {
            vm.alloc_weak_map()
        };

        let prev_temp = vm.temp_roots.len();
        vm.temp_roots.push(wm_val);

        if let Some(arg) = args.first() {
            if !arg.is_nullish() {
                let cur_wm = vm.temp_roots[prev_temp];
                let adder = vm.get_property(cur_wm, "set");
                if !vm.is_callable(adder) {
                    vm.temp_roots.truncate(prev_temp);
                    return Err(VmError::TypeError("WeakMap.prototype.set is not callable".into()));
                }
                vm.temp_roots.push(adder);
                let arg_val = *arg;
                let items = iterate_to_vec(vm, arg_val)?;
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
                    let cur_wm = vm.temp_roots[prev_temp];
                    let cur_adder = vm.temp_roots[prev_temp + 1];
                    vm.call(cur_adder, cur_wm, &[k_val, v_val])?;
                    vm.temp_roots.truncate(items_start + num_items);
                }
                vm.temp_roots.truncate(prev_temp + 1);
            }
        }
        let res = vm.temp_roots[prev_temp];
        vm.temp_roots.truncate(prev_temp);
        Ok(res)
    });
    vm.set_property(weakmap_ctor, "prototype", wm_proto);
    vm.set_property_non_enumerable(weakmap_ctor, "prototype".to_string());
    vm.set_property_non_writable(weakmap_ctor, "prototype".to_string());
    vm.set_property_non_configurable(weakmap_ctor, "prototype".to_string());
    vm.set_property(wm_proto, "constructor", weakmap_ctor);
    vm.set_property_non_enumerable(wm_proto, "constructor".to_string());
    vm.globals.insert("WeakMap".to_string(), weakmap_ctor);

    // WeakSet
    let ws_proto = vm.alloc_object();

    let ws_add = vm.alloc_native_function("add", 1, |_vm, this, args| {
        let val = args.first().copied().unwrap_or(JsValue::undefined());
        if !val.is_object() {
            return Err(VmError::TypeError("Invalid value used in weak set".into()));
        }
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::WeakSet(elements) = &mut obj.kind {
                if !elements.contains(&val) {
                    elements.push(val);
                }
                return Ok(this);
            }
        }
        Err(VmError::TypeError("Method WeakSet.prototype.add called on incompatible receiver".into()))
    });

    let ws_has = vm.alloc_native_function("has", 1, |_vm, this, args| {
        let val = args.first().copied().unwrap_or(JsValue::undefined());
        if !val.is_object() {
            return Ok(JsValue::from_boolean(false));
        }
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::WeakSet(elements) = &obj.kind {
                return Ok(JsValue::from_boolean(elements.contains(&val)));
            }
        }
        Err(VmError::TypeError("Method WeakSet.prototype.has called on incompatible receiver".into()))
    });

    let ws_delete = vm.alloc_native_function("delete", 1, |_vm, this, args| {
        let val = args.first().copied().unwrap_or(JsValue::undefined());
        if !val.is_object() {
            return Ok(JsValue::from_boolean(false));
        }
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::WeakSet(elements) = &mut obj.kind {
                if let Some(idx) = elements.iter().position(|el| *el == val) {
                    elements.remove(idx);
                    return Ok(JsValue::from_boolean(true));
                }
                return Ok(JsValue::from_boolean(false));
            }
        }
        Err(VmError::TypeError("Method WeakSet.prototype.delete called on incompatible receiver".into()))
    });

    vm.set_property(ws_proto, "add", ws_add);
    vm.set_property_non_enumerable(ws_proto, "add".to_string());
    vm.set_property(ws_proto, "has", ws_has);
    vm.set_property_non_enumerable(ws_proto, "has".to_string());
    vm.set_property(ws_proto, "delete", ws_delete);
    vm.set_property_non_enumerable(ws_proto, "delete".to_string());
    if let Some(sym_tag) = vm.symbol_to_string_tag {
        let tag_str = vm.alloc_string("WeakSet".into());
        vm.set_symbol_property(ws_proto, sym_tag, tag_str);
    }

    let weakset_ctor = vm.alloc_native_function("WeakSet", 0, |vm, this, args| {
        if !vm.is_constructing {
            return Err(VmError::TypeError("Constructor WeakSet requires 'new'".into()));
        }
        let ws_val = if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if matches!(obj.kind, ObjectKind::Ordinary) {
                obj.kind = ObjectKind::WeakSet(Vec::new());
                this
            } else {
                vm.alloc_weak_set()
            }
        } else {
            vm.alloc_weak_set()
        };

        let prev_temp = vm.temp_roots.len();
        vm.temp_roots.push(ws_val);

        if let Some(arg) = args.first() {
            if !arg.is_nullish() {
                let cur_ws = vm.temp_roots[prev_temp];
                let adder = vm.get_property(cur_ws, "add");
                if !vm.is_callable(adder) {
                    vm.temp_roots.truncate(prev_temp);
                    return Err(VmError::TypeError("WeakSet.prototype.add is not callable".into()));
                }
                vm.temp_roots.push(adder);
                let arg_val = *arg;
                let items = iterate_to_vec(vm, arg_val)?;
                let items_start = vm.temp_roots.len();
                for item in items {
                    vm.temp_roots.push(item);
                }
                let num_items = vm.temp_roots.len() - items_start;
                for i in 0..num_items {
                    let cur_item = vm.temp_roots[items_start + i];
                    let cur_ws = vm.temp_roots[prev_temp];
                    let cur_adder = vm.temp_roots[prev_temp + 1];
                    vm.call(cur_adder, cur_ws, &[cur_item])?;
                }
                vm.temp_roots.truncate(prev_temp + 1);
            }
        }
        let res = vm.temp_roots[prev_temp];
        vm.temp_roots.truncate(prev_temp);
        Ok(res)
    });
    vm.set_property(weakset_ctor, "prototype", ws_proto);
    vm.set_property_non_enumerable(weakset_ctor, "prototype".to_string());
    vm.set_property_non_writable(weakset_ctor, "prototype".to_string());
    vm.set_property_non_configurable(weakset_ctor, "prototype".to_string());
    vm.set_property(ws_proto, "constructor", weakset_ctor);
    vm.set_property_non_enumerable(ws_proto, "constructor".to_string());
    vm.globals.insert("WeakSet".to_string(), weakset_ctor);

}
