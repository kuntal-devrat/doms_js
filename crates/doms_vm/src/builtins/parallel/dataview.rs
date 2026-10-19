//! The `DataView` builtin and its accessors.

use super::*;
use num_bigint::BigInt;

fn to_index(vm: &mut Vm, val: JsValue) -> Result<usize, VmError> {
    if val.is_undefined() {
        return Ok(0);
    }
    let num = vm.to_number_value(val)?;
    if num.is_nan() {
        return Ok(0);
    }
    let int_val = num.trunc();
    if int_val == 0.0 {
        return Ok(0);
    }
    if int_val < 0.0 || num.is_infinite() || int_val > 9007199254740991.0 {
        return Err(VmError::RangeError("Index out of range".into()));
    }
    Ok(int_val as usize)
}

fn bigint_to_u64(bi: &BigInt) -> u64 {
    let (sign, digits) = bi.to_u64_digits();
    let mut val = digits.first().copied().unwrap_or(0);
    if sign == num_bigint::Sign::Minus {
        val = (!val).wrapping_add(1);
    }
    val
}

fn get_view_bounds(
    vm: &mut Vm,
    this: JsValue,
    byte_offset_val: JsValue,
    size: usize,
) -> Result<(Arc<doms_parallel::SharedArrayBuffer>, usize), VmError> {
    // 1. Let getIndex be ? ToIndex(byteOffset)
    let get_index = to_index(vm, byte_offset_val)?;

    // 2. Perform ? RequireInternalSlot(view, [[DataView]])
    let (buf, off, len) = if let Some(ptr) = this.as_object_ptr() {
        let obj = unsafe { &*(ptr as *mut JsObject) };
        if let ObjectKind::DataView { buffer, byte_offset, byte_length, .. } = &obj.kind {
            (Arc::clone(buffer), *byte_offset, *byte_length)
        } else {
            return Err(VmError::TypeError("DataView method called on non-DataView".into()));
        }
    } else {
        return Err(VmError::TypeError("DataView method called on non-DataView".into()));
    };

    // 3. If IsDetachedBuffer(buffer) is true, throw a TypeError exception.
    if buf.is_detached() {
        return Err(VmError::TypeError("ArrayBuffer is detached".into()));
    }

    // 4. If getIndex + elementSize > viewSize, throw a RangeError exception.
    if get_index + size > len {
        return Err(VmError::RangeError("Offset is outside the bounds of the DataView".into()));
    }

    Ok((buf, off + get_index))
}

fn set_view_bounds(
    vm: &mut Vm,
    this: JsValue,
    byte_offset_val: JsValue,
    size: usize,
) -> Result<(Arc<doms_parallel::SharedArrayBuffer>, usize), VmError> {
    // 1. Let getIndex be ? ToIndex(byteOffset)
    let get_index = to_index(vm, byte_offset_val)?;

    // 2. Perform ? RequireInternalSlot(view, [[DataView]])
    let (buf, off, len) = if let Some(ptr) = this.as_object_ptr() {
        let obj = unsafe { &*(ptr as *mut JsObject) };
        if let ObjectKind::DataView { buffer, byte_offset, byte_length, .. } = &obj.kind {
            (Arc::clone(buffer), *byte_offset, *byte_length)
        } else {
            return Err(VmError::TypeError("DataView method called on non-DataView".into()));
        }
    } else {
        return Err(VmError::TypeError("DataView method called on non-DataView".into()));
    };

    // 3. If IsDetachedBuffer(buffer) is true, throw a TypeError exception.
    if buf.is_detached() {
        return Err(VmError::TypeError("ArrayBuffer is detached".into()));
    }

    // 4. If getIndex + elementSize > viewSize, throw a RangeError exception.
    if get_index + size > len {
        return Err(VmError::RangeError("Offset is outside the bounds of the DataView".into()));
    }

    Ok((buf, off + get_index))
}

