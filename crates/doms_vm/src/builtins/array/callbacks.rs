//! `Array.prototype` methods taking callbacks or transforming arrays
//! (`forEach`, `filter`, `concat`, `reverse`, `reduce`, `some`, `every`).

use super::*;

#[inline]
fn index_to_js_value(idx: usize) -> JsValue {
    if idx <= i32::MAX as usize {
        JsValue::from_int32(idx as i32)
    } else {
        JsValue::from_double(idx as f64)
    }
}

pub(super) fn register_callback_methods(vm: &mut Vm, array_proto: JsValue) {
    let for_each_fn = vm.alloc_native_function("forEach", 1, |vm, this, args| {
        let obj = vm.to_object_value(this)?;
        let cb = args.first().copied().unwrap_or(JsValue::undefined());
        if !vm.is_callable(cb) {
            return Err(VmError::TypeError("forEach callback must be callable".into()));
        }
        let this_arg = args.get(1).copied().unwrap_or(JsValue::undefined());
        let len = vm.length_of_array_like(obj)?;
        let prev_temp = vm.temp_roots.len();
        vm.temp_roots.push(obj);
        vm.temp_roots.push(cb);
        vm.temp_roots.push(this_arg);
        for i in 0..len {
            let cur_this = vm.temp_roots[prev_temp];
            if vm.has_element_result(cur_this, i)? {
                let cur_this = vm.temp_roots[prev_temp];
                let el = vm.get_element_result(cur_this, i)?;
                vm.temp_roots.push(el);
                let cur_this = vm.temp_roots[prev_temp];
                let cur_cb = vm.temp_roots[prev_temp + 1];
                let cur_this_arg = vm.temp_roots[prev_temp + 2];
                let cur_el = vm.temp_roots[prev_temp + 3];
                let res = vm.call(cur_cb, cur_this_arg, &[cur_el, index_to_js_value(i), cur_this]);
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
        let obj = vm.to_object_value(this)?;
        let cb = args.first().copied().unwrap_or(JsValue::undefined());
        if !vm.is_callable(cb) {
            return Err(VmError::TypeError("filter callback must be callable".into()));
        }
        let this_arg = args.get(1).copied().unwrap_or(JsValue::undefined());
        let len = vm.length_of_array_like(obj)?;
        let a = vm.array_species_create(obj, 0)?;
        let prev_temp = vm.temp_roots.len();
        vm.temp_roots.push(obj);
        vm.temp_roots.push(cb);
        vm.temp_roots.push(this_arg);
        vm.temp_roots.push(a);
        let mut to = 0usize;
        for i in 0..len {
            let cur_this = vm.temp_roots[prev_temp];
            if vm.has_element_result(cur_this, i)? {
                let cur_this = vm.temp_roots[prev_temp];
                let el = vm.get_element_result(cur_this, i)?;
                vm.temp_roots.push(el);
                let cur_this = vm.temp_roots[prev_temp];
                let cur_cb = vm.temp_roots[prev_temp + 1];
                let cur_this_arg = vm.temp_roots[prev_temp + 2];
                let rooted_el = *vm.temp_roots.last().unwrap();
                let keep = match vm.call(
                    cur_cb,
                    cur_this_arg,
                    &[rooted_el, index_to_js_value(i), cur_this],
                ) {
                    Ok(k) => k,
                    Err(e) => {
                        vm.temp_roots.truncate(prev_temp);
                        return Err(e);
                    }
                };
                if keep.is_truthy() {
                    let cur_a = vm.temp_roots[prev_temp + 3];
                    let to_key = to.to_string();
                    vm.create_data_property_or_throw(cur_a, &to_key, rooted_el)?;
                    to += 1;
                }
                vm.temp_roots.pop();
            }
        }
        let res_a = vm.temp_roots[prev_temp + 3];
        vm.temp_roots.truncate(prev_temp);
        Ok(res_a)
    });
    vm.set_property(array_proto, "filter", filter_fn);

    let concat_fn = vm.alloc_native_function("concat", 1, |vm, this, args| {
        let obj = vm.to_object_value(this)?;
        let a = vm.array_species_create(obj, 0)?;
        let prev_temp = vm.temp_roots.len();
        vm.temp_roots.push(obj);
        for &arg in args {
            vm.temp_roots.push(arg);
        }
        vm.temp_roots.push(a);
        let num_items = args.len() + 1;
        let sym_spread = vm.symbol_is_concat_spreadable;
        let mut n = 0usize;

        for item_idx in 0..num_items {
            let item = vm.temp_roots[prev_temp + item_idx];
            let is_spreadable = if item.as_object_ptr().is_none() {
                false
            } else if let Some(sym) = sym_spread {
                let spread_val = vm.get_symbol_property_result(item, sym)?;
                if !spread_val.is_undefined() {
                    spread_val.is_truthy()
                } else {
                    vm.is_array_result(item)?
                }
            } else {
                vm.is_array_result(item)?
            };

            let item = vm.temp_roots[prev_temp + item_idx];
            if is_spreadable {
                let len = vm.length_of_array_like(item)?;
                if (n as f64) + (len as f64) > 9007199254740991.0 {
                    vm.temp_roots.truncate(prev_temp);
                    return Err(VmError::TypeError(
                        "Array.prototype.concat length exceeds MAX_SAFE_INTEGER".into(),
                    ));
                }
                for i in 0..len {
                    let item = vm.temp_roots[prev_temp + item_idx];
                    let exists = vm.has_element_result(item, i)?;
                    if exists {
                        let item = vm.temp_roots[prev_temp + item_idx];
                        let el = vm.get_element_result(item, i)?;
                        let cur_a = vm.temp_roots[prev_temp + num_items];
                        vm.create_data_property_or_throw(cur_a, &n.to_string(), el)?;
                    }
                    n += 1;
                }
            } else {
                if (n as f64) + 1.0 > 9007199254740991.0 {
                    vm.temp_roots.truncate(prev_temp);
                    return Err(VmError::TypeError(
                        "Array.prototype.concat length exceeds MAX_SAFE_INTEGER".into(),
                    ));
                }
                let cur_a = vm.temp_roots[prev_temp + num_items];
                vm.create_data_property_or_throw(cur_a, &n.to_string(), item)?;
                n += 1;
            }
        }

        let res_a = vm.temp_roots[prev_temp + num_items];
        vm.set_property_throw(res_a, "length", JsValue::from_double(n as f64))?;
        vm.temp_roots.truncate(prev_temp);
        Ok(res_a)
    });
    vm.set_property(array_proto, "concat", concat_fn);

    let reverse_fn = vm.alloc_native_function("reverse", 0, |vm, this, _args| {
        let obj = vm.to_object_value(this)?;
        let len = vm.length_of_array_like(obj)?;
        if let Some(ptr) = obj.as_object_ptr() {
            let jsobj = unsafe { &mut *(ptr as *mut JsObject) };
            if jsobj.accessors.is_none() && !matches!(jsobj.kind, ObjectKind::Proxy { .. }) {
                if let ObjectKind::Array(ref mut elems) = jsobj.kind {
                    if elems.len() == len && !elems.iter().any(|v| v.is_hole()) {
                        elems.reverse();
                        return Ok(obj);
                    }
                }
            }
        }
        let prev_temp = vm.temp_roots.len();
        vm.temp_roots.push(obj);
        let mut lower = 0usize;
        let middle = len / 2;
        while lower < middle {
            let upper = len - lower - 1;
            let lower_s = lower.to_string();
            let upper_s = upper.to_string();
            let cur_this = vm.temp_roots[prev_temp];
            let lower_exists = vm.has_property(cur_this, &lower_s)?;
            let lower_val = if lower_exists {
                let cur_this = vm.temp_roots[prev_temp];
                vm.get_property_result(cur_this, &lower_s)?
            } else {
                JsValue::undefined()
            };
            let cur_this = vm.temp_roots[prev_temp];
            let upper_exists = vm.has_property(cur_this, &upper_s)?;
            let upper_val = if upper_exists {
                let cur_this = vm.temp_roots[prev_temp];
                vm.get_property_result(cur_this, &upper_s)?
            } else {
                JsValue::undefined()
            };

            let cur_this = vm.temp_roots[prev_temp];
            if lower_exists && upper_exists {
                vm.set_property_throw(cur_this, &lower_s, upper_val)?;
                let cur_this = vm.temp_roots[prev_temp];
                vm.set_property_throw(cur_this, &upper_s, lower_val)?;
            } else if !lower_exists && upper_exists {
                vm.set_property_throw(cur_this, &lower_s, upper_val)?;
                let cur_this = vm.temp_roots[prev_temp];
                vm.delete_property_or_throw(cur_this, &upper_s)?;
            } else if lower_exists && !upper_exists {
                vm.delete_property_or_throw(cur_this, &lower_s)?;
                let cur_this = vm.temp_roots[prev_temp];
                vm.set_property_throw(cur_this, &upper_s, lower_val)?;
            }
            lower += 1;
        }
        let cur_this = vm.temp_roots[prev_temp];
        vm.temp_roots.truncate(prev_temp);
        Ok(cur_this)
    });
    vm.set_property(array_proto, "reverse", reverse_fn);

    let reduce_fn = vm.alloc_native_function("reduce", 1, |vm, this, args| {
        let obj = vm.to_object_value(this)?;
        let cb = args.first().copied().unwrap_or(JsValue::undefined());
        if !vm.is_callable(cb) {
            return Err(VmError::TypeError("reduce callback must be callable".into()));
        }
        let len = vm.length_of_array_like(obj)?;

        let prev_temp = vm.temp_roots.len();
        vm.temp_roots.push(obj);
        vm.temp_roots.push(cb);

        let mut k = 0usize;
        let acc = if args.len() > 1 {
            args[1]
        } else {
            let mut found = None;
            while k < len {
                let cur_this = vm.temp_roots[prev_temp];
                if vm.has_element_result(cur_this, k)? {
                    let cur_this = vm.temp_roots[prev_temp];
                    let val = vm.get_element_result(cur_this, k)?;
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
                    return Err(VmError::TypeError(
                        "Reduce of empty array with no initial value".into(),
                    ));
                }
            }
        };

        vm.temp_roots.push(acc); // prev_temp + 2

        while k < len {
            let cur_this = vm.temp_roots[prev_temp];
            if vm.has_element_result(cur_this, k)? {
                let cur_this = vm.temp_roots[prev_temp];
                let el = vm.get_element_result(cur_this, k)?;
                vm.temp_roots.push(el); // prev_temp + 3
                let cur_this = vm.temp_roots[prev_temp];
                let cur_cb = vm.temp_roots[prev_temp + 1];
                let cur_acc = vm.temp_roots[prev_temp + 2];
                let cur_el = vm.temp_roots[prev_temp + 3];
                let res = vm.call(
                    cur_cb,
                    JsValue::undefined(),
                    &[cur_acc, cur_el, index_to_js_value(k), cur_this],
                );
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
        let obj = vm.to_object_value(this)?;
        let cb = args.first().copied().unwrap_or(JsValue::undefined());
        if !vm.is_callable(cb) {
            return Err(VmError::TypeError("some callback must be callable".into()));
        }
        let this_arg = args.get(1).copied().unwrap_or(JsValue::undefined());
        let len = vm.length_of_array_like(obj)?;
        let prev_temp = vm.temp_roots.len();
        vm.temp_roots.push(obj);
        vm.temp_roots.push(cb);
        vm.temp_roots.push(this_arg);
        for i in 0..len {
            let cur_this = vm.temp_roots[prev_temp];
            if vm.has_element_result(cur_this, i)? {
                let cur_this = vm.temp_roots[prev_temp];
                let el = vm.get_element_result(cur_this, i)?;
                vm.temp_roots.push(el);
                let cur_this = vm.temp_roots[prev_temp];
                let cur_cb = vm.temp_roots[prev_temp + 1];
                let cur_this_arg = vm.temp_roots[prev_temp + 2];
                let cur_el = vm.temp_roots[prev_temp + 3];
                let res = vm.call(cur_cb, cur_this_arg, &[cur_el, index_to_js_value(i), cur_this]);
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
        let obj = vm.to_object_value(this)?;
        let cb = args.first().copied().unwrap_or(JsValue::undefined());
        if !vm.is_callable(cb) {
            return Err(VmError::TypeError("every callback must be callable".into()));
        }
        let this_arg = args.get(1).copied().unwrap_or(JsValue::undefined());
        let len = vm.length_of_array_like(obj)?;
        let prev_temp = vm.temp_roots.len();
        vm.temp_roots.push(obj);
        vm.temp_roots.push(cb);
        vm.temp_roots.push(this_arg);
        for i in 0..len {
            let cur_this = vm.temp_roots[prev_temp];
            if vm.has_element_result(cur_this, i)? {
                let cur_this = vm.temp_roots[prev_temp];
                let el = vm.get_element_result(cur_this, i)?;
                vm.temp_roots.push(el);
                let cur_this = vm.temp_roots[prev_temp];
                let cur_cb = vm.temp_roots[prev_temp + 1];
                let cur_this_arg = vm.temp_roots[prev_temp + 2];
                let cur_el = vm.temp_roots[prev_temp + 3];
                let res = vm.call(cur_cb, cur_this_arg, &[cur_el, index_to_js_value(i), cur_this]);
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
        let obj = vm.to_object_value(this)?;
        let cb = args.first().copied().unwrap_or(JsValue::undefined());
        if !vm.is_callable(cb) {
            return Err(VmError::TypeError(
                "Array.prototype.find callback must be a function".into(),
            ));
        }
        let this_arg = args.get(1).copied().unwrap_or(JsValue::undefined());
        let len = vm.length_of_array_like(obj)?;
        let prev_temp = vm.temp_roots.len();
        vm.temp_roots.push(obj);
        vm.temp_roots.push(cb);
        vm.temp_roots.push(this_arg);
        for i in 0..len {
            let cur_this = vm.temp_roots[prev_temp];
            let mut el = vm.get_element_result(cur_this, i)?;
            if el.is_hole() {
                el = JsValue::undefined();
            }
            vm.temp_roots.push(el);
            let cur_this = vm.temp_roots[prev_temp];
            let cur_cb = vm.temp_roots[prev_temp + 1];
            let cur_this_arg = vm.temp_roots[prev_temp + 2];
            let cur_el = vm.temp_roots[prev_temp + 3];
            let res = vm.call(cur_cb, cur_this_arg, &[cur_el, index_to_js_value(i), cur_this]);
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
        let obj = vm.to_object_value(this)?;
        let cb = args.first().copied().unwrap_or(JsValue::undefined());
        if !vm.is_callable(cb) {
            return Err(VmError::TypeError(
                "Array.prototype.findIndex callback must be a function".into(),
            ));
        }
        let this_arg = args.get(1).copied().unwrap_or(JsValue::undefined());
        let len = vm.length_of_array_like(obj)?;
        let prev_temp = vm.temp_roots.len();
        vm.temp_roots.push(obj);
        vm.temp_roots.push(cb);
        vm.temp_roots.push(this_arg);
        for i in 0..len {
            let cur_this = vm.temp_roots[prev_temp];
            let mut el = vm.get_element_result(cur_this, i)?;
            if el.is_hole() {
                el = JsValue::undefined();
            }
            vm.temp_roots.push(el);
            let cur_this = vm.temp_roots[prev_temp];
            let cur_cb = vm.temp_roots[prev_temp + 1];
            let cur_this_arg = vm.temp_roots[prev_temp + 2];
            let cur_el = vm.temp_roots[prev_temp + 3];
            let res = vm.call(cur_cb, cur_this_arg, &[cur_el, index_to_js_value(i), cur_this]);
            vm.temp_roots.pop();
            match res {
                Ok(r) => {
                    if r.is_truthy() {
                        vm.temp_roots.truncate(prev_temp);
                        return Ok(index_to_js_value(i));
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
        let obj = vm.to_object_value(this)?;
        let cb = args.first().copied().unwrap_or(JsValue::undefined());
        if !vm.is_callable(cb) {
            return Err(VmError::TypeError(
                "Array.prototype.findLast callback must be a function".into(),
            ));
        }
        let this_arg = args.get(1).copied().unwrap_or(JsValue::undefined());
        let len = vm.length_of_array_like(obj)?;
        let prev_temp = vm.temp_roots.len();
        vm.temp_roots.push(obj);
        vm.temp_roots.push(cb);
        vm.temp_roots.push(this_arg);
        for i in (0..len).rev() {
            let cur_this = vm.temp_roots[prev_temp];
            let mut el = vm.get_element_result(cur_this, i)?;
            if el.is_hole() {
                el = JsValue::undefined();
            }
            vm.temp_roots.push(el);
            let cur_this = vm.temp_roots[prev_temp];
            let cur_cb = vm.temp_roots[prev_temp + 1];
            let cur_this_arg = vm.temp_roots[prev_temp + 2];
            let cur_el = vm.temp_roots[prev_temp + 3];
            let res = vm.call(cur_cb, cur_this_arg, &[cur_el, index_to_js_value(i), cur_this]);
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
        let obj = vm.to_object_value(this)?;
        let cb = args.first().copied().unwrap_or(JsValue::undefined());
        if !vm.is_callable(cb) {
            return Err(VmError::TypeError(
                "Array.prototype.findLastIndex callback must be a function".into(),
            ));
        }
        let this_arg = args.get(1).copied().unwrap_or(JsValue::undefined());
        let len = vm.length_of_array_like(obj)?;
        let prev_temp = vm.temp_roots.len();
        vm.temp_roots.push(obj);
        vm.temp_roots.push(cb);
        vm.temp_roots.push(this_arg);
        for i in (0..len).rev() {
            let cur_this = vm.temp_roots[prev_temp];
            let mut el = vm.get_element_result(cur_this, i)?;
            if el.is_hole() {
                el = JsValue::undefined();
            }
            vm.temp_roots.push(el);
            let cur_this = vm.temp_roots[prev_temp];
            let cur_cb = vm.temp_roots[prev_temp + 1];
            let cur_this_arg = vm.temp_roots[prev_temp + 2];
            let cur_el = vm.temp_roots[prev_temp + 3];
            let res = vm.call(cur_cb, cur_this_arg, &[cur_el, index_to_js_value(i), cur_this]);
            vm.temp_roots.pop();
            match res {
                Ok(r) => {
                    if r.is_truthy() {
                        vm.temp_roots.truncate(prev_temp);
                        return Ok(index_to_js_value(i));
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
        let source_len = vm.length_of_array_like(obj)?;
        let target = vm.array_species_create(obj, 0)?;
        flatten_into_array(vm, target, obj, source_len, 0, depth, None, JsValue::undefined())?;
        Ok(target)
    });
    vm.set_property(array_proto, "flat", flat_fn);

    let flat_map_fn = vm.alloc_native_function("flatMap", 1, |vm, this, args| {
        let obj = vm.to_object_value(this)?;
        let cb = args.first().copied().unwrap_or(JsValue::undefined());
        if !vm.is_callable(cb) {
            return Err(VmError::TypeError(
                "Array.prototype.flatMap callback must be a function".into(),
            ));
        }
        let this_arg = args.get(1).copied().unwrap_or(JsValue::undefined());
        let source_len = vm.length_of_array_like(obj)?;
        let target = vm.array_species_create(obj, 0)?;
        flatten_into_array(vm, target, obj, source_len, 0, 1, Some(cb), this_arg)?;
        Ok(target)
    });
    vm.set_property(array_proto, "flatMap", flat_map_fn);

    let sort_fn = vm.alloc_native_function("sort", 1, |vm, this, args| {
        let compare_fn = args.first().copied();
        if let Some(cf) = compare_fn {
            if !cf.is_undefined() && !vm.is_callable(cf) {
                return Err(VmError::TypeError(
                    "The comparison function must be either a function or undefined".into(),
                ));
            }
        }
        let obj = vm.to_object_value(this)?;
        let prev_temp = vm.temp_roots.len();
        vm.temp_roots.push(obj);
        let compare_fn_rooted = compare_fn.unwrap_or(JsValue::undefined());
        vm.temp_roots.push(compare_fn_rooted);

        let cur_this = vm.temp_roots[prev_temp];
        let len = vm.length_of_array_like(cur_this)?;
        let elements_start = vm.temp_roots.len();
        let cur_this = vm.temp_roots[prev_temp];
        for i in 0..len {
            let el = vm.get_element_result(cur_this, i)?;
            vm.temp_roots.push(el);
        }

        let res = sort_rooted_elements(vm, elements_start, len, prev_temp + 1);
        if let Err(e) = res {
            vm.temp_roots.truncate(prev_temp);
            return Err(e);
        }

        let cur_this = vm.temp_roots[prev_temp];
        if let Some(ptr) = cur_this.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if obj.accessors.is_none() && !matches!(obj.kind, ObjectKind::Proxy { .. }) {
                if let ObjectKind::Array(elements) = &mut obj.kind {
                    elements.clear();
                    for i in 0..len {
                        elements.push(vm.temp_roots[elements_start + i]);
                    }
                    vm.temp_roots.truncate(prev_temp);
                    return Ok(cur_this);
                }
            }
        }

        for i in 0..len {
            let el = vm.temp_roots[elements_start + i];
            let key = i.to_string();
            let cur_this = vm.temp_roots[prev_temp];
            if el.is_hole() {
                vm.delete_property_or_throw(cur_this, &key)?;
            } else {
                vm.set_property_throw(cur_this, &key, el)?;
            }
        }
        let cur_this = vm.temp_roots[prev_temp];
        vm.temp_roots.truncate(prev_temp);
        Ok(cur_this)
    });
    vm.set_property(array_proto, "sort", sort_fn);

    let to_sorted_fn = vm.alloc_native_function("toSorted", 1, |vm, this, args| {
        let compare_fn = args.first().copied();
        if let Some(cf) = compare_fn {
            if !cf.is_undefined() && !vm.is_callable(cf) {
                return Err(VmError::TypeError(
                    "The comparison function must be either a function or undefined".into(),
                ));
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
        let obj = vm.to_object_value(this)?;
        let cb = args.first().copied().unwrap_or(JsValue::undefined());
        if !vm.is_callable(cb) {
            return Err(VmError::TypeError(
                "Array.prototype.reduceRight callback must be a function".into(),
            ));
        }
        let len = vm.length_of_array_like(obj)?;

        let prev_temp = vm.temp_roots.len();
        vm.temp_roots.push(obj);
        vm.temp_roots.push(cb);

        let mut k = len;
        let acc = if args.len() > 1 {
            args[1]
        } else {
            let mut found = None;
            while k > 0 {
                k -= 1;
                let cur_this = vm.temp_roots[prev_temp];
                if vm.has_element_result(cur_this, k)? {
                    let cur_this = vm.temp_roots[prev_temp];
                    let val = vm.get_element_result(cur_this, k)?;
                    found = Some(val);
                    break;
                }
            }
            match found {
                Some(val) => val,
                None => {
                    vm.temp_roots.truncate(prev_temp);
                    return Err(VmError::TypeError(
                        "Reduce of empty array with no initial value".into(),
                    ));
                }
            }
        };

        vm.temp_roots.push(acc); // prev_temp + 2

        while k > 0 {
            k -= 1;
            let cur_this = vm.temp_roots[prev_temp];
            if vm.has_element_result(cur_this, k)? {
                let cur_this = vm.temp_roots[prev_temp];
                let el = vm.get_element_result(cur_this, k)?;
                vm.temp_roots.push(el); // prev_temp + 3
                let cur_this = vm.temp_roots[prev_temp];
                let cur_cb = vm.temp_roots[prev_temp + 1];
                let cur_acc = vm.temp_roots[prev_temp + 2];
                let cur_el = vm.temp_roots[prev_temp + 3];
                let res = vm.call(
                    cur_cb,
                    JsValue::undefined(),
                    &[cur_acc, cur_el, index_to_js_value(k), cur_this],
                );
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
                    let n = match vm.to_number_value(res) {
                        Ok(num) => num,
                        Err(e) => {
                            err = Some(e);
                            return std::cmp::Ordering::Equal;
                        }
                    };
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
            if a.is_symbol() || b.is_symbol() {
                err = Some(VmError::TypeError("Cannot convert a Symbol value to a string".into()));
                return std::cmp::Ordering::Equal;
            }
            let sa = vm.to_string_value(a);
            let sb = vm.to_string_value(b);
            sa.cmp(&sb)
        }
    });

    if let Some(e) = err {
        return Err(e);
    }

    let sorted: Vec<JsValue> =
        indices.iter().map(|&idx| vm.temp_roots[elements_start + idx]).collect();
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
                element = match vm.call(
                    cur_mapper,
                    cur_this_arg,
                    &[element, JsValue::from_int32(source_idx as i32), cur_source],
                ) {
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
                target_idx = flatten_into_array(
                    vm,
                    cur_target,
                    element,
                    el_len,
                    target_idx,
                    next_depth,
                    None,
                    JsValue::undefined(),
                )?;
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
