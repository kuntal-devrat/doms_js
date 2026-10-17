//! `Array.prototype` methods taking callbacks or transforming arrays
//! (`forEach`, `filter`, `concat`, `reverse`, `reduce`, `some`, `every`).

use super::*;

pub(super) fn register_callback_methods(vm: &mut Vm, array_proto: JsValue) {
    let for_each_fn = vm.alloc_native_function("forEach", 1, |vm, this, args| {
        let cb = args.first().copied().unwrap_or(JsValue::undefined());
        if !vm.is_callable(cb) {
            return Err(VmError::TypeError("forEach callback must be callable".into()));
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
                let cur_el = vm.temp_roots[prev_temp + 3];
                let res = vm.call(cur_cb, cur_this_arg, &[cur_el, JsValue::from_int32(i as i32), cur_this]);
                vm.temp_roots.pop();
                if let Err(e) = res {
                    vm.temp_roots.truncate(prev_temp);
                    return Err(e);
                }
            }
        }
        vm.temp_roots.truncate(prev_temp);
        Ok(JsValue::undefined())
    });
    vm.set_property(array_proto, "forEach", for_each_fn);

    let filter_fn = vm.alloc_native_function("filter", 1, |vm, this, args| {
        let cb = args.first().copied().unwrap_or(JsValue::undefined());
        if !vm.is_callable(cb) {
            return Err(VmError::TypeError("filter callback must be callable".into()));
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
        let filtered_start = vm.temp_roots.len();
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
                let rooted_el = *vm.temp_roots.last().unwrap();
                let keep = match vm.call(cur_cb, cur_this_arg, &[rooted_el, JsValue::from_int32(i as i32), cur_this]) {
                    Ok(k) => k,
                    Err(e) => {
                        vm.temp_roots.truncate(prev_temp);
                        return Err(e);
                    }
                };
                if !keep.is_truthy() {
                    vm.temp_roots.pop();
                }
            }
        }
        let mut filtered = Vec::with_capacity(vm.temp_roots.len() - filtered_start);
        for k in filtered_start..vm.temp_roots.len() {
            filtered.push(vm.temp_roots[k]);
        }
        vm.temp_roots.truncate(prev_temp);
        Ok(vm.alloc_array(filtered))
    });
    vm.set_property(array_proto, "filter", filter_fn);

    let concat_fn = vm.alloc_native_function("concat", 1, |vm, this, args| {
        let prev_temp = vm.temp_roots.len();
        vm.temp_roots.push(this);
        for &arg in args {
            vm.temp_roots.push(arg);
        }
        let num_items = args.len() + 1;
        let sym_spread = vm.symbol_is_concat_spreadable;
        let out_start = vm.temp_roots.len();

        for item_idx in 0..num_items {
            let item = vm.temp_roots[prev_temp + item_idx];
            let is_spreadable = if let Some(sym) = sym_spread {
                let spread_val = vm.get_symbol_property_result(item, sym).unwrap_or(JsValue::undefined());
                if !spread_val.is_undefined() {
                    spread_val.is_truthy()
                } else if let Some(ptr) = item.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    matches!(obj.kind, ObjectKind::Array(_))
                } else {
                    false
                }
            } else if let Some(ptr) = item.as_object_ptr() {
                let obj = unsafe { &*(ptr as *mut JsObject) };
                matches!(obj.kind, ObjectKind::Array(_))
            } else {
                false
            };

            let item = vm.temp_roots[prev_temp + item_idx];
            if is_spreadable {
                let len_val = vm.get_property(item, "length");
                let len = len_val.to_number().unwrap_or(0.0) as usize;
                if len > 100_000 {
                    vm.temp_roots.truncate(prev_temp);
                    return Err(VmError::RangeError("Invalid array length".into()));
                }
                for i in 0..len {
                    let item = vm.temp_roots[prev_temp + item_idx];
                    let el = vm.get_element(item, i);
                    vm.temp_roots.push(el);
                }
            } else {
                vm.temp_roots.push(item);
            }
        }

        let total_len = vm.temp_roots.len() - out_start;
        if total_len > 100_000 {
            vm.temp_roots.truncate(prev_temp);
            return Err(VmError::RangeError("Invalid array length".into()));
        }
        let mut out = Vec::with_capacity(total_len);
        for k in out_start..vm.temp_roots.len() {
            out.push(vm.temp_roots[k]);
        }
        vm.temp_roots.truncate(prev_temp);
        Ok(vm.alloc_array(out))
    });
    vm.set_property(array_proto, "concat", concat_fn);

    let reverse_fn = vm.alloc_native_function("reverse", 0, |vm, this, _args| {
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::Array(ref mut elems) = obj.kind {
                elems.reverse();
                return Ok(this);
            }
        }
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
        let mut i = 0;
        let mut j = len.saturating_sub(1);
        while i < j {
            let cur_this = vm.temp_roots[prev_temp];
            let vi = vm.get_element(cur_this, i);
            let vj = vm.get_element(cur_this, j);
            vm.set_element(cur_this, i, vj);
            vm.set_element(cur_this, j, vi);
            i += 1;
            j = j.saturating_sub(1);
        }
        let cur_this = vm.temp_roots[prev_temp];
        vm.temp_roots.truncate(prev_temp);
        Ok(cur_this)
    });
    vm.set_property(array_proto, "reverse", reverse_fn);

    let reduce_fn = vm.alloc_native_function("reduce", 1, |vm, this, args| {
        let cb = args.first().copied().unwrap_or(JsValue::undefined());
        if !vm.is_callable(cb) {
            return Err(VmError::TypeError("reduce callback must be callable".into()));
        }
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

        let mut k = 0usize;
        let acc = if args.len() > 1 {
            args[1]
        } else {
            let mut found = None;
            while k < len {
                let cur_this = vm.temp_roots[prev_temp];
                let is_present = if let Some(ptr) = cur_this.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    if let ObjectKind::Array(elements) = &obj.kind {
                        if k < elements.len() && !elements[k].is_hole() {
                            true
                        } else {
                            let key = k.to_string();
                            vm.has_property(cur_this, &key).unwrap_or(false)
                        }
                    } else {
                        let key = k.to_string();
                        vm.has_property(cur_this, &key).unwrap_or(false)
                    }
                } else {
                    let key = k.to_string();
                    vm.has_property(cur_this, &key).unwrap_or(false)
                };

                if is_present {
                    let cur_this = vm.temp_roots[prev_temp];
                    let val = vm.get_element(cur_this, k);
                    found = Some(val);
                    k += 1;
                    break;
                }
                k += 1;
            }
            match found {
                Some(val) => val,
                None => {
                    vm.temp_roots.truncate(prev_temp);
                    return Err(VmError::TypeError("Reduce of empty array with no initial value".into()));
                }
            }
        };

        vm.temp_roots.push(acc); // prev_temp + 2

        while k < len {
            let cur_this = vm.temp_roots[prev_temp];
            let is_present = if let Some(ptr) = cur_this.as_object_ptr() {
                let obj = unsafe { &*(ptr as *mut JsObject) };
                if let ObjectKind::Array(elements) = &obj.kind {
                    if k < elements.len() && !elements[k].is_hole() {
                        true
                    } else {
                        let key = k.to_string();
                        vm.has_property(cur_this, &key).unwrap_or(false)
                    }
                } else {
                    let key = k.to_string();
                    vm.has_property(cur_this, &key).unwrap_or(false)
                }
            } else {
                let key = k.to_string();
                vm.has_property(cur_this, &key).unwrap_or(false)
            };

            if is_present {
                let cur_this = vm.temp_roots[prev_temp];
                let el = vm.get_element(cur_this, k);
                vm.temp_roots.push(el); // prev_temp + 3
                let cur_this = vm.temp_roots[prev_temp];
                let cur_cb = vm.temp_roots[prev_temp + 1];
                let cur_acc = vm.temp_roots[prev_temp + 2];
                let cur_el = vm.temp_roots[prev_temp + 3];
                let res = vm.call(cur_cb, JsValue::undefined(), &[cur_acc, cur_el, JsValue::from_int32(k as i32), cur_this]);
                vm.temp_roots.pop(); // pop el
                match res {
                    Ok(new_acc) => {
                        vm.temp_roots[prev_temp + 2] = new_acc;
                    }
                    Err(e) => {
                        vm.temp_roots.truncate(prev_temp);
                        return Err(e);
                    }
                }
            }
            k += 1;
        }

        let final_acc = vm.temp_roots[prev_temp + 2];
        vm.temp_roots.truncate(prev_temp);
        Ok(final_acc)
    });
    vm.set_property(array_proto, "reduce", reduce_fn);

    let some_fn = vm.alloc_native_function("some", 1, |vm, this, args| {
        let cb = args.first().copied().unwrap_or(JsValue::undefined());
        if !vm.is_callable(cb) {
            return Err(VmError::TypeError("some callback must be callable".into()));
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
                let cur_el = vm.temp_roots[prev_temp + 3];
                let res = vm.call(cur_cb, cur_this_arg, &[cur_el, JsValue::from_int32(i as i32), cur_this]);
                vm.temp_roots.pop();
                match res {
                    Ok(r) => {
                        if r.is_truthy() {
                            vm.temp_roots.truncate(prev_temp);
                            return Ok(JsValue::from_boolean(true));
                        }
                    }
                    Err(e) => {
                        vm.temp_roots.truncate(prev_temp);
                        return Err(e);
                    }
                }
            }
        }
        vm.temp_roots.truncate(prev_temp);
        Ok(JsValue::from_boolean(false))
    });
    vm.set_property(array_proto, "some", some_fn);

    let every_fn = vm.alloc_native_function("every", 1, |vm, this, args| {
        let cb = args.first().copied().unwrap_or(JsValue::undefined());
        if !vm.is_callable(cb) {
            return Err(VmError::TypeError("every callback must be callable".into()));
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
                let cur_el = vm.temp_roots[prev_temp + 3];
                let res = vm.call(cur_cb, cur_this_arg, &[cur_el, JsValue::from_int32(i as i32), cur_this]);
                vm.temp_roots.pop();
                match res {
                    Ok(r) => {
                        if !r.is_truthy() {
                            vm.temp_roots.truncate(prev_temp);
                            return Ok(JsValue::from_boolean(false));
                        }
                    }
                    Err(e) => {
                        vm.temp_roots.truncate(prev_temp);
                        return Err(e);
                    }
                }
            }
        }
        vm.temp_roots.truncate(prev_temp);
        Ok(JsValue::from_boolean(true))
    });
    vm.set_property(array_proto, "every", every_fn);

    let find_fn = vm.alloc_native_function("find", 1, |vm, this, args| {
        let cb = args.first().copied().unwrap_or(JsValue::undefined());
        if !vm.is_callable(cb) {
            return Err(VmError::TypeError("Array.prototype.find callback must be a function".into()));
        }
        let this_arg = args.get(1).copied().unwrap_or(JsValue::undefined());
        let len_val = vm.get_property(this, "length");
        let len = len_val.to_number().unwrap_or(0.0).max(0.0) as usize;
        let prev_temp = vm.temp_roots.len();
        vm.temp_roots.push(this);
        vm.temp_roots.push(cb);
        vm.temp_roots.push(this_arg);
        for i in 0..len {
            let cur_this = vm.temp_roots[prev_temp];
            let mut el = vm.get_element(cur_this, i);
            if el.is_hole() {
                el = JsValue::undefined();
            }
            vm.temp_roots.push(el);
            let cur_this = vm.temp_roots[prev_temp];
            let cur_cb = vm.temp_roots[prev_temp + 1];
            let cur_this_arg = vm.temp_roots[prev_temp + 2];
            let cur_el = vm.temp_roots[prev_temp + 3];
            let res = vm.call(cur_cb, cur_this_arg, &[cur_el, JsValue::from_int32(i as i32), cur_this]);
            match res {
                Ok(r) => {
                    if r.is_truthy() {
                        let found = vm.temp_roots[prev_temp + 3];
                        vm.temp_roots.truncate(prev_temp);
                        return Ok(found);
                    }
                    vm.temp_roots.pop();
                }
                Err(e) => {
                    vm.temp_roots.truncate(prev_temp);
                    return Err(e);
                }
            }
        }
        vm.temp_roots.truncate(prev_temp);
        Ok(JsValue::undefined())
    });
    vm.set_property(array_proto, "find", find_fn);

    let find_index_fn = vm.alloc_native_function("findIndex", 1, |vm, this, args| {
        let cb = args.first().copied().unwrap_or(JsValue::undefined());
        if !vm.is_callable(cb) {
            return Err(VmError::TypeError("Array.prototype.findIndex callback must be a function".into()));
        }
        let this_arg = args.get(1).copied().unwrap_or(JsValue::undefined());
        let len_val = vm.get_property(this, "length");
        let len = len_val.to_number().unwrap_or(0.0).max(0.0) as usize;
        let prev_temp = vm.temp_roots.len();
        vm.temp_roots.push(this);
        vm.temp_roots.push(cb);
        vm.temp_roots.push(this_arg);
        for i in 0..len {
            let cur_this = vm.temp_roots[prev_temp];
            let mut el = vm.get_element(cur_this, i);
            if el.is_hole() {
                el = JsValue::undefined();
            }
            vm.temp_roots.push(el);
            let cur_this = vm.temp_roots[prev_temp];
            let cur_cb = vm.temp_roots[prev_temp + 1];
            let cur_this_arg = vm.temp_roots[prev_temp + 2];
            let cur_el = vm.temp_roots[prev_temp + 3];
            let res = vm.call(cur_cb, cur_this_arg, &[cur_el, JsValue::from_int32(i as i32), cur_this]);
            vm.temp_roots.pop();
            match res {
                Ok(r) => {
                    if r.is_truthy() {
                        vm.temp_roots.truncate(prev_temp);
                        return Ok(JsValue::from_int32(i as i32));
                    }
                }
                Err(e) => {
                    vm.temp_roots.truncate(prev_temp);
                    return Err(e);
                }
            }
        }
        vm.temp_roots.truncate(prev_temp);
        Ok(JsValue::from_int32(-1))
    });
    vm.set_property(array_proto, "findIndex", find_index_fn);

    let find_last_fn = vm.alloc_native_function("findLast", 1, |vm, this, args| {
        let cb = args.first().copied().unwrap_or(JsValue::undefined());
        if !vm.is_callable(cb) {
            return Err(VmError::TypeError("Array.prototype.findLast callback must be a function".into()));
        }
        let this_arg = args.get(1).copied().unwrap_or(JsValue::undefined());
        let len_val = vm.get_property(this, "length");
        let len = len_val.to_number().unwrap_or(0.0).max(0.0) as usize;
        let prev_temp = vm.temp_roots.len();
        vm.temp_roots.push(this);
        vm.temp_roots.push(cb);
        vm.temp_roots.push(this_arg);
        for i in (0..len).rev() {
            let cur_this = vm.temp_roots[prev_temp];
            let mut el = vm.get_element(cur_this, i);
            if el.is_hole() {
                el = JsValue::undefined();
            }
            vm.temp_roots.push(el);
            let cur_this = vm.temp_roots[prev_temp];
            let cur_cb = vm.temp_roots[prev_temp + 1];
            let cur_this_arg = vm.temp_roots[prev_temp + 2];
            let cur_el = vm.temp_roots[prev_temp + 3];
            let res = vm.call(cur_cb, cur_this_arg, &[cur_el, JsValue::from_int32(i as i32), cur_this]);
            match res {
                Ok(r) => {
                    if r.is_truthy() {
                        let found = vm.temp_roots[prev_temp + 3];
                        vm.temp_roots.truncate(prev_temp);
                        return Ok(found);
                    }
                    vm.temp_roots.pop();
                }
                Err(e) => {
                    vm.temp_roots.truncate(prev_temp);
                    return Err(e);
                }
            }
        }
        vm.temp_roots.truncate(prev_temp);
        Ok(JsValue::undefined())
    });
    vm.set_property(array_proto, "findLast", find_last_fn);

    let find_last_index_fn = vm.alloc_native_function("findLastIndex", 1, |vm, this, args| {
        let cb = args.first().copied().unwrap_or(JsValue::undefined());
        if !vm.is_callable(cb) {
            return Err(VmError::TypeError("Array.prototype.findLastIndex callback must be a function".into()));
        }
        let this_arg = args.get(1).copied().unwrap_or(JsValue::undefined());
        let len_val = vm.get_property(this, "length");
        let len = len_val.to_number().unwrap_or(0.0).max(0.0) as usize;
        let prev_temp = vm.temp_roots.len();
        vm.temp_roots.push(this);
        vm.temp_roots.push(cb);
        vm.temp_roots.push(this_arg);
        for i in (0..len).rev() {
            let cur_this = vm.temp_roots[prev_temp];
            let mut el = vm.get_element(cur_this, i);
            if el.is_hole() {
                el = JsValue::undefined();
            }
            vm.temp_roots.push(el);
            let cur_this = vm.temp_roots[prev_temp];
            let cur_cb = vm.temp_roots[prev_temp + 1];
            let cur_this_arg = vm.temp_roots[prev_temp + 2];
            let cur_el = vm.temp_roots[prev_temp + 3];
            let res = vm.call(cur_cb, cur_this_arg, &[cur_el, JsValue::from_int32(i as i32), cur_this]);
            vm.temp_roots.pop();
            match res {
                Ok(r) => {
                    if r.is_truthy() {
                        vm.temp_roots.truncate(prev_temp);
                        return Ok(JsValue::from_int32(i as i32));
                    }
                }
                Err(e) => {
                    vm.temp_roots.truncate(prev_temp);
                    return Err(e);
                }
            }
        }
        vm.temp_roots.truncate(prev_temp);
        Ok(JsValue::from_int32(-1))
    });
    vm.set_property(array_proto, "findLastIndex", find_last_index_fn);

    let flat_fn = vm.alloc_native_function("flat", 0, |vm, this, args| {
        let obj = vm.to_object_value(this)?;
        let depth = if let Some(d_arg) = args.first() {
            if d_arg.is_undefined() {
                1i64
            } else {
                match vm.to_number_value(*d_arg) {
                    Ok(n) if n.is_nan() => 0i64,
                    Ok(n) => n.trunc() as i64,
                    Err(e) => return Err(e),
                }
            }
        } else {
            1i64
        };
        let len_val = vm.get_property_result(obj, "length")?;
        let source_len_num = vm.to_number_value(len_val)?;
        let source_len = if source_len_num.is_nan() || source_len_num <= 0.0 {
            0usize
        } else {
            (source_len_num.trunc() as i64).min(9007199254740991) as usize
        };
        let target = vm.array_species_create(obj, 0)?;
        flatten_into_array(vm, target, obj, source_len, 0, depth, None, JsValue::undefined())?;
        Ok(target)
    });
    vm.set_property(array_proto, "flat", flat_fn);

    let flat_map_fn = vm.alloc_native_function("flatMap", 1, |vm, this, args| {
        let obj = vm.to_object_value(this)?;
        let cb = args.first().copied().unwrap_or(JsValue::undefined());
        if !vm.is_callable(cb) {
            return Err(VmError::TypeError("Array.prototype.flatMap callback must be a function".into()));
        }
        let this_arg = args.get(1).copied().unwrap_or(JsValue::undefined());
        let len_val = vm.get_property_result(obj, "length")?;
        let source_len_num = vm.to_number_value(len_val)?;
        let source_len = if source_len_num.is_nan() || source_len_num <= 0.0 {
            0usize
        } else {
            (source_len_num.trunc() as i64).min(9007199254740991) as usize
        };
        let target = vm.array_species_create(obj, 0)?;
        flatten_into_array(vm, target, obj, source_len, 0, 1, Some(cb), this_arg)?;
        Ok(target)
    });
    vm.set_property(array_proto, "flatMap", flat_map_fn);

    let sort_fn = vm.alloc_native_function("sort", 1, |vm, this, args| {
        let compare_fn = args.first().copied();
        if let Some(cf) = compare_fn {
            if !cf.is_undefined() && !vm.is_callable(cf) {
                return Err(VmError::TypeError("The comparison function must be either a function or undefined".into()));
            }
        }
        let prev_temp = vm.temp_roots.len();
        vm.temp_roots.push(this);
        let compare_fn_rooted = compare_fn.unwrap_or(JsValue::undefined());
        vm.temp_roots.push(compare_fn_rooted);

        let len_val = vm.get_property(this, "length");
        let len = len_val.to_number().unwrap_or(0.0).max(0.0) as usize;
        let elements_start = vm.temp_roots.len();
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::Array(elements) = &obj.kind {
                for el in elements.iter().take(len) {
                    vm.temp_roots.push(*el);
                }
            }
        }
        if vm.temp_roots.len() == elements_start {
            for i in 0..len {
                let el = vm.get_element(this, i);
                vm.temp_roots.push(el);
            }
        }

        let res = sort_rooted_elements(vm, elements_start, len, prev_temp + 1);
        if let Err(e) = res {
            vm.temp_roots.truncate(prev_temp);
            return Err(e);
        }

        let cur_this = vm.temp_roots[prev_temp];
        if let Some(ptr) = cur_this.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::Array(elements) = &mut obj.kind {
                elements.clear();
                for i in 0..len {
                    elements.push(vm.temp_roots[elements_start + i]);
                }
                vm.temp_roots.truncate(prev_temp);
                return Ok(cur_this);
            }
        }

        for i in 0..len {
            let el = vm.temp_roots[elements_start + i];
            vm.set_element(cur_this, i, el);
        }
        vm.temp_roots.truncate(prev_temp);
        Ok(cur_this)
    });
    vm.set_property(array_proto, "sort", sort_fn);

    let to_sorted_fn = vm.alloc_native_function("toSorted", 1, |vm, this, args| {
        let compare_fn = args.first().copied();
        if let Some(cf) = compare_fn {
            if !cf.is_undefined() && !vm.is_callable(cf) {
                return Err(VmError::TypeError("The comparison function must be either a function or undefined".into()));
            }
        }
        let obj = vm.to_object_value(this)?;
        let prev_temp = vm.temp_roots.len();
        vm.temp_roots.push(obj);
        let compare_fn_rooted = compare_fn.unwrap_or(JsValue::undefined());
        vm.temp_roots.push(compare_fn_rooted);

        let len_val = vm.get_property(obj, "length");
        let len_num = vm.to_number_value(len_val)?;
        if len_num > 4294967295.0 {
            vm.temp_roots.truncate(prev_temp);
            return Err(VmError::RangeError("Invalid array length".into()));
        }
        let len = if len_num.is_nan() || len_num <= 0.0 {
            0usize
        } else {
            (len_num.trunc() as i64).min(4294967295) as usize
        };
        if len > 50_000_000 {
            vm.temp_roots.truncate(prev_temp);
            return Err(VmError::RangeError("Invalid array length".into()));
        }
        let elements_start = vm.temp_roots.len();
        let cur_obj = vm.temp_roots[prev_temp];
        for i in 0..len {
            let mut val = vm.get_element_result(cur_obj, i)?;
            if val.is_hole() {
                val = JsValue::undefined();
            }
            vm.temp_roots.push(val);
        }
        let res = sort_rooted_elements(vm, elements_start, len, prev_temp + 1);
        if let Err(e) = res {
            vm.temp_roots.truncate(prev_temp);
            return Err(e);
        }
        let mut sorted_elements = Vec::with_capacity(len);
        for i in 0..len {
            sorted_elements.push(vm.temp_roots[elements_start + i]);
        }
        vm.temp_roots.truncate(prev_temp);
        Ok(vm.alloc_array(sorted_elements))
    });
    vm.set_property(array_proto, "toSorted", to_sorted_fn);

    let to_reversed_fn = vm.alloc_native_function("toReversed", 0, |vm, this, _args| {
        let obj = vm.to_object_value(this)?;
        let len_val = vm.get_property(obj, "length");
        let len_num = vm.to_number_value(len_val)?;
        if len_num > 4294967295.0 {
            return Err(VmError::RangeError("Invalid array length".into()));
        }
        let len = if len_num.is_nan() || len_num <= 0.0 {
            0usize
        } else {
            (len_num.trunc() as i64).min(4294967295) as usize
        };
        if len > 50_000_000 {
            return Err(VmError::RangeError("Invalid array length".into()));
        }
        let mut elements = Vec::with_capacity(len);
        for k in 0..len {
            let from = len - k - 1;
            let mut val = vm.get_element_result(obj, from)?;
            if val.is_hole() {
                val = JsValue::undefined();
            }
            elements.push(val);
        }
        Ok(vm.alloc_array(elements))
    });
    vm.set_property(array_proto, "toReversed", to_reversed_fn);

    let reduce_right_fn = vm.alloc_native_function("reduceRight", 1, |vm, this, args| {
        let cb = args.first().copied().unwrap_or(JsValue::undefined());
        if !vm.is_callable(cb) {
            return Err(VmError::TypeError("Array.prototype.reduceRight callback must be a function".into()));
        }
        let len_val = vm.get_property(this, "length");
        let len = len_val.to_number().unwrap_or(0.0).max(0.0) as usize;
        if len == 0 && args.len() < 2 {
            return Err(VmError::TypeError("Reduce of empty array with no initial value".into()));
        }

        let prev_temp = vm.temp_roots.len();
        vm.temp_roots.push(this);
        vm.temp_roots.push(cb);

        let mut idx = len as i64 - 1;
        let acc = if args.len() >= 2 {
            args[1]
        } else {
            let mut found = None;
            while idx >= 0 {
                let cur_this = vm.temp_roots[prev_temp];
                let is_present = if let Some(ptr) = cur_this.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    if let ObjectKind::Array(elements) = &obj.kind {
                        if (idx as usize) < elements.len() && !elements[idx as usize].is_hole() {
                            true
                        } else {
                            let key = idx.to_string();
                            vm.has_property(cur_this, &key).unwrap_or(false)
                        }
                    } else {
                        let key = idx.to_string();
                        vm.has_property(cur_this, &key).unwrap_or(false)
                    }
                } else {
                    let key = idx.to_string();
                    vm.has_property(cur_this, &key).unwrap_or(false)
                };

                if is_present {
                    let cur_this = vm.temp_roots[prev_temp];
                    found = Some(vm.get_element(cur_this, idx as usize));
                    idx -= 1;
                    break;
                }
                idx -= 1;
            }
            match found {
                Some(val) => val,
                None => {
                    vm.temp_roots.truncate(prev_temp);
                    return Err(VmError::TypeError("Reduce of empty array with no initial value".into()));
                }
            }
        };

        vm.temp_roots.push(acc); // prev_temp + 2

        let mut i = idx;
        while i >= 0 {
            let cur_this = vm.temp_roots[prev_temp];
            let is_present = if let Some(ptr) = cur_this.as_object_ptr() {
                let obj = unsafe { &*(ptr as *mut JsObject) };
                if let ObjectKind::Array(elements) = &obj.kind {
                    if (i as usize) < elements.len() && !elements[i as usize].is_hole() {
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
                let el = vm.get_element(cur_this, i as usize);
                vm.temp_roots.push(el); // prev_temp + 3
                let cur_this = vm.temp_roots[prev_temp];
                let cur_cb = vm.temp_roots[prev_temp + 1];
                let cur_acc = vm.temp_roots[prev_temp + 2];
                let cur_el = vm.temp_roots[prev_temp + 3];
                let res = vm.call(cur_cb, JsValue::undefined(), &[cur_acc, cur_el, JsValue::from_int32(i as i32), cur_this]);
                vm.temp_roots.pop();
                match res {
                    Ok(new_acc) => {
                        vm.temp_roots[prev_temp + 2] = new_acc;
                    }
                    Err(e) => {
                        vm.temp_roots.truncate(prev_temp);
                        return Err(e);
                    }
                }
            }
            i -= 1;
        }
        let final_acc = vm.temp_roots[prev_temp + 2];
        vm.temp_roots.truncate(prev_temp);
        Ok(final_acc)
    });
    vm.set_property(array_proto, "reduceRight", reduce_right_fn);
}

