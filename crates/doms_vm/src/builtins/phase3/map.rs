//! The `Map` global and `Map.prototype`.

use super::*;

pub(super) fn register_map_builtins(vm: &mut Vm) {
    // ── Map ──
    let map_proto = vm.alloc_object();

    let map_get = vm.alloc_native_function("get", 1, |_vm, this, args| {
        let key = args.first().copied().unwrap_or(JsValue::undefined());
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::Map(entries) = &obj.kind {
                for entry in entries {
                    if let Some((k, v)) = entry {
                        if crate::object::js_values_equal(*k, key) {
                            return Ok(*v);
                        }
                    }
                }
                return Ok(JsValue::undefined());
            }
        }
        Err(VmError::TypeError("Method Map.prototype.get called on incompatible receiver".into()))
    });

    let map_set = vm.alloc_native_function("set", 2, |_vm, this, args| {
        let key = args.first().copied().unwrap_or(JsValue::undefined());
        let val = args.get(1).copied().unwrap_or(JsValue::undefined());
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::Map(entries) = &mut obj.kind {
                for entry in entries.iter_mut() {
                    if let Some((k, v)) = entry {
                        if crate::object::js_values_equal(*k, key) {
                            *v = val;
                            return Ok(this);
                        }
                    }
                }
                entries.push(Some((key, val)));
                return Ok(this);
            }
        }
        Err(VmError::TypeError("Method Map.prototype.set called on incompatible receiver".into()))
    });

    let map_has = vm.alloc_native_function("has", 1, |_vm, this, args| {
        let key = args.first().copied().unwrap_or(JsValue::undefined());
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::Map(entries) = &obj.kind {
                for entry in entries {
                    if let Some((k, _)) = entry {
                        if crate::object::js_values_equal(*k, key) {
                            return Ok(JsValue::from_boolean(true));
                        }
                    }
                }
                return Ok(JsValue::from_boolean(false));
            }
        }
        Err(VmError::TypeError("Method Map.prototype.has called on incompatible receiver".into()))
    });

    let map_delete = vm.alloc_native_function("delete", 1, |_vm, this, args| {
        let key = args.first().copied().unwrap_or(JsValue::undefined());
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::Map(entries) = &mut obj.kind {
                for entry in entries.iter_mut() {
                    if let Some((k, _)) = entry {
                        if crate::object::js_values_equal(*k, key) {
                            *entry = None;
                            return Ok(JsValue::from_boolean(true));
                        }
                    }
                }
                return Ok(JsValue::from_boolean(false));
            }
        }
        Err(VmError::TypeError("Method Map.prototype.delete called on incompatible receiver".into()))
    });

    let map_clear = vm.alloc_native_function("clear", 0, |_vm, this, _args| {
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::Map(entries) = &mut obj.kind {
                for entry in entries.iter_mut() {
                    *entry = None;
                }
                return Ok(JsValue::undefined());
            }
        }
        Err(VmError::TypeError("Method Map.prototype.clear called on incompatible receiver".into()))
    });

    let map_for_each = vm.alloc_native_function("forEach", 1, |vm, this, args| {
        let cb = args.first().copied().unwrap_or(JsValue::undefined());
        if !vm.is_callable(cb) {
            return Err(VmError::TypeError("Callback is not a function".into()));
        }
        let this_arg = args.get(1).copied().unwrap_or(JsValue::undefined());
        let mut idx = 0;
        loop {
            let entry = if let Some(ptr) = this.as_object_ptr() {
                let obj = unsafe { &*(ptr as *mut JsObject) };
                if let ObjectKind::Map(entries) = &obj.kind {
                    if idx < entries.len() {
                        entries[idx]
                    } else {
                        break;
                    }
                } else {
                    return Err(VmError::TypeError("Method Map.prototype.forEach called on incompatible receiver".into()));
                }
            } else {
                return Err(VmError::TypeError("Method Map.prototype.forEach called on incompatible receiver".into()));
            };
            idx += 1;
            if let Some((k, v)) = entry {
                let _ = vm.call(cb, this_arg, &[v, k, this])?;
            }
        }
        Ok(JsValue::undefined())
    });

    let map_entries = vm.alloc_native_function("entries", 0, |vm, this, _args| {
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::Map(_) = &obj.kind {
                return Ok(vm.alloc_map_iterator(this, MapIterationKind::KeyValue));
            }
        }
        Err(VmError::TypeError("Method Map.prototype.entries called on incompatible receiver".into()))
    });

    let map_keys = vm.alloc_native_function("keys", 0, |vm, this, _args| {
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::Map(_) = &obj.kind {
                return Ok(vm.alloc_map_iterator(this, MapIterationKind::Key));
            }
        }
        Err(VmError::TypeError("Method Map.prototype.keys called on incompatible receiver".into()))
    });

    let map_values = vm.alloc_native_function("values", 0, |vm, this, _args| {
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::Map(_) = &obj.kind {
                return Ok(vm.alloc_map_iterator(this, MapIterationKind::Value));
            }
        }
        Err(VmError::TypeError("Method Map.prototype.values called on incompatible receiver".into()))
    });

    let map_size_getter = vm.alloc_native_function("get size", 0, |_vm, this, _args| {
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::Map(entries) = &obj.kind {
                let count = entries.iter().filter(|e| e.is_some()).count();
                return Ok(JsValue::from_int32(count as i32));
            }
        }
        Err(VmError::TypeError("Method get Map.prototype.size called on incompatible receiver".into()))
    });

    vm.set_property(map_proto, "get", map_get);
    vm.non_enumerable_props.insert((map_proto, "get".to_string()));
    vm.set_property(map_proto, "set", map_set);
    vm.non_enumerable_props.insert((map_proto, "set".to_string()));
    vm.set_property(map_proto, "has", map_has);
    vm.non_enumerable_props.insert((map_proto, "has".to_string()));
    vm.set_property(map_proto, "delete", map_delete);
    vm.non_enumerable_props.insert((map_proto, "delete".to_string()));
    vm.set_property(map_proto, "clear", map_clear);
    vm.non_enumerable_props.insert((map_proto, "clear".to_string()));
    vm.set_property(map_proto, "forEach", map_for_each);
    vm.non_enumerable_props.insert((map_proto, "forEach".to_string()));
    vm.set_property(map_proto, "entries", map_entries);
    vm.non_enumerable_props.insert((map_proto, "entries".to_string()));
    vm.set_property(map_proto, "keys", map_keys);
    vm.non_enumerable_props.insert((map_proto, "keys".to_string()));
    vm.set_property(map_proto, "values", map_values);
    vm.non_enumerable_props.insert((map_proto, "values".to_string()));
    if let Some(ptr) = map_proto.as_object_ptr() {
        let obj = unsafe { &mut *(ptr as *mut JsObject) };
        obj.define_getter("size".to_string(), map_size_getter);
    }
    vm.non_enumerable_props.insert((map_proto, "size".to_string()));
    if let Some(sym_iter) = vm.symbol_iterator {
        vm.set_symbol_property(map_proto, sym_iter, map_entries);
    }
    if let Some(sym_tag) = vm.symbol_to_string_tag {
        let tag_str = vm.alloc_string("Map".into());
        vm.set_symbol_property(map_proto, sym_tag, tag_str);
    }

    let map_group_by = vm.alloc_native_function("groupBy", 2, |vm, _this, args| {
        let items = args.first().copied().unwrap_or(JsValue::undefined());
        let callback = args.get(1).copied().unwrap_or(JsValue::undefined());
        if !vm.is_callable(callback) {
            return Err(VmError::TypeError("Map.groupBy callback must be callable".into()));
        }
        let map_val = vm.alloc_map();
        let len_val = vm.get_property(items, "length");
        let raw_len = len_val.to_number().unwrap_or(0.0);
        if raw_len > 4_294_967_295.0 || raw_len < 0.0 || raw_len.is_infinite() {
            return Err(VmError::RangeError("Invalid array length".into()));
        }
        let len = raw_len as usize;
        if len > 100_000 {
            return Err(VmError::RangeError("Invalid array length".into()));
        }
        for i in 0..len {
            let item = vm.get_element(items, i);
            let key_val = vm.call(callback, JsValue::undefined(), &[item, JsValue::from_int32(i as i32)])?;
            let has_fn = vm.get_property(map_val, "has");
            let has_res = vm.call(has_fn, map_val, &[key_val]).unwrap_or_else(|_| JsValue::from_boolean(false));
            if has_res.is_truthy() {
                let get_fn = vm.get_property(map_val, "get");
                let arr = vm.call(get_fn, map_val, &[key_val])?;
                if let Some(ptr) = arr.as_object_ptr() {
                    let arr_obj = unsafe { &mut *(ptr as *mut JsObject) };
                    if let ObjectKind::Array(ref mut elems) = arr_obj.kind {
                        elems.push(item);
                    }
                }
            } else {
                let arr = vm.alloc_array(vec![item]);
                let set_fn = vm.get_property(map_val, "set");
                let _ = vm.call(set_fn, map_val, &[key_val, arr]);
            }
        }
        Ok(map_val)
    });

    let map_ctor = vm.alloc_native_function("Map", 0, |vm, this, args| {
        if !vm.is_constructing {
            return Err(VmError::TypeError("Constructor Map requires 'new'".into()));
        }
        let map_val = if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if matches!(obj.kind, ObjectKind::Ordinary) {
                obj.kind = ObjectKind::Map(Vec::new());
                this
            } else {
                vm.alloc_map()
            }
        } else {
            vm.alloc_map()
        };

        let prev_temp = vm.temp_roots.len();
        vm.temp_roots.push(map_val);

        if let Some(arg) = args.first() {
            if !arg.is_nullish() {
                let cur_map = vm.temp_roots[prev_temp];
                let adder = vm.get_property(cur_map, "set");
                if !vm.is_callable(adder) {
                    vm.temp_roots.truncate(prev_temp);
                    return Err(VmError::TypeError("Map.prototype.set is not callable".into()));
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
                    let cur_map = vm.temp_roots[prev_temp];
                    let cur_adder = vm.temp_roots[prev_temp + 1];
                    vm.call(cur_adder, cur_map, &[k_val, v_val])?;
                    vm.temp_roots.truncate(items_start + num_items);
                }
                vm.temp_roots.truncate(prev_temp + 1);
            }
        }
        let res = vm.temp_roots[prev_temp];
        vm.temp_roots.truncate(prev_temp);
        Ok(res)
    });
    vm.set_property(map_ctor, "groupBy", map_group_by);
    vm.set_property(map_ctor, "prototype", map_proto);
    vm.non_enumerable_props.insert((map_ctor, "prototype".to_string()));
    vm.non_writable_props.insert((map_ctor, "prototype".to_string()));
    vm.non_configurable_props.insert((map_ctor, "prototype".to_string()));
    vm.set_property(map_proto, "constructor", map_ctor);
    vm.non_enumerable_props.insert((map_proto, "constructor".to_string()));
    add_species_getter(vm, map_ctor);
    vm.globals.insert("Map".to_string(), map_ctor);

}
