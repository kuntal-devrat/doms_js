//! Shared `TypedArray.prototype` methods.

use super::*;

pub(super) fn register_prototype_methods(vm: &mut Vm, proto: JsValue) {
    let subarray_fn = vm.alloc_native_function("subarray", 2, |vm, this, args| {
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::TypedArray(ta) = &obj.kind {
                let len = ta.length as i32;
                let start = args.first().and_then(|v| v.to_int32()).unwrap_or(0);
                let end = args.get(1).and_then(|v| v.to_int32()).unwrap_or(len);
                let s = if start < 0 { (len + start).max(0) } else { start.min(len) } as usize;
                let e = if end < 0 { (len + end).max(0) } else { end.min(len) } as usize;
                let count = if e > s { e - s } else { 0 };
                let bpe = ta.kind.bytes_per_element();
                let byte_offset = ta.byte_offset + s * bpe;
                return Ok(vm.alloc_typed_array(JsTypedArray {
                    buffer: Arc::clone(&ta.buffer),
                    kind: ta.kind,
                    length: count,
                    byte_offset,
                }));
            }
        }
        Err(VmError::TypeError("Method called on non-TypedArray".into()))
    });
    vm.set_property(proto, "subarray", subarray_fn);

    let set_fn = vm.alloc_native_function("set", 1, |vm, this, args| {
        let src = args.first().copied().unwrap_or(JsValue::undefined());
        let offset = args.get(1).and_then(|v| v.to_int32()).unwrap_or(0).max(0) as usize;
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::TypedArray(dst_ta) = &mut obj.kind {
                if let Some(src_ptr) = src.as_object_ptr() {
                    let src_obj = unsafe { &*(src_ptr as *mut JsObject) };
                    if let ObjectKind::TypedArray(src_ta) = &src_obj.kind {
                        if offset + src_ta.length > dst_ta.length {
                            return Err(VmError::RangeError("Source is too large for target array".into()));
                        }
                        for i in 0..src_ta.length {
                            if let Some(val) = Vm::read_typed_array_element(src_ta, i) {
                                Vm::write_typed_array_element(dst_ta, offset + i, val);
                            }
                        }
                        return Ok(JsValue::undefined());
                    }
                }
                let len = vm.get_property(src, "length").to_number().unwrap_or(0.0) as usize;
                if offset + len > dst_ta.length {
                    return Err(VmError::RangeError("Source is too large for target array".into()));
                }
                for i in 0..len {
                    let val = vm.get_element(src, i);
                    Vm::write_typed_array_element(dst_ta, offset + i, val);
                }
                return Ok(JsValue::undefined());
            }
        }
        Err(VmError::TypeError("Method called on non-TypedArray".into()))
    });
    vm.set_property(proto, "set", set_fn);

    let slice_fn = vm.alloc_native_function("slice", 2, |vm, this, args| {
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::TypedArray(ta) = &obj.kind {
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
                    if let Some(val) = Vm::read_typed_array_element(ta, s + i) {
                        Vm::write_typed_array_element(&mut dst_ta, i, val);
                    }
                }
                return Ok(vm.alloc_typed_array(dst_ta));
            }
        }
        Err(VmError::TypeError("Method called on non-TypedArray".into()))
    });
    vm.set_property(proto, "slice", slice_fn);

    let fill_fn = vm.alloc_native_function("fill", 1, |_vm, this, args| {
        let value = args.first().copied().unwrap_or(JsValue::undefined());
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::TypedArray(ta) = &mut obj.kind {
                let len = ta.length as i32;
                let start = args.get(1).and_then(|v| v.to_int32()).unwrap_or(0);
                let end = args.get(2).and_then(|v| v.to_int32()).unwrap_or(len);
                let s = if start < 0 { (len + start).max(0) } else { start.min(len) } as usize;
                let e = if end < 0 { (len + end).max(0) } else { end.min(len) } as usize;
                for i in s..e {
                    Vm::write_typed_array_element(ta, i, value);
                }
                return Ok(this);
            }
        }
        Err(VmError::TypeError("Method called on non-TypedArray".into()))
    });
    vm.set_property(proto, "fill", fill_fn);

    let index_of_fn = vm.alloc_native_function("indexOf", 1, |_vm, this, args| {
        let target = args.first().and_then(|v| v.to_number()).unwrap_or(0.0);
        let from_idx = args.get(1).and_then(|v| v.to_int32()).unwrap_or(0).max(0) as usize;
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::TypedArray(ta) = &obj.kind {
                for i in from_idx..ta.length {
                    if let Some(val) = Vm::read_typed_array_element(ta, i) {
                        if let Some(n) = val.to_number() {
                            if n == target {
                                return Ok(JsValue::from_int32(i as i32));
                            }
                        }
                    }
                }
                return Ok(JsValue::from_int32(-1));
            }
        }
        Ok(JsValue::from_int32(-1))
    });
    vm.set_property(proto, "indexOf", index_of_fn);

    let includes_fn = vm.alloc_native_function("includes", 1, |_vm, this, args| {
        let target = args.first().and_then(|v| v.to_number()).unwrap_or(0.0);
        let from_idx = args.get(1).and_then(|v| v.to_int32()).unwrap_or(0).max(0) as usize;
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::TypedArray(ta) = &obj.kind {
                for i in from_idx..ta.length {
                    if let Some(val) = Vm::read_typed_array_element(ta, i) {
                        if let Some(n) = val.to_number() {
                            if n == target || (n.is_nan() && target.is_nan()) {
                                return Ok(JsValue::from_boolean(true));
                            }
                        }
                    }
                }
            }
        }
        Ok(JsValue::from_boolean(false))
    });
    vm.set_property(proto, "includes", includes_fn);

    let join_fn = vm.alloc_native_function("join", 1, |vm, this, args| {
        let sep = args.first().map(|v| vm.to_string_value(*v)).unwrap_or_else(|| ",".to_string());
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::TypedArray(ta) = &obj.kind {
                let mut parts = Vec::with_capacity(ta.length);
                for i in 0..ta.length {
                    if let Some(val) = Vm::read_typed_array_element(ta, i) {
                        parts.push(vm.to_string_value(val));
                    }
                }
                return Ok(vm.alloc_string(parts.join(&sep)));
            }
        }
        Ok(vm.alloc_string("".to_string()))
    });
    vm.set_property(proto, "join", join_fn);

    let values_fn = vm.alloc_native_function("values", 0, |vm, this, _args| {
        create_array_iterator(vm, this, IterKind::Values)
    });
    let keys_fn = vm.alloc_native_function("keys", 0, |vm, this, _args| {
        create_array_iterator(vm, this, IterKind::Keys)
    });
    let entries_fn = vm.alloc_native_function("entries", 0, |vm, this, _args| {
        create_array_iterator(vm, this, IterKind::Entries)
    });
    vm.set_property(proto, "values", values_fn);
    vm.set_property(proto, "keys", keys_fn);
    vm.set_property(proto, "entries", entries_fn);
    if let Some(sym_iter) = vm.symbol_iterator {
        vm.set_symbol_property(proto, sym_iter, values_fn);
    }
}
