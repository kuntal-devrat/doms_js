//! The `Atomics` namespace.

use super::*;

pub(super) fn register_atomics(vm: &mut Vm) {
    let atomics_obj = vm.alloc_object();

    let atomics_load = vm.alloc_native_function("load", 2, |_vm, _this, args| {
        let ta_val = args.first().copied().unwrap_or(JsValue::undefined());
        let idx = args.get(1).and_then(|v| v.to_int32()).unwrap_or(0).max(0) as usize;
        if let Some(ptr) = ta_val.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::TypedArray(ta) = &obj.kind {
                if let Some(val) = Vm::read_typed_array_element(ta, idx) {
                    return Ok(val);
                }
            }
        }
        Err(VmError::TypeError("Atomics.load requires a TypedArray".into()))
    });

    let atomics_store = vm.alloc_native_function("store", 3, |_vm, _this, args| {
        let ta_val = args.first().copied().unwrap_or(JsValue::undefined());
        let idx = args.get(1).and_then(|v| v.to_int32()).unwrap_or(0).max(0) as usize;
        let val = args.get(2).copied().unwrap_or(JsValue::undefined());
        if let Some(ptr) = ta_val.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::TypedArray(ta) = &mut obj.kind {
                if Vm::write_typed_array_element(ta, idx, val) {
                    return Ok(val);
                }
            }
        }
        Err(VmError::TypeError("Atomics.store requires a TypedArray".into()))
    });

    let atomics_add = vm.alloc_native_function("add", 3, |_vm, _this, args| {
        let ta_val = args.first().copied().unwrap_or(JsValue::undefined());
        let idx = args.get(1).and_then(|v| v.to_int32()).unwrap_or(0).max(0) as usize;
        let value = args.get(2).and_then(|v| v.to_number()).unwrap_or(0.0) as i32;
        if let Some(ptr) = ta_val.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::TypedArray(ta) = &mut obj.kind {
                if let Some(old) = Vm::read_typed_array_element(ta, idx) {
                    let old_n = old.to_number().unwrap_or(0.0) as i32;
                    let new_val = JsValue::from_int32(old_n + value);
                    Vm::write_typed_array_element(ta, idx, new_val);
                    return Ok(old);
                }
            }
        }
        Err(VmError::TypeError("Atomics.add requires a TypedArray".into()))
    });

    let atomics_sub = vm.alloc_native_function("sub", 3, |_vm, _this, args| {
        let ta_val = args.first().copied().unwrap_or(JsValue::undefined());
        let idx = args.get(1).and_then(|v| v.to_int32()).unwrap_or(0).max(0) as usize;
        let value = args.get(2).and_then(|v| v.to_number()).unwrap_or(0.0) as i32;
        if let Some(ptr) = ta_val.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::TypedArray(ta) = &mut obj.kind {
                if let Some(old) = Vm::read_typed_array_element(ta, idx) {
                    let old_n = old.to_number().unwrap_or(0.0) as i32;
                    let new_val = JsValue::from_int32(old_n - value);
                    Vm::write_typed_array_element(ta, idx, new_val);
                    return Ok(old);
                }
            }
        }
        Err(VmError::TypeError("Atomics.sub requires a TypedArray".into()))
    });

    let atomics_and = vm.alloc_native_function("and", 3, |_vm, _this, args| {
        let ta_val = args.first().copied().unwrap_or(JsValue::undefined());
        let idx = args.get(1).and_then(|v| v.to_int32()).unwrap_or(0).max(0) as usize;
        let value = args.get(2).and_then(|v| v.to_number()).unwrap_or(0.0) as i32;
        if let Some(ptr) = ta_val.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::TypedArray(ta) = &mut obj.kind {
                if let Some(old) = Vm::read_typed_array_element(ta, idx) {
                    let old_n = old.to_number().unwrap_or(0.0) as i32;
                    let new_val = JsValue::from_int32(old_n & value);
                    Vm::write_typed_array_element(ta, idx, new_val);
                    return Ok(old);
                }
            }
        }
        Err(VmError::TypeError("Atomics.and requires a TypedArray".into()))
    });

    let atomics_or = vm.alloc_native_function("or", 3, |_vm, _this, args| {
        let ta_val = args.first().copied().unwrap_or(JsValue::undefined());
        let idx = args.get(1).and_then(|v| v.to_int32()).unwrap_or(0).max(0) as usize;
        let value = args.get(2).and_then(|v| v.to_number()).unwrap_or(0.0) as i32;
        if let Some(ptr) = ta_val.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::TypedArray(ta) = &mut obj.kind {
                if let Some(old) = Vm::read_typed_array_element(ta, idx) {
                    let old_n = old.to_number().unwrap_or(0.0) as i32;
                    let new_val = JsValue::from_int32(old_n | value);
                    Vm::write_typed_array_element(ta, idx, new_val);
                    return Ok(old);
                }
            }
        }
        Err(VmError::TypeError("Atomics.or requires a TypedArray".into()))
    });

    let atomics_xor = vm.alloc_native_function("xor", 3, |_vm, _this, args| {
        let ta_val = args.first().copied().unwrap_or(JsValue::undefined());
        let idx = args.get(1).and_then(|v| v.to_int32()).unwrap_or(0).max(0) as usize;
        let value = args.get(2).and_then(|v| v.to_number()).unwrap_or(0.0) as i32;
        if let Some(ptr) = ta_val.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::TypedArray(ta) = &mut obj.kind {
                if let Some(old) = Vm::read_typed_array_element(ta, idx) {
                    let old_n = old.to_number().unwrap_or(0.0) as i32;
                    let new_val = JsValue::from_int32(old_n ^ value);
                    Vm::write_typed_array_element(ta, idx, new_val);
                    return Ok(old);
                }
            }
        }
        Err(VmError::TypeError("Atomics.xor requires a TypedArray".into()))
    });

    let atomics_exchange = vm.alloc_native_function("exchange", 3, |_vm, _this, args| {
        let ta_val = args.first().copied().unwrap_or(JsValue::undefined());
        let idx = args.get(1).and_then(|v| v.to_int32()).unwrap_or(0).max(0) as usize;
        let val = args.get(2).copied().unwrap_or(JsValue::undefined());
        if let Some(ptr) = ta_val.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::TypedArray(ta) = &mut obj.kind {
                if let Some(old) = Vm::read_typed_array_element(ta, idx) {
                    Vm::write_typed_array_element(ta, idx, val);
                    return Ok(old);
                }
            }
        }
        Err(VmError::TypeError("Atomics.exchange requires a TypedArray".into()))
    });

    let atomics_cmpxchg = vm.alloc_native_function("compareExchange", 4, |_vm, _this, args| {
        let ta_val = args.first().copied().unwrap_or(JsValue::undefined());
        let idx = args.get(1).and_then(|v| v.to_int32()).unwrap_or(0).max(0) as usize;
        let expected = args.get(2).and_then(|v| v.to_number()).unwrap_or(0.0) as i32;
        let replacement = args.get(3).copied().unwrap_or(JsValue::undefined());
        if let Some(ptr) = ta_val.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::TypedArray(ta) = &mut obj.kind {
                if let Some(old) = Vm::read_typed_array_element(ta, idx) {
                    let old_n = old.to_number().unwrap_or(0.0) as i32;
                    if old_n == expected {
                        Vm::write_typed_array_element(ta, idx, replacement);
                    }
                    return Ok(old);
                }
            }
        }
        Err(VmError::TypeError("Atomics.compareExchange requires a TypedArray".into()))
    });

    let atomics_lockfree = vm.alloc_native_function("isLockFree", 1, |_vm, _this, args| {
        let size = args.first().and_then(|v| v.to_int32()).unwrap_or(0).max(0) as usize;
        Ok(JsValue::from_boolean(doms_parallel::Atomics::is_lock_free(size)))
    });

    let atomics_wait = vm.alloc_native_function("wait", 4, |vm, _this, args| {
        let ta_val = args.first().copied().unwrap_or(JsValue::undefined());
        let idx = args.get(1).and_then(|v| v.to_int32()).unwrap_or(0).max(0) as usize;
        let expected = args.get(2).and_then(|v| v.to_int32()).unwrap_or(0);
        let timeout = match args.get(3) {
            Some(v) if !v.is_undefined() => {
                let ms = v.to_number().unwrap_or(0.0);
                if ms.is_nan() || ms < 0.0 {
                    Some(std::time::Duration::ZERO)
                } else {
                    Some(std::time::Duration::from_millis(ms as u64))
                }
            }
            _ => None,
        };

        if let Some(ptr) = ta_val.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::TypedArray(ta) = &obj.kind {
                if ta.kind != TypedArrayKind::Int32 {
                    return Err(VmError::TypeError("Atomics.wait requires an Int32Array".into()));
                }
                let byte_off = ta.byte_offset + idx * 4;
                let res = doms_parallel::Atomics::wait(&ta.buffer, byte_off, expected, timeout)
                    .map_err(|e| VmError::RuntimeError(e.to_string()))?;
                return Ok(vm.alloc_string(res.to_string()));
            }
        }
        Err(VmError::TypeError("Atomics.wait requires an Int32Array".into()))
    });

    let atomics_notify = vm.alloc_native_function("notify", 3, |_vm, _this, args| {
        let ta_val = args.first().copied().unwrap_or(JsValue::undefined());
        let idx = args.get(1).and_then(|v| v.to_int32()).unwrap_or(0).max(0) as usize;
        let count = match args.get(2) {
            Some(v) if !v.is_undefined() => {
                let n = v.to_number().unwrap_or(f64::INFINITY);
                if n.is_infinite() || n >= (usize::MAX as f64) {
                    usize::MAX
                } else {
                    n.max(0.0) as usize
                }
            }
            _ => usize::MAX,
        };

        if let Some(ptr) = ta_val.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::TypedArray(ta) = &obj.kind {
                let byte_off = ta.byte_offset + idx * ta.kind.bytes_per_element();
                let woken = doms_parallel::Atomics::notify(&ta.buffer, byte_off, count)
                    .map_err(|e| VmError::RuntimeError(e.to_string()))?;
                return Ok(JsValue::from_int32(woken as i32));
            }
        }
        Err(VmError::TypeError("Atomics.notify requires a TypedArray".into()))
    });

    vm.set_property(atomics_obj, "load", atomics_load);
    vm.set_property(atomics_obj, "store", atomics_store);
    vm.set_property(atomics_obj, "add", atomics_add);
    vm.set_property(atomics_obj, "sub", atomics_sub);
    vm.set_property(atomics_obj, "and", atomics_and);
    vm.set_property(atomics_obj, "or", atomics_or);
    vm.set_property(atomics_obj, "xor", atomics_xor);
    vm.set_property(atomics_obj, "exchange", atomics_exchange);
    vm.set_property(atomics_obj, "compareExchange", atomics_cmpxchg);
    vm.set_property(atomics_obj, "isLockFree", atomics_lockfree);
    vm.set_property(atomics_obj, "wait", atomics_wait);
    vm.set_property(atomics_obj, "notify", atomics_notify);
    vm.globals.insert("Atomics".to_string(), atomics_obj);
    if vm.global_object.is_object() {
        vm.set_property(vm.global_object, "Atomics", atomics_obj);
    }
}
