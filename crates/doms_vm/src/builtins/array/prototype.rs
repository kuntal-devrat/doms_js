//! `Array.prototype` creation plus the core non-callback methods
//! (`map`, `join`, `push`, `pop`, `shift`, `unshift`, `slice`, `indexOf`,
//! `includes`).

use super::*;

pub(super) fn register_prototype_methods(vm: &mut Vm) -> JsValue {
    let array_proto = vm.alloc_object();
    vm.array_proto = Some(array_proto);
    vm.set_property(array_proto, "length", JsValue::from_int32(0));
    vm.non_enumerable_props.insert((array_proto, "length".to_string()));
    vm.non_configurable_props.insert((array_proto, "length".to_string()));

    let map_fn = vm.alloc_native_function("map", 1, |vm, this, args| {
        let cb = args.first().copied().unwrap_or(JsValue::undefined());
        if !vm.is_callable(cb) {
            return Err(VmError::TypeError("map callback must be callable".into()));
        }
        let this_arg = args.get(1).copied().unwrap_or(JsValue::undefined());
        let len_val = vm.get_property(this, "length");
        let raw_len = len_val.to_number().unwrap_or(0.0);
        if raw_len > 4_294_967_295.0 || raw_len < 0.0 || raw_len.is_infinite() {
            return Err(VmError::RangeError("Invalid array length".into()));
        }
        let len = raw_len as usize;
        if len > 100_000 {
            return Err(VmError::RangeError("Invalid array length".into()));
        }
        let prev_temp = vm.temp_roots.len();
        vm.temp_roots.push(this);
        vm.temp_roots.push(cb);
        vm.temp_roots.push(this_arg);
        let mapped_start = vm.temp_roots.len();
        for i in 0..len {
            let cur_this = vm.temp_roots[prev_temp];
            let is_present = if let Some(ptr) = cur_this.as_object_ptr() {
                let obj = unsafe { &*(ptr as *mut JsObject) };
                if let ObjectKind::Array(elements) = &obj.kind {
                    if i < elements.len() && !elements[i].is_hole() {
                        true
                    } else {
                        let key = i.to_string();
                        vm.has_property(cur_this, &key).unwrap_or(false)
                    }
                } else {
                    let key = i.to_string();
                    vm.has_property(cur_this, &key).unwrap_or(false)
                }
            } else {
                let key = i.to_string();
                vm.has_property(cur_this, &key).unwrap_or(false)
            };

            if is_present {
                let cur_this = vm.temp_roots[prev_temp];
                let el = vm.get_element(cur_this, i);
                vm.temp_roots.push(el);
                let cur_this = vm.temp_roots[prev_temp];
                let cur_cb = vm.temp_roots[prev_temp + 1];
                let cur_this_arg = vm.temp_roots[prev_temp + 2];
                let cur_el = vm.temp_roots.pop().unwrap();
                let res = match vm.call(cur_cb, cur_this_arg, &[cur_el, JsValue::from_int32(i as i32), cur_this]) {
                    Ok(r) => r,
                    Err(e) => {
                        vm.temp_roots.truncate(prev_temp);
                        return Err(e);
                    }
                };
                vm.temp_roots.push(res);
            } else {
                vm.temp_roots.push(JsValue::hole());
            }
        }
        let mut mapped = Vec::with_capacity(vm.temp_roots.len() - mapped_start);
        for k in mapped_start..vm.temp_roots.len() {
            mapped.push(vm.temp_roots[k]);
        }
        vm.temp_roots.truncate(prev_temp);
        Ok(vm.alloc_array(mapped))
    });
    vm.set_property(array_proto, "map", map_fn);

    let join_fn = vm.alloc_native_function("join", 1, |vm, this, args| {
        let sep = args.first().map(|v| vm.to_string_value(*v)).unwrap_or_else(|| ",".to_string());
        let len_val = vm.get_property(this, "length");
        let raw_len = len_val.to_number().unwrap_or(0.0);
        if raw_len > 4_294_967_295.0 || raw_len < 0.0 || raw_len.is_infinite() {
            return Err(VmError::RangeError("Invalid array length".into()));
        }
        let len = raw_len as usize;
        if len > 100_000 {
            return Err(VmError::RangeError("Invalid array length".into()));
        }
        let mut parts = Vec::with_capacity(len.min(65536));
        for i in 0..len {
            let el = vm.get_element(this, i);
            parts.push(vm.to_string_value(el));
        }
        Ok(vm.alloc_string(parts.join(&sep)))
    });
    vm.set_property(array_proto, "join", join_fn);

    let to_string_fn = vm.alloc_native_function("toString", 0, |vm, this, _args| {
        let join = vm.get_property(this, "join");
        if vm.is_callable(join) {
            vm.call(join, this, &[])
        } else {
            let obj_proto = vm.object_proto.unwrap_or(JsValue::undefined());
            let obj_to_string = vm.get_property(obj_proto, "toString");
            if vm.is_callable(obj_to_string) {
                vm.call(obj_to_string, this, &[])
            } else {
                Ok(vm.alloc_string("[object Array]".into()))
            }
        }
    });
    vm.set_property(array_proto, "toString", to_string_fn);
    vm.set_property(array_proto, "toLocaleString", to_string_fn);

    let push_fn = vm.alloc_native_function("push", 1, |_vm, this, args| {
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::Array(elements) = &mut obj.kind {
                if elements.len() + args.len() > 100_000 {
                    return Err(VmError::RangeError("Invalid array length".into()));
                }
                for arg in args {
                    elements.push(*arg);
                }
                return Ok(JsValue::from_int32(elements.len() as i32));
            }
        }
        Ok(JsValue::undefined())
    });
    vm.set_property(array_proto, "push", push_fn);

    let pop_fn = vm.alloc_native_function("pop", 0, |_vm, this, _args| {
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::Array(elements) = &mut obj.kind {
                return Ok(elements.pop().unwrap_or(JsValue::undefined()));
            }
        }
        Ok(JsValue::undefined())
    });
    vm.set_property(array_proto, "pop", pop_fn);

    let shift_fn = vm.alloc_native_function("shift", 0, |_vm, this, _args| {
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::Array(elements) = &mut obj.kind {
                if !elements.is_empty() {
                    return Ok(elements.remove(0));
                }
            }
        }
        Ok(JsValue::undefined())
    });
    vm.set_property(array_proto, "shift", shift_fn);

    let unshift_fn = vm.alloc_native_function("unshift", 1, |_vm, this, args| {
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::Array(elements) = &mut obj.kind {
                if elements.len() + args.len() > 100_000 {
                    return Err(VmError::RangeError("Invalid array length".into()));
                }
                for (i, arg) in args.iter().enumerate() {
                    elements.insert(i, *arg);
                }
                return Ok(JsValue::from_int32(elements.len() as i32));
            }
        }
        Ok(JsValue::undefined())
    });
    vm.set_property(array_proto, "unshift", unshift_fn);

    let slice_fn = vm.alloc_native_function("slice", 2, |vm, this, args| {
        let len_val = vm.get_property(this, "length");
        let raw_len = len_val.to_number().unwrap_or(0.0);
        if raw_len > 4_294_967_295.0 || raw_len < 0.0 || raw_len.is_infinite() {
            return Err(VmError::RangeError("Invalid array length".into()));
        }
        let len = raw_len as usize;
        let start = args.first().and_then(|v| v.to_number()).unwrap_or(0.0);
        let start_idx = if start < 0.0 {
            (len as f64 + start).max(0.0) as usize
        } else {
            (start as usize).min(len)
        };
        let end = args.get(1).and_then(|v| v.to_number()).unwrap_or(len as f64);
        let end_idx = if end < 0.0 {
            (len as f64 + end).max(0.0) as usize
        } else {
            (end as usize).min(len)
        };
        if end_idx >= start_idx && (end_idx - start_idx) > 100_000 {
            return Err(VmError::RangeError("Invalid array length".into()));
        }
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::Array(elements) = &obj.kind {
                let actual_len = elements.len();
                let s = start_idx.min(actual_len);
                let e = end_idx.min(actual_len);
                let sliced = if s < e { elements[s..e].to_vec() } else { Vec::new() };
                return Ok(vm.alloc_array(sliced));
            }
        }
        let mut sliced = Vec::new();
        if start_idx < end_idx {
            for i in start_idx..end_idx {
                sliced.push(vm.get_element(this, i));
            }
        }
        Ok(vm.alloc_array(sliced))
    });
    vm.set_property(array_proto, "slice", slice_fn);

    let index_of_fn = vm.alloc_native_function("indexOf", 1, |_vm, this, args| {
        let search = args.first().copied().unwrap_or(JsValue::undefined());
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::Array(elements) = &obj.kind {
                for (i, el) in elements.iter().enumerate() {
                    if crate::object::js_strict_equal(*el, search) {
                        return Ok(JsValue::from_int32(i as i32));
                    }
                }
            }
        }
        Ok(JsValue::from_int32(-1))
    });
    vm.set_property(array_proto, "indexOf", index_of_fn);

    let includes_fn = vm.alloc_native_function("includes", 1, |_vm, this, args| {
        let search = args.first().copied().unwrap_or(JsValue::undefined());
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::Array(elements) = &obj.kind {
                for el in elements {
                    if crate::object::js_values_equal(*el, search) {
                        return Ok(JsValue::from_boolean(true));
                    }
                }
            }
        }
        Ok(JsValue::from_boolean(false))
    });
    vm.set_property(array_proto, "includes", includes_fn);

    let at_fn = vm.alloc_native_function("at", 1, |vm, this, args| {
        let obj = vm.to_object_value(this)?;
        let len_val = vm.get_property(obj, "length");
        let len = len_val.to_number().unwrap_or(0.0).max(0.0) as i64;
        let index_arg = args.first().copied().unwrap_or(JsValue::undefined());
        let relative_index = match vm.to_number_value(index_arg) {
            Ok(n) if n.is_nan() => 0i64,
            Ok(n) => n.trunc() as i64,
            Err(e) => return Err(e),
        };
        let k = if relative_index >= 0 {
            relative_index
        } else {
            len + relative_index
        };
        if k < 0 || k >= len {
            return Ok(JsValue::undefined());
        }
        Ok(vm.get_element(obj, k as usize))
    });
    vm.set_property(array_proto, "at", at_fn);

    let last_index_of_fn = vm.alloc_native_function("lastIndexOf", 1, |vm, this, args| {
        let obj = vm.to_object_value(this)?;
        let search = args.first().copied().unwrap_or(JsValue::undefined());
        let len_val = vm.get_property(obj, "length");
        let len = len_val.to_number().unwrap_or(0.0).max(0.0) as i64;
        if len == 0 {
            return Ok(JsValue::from_int32(-1));
        }

        let from_index = if let Some(from_arg) = args.get(1) {
            match vm.to_number_value(*from_arg) {
                Ok(n) if n.is_nan() => 0i64,
                Ok(n) => n.trunc() as i64,
                Err(e) => return Err(e),
            }
        } else {
            len - 1
        };

        let mut k = if from_index >= 0 {
            from_index.min(len - 1)
        } else {
            len + from_index
        };

        while k >= 0 {
            let el = vm.get_element(obj, k as usize);
            if el == search {
                return Ok(JsValue::from_int32(k as i32));
            }
            k -= 1;
        }
        Ok(JsValue::from_int32(-1))
    });
    vm.set_property(array_proto, "lastIndexOf", last_index_of_fn);

    let splice_fn = vm.alloc_native_function("splice", 2, |vm, this, args| {
        let obj = vm.to_object_value(this)?;
        let len_val = vm.get_property_result(obj, "length")?;
        let len_num = vm.to_number_value(len_val)?;
        let len = if len_num.is_nan() || len_num <= 0.0 {
            0i64
        } else if len_num >= 9007199254740991.0 {
            9007199254740991i64
        } else {
            len_num.trunc() as i64
        };

        let relative_start = if let Some(arg) = args.first() {
            let n = vm.to_number_value(*arg)?;
            if n.is_nan() {
                0i64
            } else if n == f64::NEG_INFINITY {
                -9007199254740991i64
            } else if n == f64::INFINITY {
                len
            } else {
                n.trunc() as i64
            }
        } else {
            0i64
        };

        let actual_start = if args.is_empty() {
            0i64
        } else if relative_start < 0 {
            (len + relative_start).max(0)
        } else {
            relative_start.min(len)
        };

        let insert_items = if args.len() > 2 { &args[2..] } else { &[] };
        let insert_count = insert_items.len() as i64;

        let actual_delete_count = if args.is_empty() {
            0i64
        } else if args.len() == 1 {
            len - actual_start
        } else {
            let dc_num = vm.to_number_value(args[1])?;
            let dc = if dc_num.is_nan() || dc_num <= 0.0 {
                0i64
            } else if dc_num >= 9007199254740991.0 {
                9007199254740991i64
            } else {
                dc_num.trunc() as i64
            };
            dc.clamp(0, len - actual_start)
        };

        if len + insert_count - actual_delete_count > 9007199254740991 {
            return Err(VmError::TypeError("Invalid array length".into()));
        }

        // Fast path for plain dense arrays:
        let is_plain_array = if let Some(ptr) = obj.as_object_ptr() {
            let jsobj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::Array(_) = &jsobj.kind {
                let ctor = vm.get_property(obj, "constructor");
                match vm.globals.get("Array") {
                    Some(&arr_ctor) => ctor == arr_ctor || ctor.is_undefined(),
                    None => true,
                }
            } else {
                false
            }
        } else {
            false
        };

        if is_plain_array {
            let start_idx = actual_start as usize;
            let delete_count = actual_delete_count as usize;
            let ptr = obj.as_object_ptr().unwrap();
            let jsobj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::Array(elements) = &mut jsobj.kind {
                let clamped_start = start_idx.min(elements.len());
                let clamped_delete = delete_count.min(elements.len() - clamped_start);
                let removed_elements: Vec<JsValue> = elements.splice(
                    clamped_start..clamped_start + clamped_delete,
                    insert_items.iter().copied(),
                ).collect();
                return Ok(vm.alloc_array(removed_elements));
            }
        }

        let ret_arr = vm.array_species_create(obj, actual_delete_count as usize)?;

        // Copy deleted elements
        let mut k = 0i64;
        while k < actual_delete_count {
            let from_key = (actual_start + k).to_string();
            let from_present = vm.has_property(obj, &from_key)?;
            if from_present {
                let from_val = vm.get_property_result(obj, &from_key)?;
                let to_key = k.to_string();
                vm.create_data_property_or_throw(ret_arr, &to_key, from_val)?;
            }
            k += 1;
        }
        let del_count_js = if actual_delete_count <= i32::MAX as i64 {
            JsValue::from_int32(actual_delete_count as i32)
        } else {
            JsValue::from_double(actual_delete_count as f64)
        };
        vm.set_property_result(ret_arr, "length", del_count_js)?;

        // Shift elements
        if insert_count < actual_delete_count {
            let mut k = actual_start;
            while k < len - actual_delete_count {
                let from_key = (k + actual_delete_count).to_string();
                let to_key = (k + insert_count).to_string();
                let from_present = vm.has_property(obj, &from_key)?;
                if from_present {
                    let from_val = vm.get_property_result(obj, &from_key)?;
                    vm.set_property_result(obj, &to_key, from_val)?;
                } else {
                    let _ = vm.delete_property(obj, &to_key);
                }
                k += 1;
            }
            let mut k = len;
            while k > len - actual_delete_count + insert_count {
                let to_key = (k - 1).to_string();
                let _ = vm.delete_property(obj, &to_key);
                k -= 1;
            }
        } else if insert_count > actual_delete_count {
            let mut k = len - actual_delete_count;
            while k > actual_start {
                let from_key = (k + actual_delete_count - 1).to_string();
                let to_key = (k + insert_count - 1).to_string();
                let from_present = vm.has_property(obj, &from_key)?;
                if from_present {
                    let from_val = vm.get_property_result(obj, &from_key)?;
                    vm.set_property_result(obj, &to_key, from_val)?;
                } else {
                    let _ = vm.delete_property(obj, &to_key);
                }
                k -= 1;
            }
        }

        // Insert items
        for (idx, item) in insert_items.iter().enumerate() {
            let to_key = (actual_start + idx as i64).to_string();
            vm.set_property_result(obj, &to_key, *item)?;
        }

        // Set length
        let new_len = len - actual_delete_count + insert_count;
        let new_len_js = if new_len <= i32::MAX as i64 {
            JsValue::from_int32(new_len as i32)
        } else {
            JsValue::from_double(new_len as f64)
        };
        vm.set_property_result(obj, "length", new_len_js)?;

        Ok(ret_arr)
    });
    vm.set_property(array_proto, "splice", splice_fn);

    let to_spliced_fn = vm.alloc_native_function("toSpliced", 2, |vm, this, args| {
        let obj = vm.to_object_value(this)?;
        let len_val = vm.get_property(obj, "length");
        let len_num = vm.to_number_value(len_val)?;
        let len = if len_num.is_nan() || len_num <= 0.0 {
            0i64
        } else {
            (len_num.trunc() as i64).min(9007199254740991)
        };

        let relative_start = if let Some(arg) = args.first() {
            match vm.to_number_value(*arg) {
                Ok(n) if n.is_nan() => 0i64,
                Ok(n) => n.trunc() as i64,
                Err(e) => return Err(e),
            }
        } else {
            0i64
        };

        let actual_start = if relative_start < 0 {
            (len + relative_start).max(0)
        } else {
            relative_start.min(len)
        };

        let insert_items = if args.len() > 2 { &args[2..] } else { &[] };
        let insert_count = insert_items.len() as i64;

        let actual_delete_count = if args.is_empty() {
            0i64
        } else if args.len() == 1 {
            len - actual_start
        } else {
            let sc = match vm.to_number_value(args[1]) {
                Ok(n) if n.is_nan() => 0i64,
                Ok(n) => n.trunc() as i64,
                Err(e) => return Err(e),
            };
            sc.clamp(0, len - actual_start)
        };

        let new_len = len + insert_count - actual_delete_count;
        if new_len > 9007199254740991 {
            return Err(VmError::TypeError("Array length exceeds 2^53 - 1".into()));
        }
        if new_len > 4294967295 {
            return Err(VmError::RangeError("Invalid array length".into()));
        }
        if new_len > 50_000_000 {
            return Err(VmError::RangeError("Invalid array length".into()));
        }

        let mut new_elements = Vec::with_capacity(new_len as usize);
        for i in 0..actual_start {
            let mut val = vm.get_element_result(obj, i as usize)?;
            if val.is_hole() {
                val = JsValue::undefined();
            }
            new_elements.push(val);
        }
        for item in insert_items {
            new_elements.push(*item);
        }
        let mut r = actual_start + actual_delete_count;
        while (new_elements.len() as i64) < new_len {
            let r_key = r.to_string();
            let mut val = vm.get_property_result(obj, &r_key)?;
            if val.is_hole() {
                val = JsValue::undefined();
            }
            new_elements.push(val);
            r += 1;
        }
        Ok(vm.alloc_array(new_elements))
    });
    vm.set_property(array_proto, "toSpliced", to_spliced_fn);

    let with_fn = vm.alloc_native_function("with", 2, |vm, this, args| {
        let obj = vm.to_object_value(this)?;
        let len_val = vm.get_property(obj, "length");
        let len_num = vm.to_number_value(len_val)?;
        if len_num > 4294967295.0 {
            return Err(VmError::RangeError("Invalid array length".into()));
        }
        let len = if len_num.is_nan() || len_num <= 0.0 {
            0i64
        } else {
            (len_num.trunc() as i64).min(4294967295)
        };

        let index_arg = args.first().copied().unwrap_or(JsValue::undefined());
        let relative_index = match vm.to_number_value(index_arg) {
            Ok(n) if n.is_nan() => 0i64,
            Ok(n) => n.trunc() as i64,
            Err(e) => return Err(e),
        };
        let actual_index = if relative_index >= 0 {
            relative_index
        } else {
            len + relative_index
        };

        if actual_index >= len || actual_index < 0 {
            return Err(VmError::RangeError("Invalid index".into()));
        }
        if len > 50_000_000 {
            return Err(VmError::RangeError("Invalid array length".into()));
        }

        let value = args.get(1).copied().unwrap_or(JsValue::undefined());
        let mut new_elements = Vec::with_capacity(len as usize);
        for i in 0..len {
            if i == actual_index {
                new_elements.push(value);
            } else {
                let mut val = vm.get_element(obj, i as usize);
                if val.is_hole() {
                    val = JsValue::undefined();
                }
                new_elements.push(val);
            }
        }
        Ok(vm.alloc_array(new_elements))
    });
    vm.set_property(array_proto, "with", with_fn);

    array_proto
}
