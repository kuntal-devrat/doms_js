//! Shared `TypedArray.prototype` methods and accessors.

use super::*;

pub(super) fn validate_typed_array(this: JsValue) -> Result<JsTypedArray, VmError> {
    if let Some(ptr) = this.as_object_ptr() {
        let obj = unsafe { &*(ptr as *mut JsObject) };
        if let ObjectKind::TypedArray(ta) = &obj.kind {
            if ta.buffer.is_detached() {
                return Err(VmError::TypeError("TypedArray buffer is detached".into()));
            }
            return Ok(ta.clone());
        }
    }
    Err(VmError::TypeError("Method called on non-TypedArray".into()))
}

pub(super) fn register_prototype_methods(vm: &mut Vm, proto: JsValue) {
    // 1. Getters
    let buffer_getter = vm.alloc_native_function("get buffer", 0, |vm, this, _args| {
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::TypedArray(ta) = &obj.kind {
                return Ok(vm.alloc_array_buffer(Arc::clone(&ta.buffer)));
            }
        }
        Err(VmError::TypeError("get TypedArray.prototype.buffer called on non-TypedArray".into()))
    });

    let byte_length_getter = vm.alloc_native_function("get byteLength", 0, |_vm, this, _args| {
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::TypedArray(ta) = &obj.kind {
                if ta.buffer.is_detached() {
                    return Ok(JsValue::from_int32(0));
                }
                return Ok(JsValue::from_int32((ta.length * ta.kind.bytes_per_element()) as i32));
            }
        }
        Err(VmError::TypeError("get TypedArray.prototype.byteLength called on non-TypedArray".into()))
    });

    let byte_offset_getter = vm.alloc_native_function("get byteOffset", 0, |_vm, this, _args| {
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::TypedArray(ta) = &obj.kind {
                if ta.buffer.is_detached() {
                    return Ok(JsValue::from_int32(0));
                }
                return Ok(JsValue::from_int32(ta.byte_offset as i32));
            }
        }
        Err(VmError::TypeError("get TypedArray.prototype.byteOffset called on non-TypedArray".into()))
    });

    let length_getter = vm.alloc_native_function("get length", 0, |_vm, this, _args| {
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::TypedArray(ta) = &obj.kind {
                if ta.buffer.is_detached() {
                    return Ok(JsValue::from_int32(0));
                }
                return Ok(JsValue::from_int32(ta.length as i32));
            }
        }
        Err(VmError::TypeError("get TypedArray.prototype.length called on non-TypedArray".into()))
    });

    if let Some(ptr) = proto.as_object_ptr() {
        let obj = unsafe { &mut *(ptr as *mut JsObject) };
        obj.define_getter("buffer".to_string(), buffer_getter);
        obj.define_getter("byteLength".to_string(), byte_length_getter);
        obj.define_getter("byteOffset".to_string(), byte_offset_getter);
        obj.define_getter("length".to_string(), length_getter);
    }
    vm.set_property_non_enumerable(proto, "buffer".to_string());
    vm.set_property_non_enumerable(proto, "byteLength".to_string());
    vm.set_property_non_enumerable(proto, "byteOffset".to_string());
    vm.set_property_non_enumerable(proto, "length".to_string());

    if let Some(sym_tag) = vm.symbol_to_string_tag {
        let tag_getter = vm.alloc_native_function("get [Symbol.toStringTag]", 0, |vm, this, _args| {
            if let Some(ptr) = this.as_object_ptr() {
                let obj = unsafe { &*(ptr as *mut JsObject) };
                if let ObjectKind::TypedArray(ta) = &obj.kind {
                    return Ok(vm.alloc_string(ta.kind.name().to_string()));
                }
            }
            Ok(JsValue::undefined())
        });
        if let Some(ptr) = proto.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            obj.insert_symbol_accessor_getter(sym_tag, tag_getter);
        }
    }

    // 2. Methods
    let subarray_fn = vm.alloc_native_function("subarray", 2, |vm, this, args| {
        let ta = validate_typed_array(this)?;
        let len = ta.length as i32;
        let start = args.first().and_then(|v| v.to_int32()).unwrap_or(0);
        let end = args.get(1).and_then(|v| v.to_int32()).unwrap_or(len);
        let s = if start < 0 { (len + start).max(0) } else { start.min(len) } as usize;
        let e = if end < 0 { (len + end).max(0) } else { end.min(len) } as usize;
        let count = if e > s { e - s } else { 0 };
        let bpe = ta.kind.bytes_per_element();
        let byte_offset = ta.byte_offset + s * bpe;
        Ok(vm.alloc_typed_array(JsTypedArray {
            buffer: Arc::clone(&ta.buffer),
            kind: ta.kind,
            length: count,
            byte_offset,
        }))
    });
    vm.set_property(proto, "subarray", subarray_fn);
    vm.set_property_non_enumerable(proto, "subarray".to_string());

    let set_fn = vm.alloc_native_function("set", 1, |vm, this, args| {
        let _ = validate_typed_array(this)?;
        let src = args.first().copied().unwrap_or(JsValue::undefined());
        let offset = args.get(1).and_then(|v| v.to_int32()).unwrap_or(0).max(0) as usize;
        let prev_temp = vm.temp_roots.len();
        vm.temp_roots.push(this);
        vm.temp_roots.push(src);

        let dst_ta = if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::TypedArray(ref ta) = obj.kind {
                ta.clone()
            } else {
                vm.temp_roots.truncate(prev_temp);
                return Err(VmError::TypeError("Method called on non-TypedArray".into()));
            }
        } else {
            vm.temp_roots.truncate(prev_temp);
            return Err(VmError::TypeError("Method called on non-TypedArray".into()));
        };

        let cur_src = vm.temp_roots[prev_temp + 1];
        if let Some(src_ptr) = cur_src.as_object_ptr() {
            let src_obj = unsafe { &*(src_ptr as *mut JsObject) };
            if let ObjectKind::TypedArray(src_ta) = &src_obj.kind {
                let src_ta = src_ta.clone();
                let src_is_big = matches!(src_ta.kind, TypedArrayKind::BigInt64 | TypedArrayKind::BigUint64);
                let dst_is_big = matches!(dst_ta.kind, TypedArrayKind::BigInt64 | TypedArrayKind::BigUint64);
                if src_is_big != dst_is_big {
                    vm.temp_roots.truncate(prev_temp);
                    return Err(VmError::TypeError("Cannot mix BigInt and other types in TypedArray.prototype.set".into()));
                }
                let src_len = src_ta.length;
                if offset + src_len > dst_ta.length {
                    vm.temp_roots.truncate(prev_temp);
                    return Err(VmError::RangeError("Source is too large for target array".into()));
                }
                let vals_start = vm.temp_roots.len();
                for i in 0..src_len {
                    let val = vm.read_typed_array_element_vm(&src_ta, i).unwrap_or(JsValue::undefined());
                    vm.temp_roots.push(val);
                }
                for i in 0..src_len {
                    let val = vm.temp_roots[vals_start + i];
                    vm.write_typed_array_element_vm(&dst_ta, offset + i, val)?;
                }
                vm.temp_roots.truncate(prev_temp);
                return Ok(JsValue::undefined());
            }
        }

        let cur_src = vm.temp_roots[prev_temp + 1];
        let len = vm.get_property(cur_src, "length").to_number().unwrap_or(0.0) as usize;
        if offset + len > dst_ta.length {
            vm.temp_roots.truncate(prev_temp);
            return Err(VmError::RangeError("Source is too large for target array".into()));
        }

        let vals_start = vm.temp_roots.len();
        for i in 0..len {
            let cur_src = vm.temp_roots[prev_temp + 1];
            let val = vm.get_element(cur_src, i);
            vm.temp_roots.push(val);
        }

        for i in 0..len {
            let val = vm.temp_roots[vals_start + i];
            vm.write_typed_array_element_vm(&dst_ta, offset + i, val)?;
        }
        vm.temp_roots.truncate(prev_temp);
        Ok(JsValue::undefined())
    });
    vm.set_property(proto, "set", set_fn);
    vm.set_property_non_enumerable(proto, "set".to_string());

    let slice_fn = vm.alloc_native_function("slice", 2, |vm, this, args| {
        let ta = validate_typed_array(this)?;
        let len = ta.length as i32;
        let start = args.first().and_then(|v| v.to_int32()).unwrap_or(0);
        let end = args.get(1).and_then(|v| v.to_int32()).unwrap_or(len);
        let s = if start < 0 { (len + start).max(0) } else { start.min(len) } as usize;
        let e = if end < 0 { (len + end).max(0) } else { end.min(len) } as usize;
        let count = if e > s { e - s } else { 0 };
        let bpe = ta.kind.bytes_per_element();
        let new_sab = doms_parallel::SharedArrayBuffer::try_new(count * bpe)
            .map_err(|e| VmError::RuntimeError(e.into()))?;
        let mut dst_ta = JsTypedArray {
            buffer: Arc::new(new_sab),
            kind: ta.kind,
            length: count,
            byte_offset: 0,
        };
        for i in 0..count {
            if let Some(val) = vm.read_typed_array_element_vm(&ta, s + i) {
                vm.write_typed_array_element_vm(&mut dst_ta, i, val)?;
            }
        }
        Ok(vm.alloc_typed_array(dst_ta))
    });
    vm.set_property(proto, "slice", slice_fn);
    vm.set_property_non_enumerable(proto, "slice".to_string());

    let fill_fn = vm.alloc_native_function("fill", 1, |vm, this, args| {
        let ta = validate_typed_array(this)?;
        let value = args.first().copied().unwrap_or(JsValue::undefined());
        let len = ta.length as i32;
        let start = args.get(1).and_then(|v| v.to_int32()).unwrap_or(0);
        let end = args.get(2).and_then(|v| v.to_int32()).unwrap_or(len);
        let s = if start < 0 { (len + start).max(0) } else { start.min(len) } as usize;
        let e = if end < 0 { (len + end).max(0) } else { end.min(len) } as usize;
        let ta_clone = ta.clone();
        for i in s..e {
            vm.write_typed_array_element_vm(&ta_clone, i, value)?;
        }
        Ok(this)
    });
    vm.set_property(proto, "fill", fill_fn);
    vm.set_property_non_enumerable(proto, "fill".to_string());

    let index_of_fn = vm.alloc_native_function("indexOf", 1, |vm, this, args| {
        let ta = validate_typed_array(this)?;
        let target = args.first().copied().unwrap_or(JsValue::undefined());
        let from_idx = args.get(1).and_then(|v| v.to_int32()).unwrap_or(0).max(0) as usize;
        for i in from_idx..ta.length {
            if let Some(val) = vm.read_typed_array_element_vm(&ta, i) {
                if vm.strict_equal(val, target) {
                    return Ok(JsValue::from_int32(i as i32));
                }
            }
        }
        Ok(JsValue::from_int32(-1))
    });
    vm.set_property(proto, "indexOf", index_of_fn);
    vm.set_property_non_enumerable(proto, "indexOf".to_string());

    let last_index_of_fn = vm.alloc_native_function("lastIndexOf", 1, |vm, this, args| {
        let ta = validate_typed_array(this)?;
        let target = args.first().copied().unwrap_or(JsValue::undefined());
        let len = ta.length;
        if len == 0 {
            return Ok(JsValue::from_int32(-1));
        }
        let from_idx = if let Some(arg) = args.get(1) {
            let n = arg.to_int32().unwrap_or(len as i32 - 1);
            if n < 0 {
                let k = len as i32 + n;
                if k < 0 { return Ok(JsValue::from_int32(-1)); }
                k as usize
            } else {
                (n as usize).min(len - 1)
            }
        } else {
            len - 1
        };
        for i in (0..=from_idx).rev() {
            if let Some(val) = vm.read_typed_array_element_vm(&ta, i) {
                if vm.strict_equal(val, target) {
                    return Ok(JsValue::from_int32(i as i32));
                }
            }
        }
        Ok(JsValue::from_int32(-1))
    });
    vm.set_property(proto, "lastIndexOf", last_index_of_fn);
    vm.set_property_non_enumerable(proto, "lastIndexOf".to_string());

    let includes_fn = vm.alloc_native_function("includes", 1, |vm, this, args| {
        let ta = validate_typed_array(this)?;
        let target = args.first().copied().unwrap_or(JsValue::undefined());
        let from_idx = args.get(1).and_then(|v| v.to_int32()).unwrap_or(0).max(0) as usize;
        for i in from_idx..ta.length {
            if let Some(val) = vm.read_typed_array_element_vm(&ta, i) {
                if vm.same_value_zero(val, target) {
                    return Ok(JsValue::from_boolean(true));
                }
            }
        }
        Ok(JsValue::from_boolean(false))
    });
    vm.set_property(proto, "includes", includes_fn);
    vm.set_property_non_enumerable(proto, "includes".to_string());

    let join_fn = vm.alloc_native_function("join", 1, |vm, this, args| {
        let ta = validate_typed_array(this)?;
        let sep = args.first().map(|v| vm.to_string_value(*v)).unwrap_or_else(|| ",".to_string());
        let mut parts = Vec::with_capacity(ta.length);
        for i in 0..ta.length {
            if let Some(val) = vm.read_typed_array_element_vm(&ta, i) {
                parts.push(vm.to_string_value(val));
            }
        }
        Ok(vm.alloc_string(parts.join(&sep)))
    });
    vm.set_property(proto, "join", join_fn);
    vm.set_property_non_enumerable(proto, "join".to_string());
    vm.set_property(proto, "toString", join_fn);
    vm.set_property_non_enumerable(proto, "toString".to_string());
    vm.set_property(proto, "toLocaleString", join_fn);
    vm.set_property_non_enumerable(proto, "toLocaleString".to_string());

    let values_fn = vm.alloc_native_function("values", 0, |vm, this, _args| {
        validate_typed_array(this)?;
        create_array_iterator(vm, this, IterKind::Values)
    });
    let keys_fn = vm.alloc_native_function("keys", 0, |vm, this, _args| {
        validate_typed_array(this)?;
        create_array_iterator(vm, this, IterKind::Keys)
    });
    let entries_fn = vm.alloc_native_function("entries", 0, |vm, this, _args| {
        validate_typed_array(this)?;
        create_array_iterator(vm, this, IterKind::Entries)
    });
    vm.set_property(proto, "values", values_fn);
    vm.set_property_non_enumerable(proto, "values".to_string());
    vm.set_property(proto, "keys", keys_fn);
    vm.set_property_non_enumerable(proto, "keys".to_string());
    vm.set_property(proto, "entries", entries_fn);
    vm.set_property_non_enumerable(proto, "entries".to_string());
    if let Some(sym_iter) = vm.symbol_iterator {
        vm.set_symbol_property(proto, sym_iter, values_fn);
    }

    let at_fn = vm.alloc_native_function("at", 1, |vm, this, args| {
        let ta = validate_typed_array(this)?;
        let len = ta.length as i64;
        let idx = args.first().and_then(|v| v.to_number()).unwrap_or(0.0) as i64;
        let k = if idx < 0 { len + idx } else { idx };
        if k >= 0 && k < len {
            if let Some(val) = vm.read_typed_array_element_vm(&ta, k as usize) {
                return Ok(val);
            }
        }
        Ok(JsValue::undefined())
    });
    vm.set_property(proto, "at", at_fn);
    vm.set_property_non_enumerable(proto, "at".to_string());

    let reverse_fn = vm.alloc_native_function("reverse", 0, |vm, this, _args| {
        let ta = validate_typed_array(this)?;
        let len = ta.length;
        let prev_temp = vm.temp_roots.len();
        for i in 0..len {
            let val = vm.read_typed_array_element_vm(&ta, i).unwrap_or(JsValue::undefined());
            vm.temp_roots.push(val);
        }
        let mut elements = Vec::with_capacity(len);
        for i in 0..len {
            elements.push(vm.temp_roots[prev_temp + i]);
        }
        elements.reverse();
        for (i, val) in elements.into_iter().enumerate() {
            vm.write_typed_array_element_vm(&ta, i, val)?;
        }
        vm.temp_roots.truncate(prev_temp);
        Ok(this)
    });
    vm.set_property(proto, "reverse", reverse_fn);
    vm.set_property_non_enumerable(proto, "reverse".to_string());

    let to_reversed_fn = vm.alloc_native_function("toReversed", 0, |vm, this, _args| {
        let ta = validate_typed_array(this)?;
        let len = ta.length;
        let prev_temp = vm.temp_roots.len();
        for i in 0..len {
            let val = vm.read_typed_array_element_vm(&ta, i).unwrap_or(JsValue::undefined());
            vm.temp_roots.push(val);
        }
        let mut elements = Vec::with_capacity(len);
        for i in 0..len {
            elements.push(vm.temp_roots[prev_temp + i]);
        }
        elements.reverse();
        let bpe = ta.kind.bytes_per_element();
        let new_sab = doms_parallel::SharedArrayBuffer::try_new(len * bpe)
            .map_err(|e| VmError::RuntimeError(e.into()))?;
        let mut dst_ta = JsTypedArray {
            buffer: Arc::new(new_sab),
            kind: ta.kind,
            length: len,
            byte_offset: 0,
        };
        for (i, val) in elements.into_iter().enumerate() {
            vm.write_typed_array_element_vm(&mut dst_ta, i, val)?;
        }
        vm.temp_roots.truncate(prev_temp);
        Ok(vm.alloc_typed_array(dst_ta))
    });
    vm.set_property(proto, "toReversed", to_reversed_fn);
    vm.set_property_non_enumerable(proto, "toReversed".to_string());

    let with_fn = vm.alloc_native_function("with", 2, |vm, this, args| {
        let ta = validate_typed_array(this)?;
        let len = ta.length as i64;
        let raw_idx = args.first().and_then(|v| v.to_number()).unwrap_or(0.0) as i64;
        let k = if raw_idx < 0 { len + raw_idx } else { raw_idx };
        if k < 0 || k >= len {
            return Err(VmError::RangeError("Invalid index".into()));
        }
        let value = args.get(1).copied().unwrap_or(JsValue::undefined());
        let len = ta.length;
        let bpe = ta.kind.bytes_per_element();
        let new_sab = doms_parallel::SharedArrayBuffer::try_new(len * bpe)
            .map_err(|e| VmError::RuntimeError(e.into()))?;
        let mut dst_ta = JsTypedArray {
            buffer: Arc::new(new_sab),
            kind: ta.kind,
            length: len,
            byte_offset: 0,
        };
        for i in 0..len {
            let val = if i == k as usize { value } else { vm.read_typed_array_element_vm(&ta, i).unwrap_or(JsValue::undefined()) };
            vm.write_typed_array_element_vm(&mut dst_ta, i, val)?;
        }
        Ok(vm.alloc_typed_array(dst_ta))
    });
    vm.set_property(proto, "with", with_fn);
    vm.set_property_non_enumerable(proto, "with".to_string());

    let copy_within_fn = vm.alloc_native_function("copyWithin", 2, |vm, this, args| {
        let ta = validate_typed_array(this)?;
        let len = ta.length as i64;
        let target = args.first().and_then(|v| v.to_int32()).unwrap_or(0) as i64;
        let start = args.get(1).and_then(|v| v.to_int32()).unwrap_or(0) as i64;
        let end = args.get(2).and_then(|v| v.to_int32()).unwrap_or(len as i32) as i64;
        let to = if target < 0 { (len + target).max(0) } else { target.min(len) };
        let from = if start < 0 { (len + start).max(0) } else { start.min(len) };
        let final_idx = if end < 0 { (len + end).max(0) } else { end.min(len) };
        let count = (final_idx - from).min(len - to);
        if count > 0 {
            let prev_temp = vm.temp_roots.len();
            for i in 0..count {
                let val = vm.read_typed_array_element_vm(&ta, (from + i) as usize).unwrap_or(JsValue::undefined());
                vm.temp_roots.push(val);
            }
            for i in 0..count {
                let val = vm.temp_roots[prev_temp + i as usize];
                vm.write_typed_array_element_vm(&ta, (to + i as i64) as usize, val)?;
            }
            vm.temp_roots.truncate(prev_temp);
        }
        Ok(this)
    });
    vm.set_property(proto, "copyWithin", copy_within_fn);
    vm.set_property_non_enumerable(proto, "copyWithin".to_string());

    let for_each_fn = vm.alloc_native_function("forEach", 1, |vm, this, args| {
        let callback = args.first().copied().unwrap_or(JsValue::undefined());
        if !vm.is_callable(callback) {
            return Err(VmError::TypeError("Callback must be callable".into()));
        }
        let this_arg = args.get(1).copied().unwrap_or(JsValue::undefined());
        let ta = validate_typed_array(this)?;
        let len = ta.length;
        let prev_temp = vm.temp_roots.len();
        vm.temp_roots.push(this);
        vm.temp_roots.push(callback);
        vm.temp_roots.push(this_arg);
        for i in 0..len {
            let cur_this = vm.temp_roots[prev_temp];
            let cur_cb = vm.temp_roots[prev_temp + 1];
            let cur_this_arg = vm.temp_roots[prev_temp + 2];
            let val = if let Some(ptr) = cur_this.as_object_ptr() {
                let obj = unsafe { &*(ptr as *mut JsObject) };
                if let ObjectKind::TypedArray(ta) = &obj.kind {
                    let ta_clone = ta.clone();
                    vm.read_typed_array_element_vm(&ta_clone, i).unwrap_or(JsValue::undefined())
                } else {
                    JsValue::undefined()
                }
            } else {
                JsValue::undefined()
            };
            vm.call(cur_cb, cur_this_arg, &[val, JsValue::from_int32(i as i32), cur_this])?;
        }
        vm.temp_roots.truncate(prev_temp);
        Ok(JsValue::undefined())
    });
    vm.set_property(proto, "forEach", for_each_fn);
    vm.set_property_non_enumerable(proto, "forEach".to_string());

    let map_fn = vm.alloc_native_function("map", 1, |vm, this, args| {
        let callback = args.first().copied().unwrap_or(JsValue::undefined());
        if !vm.is_callable(callback) {
            return Err(VmError::TypeError("Callback must be callable".into()));
        }
        let this_arg = args.get(1).copied().unwrap_or(JsValue::undefined());
        let ta = validate_typed_array(this)?;
        let len = ta.length;
        let bpe = ta.kind.bytes_per_element();
        let prev_temp = vm.temp_roots.len();
        vm.temp_roots.push(this);
        vm.temp_roots.push(callback);
        vm.temp_roots.push(this_arg);
        let mapped_start = vm.temp_roots.len();
        for i in 0..len {
            let cur_this = vm.temp_roots[prev_temp];
            let cur_cb = vm.temp_roots[prev_temp + 1];
            let cur_this_arg = vm.temp_roots[prev_temp + 2];
            let val = if let Some(ptr) = cur_this.as_object_ptr() {
                let obj = unsafe { &*(ptr as *mut JsObject) };
                if let ObjectKind::TypedArray(ta) = &obj.kind {
                    let ta_clone = ta.clone();
                    vm.read_typed_array_element_vm(&ta_clone, i).unwrap_or(JsValue::undefined())
                } else {
                    JsValue::undefined()
                }
            } else {
                JsValue::undefined()
            };
            let mapped = vm.call(cur_cb, cur_this_arg, &[val, JsValue::from_int32(i as i32), cur_this])?;
            vm.temp_roots.push(mapped);
        }
        let cur_this = vm.temp_roots[prev_temp];
        let kind = if let Some(ptr) = cur_this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::TypedArray(ta) = &obj.kind { ta.kind } else { TypedArrayKind::Uint8 }
        } else {
            TypedArrayKind::Uint8
        };
        let new_sab = doms_parallel::SharedArrayBuffer::try_new(len * bpe)
            .map_err(|e| VmError::RuntimeError(e.into()))?;
        let mut dst_ta = JsTypedArray {
            buffer: Arc::new(new_sab),
            kind,
            length: len,
            byte_offset: 0,
        };
        for i in 0..len {
            let val = vm.temp_roots[mapped_start + i];
            vm.write_typed_array_element_vm(&mut dst_ta, i, val)?;
        }
        vm.temp_roots.truncate(prev_temp);
        Ok(vm.alloc_typed_array(dst_ta))
    });
    vm.set_property(proto, "map", map_fn);
    vm.set_property_non_enumerable(proto, "map".to_string());

    let filter_fn = vm.alloc_native_function("filter", 1, |vm, this, args| {
        let callback = args.first().copied().unwrap_or(JsValue::undefined());
        if !vm.is_callable(callback) {
            return Err(VmError::TypeError("Callback must be callable".into()));
        }
        let this_arg = args.get(1).copied().unwrap_or(JsValue::undefined());
        let ta = validate_typed_array(this)?;
        let len = ta.length;
        let bpe = ta.kind.bytes_per_element();
        let kind = ta.kind;
        let prev_temp = vm.temp_roots.len();
        vm.temp_roots.push(this);
        vm.temp_roots.push(callback);
        vm.temp_roots.push(this_arg);
        let kept_start = vm.temp_roots.len();
        let mut count_kept = 0;
        for i in 0..len {
            let cur_this = vm.temp_roots[prev_temp];
            let cur_cb = vm.temp_roots[prev_temp + 1];
            let cur_this_arg = vm.temp_roots[prev_temp + 2];
            let val = if let Some(ptr) = cur_this.as_object_ptr() {
                let obj = unsafe { &*(ptr as *mut JsObject) };
                if let ObjectKind::TypedArray(ta) = &obj.kind {
                    let ta_clone = ta.clone();
                    vm.read_typed_array_element_vm(&ta_clone, i).unwrap_or(JsValue::undefined())
                } else {
                    JsValue::undefined()
                }
            } else {
                JsValue::undefined()
            };
            let res = vm.call(cur_cb, cur_this_arg, &[val, JsValue::from_int32(i as i32), cur_this])?;
            if res.is_truthy() {
                vm.temp_roots.push(val);
                count_kept += 1;
            }
        }
        let new_sab = doms_parallel::SharedArrayBuffer::try_new(count_kept * bpe)
            .map_err(|e| VmError::RuntimeError(e.into()))?;
        let mut dst_ta = JsTypedArray {
            buffer: Arc::new(new_sab),
            kind,
            length: count_kept,
            byte_offset: 0,
        };
        for i in 0..count_kept {
            let val = vm.temp_roots[kept_start + i];
            vm.write_typed_array_element_vm(&mut dst_ta, i, val)?;
        }
        vm.temp_roots.truncate(prev_temp);
        Ok(vm.alloc_typed_array(dst_ta))
    });
    vm.set_property(proto, "filter", filter_fn);
    vm.set_property_non_enumerable(proto, "filter".to_string());

    let some_fn = vm.alloc_native_function("some", 1, |vm, this, args| {
        let callback = args.first().copied().unwrap_or(JsValue::undefined());
        if !vm.is_callable(callback) {
            return Err(VmError::TypeError("Callback must be callable".into()));
        }
        let this_arg = args.get(1).copied().unwrap_or(JsValue::undefined());
        let ta = validate_typed_array(this)?;
        let len = ta.length;
        let prev_temp = vm.temp_roots.len();
        vm.temp_roots.push(this);
        vm.temp_roots.push(callback);
        vm.temp_roots.push(this_arg);
        for i in 0..len {
            let cur_this = vm.temp_roots[prev_temp];
            let cur_cb = vm.temp_roots[prev_temp + 1];
            let cur_this_arg = vm.temp_roots[prev_temp + 2];
            let val = if let Some(ptr) = cur_this.as_object_ptr() {
                let obj = unsafe { &*(ptr as *mut JsObject) };
                if let ObjectKind::TypedArray(ta) = &obj.kind {
                    let ta_clone = ta.clone();
                    vm.read_typed_array_element_vm(&ta_clone, i).unwrap_or(JsValue::undefined())
                } else {
                    JsValue::undefined()
                }
            } else {
                JsValue::undefined()
            };
            let res = vm.call(cur_cb, cur_this_arg, &[val, JsValue::from_int32(i as i32), cur_this])?;
            if res.is_truthy() {
                vm.temp_roots.truncate(prev_temp);
                return Ok(JsValue::from_boolean(true));
            }
        }
        vm.temp_roots.truncate(prev_temp);
        Ok(JsValue::from_boolean(false))
    });
    vm.set_property(proto, "some", some_fn);
    vm.set_property_non_enumerable(proto, "some".to_string());

    let every_fn = vm.alloc_native_function("every", 1, |vm, this, args| {
        let callback = args.first().copied().unwrap_or(JsValue::undefined());
        if !vm.is_callable(callback) {
            return Err(VmError::TypeError("Callback must be callable".into()));
        }
        let this_arg = args.get(1).copied().unwrap_or(JsValue::undefined());
        let ta = validate_typed_array(this)?;
        let len = ta.length;
        let prev_temp = vm.temp_roots.len();
        vm.temp_roots.push(this);
        vm.temp_roots.push(callback);
        vm.temp_roots.push(this_arg);
        for i in 0..len {
            let cur_this = vm.temp_roots[prev_temp];
            let cur_cb = vm.temp_roots[prev_temp + 1];
            let cur_this_arg = vm.temp_roots[prev_temp + 2];
            let val = if let Some(ptr) = cur_this.as_object_ptr() {
                let obj = unsafe { &*(ptr as *mut JsObject) };
                if let ObjectKind::TypedArray(ta) = &obj.kind {
                    let ta_clone = ta.clone();
                    vm.read_typed_array_element_vm(&ta_clone, i).unwrap_or(JsValue::undefined())
                } else {
                    JsValue::undefined()
                }
            } else {
                JsValue::undefined()
            };
            let res = vm.call(cur_cb, cur_this_arg, &[val, JsValue::from_int32(i as i32), cur_this])?;
            if !res.is_truthy() {
                vm.temp_roots.truncate(prev_temp);
                return Ok(JsValue::from_boolean(false));
            }
        }
        vm.temp_roots.truncate(prev_temp);
        Ok(JsValue::from_boolean(true))
    });
    vm.set_property(proto, "every", every_fn);
    vm.set_property_non_enumerable(proto, "every".to_string());

    let find_fn = vm.alloc_native_function("find", 1, |vm, this, args| {
        let predicate = args.first().copied().unwrap_or(JsValue::undefined());
        if !vm.is_callable(predicate) {
            return Err(VmError::TypeError("Predicate must be callable".into()));
        }
        let this_arg = args.get(1).copied().unwrap_or(JsValue::undefined());
        let ta = validate_typed_array(this)?;
        let len = ta.length;
        let prev_temp = vm.temp_roots.len();
        vm.temp_roots.push(this);
        vm.temp_roots.push(predicate);
        vm.temp_roots.push(this_arg);
        for i in 0..len {
            let cur_this = vm.temp_roots[prev_temp];
            let cur_pred = vm.temp_roots[prev_temp + 1];
            let cur_this_arg = vm.temp_roots[prev_temp + 2];
            let val = if let Some(ptr) = cur_this.as_object_ptr() {
                let obj = unsafe { &*(ptr as *mut JsObject) };
                if let ObjectKind::TypedArray(ta) = &obj.kind {
                    let ta_clone = ta.clone();
                    vm.read_typed_array_element_vm(&ta_clone, i).unwrap_or(JsValue::undefined())
                } else {
                    JsValue::undefined()
                }
            } else {
                JsValue::undefined()
            };
            let res = vm.call(cur_pred, cur_this_arg, &[val, JsValue::from_int32(i as i32), cur_this])?;
            if res.is_truthy() {
                vm.temp_roots.truncate(prev_temp);
                return Ok(val);
            }
        }
        vm.temp_roots.truncate(prev_temp);
        Ok(JsValue::undefined())
    });
    vm.set_property(proto, "find", find_fn);
    vm.set_property_non_enumerable(proto, "find".to_string());

    let find_index_fn = vm.alloc_native_function("findIndex", 1, |vm, this, args| {
        let predicate = args.first().copied().unwrap_or(JsValue::undefined());
        if !vm.is_callable(predicate) {
            return Err(VmError::TypeError("Predicate must be callable".into()));
        }
        let this_arg = args.get(1).copied().unwrap_or(JsValue::undefined());
        let ta = validate_typed_array(this)?;
        let len = ta.length;
        let prev_temp = vm.temp_roots.len();
        vm.temp_roots.push(this);
        vm.temp_roots.push(predicate);
        vm.temp_roots.push(this_arg);
        for i in 0..len {
            let cur_this = vm.temp_roots[prev_temp];
            let cur_pred = vm.temp_roots[prev_temp + 1];
            let cur_this_arg = vm.temp_roots[prev_temp + 2];
            let val = if let Some(ptr) = cur_this.as_object_ptr() {
                let obj = unsafe { &*(ptr as *mut JsObject) };
                if let ObjectKind::TypedArray(ta) = &obj.kind {
                    let ta_clone = ta.clone();
                    vm.read_typed_array_element_vm(&ta_clone, i).unwrap_or(JsValue::undefined())
                } else {
                    JsValue::undefined()
                }
            } else {
                JsValue::undefined()
            };
            let res = vm.call(cur_pred, cur_this_arg, &[val, JsValue::from_int32(i as i32), cur_this])?;
            if res.is_truthy() {
                vm.temp_roots.truncate(prev_temp);
                return Ok(JsValue::from_int32(i as i32));
            }
        }
        vm.temp_roots.truncate(prev_temp);
        Ok(JsValue::from_int32(-1))
    });
    vm.set_property(proto, "findIndex", find_index_fn);
    vm.set_property_non_enumerable(proto, "findIndex".to_string());

    let find_last_fn = vm.alloc_native_function("findLast", 1, |vm, this, args| {
        let predicate = args.first().copied().unwrap_or(JsValue::undefined());
        if !vm.is_callable(predicate) {
            return Err(VmError::TypeError("Predicate must be callable".into()));
        }
        let this_arg = args.get(1).copied().unwrap_or(JsValue::undefined());
        let ta = validate_typed_array(this)?;
        let len = ta.length;
        let prev_temp = vm.temp_roots.len();
        vm.temp_roots.push(this);
        vm.temp_roots.push(predicate);
        vm.temp_roots.push(this_arg);
        for i in (0..len).rev() {
            let cur_this = vm.temp_roots[prev_temp];
            let cur_pred = vm.temp_roots[prev_temp + 1];
            let cur_this_arg = vm.temp_roots[prev_temp + 2];
            let val = if let Some(ptr) = cur_this.as_object_ptr() {
                let obj = unsafe { &*(ptr as *mut JsObject) };
                if let ObjectKind::TypedArray(ta) = &obj.kind {
                    let ta_clone = ta.clone();
                    vm.read_typed_array_element_vm(&ta_clone, i).unwrap_or(JsValue::undefined())
                } else {
                    JsValue::undefined()
                }
            } else {
                JsValue::undefined()
            };
            let res = vm.call(cur_pred, cur_this_arg, &[val, JsValue::from_int32(i as i32), cur_this])?;
            if res.is_truthy() {
                vm.temp_roots.truncate(prev_temp);
                return Ok(val);
            }
        }
        vm.temp_roots.truncate(prev_temp);
        Ok(JsValue::undefined())
    });
    vm.set_property(proto, "findLast", find_last_fn);
    vm.set_property_non_enumerable(proto, "findLast".to_string());

    let find_last_index_fn = vm.alloc_native_function("findLastIndex", 1, |vm, this, args| {
        let predicate = args.first().copied().unwrap_or(JsValue::undefined());
        if !vm.is_callable(predicate) {
            return Err(VmError::TypeError("Predicate must be callable".into()));
        }
        let this_arg = args.get(1).copied().unwrap_or(JsValue::undefined());
        let ta = validate_typed_array(this)?;
        let len = ta.length;
        let prev_temp = vm.temp_roots.len();
        vm.temp_roots.push(this);
        vm.temp_roots.push(predicate);
        vm.temp_roots.push(this_arg);
        for i in (0..len).rev() {
            let cur_this = vm.temp_roots[prev_temp];
            let cur_pred = vm.temp_roots[prev_temp + 1];
            let cur_this_arg = vm.temp_roots[prev_temp + 2];
            let val = if let Some(ptr) = cur_this.as_object_ptr() {
                let obj = unsafe { &*(ptr as *mut JsObject) };
                if let ObjectKind::TypedArray(ta) = &obj.kind {
                    let ta_clone = ta.clone();
                    vm.read_typed_array_element_vm(&ta_clone, i).unwrap_or(JsValue::undefined())
                } else {
                    JsValue::undefined()
                }
            } else {
                JsValue::undefined()
            };
            let res = vm.call(cur_pred, cur_this_arg, &[val, JsValue::from_int32(i as i32), cur_this])?;
            if res.is_truthy() {
                vm.temp_roots.truncate(prev_temp);
                return Ok(JsValue::from_int32(i as i32));
            }
        }
        vm.temp_roots.truncate(prev_temp);
        Ok(JsValue::from_int32(-1))
    });
    vm.set_property(proto, "findLastIndex", find_last_index_fn);
    vm.set_property_non_enumerable(proto, "findLastIndex".to_string());

    let reduce_fn = vm.alloc_native_function("reduce", 1, |vm, this, args| {
        let callback = args.first().copied().unwrap_or(JsValue::undefined());
        if !vm.is_callable(callback) {
            return Err(VmError::TypeError("Callback must be callable".into()));
        }
        let ta = validate_typed_array(this)?;
        let len = ta.length;
        let has_init = args.len() > 1;
        let (mut accum, start_idx) = if has_init {
            (args[1], 0)
        } else if len > 0 {
            (vm.read_typed_array_element_vm(&ta, 0).unwrap_or(JsValue::undefined()), 1)
        } else {
            return Err(VmError::TypeError("Reduce of empty typed array with no initial value".into()));
        };
        let prev_temp = vm.temp_roots.len();
        vm.temp_roots.push(this);
        vm.temp_roots.push(callback);
        vm.temp_roots.push(accum);
        for i in start_idx..len {
            let cur_this = vm.temp_roots[prev_temp];
            let cur_cb = vm.temp_roots[prev_temp + 1];
            let cur_accum = vm.temp_roots[prev_temp + 2];
            let val = if let Some(ptr) = cur_this.as_object_ptr() {
                let obj = unsafe { &*(ptr as *mut JsObject) };
                if let ObjectKind::TypedArray(ta) = &obj.kind {
                    let ta_clone = ta.clone();
                    vm.read_typed_array_element_vm(&ta_clone, i).unwrap_or(JsValue::undefined())
                } else {
                    JsValue::undefined()
                }
            } else {
                JsValue::undefined()
            };
            accum = vm.call(cur_cb, JsValue::undefined(), &[cur_accum, val, JsValue::from_int32(i as i32), cur_this])?;
            vm.temp_roots[prev_temp + 2] = accum;
        }
        vm.temp_roots.truncate(prev_temp);
        Ok(accum)
    });
    vm.set_property(proto, "reduce", reduce_fn);
    vm.set_property_non_enumerable(proto, "reduce".to_string());

    let reduce_right_fn = vm.alloc_native_function("reduceRight", 1, |vm, this, args| {
        let callback = args.first().copied().unwrap_or(JsValue::undefined());
        if !vm.is_callable(callback) {
            return Err(VmError::TypeError("Callback must be callable".into()));
        }
        let ta = validate_typed_array(this)?;
        let len = ta.length;
        let has_init = args.len() > 1;
        let (mut accum, start_idx) = if has_init {
            (args[1], len as i64 - 1)
        } else if len > 0 {
            (vm.read_typed_array_element_vm(&ta, len - 1).unwrap_or(JsValue::undefined()), len as i64 - 2)
        } else {
            return Err(VmError::TypeError("Reduce of empty typed array with no initial value".into()));
        };
        let prev_temp = vm.temp_roots.len();
        vm.temp_roots.push(this);
        vm.temp_roots.push(callback);
        vm.temp_roots.push(accum);
        let mut i = start_idx;
        while i >= 0 {
            let cur_this = vm.temp_roots[prev_temp];
            let cur_cb = vm.temp_roots[prev_temp + 1];
            let cur_accum = vm.temp_roots[prev_temp + 2];
            let val = if let Some(ptr) = cur_this.as_object_ptr() {
                let obj = unsafe { &*(ptr as *mut JsObject) };
                if let ObjectKind::TypedArray(ta) = &obj.kind {
                    let ta_clone = ta.clone();
                    vm.read_typed_array_element_vm(&ta_clone, i as usize).unwrap_or(JsValue::undefined())
                } else {
                    JsValue::undefined()
                }
            } else {
                JsValue::undefined()
            };
            accum = vm.call(cur_cb, JsValue::undefined(), &[cur_accum, val, JsValue::from_int32(i as i32), cur_this])?;
            vm.temp_roots[prev_temp + 2] = accum;
            i -= 1;
        }
        vm.temp_roots.truncate(prev_temp);
        Ok(accum)
    });
    vm.set_property(proto, "reduceRight", reduce_right_fn);
    vm.set_property_non_enumerable(proto, "reduceRight".to_string());

    let sort_fn = vm.alloc_native_function("sort", 1, |vm, this, args| {
        let ta = validate_typed_array(this)?;
        let len = ta.length;
        let comparefn = args.first().copied().unwrap_or(JsValue::undefined());
        let has_cmp = vm.is_callable(comparefn);
        let prev_temp = vm.temp_roots.len();
        let vals_start = prev_temp;
        for i in 0..len {
            let val = vm.read_typed_array_element_vm(&ta, i).unwrap_or(JsValue::undefined());
            vm.temp_roots.push(val);
        }
        if has_cmp {
            let cmp_idx = vm.temp_roots.len();
            vm.temp_roots.push(comparefn);
            for i in 0..len {
                for j in (i + 1)..len {
                    let cmp = vm.temp_roots[cmp_idx];
                    let a = vm.temp_roots[vals_start + i];
                    let b = vm.temp_roots[vals_start + j];
                    let res = vm.call(cmp, JsValue::undefined(), &[a, b])?;
                    let n = res.to_number().unwrap_or(0.0);
                    if n > 0.0 {
                        vm.temp_roots.swap(vals_start + i, vals_start + j);
                    }
                }
            }
        } else {
            // Numeric ascending sort
            let mut slice: Vec<JsValue> = vm.temp_roots[vals_start..vals_start + len].to_vec();
            slice.sort_by(|&a, &b| {
                if let (Some(ba), Some(bb)) = (vm.as_bigint_val(a), vm.as_bigint_val(b)) {
                    ba.cmp(&bb)
                } else {
                    let na = a.to_number().unwrap_or(0.0);
                    let nb = b.to_number().unwrap_or(0.0);
                    if na.is_nan() && nb.is_nan() {
                        std::cmp::Ordering::Equal
                    } else if na.is_nan() {
                        std::cmp::Ordering::Greater
                    } else if nb.is_nan() {
                        std::cmp::Ordering::Less
                    } else {
                        na.partial_cmp(&nb).unwrap_or(std::cmp::Ordering::Equal)
                    }
                }
            });
            for (idx, val) in slice.into_iter().enumerate() {
                vm.temp_roots[vals_start + idx] = val;
            }
        }
        let ta_clone = ta.clone();
        for i in 0..len {
            let val = vm.temp_roots[vals_start + i];
            vm.write_typed_array_element_vm(&ta_clone, i, val)?;
        }
        vm.temp_roots.truncate(prev_temp);
        Ok(this)
    });
    vm.set_property(proto, "sort", sort_fn);
    vm.set_property_non_enumerable(proto, "sort".to_string());

    let to_sorted_fn = vm.alloc_native_function("toSorted", 1, |vm, this, args| {
        let ta = validate_typed_array(this)?;
        let len = ta.length;
        let bpe = ta.kind.bytes_per_element();
        let comparefn = args.first().copied().unwrap_or(JsValue::undefined());
        let has_cmp = vm.is_callable(comparefn);
        let prev_temp = vm.temp_roots.len();
        let vals_start = prev_temp;
        for i in 0..len {
            let val = vm.read_typed_array_element_vm(&ta, i).unwrap_or(JsValue::undefined());
            vm.temp_roots.push(val);
        }
        if has_cmp {
            let cmp_idx = vm.temp_roots.len();
            vm.temp_roots.push(comparefn);
            for i in 0..len {
                for j in (i + 1)..len {
                    let cmp = vm.temp_roots[cmp_idx];
                    let a = vm.temp_roots[vals_start + i];
                    let b = vm.temp_roots[vals_start + j];
                    let res = vm.call(cmp, JsValue::undefined(), &[a, b])?;
                    let n = res.to_number().unwrap_or(0.0);
                    if n > 0.0 {
                        vm.temp_roots.swap(vals_start + i, vals_start + j);
                    }
                }
            }
        } else {
            let mut slice: Vec<JsValue> = vm.temp_roots[vals_start..vals_start + len].to_vec();
            slice.sort_by(|&a, &b| {
                if let (Some(ba), Some(bb)) = (vm.as_bigint_val(a), vm.as_bigint_val(b)) {
                    ba.cmp(&bb)
                } else {
                    let na = a.to_number().unwrap_or(0.0);
                    let nb = b.to_number().unwrap_or(0.0);
                    if na.is_nan() && nb.is_nan() {
                        std::cmp::Ordering::Equal
                    } else if na.is_nan() {
                        std::cmp::Ordering::Greater
                    } else if nb.is_nan() {
                        std::cmp::Ordering::Less
                    } else {
                        na.partial_cmp(&nb).unwrap_or(std::cmp::Ordering::Equal)
                    }
                }
            });
            for (idx, val) in slice.into_iter().enumerate() {
                vm.temp_roots[vals_start + idx] = val;
            }
        }
        let new_sab = doms_parallel::SharedArrayBuffer::try_new(len * bpe)
            .map_err(|e| VmError::RuntimeError(e.into()))?;
        let mut dst_ta = JsTypedArray {
            buffer: Arc::new(new_sab),
            kind: ta.kind,
            length: len,
            byte_offset: 0,
        };
        for i in 0..len {
            let val = vm.temp_roots[vals_start + i];
            vm.write_typed_array_element_vm(&mut dst_ta, i, val)?;
        }
        vm.temp_roots.truncate(prev_temp);
        Ok(vm.alloc_typed_array(dst_ta))
    });
    vm.set_property(proto, "toSorted", to_sorted_fn);
    vm.set_property_non_enumerable(proto, "toSorted".to_string());
}
