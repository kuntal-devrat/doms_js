//! `Array.prototype` methods taking callbacks or transforming arrays
//! (`forEach`, `filter`, `concat`, `reverse`, `reduce`, `some`, `every`).

use super::*;

pub(super) fn register_callback_methods(vm: &mut Vm, array_proto: JsValue) {
    let for_each_fn = vm.alloc_native_function("forEach", 1, |vm, this, args| {
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
        for i in 0..len {
            let el = vm.get_element(this, i);
            let _ = vm.call(cb, JsValue::undefined(), &[el, JsValue::from_int32(i as i32), this])?;
        }
        Ok(JsValue::undefined())
    });
    vm.set_property(array_proto, "forEach", for_each_fn);

    let filter_fn = vm.alloc_native_function("filter", 1, |vm, this, args| {
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
        let mut filtered = Vec::new();
        for i in 0..len {
            let el = vm.get_element(this, i);
            let keep = vm.call(cb, JsValue::undefined(), &[el, JsValue::from_int32(i as i32), this])?;
            if keep.is_truthy() {
                filtered.push(el);
            }
        }
        Ok(vm.alloc_array(filtered))
    });
    vm.set_property(array_proto, "filter", filter_fn);

    let concat_fn = vm.alloc_native_function("concat", 1, |vm, this, args| {
        let mut out = Vec::new();
        let sym_spread = vm.symbol_is_concat_spreadable;
        let mut items = Vec::with_capacity(args.len() + 1);
        items.push(this);
        items.extend_from_slice(args);

        for item in items {
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

            if is_spreadable {
                let len_val = vm.get_property(item, "length");
                let len = len_val.to_number().unwrap_or(0.0) as usize;
                if len > 100_000 {
                    return Err(VmError::RangeError("Invalid array length".into()));
                }
                for i in 0..len {
                    out.push(vm.get_element(item, i));
                }
            } else {
                out.push(item);
            }
        }

        if out.len() > 100_000 {
            return Err(VmError::RangeError("Invalid array length".into()));
        }
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
        let mut i = 0;
        let mut j = len.saturating_sub(1);
        while i < j {
            let vi = vm.get_element(this, i);
            let vj = vm.get_element(this, j);
            vm.set_element(this, i, vj);
            vm.set_element(this, j, vi);
            i += 1;
            j = j.saturating_sub(1);
        }
        Ok(this)
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
        let (mut acc, start_i) = if args.len() > 1 {
            (args[1], 0)
        } else if len > 0 {
            (vm.get_element(this, 0), 1)
        } else {
            return Err(VmError::TypeError("Reduce of empty array with no initial value".into()));
        };
        for i in start_i..len {
            let el = vm.get_element(this, i);
            acc = vm.call(cb, JsValue::undefined(), &[acc, el, JsValue::from_int32(i as i32), this])?;
        }
        Ok(acc)
    });
    vm.set_property(array_proto, "reduce", reduce_fn);

    let some_fn = vm.alloc_native_function("some", 1, |vm, this, args| {
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
        for i in 0..len {
            let el = vm.get_element(this, i);
            let res = vm.call(cb, JsValue::undefined(), &[el, JsValue::from_int32(i as i32), this])?;
            if res.is_truthy() {
                return Ok(JsValue::from_boolean(true));
            }
        }
        Ok(JsValue::from_boolean(false))
    });
    vm.set_property(array_proto, "some", some_fn);

    let every_fn = vm.alloc_native_function("every", 1, |vm, this, args| {
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
        for i in 0..len {
            let el = vm.get_element(this, i);
            let res = vm.call(cb, JsValue::undefined(), &[el, JsValue::from_int32(i as i32), this])?;
            if !res.is_truthy() {
                return Ok(JsValue::from_boolean(false));
            }
        }
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
        for i in 0..len {
            let el = vm.get_element(this, i);
            let res = vm.call(cb, this_arg, &[el, JsValue::from_int32(i as i32), this])?;
            if res.is_truthy() {
                return Ok(el);
            }
        }
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
        for i in 0..len {
            let el = vm.get_element(this, i);
            let res = vm.call(cb, this_arg, &[el, JsValue::from_int32(i as i32), this])?;
            if res.is_truthy() {
                return Ok(JsValue::from_int32(i as i32));
            }
        }
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
        for i in (0..len).rev() {
            let el = vm.get_element(this, i);
            let res = vm.call(cb, this_arg, &[el, JsValue::from_int32(i as i32), this])?;
            if res.is_truthy() {
                return Ok(el);
            }
        }
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
        for i in (0..len).rev() {
            let el = vm.get_element(this, i);
            let res = vm.call(cb, this_arg, &[el, JsValue::from_int32(i as i32), this])?;
            if res.is_truthy() {
                return Ok(JsValue::from_int32(i as i32));
            }
        }
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
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::Array(elements) = &mut obj.kind {
                sort_elements(vm, elements, compare_fn)?;
                return Ok(this);
            }
        }
        let len_val = vm.get_property(this, "length");
        let len = len_val.to_number().unwrap_or(0.0).max(0.0) as usize;
        let mut elements = Vec::with_capacity(len);
        for i in 0..len {
            elements.push(vm.get_element_result(this, i)?);
        }
        sort_elements(vm, &mut elements, compare_fn)?;
        for (i, el) in elements.into_iter().enumerate() {
            vm.set_element(this, i, el);
        }
        Ok(this)
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
        for i in 0..len {
            let mut val = vm.get_element_result(obj, i)?;
            if val.is_hole() {
                val = JsValue::undefined();
            }
            elements.push(val);
        }
        sort_elements(vm, &mut elements, compare_fn)?;
        Ok(vm.alloc_array(elements))
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

        let (mut acc, start_idx) = if args.len() >= 2 {
            (args[1], len as i64 - 1)
        } else {
            let mut found = None;
            let mut idx = len as i64 - 1;
            while idx >= 0 {
                if vm.has_own_property(this, &idx.to_string()) {
                    found = Some((vm.get_element(this, idx as usize), idx - 1));
                    break;
                }
                idx -= 1;
            }
            match found {
                Some((val, next_idx)) => (val, next_idx),
                None => return Err(VmError::TypeError("Reduce of empty array with no initial value".into())),
            }
        };

        let mut i = start_idx;
        while i >= 0 {
            if vm.has_own_property(this, &i.to_string()) {
                let el = vm.get_element(this, i as usize);
                acc = vm.call(cb, JsValue::undefined(), &[acc, el, JsValue::from_int32(i as i32), this])?;
            }
            i -= 1;
        }
        Ok(acc)
    });
    vm.set_property(array_proto, "reduceRight", reduce_right_fn);
}

