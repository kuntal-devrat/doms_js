//! The `Atomics` namespace.

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

fn validate_integer_typed_array(
    vm: &mut Vm,
    ta_val: JsValue,
    idx_val: JsValue,
    only_int32: bool,
) -> Result<(JsTypedArray, usize), VmError> {
    // 1. Validate TypedArray object
    let ta = if let Some(ptr) = ta_val.as_object_ptr() {
        let obj = unsafe { &*(ptr as *mut JsObject) };
        if let ObjectKind::TypedArray(ta) = &obj.kind {
            ta.clone()
        } else {
            return Err(VmError::TypeError("Atomics operation requires a TypedArray".into()));
        }
    } else {
        return Err(VmError::TypeError("Atomics operation requires a TypedArray".into()));
    };

    // 2. Validate typeName
    if only_int32 {
        if ta.kind != TypedArrayKind::Int32 && ta.kind != TypedArrayKind::BigInt64 {
            return Err(VmError::TypeError("Atomics wait/notify requires an Int32Array or BigInt64Array".into()));
        }
    } else {
        match ta.kind {
            TypedArrayKind::Int8
            | TypedArrayKind::Uint8
            | TypedArrayKind::Int16
            | TypedArrayKind::Uint16
            | TypedArrayKind::Int32
            | TypedArrayKind::Uint32
            | TypedArrayKind::BigInt64
            | TypedArrayKind::BigUint64 => {}
            _ => {
                return Err(VmError::TypeError("Atomics operation requires an integer TypedArray".into()));
            }
        }
    }

    // 3. Coerce index via ToIndex
    let idx = to_index(vm, idx_val)?;

    // 4. Check buffer detached
    if ta.buffer.is_detached() {
        return Err(VmError::TypeError("ArrayBuffer is detached".into()));
    }

    // 5. Check bounds
    if idx >= ta.length {
        return Err(VmError::RangeError("Atomics index out of bounds".into()));
    }

    Ok((ta, idx))
}

