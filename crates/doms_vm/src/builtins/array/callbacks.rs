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
}
