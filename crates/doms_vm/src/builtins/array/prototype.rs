//! `Array.prototype` creation plus the core non-callback methods
//! (`map`, `join`, `push`, `pop`, `shift`, `unshift`, `slice`, `indexOf`,
//! `includes`).

use super::*;

#[inline]
fn index_to_js_value(k: i64) -> JsValue {
    if k >= 0 && k <= i32::MAX as i64 {
        JsValue::from_int32(k as i32)
    } else if k < 0 {
        JsValue::from_int32(-1)
    } else {
        JsValue::from_double(k as f64)
    }
}

pub(super) fn register_prototype_methods(vm: &mut Vm) -> JsValue {
    let array_proto = vm.alloc_array(Vec::new());
    vm.array_proto = Some(array_proto);
    vm.set_property(array_proto, "length", JsValue::from_int32(0));
    vm.set_property_non_enumerable(array_proto, "length".to_string());
    vm.set_property_non_configurable(array_proto, "length".to_string());

    let map_fn = vm.alloc_native_function("map", 1, |vm, this, args| {
        let obj = vm.to_object_value(this)?;
        let len = vm.length_of_array_like(obj)?;
        let cb = args.first().copied().unwrap_or(JsValue::undefined());
        if !vm.is_callable(cb) {
            return Err(VmError::TypeError("map callback must be callable".into()));
        }
        let this_arg = args.get(1).copied().unwrap_or(JsValue::undefined());
        let a = vm.array_species_create(obj, len)?;
        let prev_temp = vm.temp_roots.len();
        vm.temp_roots.push(obj);
        vm.temp_roots.push(cb);
        vm.temp_roots.push(this_arg);
        vm.temp_roots.push(a);
        for i in 0..len {
            let cur_this = vm.temp_roots[prev_temp];
            let is_present = vm.has_element(cur_this, i);
            if is_present {
                let cur_this = vm.temp_roots[prev_temp];
                let el = vm.get_element(cur_this, i);
                vm.temp_roots.push(el);
                let cur_this = vm.temp_roots[prev_temp];
                let cur_cb = vm.temp_roots[prev_temp + 1];
                let cur_this_arg = vm.temp_roots[prev_temp + 2];
                let cur_el = vm.temp_roots.pop().unwrap();
                let res = match vm.call(
                    cur_cb,
                    cur_this_arg,
                    &[cur_el, JsValue::from_int32(i as i32), cur_this],
                ) {
                    Ok(r) => r,
                    Err(e) => {
                        vm.temp_roots.truncate(prev_temp);
                        return Err(e);
                    }
                };
                let cur_a = vm.temp_roots[prev_temp + 3];
                let key = i.to_string();
                vm.create_data_property_or_throw(cur_a, &key, res)?;
            }
        }
        let res_a = vm.temp_roots[prev_temp + 3];
        vm.temp_roots.truncate(prev_temp);
        Ok(res_a)
    });
    vm.set_property(array_proto, "map", map_fn);

    let join_fn = vm.alloc_native_function("join", 1, |vm, this, args| {
        let obj = vm.to_object_value(this)?;
        let sep = match args.first() {
            Some(v) if !v.is_undefined() => vm.to_string_spec(*v)?,
            _ => ",".to_string(),
        };
        let len = vm.length_of_array_like(obj)?;
        let mut parts = Vec::with_capacity(len.min(65536));
        for i in 0..len {
            let el = vm.get_element(obj, i);
            if el.is_nullish() || el.is_hole() {
                parts.push(String::new());
            } else {
                parts.push(vm.to_string_spec(el)?);
            }
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

    let push_fn = vm.alloc_native_function("push", 1, |vm, this, args| {
        let obj = vm.to_object_value(this)?;
        let len_val = vm.get_property_result(obj, "length")?;
        let raw_len = vm.to_number_value(len_val)?;
        let mut len = if raw_len.is_nan() || raw_len <= 0.0 {
            0u64
        } else if raw_len >= 9007199254740991.0 {
            9007199254740991u64
        } else {
            raw_len.trunc() as u64
        };
        let arg_count = args.len() as u64;
        if len + arg_count > 9007199254740991 {
            return Err(VmError::TypeError("Array length exceeded maximum limit".into()));
        }
        for arg in args {
            let key = len.to_string();
            vm.set_property_throw(obj, &key, *arg)?;
            len += 1;
        }
        let len_js = JsValue::from_double(len as f64);
        vm.set_property_throw(obj, "length", len_js)?;
        Ok(len_js)
    });
    vm.set_property(array_proto, "push", push_fn);

    let pop_fn = vm.alloc_native_function("pop", 0, |vm, this, _args| {
        let obj = vm.to_object_value(this)?;
        let is_dense_extensible_array = if let Some(ptr) = obj.as_object_ptr() {
            let js_obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::Array(elements) = &js_obj.kind {
                js_obj.is_extensible()
                    && js_obj.accessors.is_none()
                    && js_obj.is_array_length_writable()
                    && !elements.iter().any(|v| v.is_hole())
                    && vm
                        .shapes
                        .get_property_info(js_obj.shape_id, "length")
                        .map(|i| i.attributes.writable)
                        .unwrap_or(true)
            } else {
                false
            }
        } else {
            false
        };

        let len_val = vm.get_property_result(obj, "length")?;
        let raw_len = vm.to_number_value(len_val)?;
        let len = if raw_len.is_nan() || raw_len <= 0.0 {
            0u64
        } else if raw_len >= 9007199254740991.0 {
            9007199254740991u64
        } else {
            raw_len.trunc() as u64
        };
        if len == 0 {
            vm.set_property_throw(obj, "length", JsValue::from_int32(0))?;
            return Ok(JsValue::undefined());
        }

        if is_dense_extensible_array {
            if let Some(ptr) = obj.as_object_ptr() {
                let js_obj = unsafe { &mut *(ptr as *mut JsObject) };
                if let ObjectKind::Array(elements) = &mut js_obj.kind {
                    return Ok(elements.pop().unwrap_or(JsValue::undefined()));
                }
            }
        }

        let new_len = len - 1;
        let key = new_len.to_string();
        let el = vm.get_property_result(obj, &key)?;
        vm.delete_property_or_throw(obj, &key)?;
        vm.set_property_throw(obj, "length", JsValue::from_double(new_len as f64))?;
        Ok(el)
    });
    vm.set_property(array_proto, "pop", pop_fn);

    let shift_fn = vm.alloc_native_function("shift", 0, |vm, this, _args| {
        let obj = vm.to_object_value(this)?;
        let is_dense_extensible_array = if let Some(ptr) = obj.as_object_ptr() {
            let js_obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::Array(elements) = &js_obj.kind {
                js_obj.is_extensible()
                    && js_obj.accessors.is_none()
                    && js_obj.is_array_length_writable()
                    && !elements.iter().any(|v| v.is_hole())
                    && vm
                        .shapes
                        .get_property_info(js_obj.shape_id, "length")
                        .map(|i| i.attributes.writable)
                        .unwrap_or(true)
            } else {
                false
            }
        } else {
            false
        };

        let len_val = vm.get_property_result(obj, "length")?;
        let raw_len = vm.to_number_value(len_val)?;
        let len = if raw_len.is_nan() || raw_len <= 0.0 {
            0u64
        } else if raw_len >= 9007199254740991.0 {
            9007199254740991u64
        } else {
            raw_len.trunc() as u64
        };
        if len == 0 {
            vm.set_property_throw(obj, "length", JsValue::from_int32(0))?;
            return Ok(JsValue::undefined());
        }

        if is_dense_extensible_array {
            if let Some(ptr) = obj.as_object_ptr() {
                let js_obj = unsafe { &mut *(ptr as *mut JsObject) };
                if let ObjectKind::Array(elements) = &mut js_obj.kind {
                    if !elements.is_empty() {
                        return Ok(elements.remove(0));
                    }
                }
            }
        }

        let first = vm.get_property_result(obj, "0")?;
        let mut k = 1u64;
        let mut iterations = 0;
        while k < len {
            iterations += 1;
            if iterations % 1024 == 0 {
                if let Some(d) = vm.deadline {
                    if std::time::Instant::now() > d {
                        return Err(VmError::RuntimeError("Execution timeout exceeded".into()));
                    }
                }
            }
            if iterations > 100_000 {
                break;
            }
            let from_key = k.to_string();
            let to_key = (k - 1).to_string();
            let from_present = vm.has_property(obj, &from_key)?;
            if from_present {
                let from_val = vm.get_property_result(obj, &from_key)?;
                vm.set_property_throw(obj, &to_key, from_val)?;
            } else {
                vm.delete_property_or_throw(obj, &to_key)?;
            }
            k += 1;
        }
        let key = (len - 1).to_string();
        vm.delete_property_or_throw(obj, &key)?;
        let new_len = len - 1;
        vm.set_property_throw(obj, "length", JsValue::from_double(new_len as f64))?;
        Ok(first)
    });
    vm.set_property(array_proto, "shift", shift_fn);

    let unshift_fn = vm.alloc_native_function("unshift", 1, |vm, this, args| {
        let obj = vm.to_object_value(this)?;
        let len_val = vm.get_property_result(obj, "length")?;
        let raw_len = vm.to_number_value(len_val)?;
        let len = if raw_len.is_nan() || raw_len <= 0.0 {
            0u64
        } else if raw_len >= 9007199254740991.0 {
            9007199254740991u64
        } else {
            raw_len.trunc() as u64
        };
        let arg_count = args.len() as u64;
        if arg_count == 0 {
            let len_js = JsValue::from_double(len as f64);
            vm.set_property_throw(obj, "length", len_js)?;
            return Ok(len_js);
        }
        if len + arg_count > 9007199254740991 {
            return Err(VmError::TypeError("Array length exceeded maximum limit".into()));
        }

        let mut k = len;
        let mut iterations = 0;
        while k > 0 {
            iterations += 1;
            if iterations % 1024 == 0 {
                if let Some(d) = vm.deadline {
                    if std::time::Instant::now() > d {
                        return Err(VmError::RuntimeError("Execution timeout exceeded".into()));
                    }
                }
            }
            if iterations > 100_000 {
                break;
            }
            let from_key = (k - 1).to_string();
            let to_key = (k + arg_count - 1).to_string();
            let from_present = vm.has_property(obj, &from_key)?;
            if from_present {
                let from_val = vm.get_property_result(obj, &from_key)?;
                vm.set_property_throw(obj, &to_key, from_val)?;
            } else {
                vm.delete_property_or_throw(obj, &to_key)?;
            }
            k -= 1;
        }
        for (i, arg) in args.iter().enumerate() {
            let key = i.to_string();
            vm.set_property_throw(obj, &key, *arg)?;
        }
        let new_len = JsValue::from_double((len + arg_count) as f64);
        vm.set_property_throw(obj, "length", new_len)?;
        Ok(new_len)
    });
    vm.set_property(array_proto, "unshift", unshift_fn);

    let slice_fn = vm.alloc_native_function("slice", 2, |vm, this, args| {
        let obj = vm.to_object_value(this)?;
        let len = vm.length_of_array_like(obj)?;
        let start =
            if let Some(arg) = args.first() { vm.to_integer_or_infinity(*arg)? } else { 0.0 };
        let start_idx = if start < 0.0 {
            (len as f64 + start).max(0.0) as usize
        } else {
            (start as usize).min(len)
        };
        let end = if let Some(arg) = args.get(1) {
            if arg.is_undefined() { len as f64 } else { vm.to_integer_or_infinity(*arg)? }
        } else {
            len as f64
        };
        let end_idx =
            if end < 0.0 { (len as f64 + end).max(0.0) as usize } else { (end as usize).min(len) };
        let count = if end_idx > start_idx { end_idx - start_idx } else { 0 };
        let a = vm.array_species_create(obj, count)?;
        if let Some(ptr) = obj.as_object_ptr() {
            let js_obj = unsafe { &*(ptr as *mut JsObject) };
            if js_obj.accessors.is_none() {
                if let ObjectKind::Array(elements) = &js_obj.kind {
                    if let Some(a_ptr) = a.as_object_ptr() {
                        let a_obj = unsafe { &mut *(a_ptr as *mut JsObject) };
                        if let ObjectKind::Array(a_elements) = &mut a_obj.kind {
                            let actual_len = elements.len();
                            let s = start_idx.min(actual_len);
                            let e = end_idx.min(actual_len);
                            if s < e {
                                *a_elements = elements[s..e].to_vec();
                            } else {
                                a_elements.clear();
                            }
                            return Ok(a);
                        }
                    }
                }
            }
        }
        let mut n = 0usize;
        for i in start_idx..end_idx {
            if vm.has_element(obj, i) {
                let el = vm.get_property_result(obj, &i.to_string())?;
                vm.create_data_property_or_throw(a, &n.to_string(), el)?;
            }
            n += 1;
        }
        vm.set_property_throw(a, "length", JsValue::from_double(n as f64))?;
        Ok(a)
    });
    vm.set_property(array_proto, "slice", slice_fn);

    let index_of_fn = vm.alloc_native_function("indexOf", 1, |vm, this, args| {
        let obj = vm.to_object_value(this)?;
        let search = args.first().copied().unwrap_or(JsValue::undefined());
        let len = vm.length_of_array_like(obj)? as i64;
        if len == 0 {
            return Ok(JsValue::from_int32(-1));
        }

        let from_index = if let Some(from_arg) = args.get(1) {
            let num = vm.to_number_value(*from_arg)?;
            if num.is_nan() { 0i64 } else { num.trunc() as i64 }
        } else {
            0i64
        };

        if from_index >= len {
            return Ok(JsValue::from_int32(-1));
        }

        let mut k = if from_index >= 0 { from_index } else { len + from_index };
        if k < 0 {
            k = 0;
        }

        // Fast path for native Array without accessors and without holes
        if let Some(ptr) = obj.as_object_ptr() {
            let js_obj = unsafe { &*(ptr as *mut JsObject) };
            if js_obj.accessors.is_none() {
                if let ObjectKind::Array(elements) = &js_obj.kind {
                    let has_holes = elements.iter().any(|v| v.is_hole());
                    let actual_len = (elements.len() as i64).min(len);
                    if !has_holes && actual_len >= len {
                        while k < actual_len {
                            let el = elements[k as usize];
                            if crate::object::js_strict_equal(el, search) {
                                return Ok(index_to_js_value(k));
                            }
                            k += 1;
                        }
                        return Ok(JsValue::from_int32(-1));
                    }
                }
            }
        }

        // Generic object path
        while k < len {
            let key = k.to_string();
            if vm.has_property(obj, &key).unwrap_or(false) {
                let el = vm.get_property_result(obj, &key)?;
                if crate::object::js_strict_equal(el, search) {
                    return Ok(index_to_js_value(k));
                }
            }
            k += 1;
        }
        Ok(JsValue::from_int32(-1))
    });
    vm.set_property(array_proto, "indexOf", index_of_fn);

    let includes_fn = vm.alloc_native_function("includes", 1, |vm, this, args| {
        let obj = vm.to_object_value(this)?;
        let search = args.first().copied().unwrap_or(JsValue::undefined());
        let len = vm.length_of_array_like(obj)? as i64;
        if len == 0 {
            return Ok(JsValue::from_boolean(false));
        }

        let from_index = if let Some(from_arg) = args.get(1) {
            let num = vm.to_number_value(*from_arg)?;
            if num.is_nan() { 0i64 } else { num.trunc() as i64 }
        } else {
            0i64
        };

        if from_index >= len {
            return Ok(JsValue::from_boolean(false));
        }

        let mut k = if from_index >= 0 { from_index } else { len + from_index };
        if k < 0 {
            k = 0;
        }

        // Fast path for native Array without accessors
        if let Some(ptr) = obj.as_object_ptr() {
            let js_obj = unsafe { &*(ptr as *mut JsObject) };
            if js_obj.accessors.is_none() {
                if let ObjectKind::Array(elements) = &js_obj.kind {
                    let actual_len = (elements.len() as i64).min(len);
                    while k < actual_len {
                        let el = elements[k as usize];
                        let val = if el.is_hole() { JsValue::undefined() } else { el };
                        if crate::object::js_values_equal(val, search) {
                            return Ok(JsValue::from_boolean(true));
                        }
                        k += 1;
                    }
                    if k < len && search.is_undefined() {
                        return Ok(JsValue::from_boolean(true));
                    }
                    return Ok(JsValue::from_boolean(false));
                }
            }
        }

        // Generic object path
        while k < len {
            let key = k.to_string();
            let el = vm.get_property_result(obj, &key)?;
            if crate::object::js_values_equal(el, search) {
                return Ok(JsValue::from_boolean(true));
            }
            k += 1;
        }
        Ok(JsValue::from_boolean(false))
    });
    vm.set_property(array_proto, "includes", includes_fn);

    let at_fn = vm.alloc_native_function("at", 1, |vm, this, args| {
        let obj = vm.to_object_value(this)?;
        let len = vm.length_of_array_like(obj)? as i64;
        let index_arg = args.first().copied().unwrap_or(JsValue::undefined());
        let relative_index = match vm.to_number_value(index_arg) {
            Ok(n) if n.is_nan() => 0i64,
            Ok(n) => n.trunc() as i64,
            Err(e) => return Err(e),
        };
        let k = if relative_index >= 0 { relative_index } else { len + relative_index };
        if k < 0 || k >= len {
            return Ok(JsValue::undefined());
        }
        Ok(vm.get_element(obj, k as usize))
    });
    vm.set_property(array_proto, "at", at_fn);

    let last_index_of_fn = vm.alloc_native_function("lastIndexOf", 1, |vm, this, args| {
        let obj = vm.to_object_value(this)?;
        let search = args.first().copied().unwrap_or(JsValue::undefined());
        let len = vm.length_of_array_like(obj)? as i64;
        if len == 0 {
            return Ok(JsValue::from_int32(-1));
        }

        let from_index = if let Some(from_arg) = args.get(1) {
            let num = vm.to_number_value(*from_arg)?;
            if num.is_nan() { 0i64 } else { num.trunc() as i64 }
        } else {
            len - 1
        };

        let mut k = if from_index >= 0 { from_index.min(len - 1) } else { len + from_index };

        // Fast path for native Array without accessors and without holes
        if let Some(ptr) = obj.as_object_ptr() {
            let js_obj = unsafe { &*(ptr as *mut JsObject) };
            if js_obj.accessors.is_none() {
                if let ObjectKind::Array(elements) = &js_obj.kind {
                    let has_holes = elements.iter().any(|v| v.is_hole());
                    let actual_len = elements.len() as i64;
                    if !has_holes && actual_len >= len {
                        while k >= 0 {
                            let el = elements[k as usize];
                            if crate::object::js_strict_equal(el, search) {
                                return Ok(index_to_js_value(k));
                            }
                            k -= 1;
                        }
                        return Ok(JsValue::from_int32(-1));
                    }
                }
            }
        }

        // Generic object path
        while k >= 0 {
            let key = k.to_string();
            if vm.has_property(obj, &key).unwrap_or(false) {
                let el = vm.get_property_result(obj, &key)?;
                if crate::object::js_strict_equal(el, search) {
                    return Ok(index_to_js_value(k));
                }
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
                let removed_elements: Vec<JsValue> = elements
                    .splice(
                        clamped_start..clamped_start + clamped_delete,
                        insert_items.iter().copied(),
                    )
                    .collect();
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
        let len_val = vm.get_property_result(obj, "length")?;
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
        let raw_num = vm.to_number_value(index_arg)?;
        let relative_index =
            if raw_num.is_nan() || raw_num == 0.0 { 0i64 } else { raw_num.trunc() as i64 };
        let actual_index = if relative_index >= 0 { relative_index } else { len + relative_index };

        if actual_index >= len || actual_index < 0 {
            return Err(VmError::RangeError("Invalid index".into()));
        }

        let value = args.get(1).copied().unwrap_or(JsValue::undefined());
        let mut new_elements = Vec::with_capacity(len as usize);
        for i in 0..len {
            if i == actual_index {
                new_elements.push(value);
            } else {
                let key = i.to_string();
                let mut val = vm.get_property_result(obj, &key)?;
                if val.is_hole() {
                    val = JsValue::undefined();
                }
                new_elements.push(val);
            }
        }
        Ok(vm.alloc_array(new_elements))
    });
    vm.set_property(array_proto, "with", with_fn);

    let copy_within_fn = vm.alloc_native_function("copyWithin", 2, |vm, this, args| {
        let obj = vm.to_object_value(this)?;
        let len = vm.length_of_array_like(obj)? as i64;
        let relative_target = if let Some(arg) = args.first() {
            let n = vm.to_number_value(*arg)?;
            if n.is_nan() {
                0i64
            } else if n <= -9007199254740991.0 {
                -9007199254740991i64
            } else if n >= 9007199254740991.0 {
                9007199254740991i64
            } else {
                n.trunc() as i64
            }
        } else {
            0i64
        };
        let mut to = if relative_target < 0 {
            (len + relative_target).max(0)
        } else {
            relative_target.min(len)
        };

        let relative_start = if let Some(arg) = args.get(1) {
            let n = vm.to_number_value(*arg)?;
            if n.is_nan() {
                0i64
            } else if n <= -9007199254740991.0 {
                -9007199254740991i64
            } else if n >= 9007199254740991.0 {
                9007199254740991i64
            } else {
                n.trunc() as i64
            }
        } else {
            0i64
        };
        let mut from = if relative_start < 0 {
            (len + relative_start).max(0)
        } else {
            relative_start.min(len)
        };

        let relative_end = if let Some(arg) = args.get(2) {
            if arg.is_undefined() {
                len
            } else {
                let n = vm.to_number_value(*arg)?;
                if n.is_nan() {
                    0i64
                } else if n <= -9007199254740991.0 {
                    -9007199254740991i64
                } else if n >= 9007199254740991.0 {
                    9007199254740991i64
                } else {
                    n.trunc() as i64
                }
            }
        } else {
            len
        };
        let final_end =
            if relative_end < 0 { (len + relative_end).max(0) } else { relative_end.min(len) };

        let mut count = (final_end - from).min(len - to);
        let direction = if from < to && to < from + count {
            from = from + count - 1;
            to = to + count - 1;
            -1i64
        } else {
            1i64
        };

        let prev_temp = vm.temp_roots.len();
        vm.temp_roots.push(obj);

        while count > 0 {
            let cur_this = vm.temp_roots[prev_temp];
            let from_key = from.to_string();
            let to_key = to.to_string();
            let from_present = vm.has_property(cur_this, &from_key)?;
            if from_present {
                let from_val = vm.get_property_result(cur_this, &from_key)?;
                vm.temp_roots.push(from_val);
                let cur_this = vm.temp_roots[prev_temp];
                let cur_val = vm.temp_roots.pop().unwrap();
                vm.set_property_result(cur_this, &to_key, cur_val)?;
            } else {
                vm.delete_property_or_throw(cur_this, &to_key)?;
            }
            from += direction;
            to += direction;
            count -= 1;
        }

        vm.temp_roots.truncate(prev_temp);
        Ok(obj)
    });
    vm.set_property(array_proto, "copyWithin", copy_within_fn);

    let fill_fn = vm.alloc_native_function("fill", 1, |vm, this, args| {
        let obj = vm.to_object_value(this)?;
        let len = vm.length_of_array_like(obj)? as i64;
        let value = args.first().copied().unwrap_or(JsValue::undefined());

        let relative_start = if let Some(arg) = args.get(1) {
            let n = vm.to_number_value(*arg)?;
            if n.is_nan() {
                0i64
            } else if n <= -9007199254740991.0 {
                -9007199254740991i64
            } else if n >= 9007199254740991.0 {
                9007199254740991i64
            } else {
                n.trunc() as i64
            }
        } else {
            0i64
        };
        let mut k = if relative_start < 0 {
            (len + relative_start).max(0)
        } else {
            relative_start.min(len)
        };

        let relative_end = if let Some(arg) = args.get(2) {
            if arg.is_undefined() {
                len
            } else {
                let n = vm.to_number_value(*arg)?;
                if n.is_nan() {
                    0i64
                } else if n <= -9007199254740991.0 {
                    -9007199254740991i64
                } else if n >= 9007199254740991.0 {
                    9007199254740991i64
                } else {
                    n.trunc() as i64
                }
            }
        } else {
            len
        };
        let final_end =
            if relative_end < 0 { (len + relative_end).max(0) } else { relative_end.min(len) };

        let prev_temp = vm.temp_roots.len();
        vm.temp_roots.push(obj);
        vm.temp_roots.push(value);

        while k < final_end {
            let cur_this = vm.temp_roots[prev_temp];
            let cur_val = vm.temp_roots[prev_temp + 1];
            let pk = k.to_string();
            vm.set_property_result(cur_this, &pk, cur_val)?;
            k += 1;
        }

        vm.temp_roots.truncate(prev_temp);
        Ok(obj)
    });
    vm.set_property(array_proto, "fill", fill_fn);

    array_proto
}
