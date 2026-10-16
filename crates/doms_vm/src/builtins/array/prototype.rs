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
                    if *el == search {
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
                    if *el == search {
                        return Ok(JsValue::from_boolean(true));
                    }
                }
            }
        }
        Ok(JsValue::from_boolean(false))
    });
    vm.set_property(array_proto, "includes", includes_fn);
    array_proto
}