pub(super) fn register_dataview(vm: &mut Vm) {
    // 1. DataView Constructor
    let dv_ctor = vm.alloc_native_function("DataView", 1, |vm, _this, args| {
        if !vm.is_constructing {
            return Err(VmError::TypeError("Constructor DataView requires 'new'".into()));
        }

        let buffer_val = args.first().copied().unwrap_or(JsValue::undefined());
        let (sab, buf_len) = if let Some(ptr) = buffer_val.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            match &obj.kind {
                ObjectKind::ArrayBuffer(b) | ObjectKind::SharedArrayBuffer(b) => {
                    (Arc::clone(b), b.byte_length())
                }
                _ => return Err(VmError::TypeError("DataView requires an ArrayBuffer or SharedArrayBuffer".into())),
            }
        } else {
            return Err(VmError::TypeError("DataView requires an ArrayBuffer or SharedArrayBuffer".into()));
        };

        let byte_offset = if let Some(off_val) = args.get(1) {
            to_index(vm, *off_val)?
        } else {
            0
        };

        if sab.is_detached() {
            return Err(VmError::TypeError("ArrayBuffer is detached".into()));
        }

        if byte_offset > buf_len {
            return Err(VmError::RangeError("Start offset is outside the bounds of the buffer".into()));
        }

        let byte_length = match args.get(2) {
            Some(len_val) if !len_val.is_undefined() => {
                let l = to_index(vm, *len_val)?;
                if byte_offset + l > buf_len {
                    return Err(VmError::RangeError("Invalid DataView length".into()));
                }
                l
            }
            _ => buf_len - byte_offset,
        };

        if sab.is_detached() {
            return Err(VmError::TypeError("ArrayBuffer is detached".into()));
        }

        Ok(vm.alloc_data_view(buffer_val, sab, byte_offset, byte_length))
    });

    let dv_proto = vm.alloc_object();

    // 2. DataView.prototype.constructor
    vm.set_property(dv_proto, "constructor", dv_ctor);
    vm.set_property_non_enumerable(dv_proto, "constructor".to_string());

    // 3. DataView.prototype[Symbol.toStringTag]
    if let Some(sym_tag) = vm.symbol_to_string_tag {
        let tag_val = vm.alloc_string("DataView".into());
        vm.set_symbol_property(dv_proto, sym_tag, tag_val);
        if let Some(ptr) = dv_proto.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            obj.set_symbol_attributes(sym_tag, PropertyAttributes {
                writable: false,
                enumerable: false,
                configurable: true,
            });
        }
    }

    // 4. Accessors: buffer, byteLength, byteOffset
    let buffer_getter = vm.alloc_native_function("get buffer", 0, |_vm, this, _args| {
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::DataView { buffer_val, .. } = &obj.kind {
                return Ok(*buffer_val);
            }
        }
        Err(VmError::TypeError("get DataView.prototype.buffer called on incompatible receiver".into()))
    });

    let byte_length_getter = vm.alloc_native_function("get byteLength", 0, |_vm, this, _args| {
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::DataView { buffer, byte_length, .. } = &obj.kind {
                if buffer.is_detached() {
                    return Err(VmError::TypeError("DataView buffer is detached".into()));
                }
                let len = *byte_length;
                return Ok(if len <= i32::MAX as usize {
                    JsValue::from_int32(len as i32)
                } else {
                    JsValue::from_double(len as f64)
                });
            }
        }
        Err(VmError::TypeError("get DataView.prototype.byteLength called on incompatible receiver".into()))
    });

    let byte_offset_getter = vm.alloc_native_function("get byteOffset", 0, |_vm, this, _args| {
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::DataView { buffer, byte_offset, .. } = &obj.kind {
                if buffer.is_detached() {
                    return Err(VmError::TypeError("DataView buffer is detached".into()));
                }
                let off = *byte_offset;
                return Ok(if off <= i32::MAX as usize {
                    JsValue::from_int32(off as i32)
                } else {
                    JsValue::from_double(off as f64)
                });
            }
        }
        Err(VmError::TypeError("get DataView.prototype.byteOffset called on incompatible receiver".into()))
    });

    if let Some(ptr) = dv_proto.as_object_ptr() {
        let obj = unsafe { &mut *(ptr as *mut JsObject) };
        obj.define_accessor("buffer".to_string(), Some(buffer_getter), None);
        obj.define_accessor("byteLength".to_string(), Some(byte_length_getter), None);
        obj.define_accessor("byteOffset".to_string(), Some(byte_offset_getter), None);
    }
    vm.set_property_non_enumerable(dv_proto, "buffer".to_string());
    vm.set_property_non_enumerable(dv_proto, "byteLength".to_string());
    vm.set_property_non_enumerable(dv_proto, "byteOffset".to_string());

    // 5. Prototype methods
    let dv_get_i8 = vm.alloc_native_function("getInt8", 1, |vm, this, args| {
        let off_arg = args.first().copied().unwrap_or(JsValue::undefined());
        let (buf, pos) = get_view_bounds(vm, this, off_arg, 1)?;
        let b = buf.get(pos).unwrap_or(0) as i8;
        Ok(JsValue::from_int32(b as i32))
    });
    let dv_get_u8 = vm.alloc_native_function("getUint8", 1, |vm, this, args| {
        let off_arg = args.first().copied().unwrap_or(JsValue::undefined());
        let (buf, pos) = get_view_bounds(vm, this, off_arg, 1)?;
        let b = buf.get(pos).unwrap_or(0);
        Ok(JsValue::from_int32(b as i32))
    });
    let dv_set_i8 = vm.alloc_native_function("setInt8", 2, |vm, this, args| {
        let off_arg = args.first().copied().unwrap_or(JsValue::undefined());
        let val_arg = args.get(1).copied().unwrap_or(JsValue::undefined());
        let get_index = to_index(vm, off_arg)?;
        let val = vm.to_number_value(val_arg)?;
        let (buf, pos) = set_view_bounds(vm, this, JsValue::from_double(get_index as f64), 1)?;
        let b = val as i32 as i8 as u8;
        buf.set(pos, b);
        Ok(JsValue::undefined())
    });
    let dv_set_u8 = vm.alloc_native_function("setUint8", 2, |vm, this, args| {
        let off_arg = args.first().copied().unwrap_or(JsValue::undefined());
        let val_arg = args.get(1).copied().unwrap_or(JsValue::undefined());
        let get_index = to_index(vm, off_arg)?;
        let val = vm.to_number_value(val_arg)?;
        let (buf, pos) = set_view_bounds(vm, this, JsValue::from_double(get_index as f64), 1)?;
        let b = val as i32 as u8;
        buf.set(pos, b);
        Ok(JsValue::undefined())
    });

    let dv_get_i16 = vm.alloc_native_function("getInt16", 1, |vm, this, args| {
        let off_arg = args.first().copied().unwrap_or(JsValue::undefined());
        let le = args.get(1).map(|v| v.is_truthy()).unwrap_or(false);
        let (buf, pos) = get_view_bounds(vm, this, off_arg, 2)?;
        let b0 = buf.get(pos).unwrap_or(0);
        let b1 = buf.get(pos + 1).unwrap_or(0);
        let v = if le { i16::from_le_bytes([b0, b1]) } else { i16::from_be_bytes([b0, b1]) };
        Ok(JsValue::from_int32(v as i32))
    });
    let dv_get_u16 = vm.alloc_native_function("getUint16", 1, |vm, this, args| {
        let off_arg = args.first().copied().unwrap_or(JsValue::undefined());
        let le = args.get(1).map(|v| v.is_truthy()).unwrap_or(false);
        let (buf, pos) = get_view_bounds(vm, this, off_arg, 2)?;
        let b0 = buf.get(pos).unwrap_or(0);
        let b1 = buf.get(pos + 1).unwrap_or(0);
        let v = if le { u16::from_le_bytes([b0, b1]) } else { u16::from_be_bytes([b0, b1]) };
        Ok(JsValue::from_int32(v as i32))
    });
    let dv_set_i16 = vm.alloc_native_function("setInt16", 2, |vm, this, args| {
        let off_arg = args.first().copied().unwrap_or(JsValue::undefined());
        let val_arg = args.get(1).copied().unwrap_or(JsValue::undefined());
        let le = args.get(2).map(|v| v.is_truthy()).unwrap_or(false);
        let get_index = to_index(vm, off_arg)?;
        let val = vm.to_number_value(val_arg)?;
        let (buf, pos) = set_view_bounds(vm, this, JsValue::from_double(get_index as f64), 2)?;
        let int_val = val as i32 as i16;
        let bytes = if le { int_val.to_le_bytes() } else { int_val.to_be_bytes() };
        buf.set(pos, bytes[0]);
        buf.set(pos + 1, bytes[1]);
        Ok(JsValue::undefined())
    });
    let dv_set_u16 = vm.alloc_native_function("setUint16", 2, |vm, this, args| {
        let off_arg = args.first().copied().unwrap_or(JsValue::undefined());
        let val_arg = args.get(1).copied().unwrap_or(JsValue::undefined());
        let le = args.get(2).map(|v| v.is_truthy()).unwrap_or(false);
        let get_index = to_index(vm, off_arg)?;
        let val = vm.to_number_value(val_arg)?;
        let (buf, pos) = set_view_bounds(vm, this, JsValue::from_double(get_index as f64), 2)?;
        let int_val = val as i32 as u16;
        let bytes = if le { int_val.to_le_bytes() } else { int_val.to_be_bytes() };
        buf.set(pos, bytes[0]);
        buf.set(pos + 1, bytes[1]);
        Ok(JsValue::undefined())
    });

    let dv_get_i32 = vm.alloc_native_function("getInt32", 1, |vm, this, args| {
        let off_arg = args.first().copied().unwrap_or(JsValue::undefined());
        let le = args.get(1).map(|v| v.is_truthy()).unwrap_or(false);
        let (buf, pos) = get_view_bounds(vm, this, off_arg, 4)?;
        let mut b = [0u8; 4];
        for i in 0..4 { b[i] = buf.get(pos + i).unwrap_or(0); }
        let v = if le { i32::from_le_bytes(b) } else { i32::from_be_bytes(b) };
        Ok(JsValue::from_int32(v))
    });
    let dv_get_u32 = vm.alloc_native_function("getUint32", 1, |vm, this, args| {
        let off_arg = args.first().copied().unwrap_or(JsValue::undefined());
        let le = args.get(1).map(|v| v.is_truthy()).unwrap_or(false);
        let (buf, pos) = get_view_bounds(vm, this, off_arg, 4)?;
        let mut b = [0u8; 4];
        for i in 0..4 { b[i] = buf.get(pos + i).unwrap_or(0); }
        let v = if le { u32::from_le_bytes(b) } else { u32::from_be_bytes(b) };
        Ok(JsValue::from_double(v as f64))
    });
    let dv_set_i32 = vm.alloc_native_function("setInt32", 2, |vm, this, args| {
        let off_arg = args.first().copied().unwrap_or(JsValue::undefined());
        let val_arg = args.get(1).copied().unwrap_or(JsValue::undefined());
        let le = args.get(2).map(|v| v.is_truthy()).unwrap_or(false);
        let get_index = to_index(vm, off_arg)?;
        let val = vm.to_number_value(val_arg)?;
        let (buf, pos) = set_view_bounds(vm, this, JsValue::from_double(get_index as f64), 4)?;
        let int_val = val as i32;
        let bytes = if le { int_val.to_le_bytes() } else { int_val.to_be_bytes() };
        for (i, &b) in bytes.iter().enumerate() { buf.set(pos + i, b); }
        Ok(JsValue::undefined())
    });
    let dv_set_u32 = vm.alloc_native_function("setUint32", 2, |vm, this, args| {
        let off_arg = args.first().copied().unwrap_or(JsValue::undefined());
        let val_arg = args.get(1).copied().unwrap_or(JsValue::undefined());
        let le = args.get(2).map(|v| v.is_truthy()).unwrap_or(false);
        let get_index = to_index(vm, off_arg)?;
        let val = vm.to_number_value(val_arg)?;
        let (buf, pos) = set_view_bounds(vm, this, JsValue::from_double(get_index as f64), 4)?;
        let int_val = val as u32;
        let bytes = if le { int_val.to_le_bytes() } else { int_val.to_be_bytes() };
        for (i, &b) in bytes.iter().enumerate() { buf.set(pos + i, b); }
        Ok(JsValue::undefined())
    });

    let dv_get_f32 = vm.alloc_native_function("getFloat32", 1, |vm, this, args| {
        let off_arg = args.first().copied().unwrap_or(JsValue::undefined());
        let le = args.get(1).map(|v| v.is_truthy()).unwrap_or(false);
        let (buf, pos) = get_view_bounds(vm, this, off_arg, 4)?;
        let mut b = [0u8; 4];
        for i in 0..4 { b[i] = buf.get(pos + i).unwrap_or(0); }
        let v = if le { f32::from_le_bytes(b) } else { f32::from_be_bytes(b) };
        Ok(JsValue::from_double(v as f64))
    });
    let dv_set_f32 = vm.alloc_native_function("setFloat32", 2, |vm, this, args| {
        let off_arg = args.first().copied().unwrap_or(JsValue::undefined());
        let val_arg = args.get(1).copied().unwrap_or(JsValue::undefined());
        let le = args.get(2).map(|v| v.is_truthy()).unwrap_or(false);
        let get_index = to_index(vm, off_arg)?;
        let val = vm.to_number_value(val_arg)?;
        let (buf, pos) = set_view_bounds(vm, this, JsValue::from_double(get_index as f64), 4)?;
        let f_val = val as f32;
        let bytes = if le { f_val.to_le_bytes() } else { f_val.to_be_bytes() };
        for (i, &b) in bytes.iter().enumerate() { buf.set(pos + i, b); }
        Ok(JsValue::undefined())
    });

    let dv_get_f64 = vm.alloc_native_function("getFloat64", 1, |vm, this, args| {
        let off_arg = args.first().copied().unwrap_or(JsValue::undefined());
        let le = args.get(1).map(|v| v.is_truthy()).unwrap_or(false);
        let (buf, pos) = get_view_bounds(vm, this, off_arg, 8)?;
        let mut b = [0u8; 8];
        for i in 0..8 { b[i] = buf.get(pos + i).unwrap_or(0); }
        let v = if le { f64::from_le_bytes(b) } else { f64::from_be_bytes(b) };
        Ok(JsValue::from_double(v))
    });
    let dv_set_f64 = vm.alloc_native_function("setFloat64", 2, |vm, this, args| {
        let off_arg = args.first().copied().unwrap_or(JsValue::undefined());
        let val_arg = args.get(1).copied().unwrap_or(JsValue::undefined());
        let le = args.get(2).map(|v| v.is_truthy()).unwrap_or(false);
        let get_index = to_index(vm, off_arg)?;
        let val = vm.to_number_value(val_arg)?;
        let (buf, pos) = set_view_bounds(vm, this, JsValue::from_double(get_index as f64), 8)?;
        let bytes = if le { val.to_le_bytes() } else { val.to_be_bytes() };
        for (i, &b) in bytes.iter().enumerate() { buf.set(pos + i, b); }
        Ok(JsValue::undefined())
    });

    let dv_get_bi64 = vm.alloc_native_function("getBigInt64", 1, |vm, this, args| {
        let off_arg = args.first().copied().unwrap_or(JsValue::undefined());
        let le = args.get(1).map(|v| v.is_truthy()).unwrap_or(false);
        let (buf, pos) = get_view_bounds(vm, this, off_arg, 8)?;
        let mut b = [0u8; 8];
        for i in 0..8 { b[i] = buf.get(pos + i).unwrap_or(0); }
        let v = if le { i64::from_le_bytes(b) } else { i64::from_be_bytes(b) };
        Ok(vm.alloc_bigint(BigInt::from(v)))
    });
    let dv_set_bi64 = vm.alloc_native_function("setBigInt64", 2, |vm, this, args| {
        let off_arg = args.first().copied().unwrap_or(JsValue::undefined());
        let val_arg = args.get(1).copied().unwrap_or(JsValue::undefined());
        let le = args.get(2).map(|v| v.is_truthy()).unwrap_or(false);
        let get_index = to_index(vm, off_arg)?;
        let bi = vm.to_bigint_op(val_arg)?;
        let val_u64 = bigint_to_u64(&bi);
        let (buf, pos) = set_view_bounds(vm, this, JsValue::from_double(get_index as f64), 8)?;
        let bytes = if le { (val_u64 as i64).to_le_bytes() } else { (val_u64 as i64).to_be_bytes() };
        for (i, &b) in bytes.iter().enumerate() { buf.set(pos + i, b); }
        Ok(JsValue::undefined())
    });
    let dv_get_bu64 = vm.alloc_native_function("getBigUint64", 1, |vm, this, args| {
        let off_arg = args.first().copied().unwrap_or(JsValue::undefined());
        let le = args.get(1).map(|v| v.is_truthy()).unwrap_or(false);
        let (buf, pos) = get_view_bounds(vm, this, off_arg, 8)?;
        let mut b = [0u8; 8];
        for i in 0..8 { b[i] = buf.get(pos + i).unwrap_or(0); }
        let v = if le { u64::from_le_bytes(b) } else { u64::from_be_bytes(b) };
        Ok(vm.alloc_bigint(BigInt::from(v)))
    });
    let dv_set_bu64 = vm.alloc_native_function("setBigUint64", 2, |vm, this, args| {
        let off_arg = args.first().copied().unwrap_or(JsValue::undefined());
        let val_arg = args.get(1).copied().unwrap_or(JsValue::undefined());
        let le = args.get(2).map(|v| v.is_truthy()).unwrap_or(false);
        let get_index = to_index(vm, off_arg)?;
        let bi = vm.to_bigint_op(val_arg)?;
        let val_u64 = bigint_to_u64(&bi);
        let (buf, pos) = set_view_bounds(vm, this, JsValue::from_double(get_index as f64), 8)?;
        let bytes = if le { val_u64.to_le_bytes() } else { val_u64.to_be_bytes() };
        for (i, &b) in bytes.iter().enumerate() { buf.set(pos + i, b); }
        Ok(JsValue::undefined())
    });

    let methods = [
        ("getInt8", dv_get_i8),
        ("getUint8", dv_get_u8),
        ("setInt8", dv_set_i8),
        ("setUint8", dv_set_u8),
        ("getInt16", dv_get_i16),
        ("getUint16", dv_get_u16),
        ("setInt16", dv_set_i16),
        ("setUint16", dv_set_u16),
        ("getInt32", dv_get_i32),
        ("getUint32", dv_get_u32),
        ("setInt32", dv_set_i32),
        ("setUint32", dv_set_u32),
        ("getFloat32", dv_get_f32),
        ("setFloat32", dv_set_f32),
        ("getFloat64", dv_get_f64),
        ("setFloat64", dv_set_f64),
        ("getBigInt64", dv_get_bi64),
        ("setBigInt64", dv_set_bi64),
        ("getBigUint64", dv_get_bu64),
        ("setBigUint64", dv_set_bu64),
    ];

    for (name, func) in methods {
        vm.set_property(dv_proto, name, func);
        vm.set_property_non_enumerable(dv_proto, name.to_string());
    }

    vm.set_property(dv_ctor, "prototype", dv_proto);
    vm.set_property_non_writable(dv_ctor, "prototype".to_string());
    vm.set_property_non_enumerable(dv_ctor, "prototype".to_string());
    vm.set_property_non_configurable(dv_ctor, "prototype".to_string());
    vm.globals.insert("DataView".to_string(), dv_ctor);
    if vm.global_object.is_object() {
        vm.set_property(vm.global_object, "DataView", dv_ctor);
    }
}