fn sort_elements(vm: &mut Vm, elements: &mut [JsValue], compare_fn: Option<JsValue>) -> Result<(), VmError> {
    let has_cb = compare_fn.is_some_and(|cf| !cf.is_undefined() && vm.is_callable(cf));
    let cb = compare_fn.unwrap_or(JsValue::undefined());
    let mut err = None;

    elements.sort_by(|&a, &b| {
        if err.is_some() {
            return std::cmp::Ordering::Equal;
        }
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
            match vm.call(cb, JsValue::undefined(), &[a, b]) {
                Ok(res) => {
                    let n = res.to_number().unwrap_or(0.0);
                    if n < 0.0 {
                        std::cmp::Ordering::Less
                    } else if n > 0.0 {
                        std::cmp::Ordering::Greater
                    } else {
                        std::cmp::Ordering::Equal
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
    for source_idx in 0..source_len {
        let p = source_idx.to_string();
        if vm.has_property(source, &p)? {
            let mut element = vm.get_property_result(source, &p)?;
            if let Some(mapper) = mapper_fn {
                element = vm.call(mapper, this_arg, &[element, JsValue::from_int32(source_idx as i32), source])?;
            }
            let should_flatten = depth > 0 && vm.is_array(element);
            if should_flatten {
                let next_depth = if depth == i64::MAX { depth } else { depth - 1 };
                let el_len_val = vm.get_property_result(element, "length")?;
                let el_len = el_len_val.to_number().unwrap_or(0.0).max(0.0) as usize;
                target_idx = flatten_into_array(vm, target, element, el_len, target_idx, next_depth, None, JsValue::undefined())?;
            } else {
                if target_idx > 9007199254740991 {
                    return Err(VmError::TypeError("Target index exceeds 2^53 - 1".into()));
                }
                let t_key = target_idx.to_string();
                vm.create_data_property_or_throw(target, &t_key, element)?;
                target_idx += 1;
            }
        }
    }
    Ok(target_idx)
}