pub(super) fn register_atomics(vm: &mut Vm) {
    let atomics_obj = vm.alloc_object();

    let atomics_load = vm.alloc_native_function("load", 2, |vm, _this, args| {
        let ta_val = args.first().copied().unwrap_or(JsValue::undefined());
        let idx_val = args.get(1).copied().unwrap_or(JsValue::undefined());
        let (ta, idx) = validate_integer_typed_array(vm, ta_val, idx_val, false)?;

        let bpe = ta.kind.bytes_per_element();
        let byte_pos = ta.byte_offset + idx * bpe;

        match ta.kind {
            TypedArrayKind::Int8 => {
                let v = ta.buffer.get(byte_pos).unwrap_or(0) as i8;
                Ok(JsValue::from_int32(v as i32))
            }
            TypedArrayKind::Uint8 => {
                let v = ta.buffer.get(byte_pos).unwrap_or(0);
                Ok(JsValue::from_int32(v as i32))
            }
            TypedArrayKind::Int16 => {
                let b0 = ta.buffer.get(byte_pos).unwrap_or(0);
                let b1 = ta.buffer.get(byte_pos + 1).unwrap_or(0);
                let v = i16::from_ne_bytes([b0, b1]);
                Ok(JsValue::from_int32(v as i32))
            }
            TypedArrayKind::Uint16 => {
                let b0 = ta.buffer.get(byte_pos).unwrap_or(0);
                let b1 = ta.buffer.get(byte_pos + 1).unwrap_or(0);
                let v = u16::from_ne_bytes([b0, b1]);
                Ok(JsValue::from_int32(v as i32))
            }
            TypedArrayKind::Int32 => {
                let mut b = [0u8; 4];
                for i in 0..4 { b[i] = ta.buffer.get(byte_pos + i).unwrap_or(0); }
                let v = i32::from_ne_bytes(b);
                Ok(JsValue::from_int32(v))
            }
            TypedArrayKind::Uint32 => {
                let mut b = [0u8; 4];
                for i in 0..4 { b[i] = ta.buffer.get(byte_pos + i).unwrap_or(0); }
                let v = u32::from_ne_bytes(b);
                Ok(JsValue::from_double(v as f64))
            }
            TypedArrayKind::BigInt64 => {
                let mut b = [0u8; 8];
                for i in 0..8 { b[i] = ta.buffer.get(byte_pos + i).unwrap_or(0); }
                let v = i64::from_ne_bytes(b);
                Ok(vm.alloc_bigint(BigInt::from(v)))
            }
            TypedArrayKind::BigUint64 => {
                let mut b = [0u8; 8];
                for i in 0..8 { b[i] = ta.buffer.get(byte_pos + i).unwrap_or(0); }
                let v = u64::from_ne_bytes(b);
                Ok(vm.alloc_bigint(BigInt::from(v)))
            }
            _ => Err(VmError::TypeError("Atomics.load requires an integer TypedArray".into())),
        }
    });

    let atomics_store = vm.alloc_native_function("store", 3, |vm, _this, args| {
        let ta_val = args.first().copied().unwrap_or(JsValue::undefined());
        let idx_val = args.get(1).copied().unwrap_or(JsValue::undefined());
        let val_arg = args.get(2).copied().unwrap_or(JsValue::undefined());
        let (ta, idx) = validate_integer_typed_array(vm, ta_val, idx_val, false)?;

        let bpe = ta.kind.bytes_per_element();
        let byte_pos = ta.byte_offset + idx * bpe;

        if ta.kind == TypedArrayKind::BigInt64 || ta.kind == TypedArrayKind::BigUint64 {
            let bi = vm.to_bigint_op(val_arg)?;
            let val_u64 = bigint_to_u64(&bi);
            let bytes = val_u64.to_ne_bytes();
            for (i, &b) in bytes.iter().enumerate() {
                ta.buffer.set(byte_pos + i, b);
            }
            Ok(vm.alloc_bigint(bi))
        } else {
            let num = vm.to_number_value(val_arg)?;
            match ta.kind {
                TypedArrayKind::Int8 => {
                    let b = num as i32 as i8 as u8;
                    ta.buffer.set(byte_pos, b);
                    Ok(JsValue::from_int32(num as i32))
                }
                TypedArrayKind::Uint8 => {
                    let b = num as i32 as u8;
                    ta.buffer.set(byte_pos, b);
                    Ok(JsValue::from_int32(num as i32))
                }
                TypedArrayKind::Int16 => {
                    let bytes = (num as i32 as i16).to_ne_bytes();
                    ta.buffer.set(byte_pos, bytes[0]);
                    ta.buffer.set(byte_pos + 1, bytes[1]);
                    Ok(JsValue::from_int32(num as i32))
                }
                TypedArrayKind::Uint16 => {
                    let bytes = (num as i32 as u16).to_ne_bytes();
                    ta.buffer.set(byte_pos, bytes[0]);
                    ta.buffer.set(byte_pos + 1, bytes[1]);
                    Ok(JsValue::from_int32(num as i32))
                }
                TypedArrayKind::Int32 => {
                    let bytes = (num as i32).to_ne_bytes();
                    for (i, &b) in bytes.iter().enumerate() { ta.buffer.set(byte_pos + i, b); }
                    Ok(JsValue::from_int32(num as i32))
                }
                TypedArrayKind::Uint32 => {
                    let bytes = (num as u32).to_ne_bytes();
                    for (i, &b) in bytes.iter().enumerate() { ta.buffer.set(byte_pos + i, b); }
                    Ok(JsValue::from_double(num as u32 as f64))
                }
                _ => Err(VmError::TypeError("Atomics.store requires an integer TypedArray".into())),
            }
        }
    });

    let make_rmw_op = |name: &'static str, op_int: fn(i32, i32) -> i32, op_bi: fn(i64, i64) -> i64| {
        move |vm: &mut Vm, _this: JsValue, args: &[JsValue]| {
            let ta_val = args.first().copied().unwrap_or(JsValue::undefined());
            let idx_val = args.get(1).copied().unwrap_or(JsValue::undefined());
            let val_arg = args.get(2).copied().unwrap_or(JsValue::undefined());
            let (ta, idx) = validate_integer_typed_array(vm, ta_val, idx_val, false)?;

            let bpe = ta.kind.bytes_per_element();
            let byte_pos = ta.byte_offset + idx * bpe;

            if ta.kind == TypedArrayKind::BigInt64 || ta.kind == TypedArrayKind::BigUint64 {
                let bi = vm.to_bigint_op(val_arg)?;
                let val_u64 = bigint_to_u64(&bi);
                let mut b = [0u8; 8];
                for i in 0..8 { b[i] = ta.buffer.get(byte_pos + i).unwrap_or(0); }
                let old = i64::from_ne_bytes(b);
                let new_val = op_bi(old, val_u64 as i64);
                let bytes = new_val.to_ne_bytes();
                for (i, &byte) in bytes.iter().enumerate() {
                    ta.buffer.set(byte_pos + i, byte);
                }
                if ta.kind == TypedArrayKind::BigInt64 {
                    Ok(vm.alloc_bigint(BigInt::from(old)))
                } else {
                    Ok(vm.alloc_bigint(BigInt::from(old as u64)))
                }
            } else {
                let num = vm.to_number_value(val_arg)?;
                let rhs = num as i32;
                match ta.kind {
                    TypedArrayKind::Int8 => {
                        let old = ta.buffer.get(byte_pos).unwrap_or(0) as i8 as i32;
                        let new_val = op_int(old, rhs) as i8 as u8;
                        ta.buffer.set(byte_pos, new_val);
                        Ok(JsValue::from_int32(old))
                    }
                    TypedArrayKind::Uint8 => {
                        let old = ta.buffer.get(byte_pos).unwrap_or(0) as i32;
                        let new_val = op_int(old, rhs) as u8;
                        ta.buffer.set(byte_pos, new_val);
                        Ok(JsValue::from_int32(old))
                    }
                    TypedArrayKind::Int16 => {
                        let b0 = ta.buffer.get(byte_pos).unwrap_or(0);
                        let b1 = ta.buffer.get(byte_pos + 1).unwrap_or(0);
                        let old = i16::from_ne_bytes([b0, b1]) as i32;
                        let new_val = (op_int(old, rhs) as i16).to_ne_bytes();
                        ta.buffer.set(byte_pos, new_val[0]);
                        ta.buffer.set(byte_pos + 1, new_val[1]);
                        Ok(JsValue::from_int32(old))
                    }
                    TypedArrayKind::Uint16 => {
                        let b0 = ta.buffer.get(byte_pos).unwrap_or(0);
                        let b1 = ta.buffer.get(byte_pos + 1).unwrap_or(0);
                        let old = u16::from_ne_bytes([b0, b1]) as i32;
                        let new_val = (op_int(old, rhs) as u16).to_ne_bytes();
                        ta.buffer.set(byte_pos, new_val[0]);
                        ta.buffer.set(byte_pos + 1, new_val[1]);
                        Ok(JsValue::from_int32(old))
                    }
                    TypedArrayKind::Int32 => {
                        let mut b = [0u8; 4];
                        for i in 0..4 { b[i] = ta.buffer.get(byte_pos + i).unwrap_or(0); }
                        let old = i32::from_ne_bytes(b);
                        let new_val = op_int(old, rhs).to_ne_bytes();
                        for (i, &byte) in new_val.iter().enumerate() { ta.buffer.set(byte_pos + i, byte); }
                        Ok(JsValue::from_int32(old))
                    }
                    TypedArrayKind::Uint32 => {
                        let mut b = [0u8; 4];
                        for i in 0..4 { b[i] = ta.buffer.get(byte_pos + i).unwrap_or(0); }
                        let old = u32::from_ne_bytes(b);
                        let new_val = (op_int(old as i32, rhs) as u32).to_ne_bytes();
                        for (i, &byte) in new_val.iter().enumerate() { ta.buffer.set(byte_pos + i, byte); }
                        Ok(JsValue::from_double(old as f64))
                    }
                    _ => Err(VmError::TypeError(format!("Atomics.{} requires an integer TypedArray", name).into())),
                }
            }
        }
    };

    let atomics_add = vm.alloc_closure_native_function("add", 3, Arc::new(make_rmw_op("add", |a, b| a.wrapping_add(b), |a, b| a.wrapping_add(b))));
    let atomics_sub = vm.alloc_closure_native_function("sub", 3, Arc::new(make_rmw_op("sub", |a, b| a.wrapping_sub(b), |a, b| a.wrapping_sub(b))));
    let atomics_and = vm.alloc_closure_native_function("and", 3, Arc::new(make_rmw_op("and", |a, b| a & b, |a, b| a & b)));
    let atomics_or = vm.alloc_closure_native_function("or", 3, Arc::new(make_rmw_op("or", |a, b| a | b, |a, b| a | b)));
    let atomics_xor = vm.alloc_closure_native_function("xor", 3, Arc::new(make_rmw_op("xor", |a, b| a ^ b, |a, b| a ^ b)));

    let atomics_exchange = vm.alloc_native_function("exchange", 3, |vm, _this, args| {
        let ta_val = args.first().copied().unwrap_or(JsValue::undefined());
        let idx_val = args.get(1).copied().unwrap_or(JsValue::undefined());
        let val_arg = args.get(2).copied().unwrap_or(JsValue::undefined());
        let (ta, idx) = validate_integer_typed_array(vm, ta_val, idx_val, false)?;

        let bpe = ta.kind.bytes_per_element();
        let byte_pos = ta.byte_offset + idx * bpe;

        if ta.kind == TypedArrayKind::BigInt64 || ta.kind == TypedArrayKind::BigUint64 {
            let bi = vm.to_bigint_op(val_arg)?;
            let val_u64 = bigint_to_u64(&bi);
            let mut b = [0u8; 8];
            for i in 0..8 { b[i] = ta.buffer.get(byte_pos + i).unwrap_or(0); }
            let old = i64::from_ne_bytes(b);
            let bytes = val_u64.to_ne_bytes();
            for (i, &byte) in bytes.iter().enumerate() { ta.buffer.set(byte_pos + i, byte); }
            if ta.kind == TypedArrayKind::BigInt64 {
                Ok(vm.alloc_bigint(BigInt::from(old)))
            } else {
                Ok(vm.alloc_bigint(BigInt::from(old as u64)))
            }
        } else {
            let num = vm.to_number_value(val_arg)?;
            match ta.kind {
                TypedArrayKind::Int8 => {
                    let old = ta.buffer.get(byte_pos).unwrap_or(0) as i8 as i32;
                    ta.buffer.set(byte_pos, num as i32 as i8 as u8);
                    Ok(JsValue::from_int32(old))
                }
                TypedArrayKind::Uint8 => {
                    let old = ta.buffer.get(byte_pos).unwrap_or(0) as i32;
                    ta.buffer.set(byte_pos, num as i32 as u8);
                    Ok(JsValue::from_int32(old))
                }
                TypedArrayKind::Int16 => {
                    let b0 = ta.buffer.get(byte_pos).unwrap_or(0);
                    let b1 = ta.buffer.get(byte_pos + 1).unwrap_or(0);
                    let old = i16::from_ne_bytes([b0, b1]) as i32;
                    let bytes = (num as i32 as i16).to_ne_bytes();
                    ta.buffer.set(byte_pos, bytes[0]);
                    ta.buffer.set(byte_pos + 1, bytes[1]);
                    Ok(JsValue::from_int32(old))
                }
                TypedArrayKind::Uint16 => {
                    let b0 = ta.buffer.get(byte_pos).unwrap_or(0);
                    let b1 = ta.buffer.get(byte_pos + 1).unwrap_or(0);
                    let old = u16::from_ne_bytes([b0, b1]) as i32;
                    let bytes = (num as i32 as u16).to_ne_bytes();
                    ta.buffer.set(byte_pos, bytes[0]);
                    ta.buffer.set(byte_pos + 1, bytes[1]);
                    Ok(JsValue::from_int32(old))
                }
                TypedArrayKind::Int32 => {
                    let mut b = [0u8; 4];
                    for i in 0..4 { b[i] = ta.buffer.get(byte_pos + i).unwrap_or(0); }
                    let old = i32::from_ne_bytes(b);
                    let bytes = (num as i32).to_ne_bytes();
                    for (i, &byte) in bytes.iter().enumerate() { ta.buffer.set(byte_pos + i, byte); }
                    Ok(JsValue::from_int32(old))
                }
                TypedArrayKind::Uint32 => {
                    let mut b = [0u8; 4];
                    for i in 0..4 { b[i] = ta.buffer.get(byte_pos + i).unwrap_or(0); }
                    let old = u32::from_ne_bytes(b);
                    let bytes = (num as u32).to_ne_bytes();
                    for (i, &byte) in bytes.iter().enumerate() { ta.buffer.set(byte_pos + i, byte); }
                    Ok(JsValue::from_double(old as f64))
                }
                _ => Err(VmError::TypeError("Atomics.exchange requires an integer TypedArray".into())),
            }
        }
    });

    let atomics_cmpxchg = vm.alloc_native_function("compareExchange", 4, |vm, _this, args| {
        let ta_val = args.first().copied().unwrap_or(JsValue::undefined());
        let idx_val = args.get(1).copied().unwrap_or(JsValue::undefined());
        let exp_arg = args.get(2).copied().unwrap_or(JsValue::undefined());
        let rep_arg = args.get(3).copied().unwrap_or(JsValue::undefined());
        let (ta, idx) = validate_integer_typed_array(vm, ta_val, idx_val, false)?;

        let bpe = ta.kind.bytes_per_element();
        let byte_pos = ta.byte_offset + idx * bpe;

        if ta.kind == TypedArrayKind::BigInt64 || ta.kind == TypedArrayKind::BigUint64 {
            let exp_bi = vm.to_bigint_op(exp_arg)?;
            let rep_bi = vm.to_bigint_op(rep_arg)?;
            let exp_u64 = bigint_to_u64(&exp_bi);
            let rep_u64 = bigint_to_u64(&rep_bi);
            let mut b = [0u8; 8];
            for i in 0..8 { b[i] = ta.buffer.get(byte_pos + i).unwrap_or(0); }
            let old = u64::from_ne_bytes(b);
            if old == exp_u64 {
                let bytes = rep_u64.to_ne_bytes();
                for (i, &byte) in bytes.iter().enumerate() { ta.buffer.set(byte_pos + i, byte); }
            }
            if ta.kind == TypedArrayKind::BigInt64 {
                Ok(vm.alloc_bigint(BigInt::from(old as i64)))
            } else {
                Ok(vm.alloc_bigint(BigInt::from(old)))
            }
        } else {
            let exp_num = vm.to_number_value(exp_arg)?;
            let rep_num = vm.to_number_value(rep_arg)?;
            match ta.kind {
                TypedArrayKind::Int8 => {
                    let old = ta.buffer.get(byte_pos).unwrap_or(0) as i8;
                    if old == exp_num as i32 as i8 {
                        ta.buffer.set(byte_pos, rep_num as i32 as i8 as u8);
                    }
                    Ok(JsValue::from_int32(old as i32))
                }
                TypedArrayKind::Uint8 => {
                    let old = ta.buffer.get(byte_pos).unwrap_or(0);
                    if old == exp_num as i32 as u8 {
                        ta.buffer.set(byte_pos, rep_num as i32 as u8);
                    }
                    Ok(JsValue::from_int32(old as i32))
                }
                TypedArrayKind::Int16 => {
                    let b0 = ta.buffer.get(byte_pos).unwrap_or(0);
                    let b1 = ta.buffer.get(byte_pos + 1).unwrap_or(0);
                    let old = i16::from_ne_bytes([b0, b1]);
                    if old == exp_num as i32 as i16 {
                        let bytes = (rep_num as i32 as i16).to_ne_bytes();
                        ta.buffer.set(byte_pos, bytes[0]);
                        ta.buffer.set(byte_pos + 1, bytes[1]);
                    }
                    Ok(JsValue::from_int32(old as i32))
                }
                TypedArrayKind::Uint16 => {
                    let b0 = ta.buffer.get(byte_pos).unwrap_or(0);
                    let b1 = ta.buffer.get(byte_pos + 1).unwrap_or(0);
                    let old = u16::from_ne_bytes([b0, b1]);
                    if old == exp_num as i32 as u16 {
                        let bytes = (rep_num as i32 as u16).to_ne_bytes();
                        ta.buffer.set(byte_pos, bytes[0]);
                        ta.buffer.set(byte_pos + 1, bytes[1]);
                    }
                    Ok(JsValue::from_int32(old as i32))
                }
                TypedArrayKind::Int32 => {
                    let mut b = [0u8; 4];
                    for i in 0..4 { b[i] = ta.buffer.get(byte_pos + i).unwrap_or(0); }
                    let old = i32::from_ne_bytes(b);
                    if old == exp_num as i32 {
                        let bytes = (rep_num as i32).to_ne_bytes();
                        for (i, &byte) in bytes.iter().enumerate() { ta.buffer.set(byte_pos + i, byte); }
                    }
                    Ok(JsValue::from_int32(old))
                }
                TypedArrayKind::Uint32 => {
                    let mut b = [0u8; 4];
                    for i in 0..4 { b[i] = ta.buffer.get(byte_pos + i).unwrap_or(0); }
                    let old = u32::from_ne_bytes(b);
                    if old == exp_num as u32 {
                        let bytes = (rep_num as u32).to_ne_bytes();
                        for (i, &byte) in bytes.iter().enumerate() { ta.buffer.set(byte_pos + i, byte); }
                    }
                    Ok(JsValue::from_double(old as f64))
                }
                _ => Err(VmError::TypeError("Atomics.compareExchange requires an integer TypedArray".into())),
            }
        }
    });

    let atomics_lockfree = vm.alloc_native_function("isLockFree", 1, |vm, _this, args| {
        let size_val = args.first().copied().unwrap_or(JsValue::undefined());
        let n = vm.to_number_value(size_val)?;
        if n.is_nan() || n.is_infinite() {
            return Ok(JsValue::from_boolean(false));
        }
        let size = n.trunc() as i64;
        let ok = matches!(size, 1 | 2 | 4 | 8);
        Ok(JsValue::from_boolean(ok))
    });

    let atomics_wait = vm.alloc_native_function("wait", 4, |vm, _this, args| {
        let ta_val = args.first().copied().unwrap_or(JsValue::undefined());
        let idx_val = args.get(1).copied().unwrap_or(JsValue::undefined());
        let exp_arg = args.get(2).copied().unwrap_or(JsValue::undefined());
        let timeout_arg = args.get(3).copied().unwrap_or(JsValue::undefined());

        let (ta, idx) = validate_integer_typed_array(vm, ta_val, idx_val, true)?;

        let expected = vm.to_number_value(exp_arg)? as i32;
        let timeout = if timeout_arg.is_undefined() {
            None
        } else {
            let ms = vm.to_number_value(timeout_arg)?;
            if ms.is_nan() || ms < 0.0 {
                Some(std::time::Duration::ZERO)
            } else {
                Some(std::time::Duration::from_millis(ms as u64))
            }
        };

        let byte_off = ta.byte_offset + idx * ta.kind.bytes_per_element();
        let res = doms_parallel::Atomics::wait(&ta.buffer, byte_off, expected, timeout)
            .map_err(|e| VmError::RuntimeError(e.to_string()))?;
        Ok(vm.alloc_string(res.to_string()))
    });

    let atomics_notify = vm.alloc_native_function("notify", 3, |vm, _this, args| {
        let ta_val = args.first().copied().unwrap_or(JsValue::undefined());
        let idx_val = args.get(1).copied().unwrap_or(JsValue::undefined());
        let count_arg = args.get(2).copied().unwrap_or(JsValue::undefined());

        let (ta, idx) = validate_integer_typed_array(vm, ta_val, idx_val, true)?;

        let count = if count_arg.is_undefined() {
            usize::MAX
        } else {
            let n = vm.to_number_value(count_arg)?;
            if n.is_nan() || n < 0.0 {
                0
            } else if n.is_infinite() || n >= (usize::MAX as f64) {
                usize::MAX
            } else {
                n.trunc() as usize
            }
        };

        let byte_off = ta.byte_offset + idx * ta.kind.bytes_per_element();
        let woken = doms_parallel::Atomics::notify(&ta.buffer, byte_off, count)
            .map_err(|e| VmError::RuntimeError(e.to_string()))?;
        Ok(JsValue::from_int32(woken as i32))
    });

    let methods = [
        ("load", atomics_load),
        ("store", atomics_store),
        ("add", atomics_add),
        ("sub", atomics_sub),
        ("and", atomics_and),
        ("or", atomics_or),
        ("xor", atomics_xor),
        ("exchange", atomics_exchange),
        ("compareExchange", atomics_cmpxchg),
        ("isLockFree", atomics_lockfree),
        ("wait", atomics_wait),
        ("notify", atomics_notify),
    ];

    for (name, func) in methods {
        vm.set_property(atomics_obj, name, func);
        vm.set_property_non_enumerable(atomics_obj, name.to_string());
    }

    if let Some(sym_tag) = vm.symbol_to_string_tag {
        let tag_val = vm.alloc_string("Atomics".into());
        vm.set_symbol_property(atomics_obj, sym_tag, tag_val);
        let key_name = format!("[Symbol:{:x}]", sym_tag.raw_bits());
        vm.set_property_non_writable(atomics_obj, key_name.clone());
        vm.set_property_non_enumerable(atomics_obj, key_name);
    }

    vm.globals.insert("Atomics".to_string(), atomics_obj);
    if vm.global_object.is_object() {
        vm.set_property(vm.global_object, "Atomics", atomics_obj);
    }
}