fn sort_rooted_elements(
    vm: &mut Vm,
    elements_start: usize,
    len: usize,
    cb_idx: usize,
) -> Result<(), VmError> {
    let cb = vm.temp_roots[cb_idx];
    let has_cb = !cb.is_undefined() && vm.is_callable(cb);
    let mut err = None;

    let mut indices: Vec<usize> = (0..len).collect();
    indices.sort_by(|&i, &j| {
        if err.is_some() {
            return std::cmp::Ordering::Equal;
        }
        let a = vm.temp_roots[elements_start + i];
        let b = vm.temp_roots[elements_start + j];

        if a.is_hole() && b.is_hole() {
            return std::cmp::Ordering::Equal;
        }
        if a.is_hole() {
            return std::cmp::Ordering::Greater;
        }
        if b.is_hole() {
            return std::cmp::Ordering::Less;
        }
        if a.is_undefined() && b.is_undefined() {
            return std::cmp::Ordering::Equal;
        }
        if a.is_undefined() {
            return std::cmp::Ordering::Greater;
        }
        if b.is_undefined() {
            return std::cmp::Ordering::Less;
        }

        if has_cb {
            let cur_cb = vm.temp_roots[cb_idx];
            match vm.call(cur_cb, JsValue::undefined(), &[a, b]) {
                Ok(res) => {
                    let n = res.to_number().unwrap_or(0.0);
                    if n.is_nan() || n == 0.0 {
                        std::cmp::Ordering::Equal
                    } else if n < 0.0 {
                        std::cmp::Ordering::Less
                    } else {
                        std::cmp::Ordering::Greater
                    }
                }
                Err(e) => {
                    err = Some(e);
                    std::cmp::Ordering::Equal
                }
            }
        } else {
            let sa = vm.to_string_value(a);
            let sb = vm.to_string_value(b);
            sa.cmp(&sb)
        }
    });

    if let Some(e) = err {
        return Err(e);
    }

    let sorted: Vec<JsValue> = indices.iter().map(|&idx| vm.temp_roots[elements_start + idx]).collect();
    for (i, el) in sorted.into_iter().enumerate() {
        vm.temp_roots[elements_start + i] = el;
    }

    Ok(())
}

