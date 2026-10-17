//! `Array.prototype` creation plus the core non-callback methods
//! (`map`, `join`, `push`, `pop`, `shift`, `unshift`, `slice`, `indexOf`,
//! `includes`).

use super::*;

pub(super) fn register_prototype_methods(vm: &mut Vm) -> JsValue {
    let array_proto = vm.alloc_object();
    vm.array_proto = Some(array_proto);

    let map_fn = vm.alloc_native_function("map", 1, |vm, this, args| {
        let cb = args.first().copied().unwrap_or(JsValue::undefined());
        let len_val = vm.get_property(this, "length");
        let raw_len = len_val.to_number().unwrap_or(0.0);
        if raw_len > 4_294_967_295.0 || raw_len < 0.0 || raw_len.is_infinite() {
            return Err(VmError::RangeError("Invalid array length".into()));
        }
        let len = raw_len as usize;
        if len > 100_000 {
            return Err(VmError::RangeError("Invalid array length".into()));
        }
        let mut mapped = Vec::with_capacity(len.min(65536));
        for i in 0..len {
            let el = vm.get_element(this, i);
            let res = vm.call(cb, JsValue::undefined(), &[el, JsValue::from_int32(i as i32), this])?;
            mapped.push(res);
        }
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
        let len_val = vm.get_property(obj, "length");
        let len = len_val.to_number().unwrap_or(0.0).max(0.0) as i64;

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

        let actual_delete_count = if args.is_empty() {
            0i64
        } else if args.len() == 1 {
            len - actual_start
        } else {
            let dc = match vm.to_number_value(args[1]) {
                Ok(n) if n.is_nan() => 0i64,
                Ok(n) => n.trunc() as i64,
                Err(e) => return Err(e),
            };
            dc.clamp(0, len - actual_start)
        };

        let insert_items = if args.len() > 2 { &args[2..] } else { &[] };
        let start_idx = actual_start as usize;
        let delete_count = actual_delete_count as usize;

        if let Some(ptr) = obj.as_object_ptr() {
            let jsobj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::Array(elements) = &mut jsobj.kind {
                let removed_elements: Vec<JsValue> = elements.splice(
                    start_idx..start_idx + delete_count,
                    insert_items.iter().copied(),
                ).collect();
                return Ok(vm.alloc_array(removed_elements));
            }
        }

        let mut removed = Vec::with_capacity(delete_count);
        for i in 0..delete_count {
            removed.push(vm.get_element(obj, start_idx + i));
        }
        let old_len = len as usize;
        let mut all = Vec::with_capacity(old_len);
        for i in 0..old_len {
            all.push(vm.get_element(obj, i));
        }
        all.splice(start_idx..start_idx + delete_count, insert_items.iter().copied());
        for (i, v) in all.into_iter().enumerate() {
            vm.set_element(obj, i, v);
        }
        let new_len = (len - actual_delete_count + insert_items.len() as i64).max(0);
        vm.set_property(obj, "length", JsValue::from_int32(new_len as i32));
        Ok(vm.alloc_array(removed))
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
