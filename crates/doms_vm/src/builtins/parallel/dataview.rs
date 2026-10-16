//! The `DataView` builtin and its accessors.

use super::*;

pub(super) fn register_dataview(vm: &mut Vm) {
    // DataView
    let dv_ctor = vm.alloc_native_function("DataView", 1, |vm, _this, args| {
        let buffer_val = args.first().copied().unwrap_or(JsValue::undefined());
        let (sab, buf_len) = if let Some(ptr) = buffer_val.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            match &obj.kind {
                ObjectKind::ArrayBuffer(b) | ObjectKind::SharedArrayBuffer(b) => (Arc::clone(b), b.byte_length()),
                _ => return Err(VmError::TypeError("DataView requires an ArrayBuffer or SharedArrayBuffer".into())),
            }
        } else {
            return Err(VmError::TypeError("DataView requires an ArrayBuffer or SharedArrayBuffer".into()));
        };

        let byte_offset = args.get(1).and_then(|v| v.to_int32()).unwrap_or(0).max(0) as usize;
        if byte_offset > buf_len {
            return Err(VmError::RangeError("Start offset is outside the bounds of the buffer".into()));
        }
        let byte_length = if let Some(len_arg) = args.get(2) {
            let l = len_arg.to_int32().unwrap_or(0).max(0) as usize;
            if byte_offset + l > buf_len {
                return Err(VmError::RangeError("Invalid DataView length".into()));
            }
            l
        } else {
            buf_len - byte_offset
        };

        Ok(vm.alloc_data_view(sab, byte_offset, byte_length))
    });

    let dv_proto = vm.alloc_object();

    fn dv_bounds(this: JsValue, args: &[JsValue], size: usize, is_setter: bool) -> Result<(Arc<doms_parallel::SharedArrayBuffer>, usize, bool), VmError> {
        let (buf, off, len) = if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::DataView { buffer, byte_offset, byte_length } = &obj.kind {
                (Arc::clone(buffer), *byte_offset, *byte_length)
            } else {
                return Err(VmError::TypeError("DataView method called on non-DataView".into()));
            }
        } else {
            return Err(VmError::TypeError("DataView method called on non-DataView".into()));
        };
        let byte_idx = args.first().and_then(|v| v.to_int32()).unwrap_or(0);
        if byte_idx < 0 || (byte_idx as usize) + size > len {
            return Err(VmError::RangeError("Offset is outside the bounds of the DataView".into()));
        }
        let le_idx = if is_setter { 2 } else { 1 };
        let le = args.get(le_idx).map(|v| v.is_truthy()).unwrap_or(false);
        Ok((buf, off + (byte_idx as usize), le))
    }

    let dv_get_i8 = vm.alloc_native_function("getInt8", 1, |_vm, this, args| {
        let (buf, pos, _) = dv_bounds(this, args, 1, false)?;
        let b = buf.get(pos).unwrap_or(0) as i8;
        Ok(JsValue::from_int32(b as i32))
    });
    let dv_get_u8 = vm.alloc_native_function("getUint8", 1, |_vm, this, args| {
        let (buf, pos, _) = dv_bounds(this, args, 1, false)?;
        let b = buf.get(pos).unwrap_or(0);
        Ok(JsValue::from_int32(b as i32))
    });
    let dv_set_i8 = vm.alloc_native_function("setInt8", 2, |vm, this, args| {
        let (buf, pos, _) = dv_bounds(this, args, 1, true)?;
        let val = args.get(1).and_then(|v| vm.to_number_value(*v).ok()).unwrap_or(0.0) as i32 as i8 as u8;
        buf.set(pos, val);
        Ok(JsValue::undefined())
    });
    let dv_set_u8 = vm.alloc_native_function("setUint8", 2, |vm, this, args| {
        let (buf, pos, _) = dv_bounds(this, args, 1, true)?;
        let val = args.get(1).and_then(|v| vm.to_number_value(*v).ok()).unwrap_or(0.0) as i32 as u8;
        buf.set(pos, val);
        Ok(JsValue::undefined())
    });

    let dv_get_i16 = vm.alloc_native_function("getInt16", 1, |_vm, this, args| {
        let (buf, pos, le) = dv_bounds(this, args, 2, false)?;
        let b0 = buf.get(pos).unwrap_or(0);
        let b1 = buf.get(pos + 1).unwrap_or(0);
        let v = if le { i16::from_le_bytes([b0, b1]) } else { i16::from_be_bytes([b0, b1]) };
        Ok(JsValue::from_int32(v as i32))
    });
    let dv_get_u16 = vm.alloc_native_function("getUint16", 1, |_vm, this, args| {
        let (buf, pos, le) = dv_bounds(this, args, 2, false)?;
        let b0 = buf.get(pos).unwrap_or(0);
        let b1 = buf.get(pos + 1).unwrap_or(0);
        let v = if le { u16::from_le_bytes([b0, b1]) } else { u16::from_be_bytes([b0, b1]) };
        Ok(JsValue::from_int32(v as i32))
    });
    let dv_set_i16 = vm.alloc_native_function("setInt16", 2, |vm, this, args| {
        let (buf, pos, le) = dv_bounds(this, args, 2, true)?;
        let val = args.get(1).and_then(|v| vm.to_number_value(*v).ok()).unwrap_or(0.0) as i32 as i16;
        let bytes = if le { val.to_le_bytes() } else { val.to_be_bytes() };
        buf.set(pos, bytes[0]);
        buf.set(pos + 1, bytes[1]);
        Ok(JsValue::undefined())
    });
    let dv_set_u16 = vm.alloc_native_function("setUint16", 2, |vm, this, args| {
        let (buf, pos, le) = dv_bounds(this, args, 2, true)?;
        let val = args.get(1).and_then(|v| vm.to_number_value(*v).ok()).unwrap_or(0.0) as u32 as u16;
        let bytes = if le { val.to_le_bytes() } else { val.to_be_bytes() };
        buf.set(pos, bytes[0]);
        buf.set(pos + 1, bytes[1]);
        Ok(JsValue::undefined())
    });

    let dv_get_i32 = vm.alloc_native_function("getInt32", 1, |_vm, this, args| {
        let (buf, pos, le) = dv_bounds(this, args, 4, false)?;
        let mut b = [0u8; 4];
        for i in 0..4 { b[i] = buf.get(pos + i).unwrap_or(0); }
        let v = if le { i32::from_le_bytes(b) } else { i32::from_be_bytes(b) };
        Ok(JsValue::from_int32(v))
    });
    let dv_get_u32 = vm.alloc_native_function("getUint32", 1, |_vm, this, args| {
        let (buf, pos, le) = dv_bounds(this, args, 4, false)?;
        let mut b = [0u8; 4];
        for i in 0..4 { b[i] = buf.get(pos + i).unwrap_or(0); }
        let v = if le { u32::from_le_bytes(b) } else { u32::from_be_bytes(b) };
        Ok(JsValue::from_double(v as f64))
    });
    let dv_set_i32 = vm.alloc_native_function("setInt32", 2, |vm, this, args| {
        let (buf, pos, le) = dv_bounds(this, args, 4, true)?;
        let val = args.get(1).and_then(|v| vm.to_number_value(*v).ok()).unwrap_or(0.0) as i32;
        let bytes = if le { val.to_le_bytes() } else { val.to_be_bytes() };
        for (i, &b) in bytes.iter().enumerate() { buf.set(pos + i, b); }
        Ok(JsValue::undefined())
    });
    let dv_set_u32 = vm.alloc_native_function("setUint32", 2, |vm, this, args| {
        let (buf, pos, le) = dv_bounds(this, args, 4, true)?;
        let val = args.get(1).and_then(|v| vm.to_number_value(*v).ok()).unwrap_or(0.0) as u32;
        let bytes = if le { val.to_le_bytes() } else { val.to_be_bytes() };
        for (i, &b) in bytes.iter().enumerate() { buf.set(pos + i, b); }
        Ok(JsValue::undefined())
    });

    let dv_get_f32 = vm.alloc_native_function("getFloat32", 1, |_vm, this, args| {
        let (buf, pos, le) = dv_bounds(this, args, 4, false)?;
        let mut b = [0u8; 4];
        for i in 0..4 { b[i] = buf.get(pos + i).unwrap_or(0); }
        let v = if le { f32::from_le_bytes(b) } else { f32::from_be_bytes(b) };
        Ok(JsValue::from_double(v as f64))
    });
    let dv_set_f32 = vm.alloc_native_function("setFloat32", 2, |vm, this, args| {
        let (buf, pos, le) = dv_bounds(this, args, 4, true)?;
        let val = args.get(1).and_then(|v| vm.to_number_value(*v).ok()).unwrap_or(0.0) as f32;
        let bytes = if le { val.to_le_bytes() } else { val.to_be_bytes() };
        for (i, &b) in bytes.iter().enumerate() { buf.set(pos + i, b); }
        Ok(JsValue::undefined())
    });

    let dv_get_f64 = vm.alloc_native_function("getFloat64", 1, |_vm, this, args| {
        let (buf, pos, le) = dv_bounds(this, args, 8, false)?;
        let mut b = [0u8; 8];
        for i in 0..8 { b[i] = buf.get(pos + i).unwrap_or(0); }
        let v = if le { f64::from_le_bytes(b) } else { f64::from_be_bytes(b) };
        Ok(JsValue::from_double(v))
    });
    let dv_set_f64 = vm.alloc_native_function("setFloat64", 2, |vm, this, args| {
        let (buf, pos, le) = dv_bounds(this, args, 8, true)?;
        let val = args.get(1).and_then(|v| vm.to_number_value(*v).ok()).unwrap_or(0.0);
        let bytes = if le { val.to_le_bytes() } else { val.to_be_bytes() };
        for (i, &b) in bytes.iter().enumerate() { buf.set(pos + i, b); }
        Ok(JsValue::undefined())
    });

    let dv_get_bi64 = vm.alloc_native_function("getBigInt64", 1, |_vm, this, args| {
        let (buf, pos, le) = dv_bounds(this, args, 8, false)?;
        let mut b = [0u8; 8];
        for i in 0..8 { b[i] = buf.get(pos + i).unwrap_or(0); }
        let v = if le { i64::from_le_bytes(b) } else { i64::from_be_bytes(b) };
        Ok(JsValue::from_double(v as f64))
    });
    let dv_set_bi64 = vm.alloc_native_function("setBigInt64", 2, |vm, this, args| {
        let (buf, pos, le) = dv_bounds(this, args, 8, true)?;
        let val = args.get(1).and_then(|v| vm.to_number_value(*v).ok()).unwrap_or(0.0) as i64;
        let bytes = if le { val.to_le_bytes() } else { val.to_be_bytes() };
        for (i, &b) in bytes.iter().enumerate() { buf.set(pos + i, b); }
        Ok(JsValue::undefined())
    });
    let dv_get_bu64 = vm.alloc_native_function("getBigUint64", 1, |_vm, this, args| {
        let (buf, pos, le) = dv_bounds(this, args, 8, false)?;
        let mut b = [0u8; 8];
        for i in 0..8 { b[i] = buf.get(pos + i).unwrap_or(0); }
        let v = if le { u64::from_le_bytes(b) } else { u64::from_be_bytes(b) };
        Ok(JsValue::from_double(v as f64))
    });
    let dv_set_bu64 = vm.alloc_native_function("setBigUint64", 2, |vm, this, args| {
        let (buf, pos, le) = dv_bounds(this, args, 8, true)?;
        let val = args.get(1).and_then(|v| vm.to_number_value(*v).ok()).unwrap_or(0.0) as u64;
        let bytes = if le { val.to_le_bytes() } else { val.to_be_bytes() };
        for (i, &b) in bytes.iter().enumerate() { buf.set(pos + i, b); }
        Ok(JsValue::undefined())
    });

    vm.set_property(dv_proto, "getInt8", dv_get_i8);
    vm.set_property(dv_proto, "getUint8", dv_get_u8);
    vm.set_property(dv_proto, "setInt8", dv_set_i8);
    vm.set_property(dv_proto, "setUint8", dv_set_u8);
    vm.set_property(dv_proto, "getInt16", dv_get_i16);
    vm.set_property(dv_proto, "getUint16", dv_get_u16);
    vm.set_property(dv_proto, "setInt16", dv_set_i16);
    vm.set_property(dv_proto, "setUint16", dv_set_u16);
    vm.set_property(dv_proto, "getInt32", dv_get_i32);
    vm.set_property(dv_proto, "getUint32", dv_get_u32);
    vm.set_property(dv_proto, "setInt32", dv_set_i32);
    vm.set_property(dv_proto, "setUint32", dv_set_u32);
    vm.set_property(dv_proto, "getFloat32", dv_get_f32);
    vm.set_property(dv_proto, "setFloat32", dv_set_f32);
    vm.set_property(dv_proto, "getFloat64", dv_get_f64);
    vm.set_property(dv_proto, "setFloat64", dv_set_f64);
    vm.set_property(dv_proto, "getBigInt64", dv_get_bi64);
    vm.set_property(dv_proto, "setBigInt64", dv_set_bi64);
    vm.set_property(dv_proto, "getBigUint64", dv_get_bu64);
    vm.set_property(dv_proto, "setBigUint64", dv_set_bu64);

    vm.set_property(dv_ctor, "prototype", dv_proto);
    vm.globals.insert("DataView".to_string(), dv_ctor);
}