fn flatten_into_array(
    vm: &mut Vm,
    target: JsValue,
    source: JsValue,
    source_len: usize,
    mut target_idx: usize,
    depth: i64,
    mapper_fn: Option<JsValue>,
    this_arg: JsValue,
) -> Result<usize, VmError> {
    let prev_temp = vm.temp_roots.len();
    vm.temp_roots.push(target);
    vm.temp_roots.push(source);
    vm.temp_roots.push(mapper_fn.unwrap_or(JsValue::undefined()));
    vm.temp_roots.push(this_arg);

    for source_idx in 0..source_len {
        let p = source_idx.to_string();
        let cur_source = vm.temp_roots[prev_temp + 1];
        if vm.has_property(cur_source, &p)? {
            let cur_source = vm.temp_roots[prev_temp + 1];
            let mut element = vm.get_property_result(cur_source, &p)?;
            if mapper_fn.is_some() {
                let cur_source = vm.temp_roots[prev_temp + 1];
                let cur_mapper = vm.temp_roots[prev_temp + 2];
                let cur_this_arg = vm.temp_roots[prev_temp + 3];
                element = match vm.call(cur_mapper, cur_this_arg, &[element, JsValue::from_int32(source_idx as i32), cur_source]) {
                    Ok(el) => el,
                    Err(e) => {
                        vm.temp_roots.truncate(prev_temp);
                        return Err(e);
                    }
                };
            }
            let should_flatten = depth > 0 && vm.is_array(element);
            if should_flatten {
                let next_depth = if depth == i64::MAX { depth } else { depth - 1 };
                let el_len_val = vm.get_property_result(element, "length")?;
                let el_len = el_len_val.to_number().unwrap_or(0.0).max(0.0) as usize;
                let cur_target = vm.temp_roots[prev_temp];
                target_idx = flatten_into_array(vm, cur_target, element, el_len, target_idx, next_depth, None, JsValue::undefined())?;
            } else {
                if target_idx > 9007199254740991 {
                    vm.temp_roots.truncate(prev_temp);
                    return Err(VmError::TypeError("Target index exceeds 2^53 - 1".into()));
                }
                let t_key = target_idx.to_string();
                let cur_target = vm.temp_roots[prev_temp];
                vm.create_data_property_or_throw(cur_target, &t_key, element)?;
                target_idx += 1;
            }
        }
    }
    vm.temp_roots.truncate(prev_temp);
    Ok(target_idx)
}

