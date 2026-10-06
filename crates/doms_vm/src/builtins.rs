//! Built-in functions, objects, and prototype methods.

use crate::interpreter::{Vm, VmError};
use crate::object::{JsObject, ObjectKind};
use crate::value::JsValue;
use std::sync::Arc;

pub fn register_builtins(vm: &mut Vm) {
    register_console(vm);
    register_math(vm);
    register_json(vm);
    register_object_builtins(vm);
    register_array_builtins(vm);
    register_global_helpers(vm);
    register_promise(vm);
    register_parallel_and_typed_arrays(vm);
    register_phase3_builtins(vm);
}

fn register_console(vm: &mut Vm) {
    let log_fn = vm.alloc_native_function("log", 1, native_console_log);
    let error_fn = vm.alloc_native_function("error", 1, native_console_error);
    let warn_fn = vm.alloc_native_function("warn", 1, native_console_warn);
    let info_fn = vm.alloc_native_function("info", 1, native_console_info);

    let console_obj = vm.alloc_object();
    vm.set_property(console_obj, "log", log_fn);
    vm.set_property(console_obj, "error", error_fn);
    vm.set_property(console_obj, "warn", warn_fn);
    vm.set_property(console_obj, "info", info_fn);

    vm.globals.insert("console".to_string(), console_obj);
}

fn native_console_log(vm: &mut Vm, _this: JsValue, args: &[JsValue]) -> Result<JsValue, VmError> {
    let parts: Vec<String> = args.iter().map(|arg| vm.to_string_value(*arg)).collect();
    println!("{}", parts.join(" "));
    Ok(JsValue::undefined())
}

fn native_console_error(vm: &mut Vm, _this: JsValue, args: &[JsValue]) -> Result<JsValue, VmError> {
    let parts: Vec<String> = args.iter().map(|arg| vm.to_string_value(*arg)).collect();
    eprintln!("{}", parts.join(" "));
    Ok(JsValue::undefined())
}

fn native_console_warn(vm: &mut Vm, _this: JsValue, args: &[JsValue]) -> Result<JsValue, VmError> {
    let parts: Vec<String> = args.iter().map(|arg| vm.to_string_value(*arg)).collect();
    eprintln!("[WARN] {}", parts.join(" "));
    Ok(JsValue::undefined())
}

fn native_console_info(vm: &mut Vm, _this: JsValue, args: &[JsValue]) -> Result<JsValue, VmError> {
    let parts: Vec<String> = args.iter().map(|arg| vm.to_string_value(*arg)).collect();
    println!("[INFO] {}", parts.join(" "));
    Ok(JsValue::undefined())
}

fn register_math(vm: &mut Vm) {
    let math_obj = vm.alloc_object();

    let abs_fn = vm.alloc_native_function("abs", 1, |_vm, _this, args| {
        let n = args.first().and_then(|v| v.to_number()).unwrap_or(f64::NAN);
        Ok(JsValue::from_double(n.abs()))
    });
    let floor_fn = vm.alloc_native_function("floor", 1, |_vm, _this, args| {
        let n = args.first().and_then(|v| v.to_number()).unwrap_or(f64::NAN);
        Ok(JsValue::from_double(n.floor()))
    });
    let ceil_fn = vm.alloc_native_function("ceil", 1, |_vm, _this, args| {
        let n = args.first().and_then(|v| v.to_number()).unwrap_or(f64::NAN);
        Ok(JsValue::from_double(n.ceil()))
    });
    let round_fn = vm.alloc_native_function("round", 1, |_vm, _this, args| {
        let n = args.first().and_then(|v| v.to_number()).unwrap_or(f64::NAN);
        Ok(JsValue::from_double(n.round()))
    });
    let sqrt_fn = vm.alloc_native_function("sqrt", 1, |_vm, _this, args| {
        let n = args.first().and_then(|v| v.to_number()).unwrap_or(f64::NAN);
        Ok(JsValue::from_double(n.sqrt()))
    });
    let pow_fn = vm.alloc_native_function("pow", 2, |_vm, _this, args| {
        let base = args.first().and_then(|v| v.to_number()).unwrap_or(f64::NAN);
        let exp = args.get(1).and_then(|v| v.to_number()).unwrap_or(f64::NAN);
        Ok(JsValue::from_double(base.powf(exp)))
    });
    let min_fn = vm.alloc_native_function("min", 2, |_vm, _this, args| {
        let mut min = f64::INFINITY;
        for a in args {
            let n = a.to_number().unwrap_or(f64::NAN);
            if n < min {
                min = n;
            }
        }
        Ok(JsValue::from_double(min))
    });
    let max_fn = vm.alloc_native_function("max", 2, |_vm, _this, args| {
        let mut max = f64::NEG_INFINITY;
        for a in args {
            let n = a.to_number().unwrap_or(f64::NAN);
            if n > max {
                max = n;
            }
        }
        Ok(JsValue::from_double(max))
    });
    let random_fn = vm.alloc_native_function("random", 0, |_vm, _this, _args| {
        // Simple deterministic PRNG based on timestamp / iteration counter
        static mut SEED: u64 = 123456789;
        let rand_val = unsafe {
            SEED = SEED.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (SEED >> 11) as f64 / (1u64 << 53) as f64
        };
        Ok(JsValue::from_double(rand_val))
    });

    vm.set_property(math_obj, "abs", abs_fn);
    vm.set_property(math_obj, "floor", floor_fn);
    vm.set_property(math_obj, "ceil", ceil_fn);
    vm.set_property(math_obj, "round", round_fn);
    vm.set_property(math_obj, "sqrt", sqrt_fn);
    vm.set_property(math_obj, "pow", pow_fn);
    vm.set_property(math_obj, "min", min_fn);
    vm.set_property(math_obj, "max", max_fn);
    vm.set_property(math_obj, "random", random_fn);
    vm.set_property(math_obj, "PI", JsValue::from_double(std::f64::consts::PI));
    vm.set_property(math_obj, "E", JsValue::from_double(std::f64::consts::E));

    vm.globals.insert("Math".to_string(), math_obj);
}

fn register_json(vm: &mut Vm) {
    let json_obj = vm.alloc_object();

    let stringify_fn = vm.alloc_native_function("stringify", 1, |vm, _this, args| {
        if let Some(arg) = args.first() {
            let s = vm.to_json_string(*arg);
            let s_val = vm.alloc_string(s);
            Ok(s_val)
        } else {
            Ok(JsValue::undefined())
        }
    });

    let parse_fn = vm.alloc_native_function("parse", 1, |vm, _this, args| {
        if let Some(arg) = args.first() {
            let text = vm.to_string_value(*arg);
            let trimmed = text.trim();
            if trimmed == "true" {
                return Ok(JsValue::from_boolean(true));
            } else if trimmed == "false" {
                return Ok(JsValue::from_boolean(false));
            } else if trimmed == "null" {
                return Ok(JsValue::null());
            } else if let Ok(num) = trimmed.parse::<f64>() {
                return Ok(JsValue::from_double(num));
            } else if trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2 {
                let inner = &trimmed[1..trimmed.len() - 1];
                let s_val = vm.alloc_string(inner.to_string());
                return Ok(s_val);
            }
            // Fallback object parse
            let obj = vm.alloc_object();
            Ok(obj)
        } else {
            Ok(JsValue::undefined())
        }
    });

    vm.set_property(json_obj, "stringify", stringify_fn);
    vm.set_property(json_obj, "parse", parse_fn);

    vm.globals.insert("JSON".to_string(), json_obj);
}

fn register_object_builtins(vm: &mut Vm) {
    let object_constructor = vm.alloc_native_function("Object", 1, |vm, _this, args| {
        if let Some(arg) = args.first() {
            if arg.is_object() {
                return Ok(*arg);
            }
        }
        Ok(vm.alloc_object())
    });

    let keys_fn = vm.alloc_native_function("keys", 1, |vm, _this, args| {
        let mut keys_to_alloc = Vec::new();
        if let Some(arg) = args.first() {
            if let Some(ptr) = arg.as_object_ptr() {
                let obj = unsafe { &*(ptr as *mut JsObject) };
                if let Some(shape) = vm.shapes.get(obj.shape_id) {
                    keys_to_alloc = shape.property_offsets.keys().cloned().collect();
                }
            }
        }
        let keys_vec: Vec<JsValue> = keys_to_alloc.into_iter().map(|k| vm.alloc_string(k)).collect();
        Ok(vm.alloc_array(keys_vec))
    });

    let values_fn = vm.alloc_native_function("values", 1, |vm, _this, args| {
        let mut vals_vec = Vec::new();
        if let Some(arg) = args.first() {
            if let Some(ptr) = arg.as_object_ptr() {
                let obj = unsafe { &*(ptr as *mut JsObject) };
                if let Some(shape) = vm.shapes.get(obj.shape_id) {
                    for info in shape.property_offsets.values() {
                        if let Some(v) = obj.get_slot(info.offset) {
                            vals_vec.push(v);
                        }
                    }
                }
            }
        }
        Ok(vm.alloc_array(vals_vec))
    });

    let entries_fn = vm.alloc_native_function("entries", 1, |vm, _this, args| {
        let mut pairs_data = Vec::new();
        if let Some(arg) = args.first() {
            if let Some(ptr) = arg.as_object_ptr() {
                let obj = unsafe { &*(ptr as *mut JsObject) };
                if let Some(shape) = vm.shapes.get(obj.shape_id) {
                    for (k, info) in &shape.property_offsets {
                        let v = obj.get_slot(info.offset).unwrap_or(JsValue::undefined());
                        pairs_data.push((k.clone(), v));
                    }
                }
            }
        }
        let entries_vec: Vec<JsValue> = pairs_data
            .into_iter()
            .map(|(k, v)| {
                let k_val = vm.alloc_string(k);
                vm.alloc_array(vec![k_val, v])
            })
            .collect();
        Ok(vm.alloc_array(entries_vec))
    });

    let assign_fn = vm.alloc_native_function("assign", 2, |vm, _this, args| {
        let target = if let Some(t) = args.first() {
            *t
        } else {
            return Err(VmError::TypeError("Cannot convert undefined or null to object".into()));
        };

        for source in args.iter().skip(1) {
            if let Some(src_ptr) = source.as_object_ptr() {
                let src_obj = unsafe { &*(src_ptr as *mut JsObject) };
                if let Some(shape) = vm.shapes.get(src_obj.shape_id) {
                    let props: Vec<(String, JsValue)> = shape
                        .property_offsets
                        .iter()
                        .filter_map(|(k, info)| src_obj.get_slot(info.offset).map(|v| (k.clone(), v)))
                        .collect();

                    for (k, v) in props {
                        vm.set_property(target, &k, v);
                    }
                }
            }
        }
        Ok(target)
    });

    vm.set_property(object_constructor, "keys", keys_fn);
    vm.set_property(object_constructor, "values", values_fn);
    vm.set_property(object_constructor, "entries", entries_fn);
    vm.set_property(object_constructor, "assign", assign_fn);

    vm.globals.insert("Object".to_string(), object_constructor);
}

fn register_array_builtins(vm: &mut Vm) {
    let array_constructor = vm.alloc_native_function("Array", 1, |vm, _this, args| {
        Ok(vm.alloc_array(args.to_vec()))
    });

    let is_array_fn = vm.alloc_native_function("isArray", 1, |_vm, _this, args| {
        if let Some(arg) = args.first() {
            if let Some(ptr) = arg.as_object_ptr() {
                let obj = unsafe { &*(ptr as *mut JsObject) };
                if matches!(obj.kind, ObjectKind::Array(_)) {
                    return Ok(JsValue::from_boolean(true));
                }
            }
        }
        Ok(JsValue::from_boolean(false))
    });

    vm.set_property(array_constructor, "isArray", is_array_fn);
    vm.globals.insert("Array".to_string(), array_constructor);
}

fn register_global_helpers(vm: &mut Vm) {
    let parse_int = vm.alloc_native_function("parseInt", 1, |vm, _this, args| {
        if let Some(arg) = args.first() {
            let s = vm.to_string_value(*arg);
            let trimmed = s.trim();
            if let Ok(i) = trimmed.parse::<i32>() {
                return Ok(JsValue::from_int32(i));
            } else if let Ok(f) = trimmed.parse::<f64>() {
                return Ok(JsValue::from_double(f.floor()));
            }
        }
        Ok(JsValue::from_double(f64::NAN))
    });

    let parse_float = vm.alloc_native_function("parseFloat", 1, |vm, _this, args| {
        if let Some(arg) = args.first() {
            let s = vm.to_string_value(*arg);
            if let Ok(f) = s.trim().parse::<f64>() {
                return Ok(JsValue::from_double(f));
            }
        }
        Ok(JsValue::from_double(f64::NAN))
    });

    let is_nan = vm.alloc_native_function("isNaN", 1, |_vm, _this, args| {
        if let Some(arg) = args.first() {
            if let Some(n) = arg.to_number() {
                return Ok(JsValue::from_boolean(n.is_nan()));
            }
        }
        Ok(JsValue::from_boolean(true))
    });

    let is_finite = vm.alloc_native_function("isFinite", 1, |_vm, _this, args| {
        if let Some(arg) = args.first() {
            if let Some(n) = arg.to_number() {
                return Ok(JsValue::from_boolean(n.is_finite()));
            }
        }
        Ok(JsValue::from_boolean(false))
    });

    vm.globals.insert("parseInt".to_string(), parse_int);
    vm.globals.insert("parseFloat".to_string(), parse_float);
    vm.globals.insert("isNaN".to_string(), is_nan);
    vm.globals.insert("isFinite".to_string(), is_finite);
    vm.globals.insert("NaN".to_string(), JsValue::from_double(f64::NAN));
    vm.globals.insert("Infinity".to_string(), JsValue::from_double(f64::INFINITY));
    vm.globals.insert("undefined".to_string(), JsValue::undefined());
}

fn register_promise(vm: &mut Vm) {
    let promise_ctor = vm.alloc_native_function("Promise", 1, |vm, _this, args| {
        let executor = args.first().copied().unwrap_or(JsValue::undefined());
        if !executor.is_object() {
            return Err(VmError::TypeError("Promise resolver is not a function".into()));
        }
        let promise_val = vm.alloc_promise(crate::object::PromiseState::Pending, JsValue::undefined());
        let p_for_res = promise_val;
        let resolve_fn = vm.alloc_closure_native_function("resolve", 1, Arc::new(move |vm: &mut Vm, _this: JsValue, args: &[JsValue]| -> Result<JsValue, VmError> {
            let res = args.first().copied().unwrap_or(JsValue::undefined());
            vm.resolve_promise(p_for_res, res);
            Ok(JsValue::undefined())
        }));
        let p_for_rej = promise_val;
        let reject_fn = vm.alloc_closure_native_function("reject", 1, Arc::new(move |vm: &mut Vm, _this: JsValue, args: &[JsValue]| -> Result<JsValue, VmError> {
            let rej = args.first().copied().unwrap_or(JsValue::undefined());
            vm.reject_promise(p_for_rej, rej);
            Ok(JsValue::undefined())
        }));

        match vm.call(executor, JsValue::undefined(), &[resolve_fn, reject_fn]) {
            Ok(_) => Ok(promise_val),
            Err(e) => {
                let err_str = vm.alloc_string(e.to_string());
                vm.reject_promise(promise_val, err_str);
                Ok(promise_val)
            }
        }
    });

    let resolve_static = vm.alloc_native_function("resolve", 1, |vm, _this, args| {
        let val = args.first().copied().unwrap_or(JsValue::undefined());
        if let Some(ptr) = val.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if matches!(obj.kind, ObjectKind::Promise(_)) {
                return Ok(val);
            }
        }
        Ok(vm.alloc_promise(crate::object::PromiseState::Fulfilled, val))
    });

    let reject_static = vm.alloc_native_function("reject", 1, |vm, _this, args| {
        let reason = args.first().copied().unwrap_or(JsValue::undefined());
        Ok(vm.alloc_promise(crate::object::PromiseState::Rejected, reason))
    });

    let all_static = vm.alloc_native_function("all", 1, |vm, _this, args| {
        let iterable = args.first().copied().unwrap_or(JsValue::undefined());
        if let Some(ptr) = iterable.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::Array(elements) = &obj.kind {
                let elements_clone = elements.clone();
                let result_promise = vm.alloc_promise(crate::object::PromiseState::Pending, JsValue::undefined());
                if elements_clone.is_empty() {
                    let empty_arr = vm.alloc_array(Vec::new());
                    vm.resolve_promise(result_promise, empty_arr);
                    return Ok(result_promise);
                }
                let count = elements_clone.len();
                let remaining = Arc::new(std::sync::atomic::AtomicUsize::new(count));
                let results = Arc::new(std::sync::Mutex::new(vec![JsValue::undefined(); count]));

                for (idx, elem) in elements_clone.into_iter().enumerate() {
                    let r_p = result_promise;
                    let rem = Arc::clone(&remaining);
                    let res_store = Arc::clone(&results);

                    let resolve_item = vm.alloc_closure_native_function("resolve_item", 1, Arc::new(move |vm: &mut Vm, _this: JsValue, args: &[JsValue]| -> Result<JsValue, VmError> {
                        let v = args.first().copied().unwrap_or(JsValue::undefined());
                        {
                            let mut lock = res_store.lock().unwrap();
                            lock[idx] = v;
                        }
                        if rem.fetch_sub(1, std::sync::atomic::Ordering::SeqCst) == 1 {
                            let final_res = {
                                let lock = res_store.lock().unwrap();
                                lock.clone()
                            };
                            let arr = vm.alloc_array(final_res);
                            vm.resolve_promise(r_p, arr);
                        }
                        Ok(JsValue::undefined())
                    }));

                    let reject_item = vm.alloc_closure_native_function("reject_item", 1, Arc::new(move |vm: &mut Vm, _this: JsValue, args: &[JsValue]| -> Result<JsValue, VmError> {
                        let err = args.first().copied().unwrap_or(JsValue::undefined());
                        vm.reject_promise(r_p, err);
                        Ok(JsValue::undefined())
                    }));

                    if let Some(p_ptr) = elem.as_object_ptr() {
                        let p_obj = unsafe { &*(p_ptr as *mut JsObject) };
                        if matches!(p_obj.kind, ObjectKind::Promise(_)) {
                            let then_fn = vm.get_property(elem, "then");
                            let _ = vm.call(then_fn, elem, &[resolve_item, reject_item]);
                            continue;
                        }
                    }
                    let _ = vm.call(resolve_item, JsValue::undefined(), &[elem]);
                }

                return Ok(result_promise);
            }
        }
        let empty_arr = vm.alloc_array(Vec::new());
        Ok(vm.alloc_promise(crate::object::PromiseState::Fulfilled, empty_arr))
    });

    let race_static = vm.alloc_native_function("race", 1, |vm, _this, args| {
        let iterable = args.first().copied().unwrap_or(JsValue::undefined());
        let result_promise = vm.alloc_promise(crate::object::PromiseState::Pending, JsValue::undefined());
        if let Some(ptr) = iterable.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::Array(elements) = &obj.kind {
                let elements_clone = elements.clone();
                for elem in elements_clone {
                    let r_p = result_promise;
                    let resolve_item = vm.alloc_closure_native_function("race_res", 1, Arc::new(move |vm: &mut Vm, _this: JsValue, args: &[JsValue]| -> Result<JsValue, VmError> {
                        let v = args.first().copied().unwrap_or(JsValue::undefined());
                        vm.resolve_promise(r_p, v);
                        Ok(JsValue::undefined())
                    }));
                    let reject_item = vm.alloc_closure_native_function("race_rej", 1, Arc::new(move |vm: &mut Vm, _this: JsValue, args: &[JsValue]| -> Result<JsValue, VmError> {
                        let err = args.first().copied().unwrap_or(JsValue::undefined());
                        vm.reject_promise(r_p, err);
                        Ok(JsValue::undefined())
                    }));

                    if let Some(p_ptr) = elem.as_object_ptr() {
                        let p_obj = unsafe { &*(p_ptr as *mut JsObject) };
                        if matches!(p_obj.kind, ObjectKind::Promise(_)) {
                            let then_fn = vm.get_property(elem, "then");
                            let _ = vm.call(then_fn, elem, &[resolve_item, reject_item]);
                            continue;
                        }
                    }
                    let _ = vm.call(resolve_item, JsValue::undefined(), &[elem]);
                }
            }
        }
        Ok(result_promise)
    });

    vm.set_property(promise_ctor, "resolve", resolve_static);
    vm.set_property(promise_ctor, "reject", reject_static);
    vm.set_property(promise_ctor, "all", all_static);
    vm.set_property(promise_ctor, "race", race_static);

    vm.globals.insert("Promise".to_string(), promise_ctor);
}

fn register_parallel_and_typed_arrays(vm: &mut Vm) {
    let sab_ctor = vm.alloc_native_function("SharedArrayBuffer", 1, |vm, _this, args| {
        let byte_len = args.first().and_then(|v| v.to_int32()).unwrap_or(0).max(0) as usize;
        let sab = Arc::new(doms_parallel::SharedArrayBuffer::new(byte_len));
        Ok(vm.alloc_shared_array_buffer(sab))
    });
    vm.globals.insert("SharedArrayBuffer".to_string(), sab_ctor);

    let int32_ctor = vm.alloc_native_function("Int32Array", 1, |vm, _this, args| {
        let arg = args.first().copied().unwrap_or(JsValue::undefined());
        if let Some(ptr) = arg.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::SharedArrayBuffer(sab) = &obj.kind {
                let len = sab.byte_length() / 4;
                return Ok(vm.alloc_typed_array(crate::object::JsTypedArray {
                    buffer: Arc::clone(sab),
                    kind: crate::object::TypedArrayKind::Int32,
                    length: len,
                }));
            }
        }
        let len = arg.to_int32().unwrap_or(0).max(0) as usize;
        let sab = Arc::new(doms_parallel::SharedArrayBuffer::new(len * 4));
        Ok(vm.alloc_typed_array(crate::object::JsTypedArray {
            buffer: sab,
            kind: crate::object::TypedArrayKind::Int32,
            length: len,
        }))
    });
    vm.globals.insert("Int32Array".to_string(), int32_ctor);

    let uint8_ctor = vm.alloc_native_function("Uint8Array", 1, |vm, _this, args| {
        let arg = args.first().copied().unwrap_or(JsValue::undefined());
        if let Some(ptr) = arg.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::SharedArrayBuffer(sab) = &obj.kind {
                let len = sab.byte_length();
                return Ok(vm.alloc_typed_array(crate::object::JsTypedArray {
                    buffer: Arc::clone(sab),
                    kind: crate::object::TypedArrayKind::Uint8,
                    length: len,
                }));
            }
        }
        let len = arg.to_int32().unwrap_or(0).max(0) as usize;
        let sab = Arc::new(doms_parallel::SharedArrayBuffer::new(len));
        Ok(vm.alloc_typed_array(crate::object::JsTypedArray {
            buffer: sab,
            kind: crate::object::TypedArrayKind::Uint8,
            length: len,
        }))
    });
    vm.globals.insert("Uint8Array".to_string(), uint8_ctor);

    let atomics_obj = vm.alloc_object();

    let atomics_load = vm.alloc_native_function("load", 2, |_vm, _this, args| {
        let ta_val = args.first().copied().unwrap_or(JsValue::undefined());
        let idx = args.get(1).and_then(|v| v.to_int32()).unwrap_or(0).max(0) as usize;
        if let Some(ptr) = ta_val.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::TypedArray(ta) = &obj.kind {
                match ta.kind {
                    crate::object::TypedArrayKind::Int32 => {
                        let off = idx * 4;
                        let b0 = ta.buffer.get(off).unwrap_or(0);
                        let b1 = ta.buffer.get(off + 1).unwrap_or(0);
                        let b2 = ta.buffer.get(off + 2).unwrap_or(0);
                        let b3 = ta.buffer.get(off + 3).unwrap_or(0);
                        let val = i32::from_le_bytes([b0, b1, b2, b3]);
                        return Ok(JsValue::from_int32(val));
                    }
                    crate::object::TypedArrayKind::Uint8 => {
                        let val = doms_parallel::Atomics::load(&ta.buffer, idx)
                            .map_err(|e| VmError::RuntimeError(e.to_string()))?;
                        return Ok(JsValue::from_int32(val as i32));
                    }
                }
            }
        }
        Err(VmError::TypeError("Atomics.load requires a TypedArray".into()))
    });

    let atomics_store = vm.alloc_native_function("store", 3, |_vm, _this, args| {
        let ta_val = args.first().copied().unwrap_or(JsValue::undefined());
        let idx = args.get(1).and_then(|v| v.to_int32()).unwrap_or(0).max(0) as usize;
        let value = args.get(2).and_then(|v| v.to_number()).unwrap_or(0.0) as i32;
        if let Some(ptr) = ta_val.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::TypedArray(ta) = &obj.kind {
                match ta.kind {
                    crate::object::TypedArrayKind::Int32 => {
                        let off = idx * 4;
                        let bytes = value.to_le_bytes();
                        for (k, &b) in bytes.iter().enumerate() {
                            ta.buffer.set(off + k, b);
                        }
                        return Ok(JsValue::from_int32(value));
                    }
                    crate::object::TypedArrayKind::Uint8 => {
                        doms_parallel::Atomics::store(&ta.buffer, idx, value as u8)
                            .map_err(|e| VmError::RuntimeError(e.to_string()))?;
                        return Ok(JsValue::from_int32(value));
                    }
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
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::TypedArray(ta) = &obj.kind {
                match ta.kind {
                    crate::object::TypedArrayKind::Int32 => {
                        let off = idx * 4;
                        let b0 = ta.buffer.get(off).unwrap_or(0);
                        let b1 = ta.buffer.get(off + 1).unwrap_or(0);
                        let b2 = ta.buffer.get(off + 2).unwrap_or(0);
                        let b3 = ta.buffer.get(off + 3).unwrap_or(0);
                        let old = i32::from_le_bytes([b0, b1, b2, b3]);
                        let new_val = old + value;
                        let bytes = new_val.to_le_bytes();
                        for (k, &b) in bytes.iter().enumerate() {
                            ta.buffer.set(off + k, b);
                        }
                        return Ok(JsValue::from_int32(old));
                    }
                    crate::object::TypedArrayKind::Uint8 => {
                        let old = doms_parallel::Atomics::add(&ta.buffer, idx, value as u8)
                            .map_err(|e| VmError::RuntimeError(e.to_string()))?;
                        return Ok(JsValue::from_int32(old as i32));
                    }
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
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::TypedArray(ta) = &obj.kind {
                match ta.kind {
                    crate::object::TypedArrayKind::Int32 => {
                        let off = idx * 4;
                        let b0 = ta.buffer.get(off).unwrap_or(0);
                        let b1 = ta.buffer.get(off + 1).unwrap_or(0);
                        let b2 = ta.buffer.get(off + 2).unwrap_or(0);
                        let b3 = ta.buffer.get(off + 3).unwrap_or(0);
                        let old = i32::from_le_bytes([b0, b1, b2, b3]);
                        let new_val = old - value;
                        let bytes = new_val.to_le_bytes();
                        for (k, &b) in bytes.iter().enumerate() {
                            ta.buffer.set(off + k, b);
                        }
                        return Ok(JsValue::from_int32(old));
                    }
                    crate::object::TypedArrayKind::Uint8 => {
                        let old = doms_parallel::Atomics::sub(&ta.buffer, idx, value as u8)
                            .map_err(|e| VmError::RuntimeError(e.to_string()))?;
                        return Ok(JsValue::from_int32(old as i32));
                    }
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
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::TypedArray(ta) = &obj.kind {
                let off = idx * 4;
                let b0 = ta.buffer.get(off).unwrap_or(0);
                let b1 = ta.buffer.get(off + 1).unwrap_or(0);
                let b2 = ta.buffer.get(off + 2).unwrap_or(0);
                let b3 = ta.buffer.get(off + 3).unwrap_or(0);
                let old = i32::from_le_bytes([b0, b1, b2, b3]);
                let new_val = old & value;
                let bytes = new_val.to_le_bytes();
                for (k, &b) in bytes.iter().enumerate() {
                    ta.buffer.set(off + k, b);
                }
                return Ok(JsValue::from_int32(old));
            }
        }
        Err(VmError::TypeError("Atomics.and requires a TypedArray".into()))
    });

    let atomics_or = vm.alloc_native_function("or", 3, |_vm, _this, args| {
        let ta_val = args.first().copied().unwrap_or(JsValue::undefined());
        let idx = args.get(1).and_then(|v| v.to_int32()).unwrap_or(0).max(0) as usize;
        let value = args.get(2).and_then(|v| v.to_number()).unwrap_or(0.0) as i32;
        if let Some(ptr) = ta_val.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::TypedArray(ta) = &obj.kind {
                let off = idx * 4;
                let b0 = ta.buffer.get(off).unwrap_or(0);
                let b1 = ta.buffer.get(off + 1).unwrap_or(0);
                let b2 = ta.buffer.get(off + 2).unwrap_or(0);
                let b3 = ta.buffer.get(off + 3).unwrap_or(0);
                let old = i32::from_le_bytes([b0, b1, b2, b3]);
                let new_val = old | value;
                let bytes = new_val.to_le_bytes();
                for (k, &b) in bytes.iter().enumerate() {
                    ta.buffer.set(off + k, b);
                }
                return Ok(JsValue::from_int32(old));
            }
        }
        Err(VmError::TypeError("Atomics.or requires a TypedArray".into()))
    });

    let atomics_xor = vm.alloc_native_function("xor", 3, |_vm, _this, args| {
        let ta_val = args.first().copied().unwrap_or(JsValue::undefined());
        let idx = args.get(1).and_then(|v| v.to_int32()).unwrap_or(0).max(0) as usize;
        let value = args.get(2).and_then(|v| v.to_number()).unwrap_or(0.0) as i32;
        if let Some(ptr) = ta_val.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::TypedArray(ta) = &obj.kind {
                let off = idx * 4;
                let b0 = ta.buffer.get(off).unwrap_or(0);
                let b1 = ta.buffer.get(off + 1).unwrap_or(0);
                let b2 = ta.buffer.get(off + 2).unwrap_or(0);
                let b3 = ta.buffer.get(off + 3).unwrap_or(0);
                let old = i32::from_le_bytes([b0, b1, b2, b3]);
                let new_val = old ^ value;
                let bytes = new_val.to_le_bytes();
                for (k, &b) in bytes.iter().enumerate() {
                    ta.buffer.set(off + k, b);
                }
                return Ok(JsValue::from_int32(old));
            }
        }
        Err(VmError::TypeError("Atomics.xor requires a TypedArray".into()))
    });

    let atomics_cmpxchg = vm.alloc_native_function("compareExchange", 4, |_vm, _this, args| {
        let ta_val = args.first().copied().unwrap_or(JsValue::undefined());
        let idx = args.get(1).and_then(|v| v.to_int32()).unwrap_or(0).max(0) as usize;
        let expected = args.get(2).and_then(|v| v.to_number()).unwrap_or(0.0) as i32;
        let replacement = args.get(3).and_then(|v| v.to_number()).unwrap_or(0.0) as i32;
        if let Some(ptr) = ta_val.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::TypedArray(ta) = &obj.kind {
                match ta.kind {
                    crate::object::TypedArrayKind::Int32 => {
                        let off = idx * 4;
                        let b0 = ta.buffer.get(off).unwrap_or(0);
                        let b1 = ta.buffer.get(off + 1).unwrap_or(0);
                        let b2 = ta.buffer.get(off + 2).unwrap_or(0);
                        let b3 = ta.buffer.get(off + 3).unwrap_or(0);
                        let old = i32::from_le_bytes([b0, b1, b2, b3]);
                        if old == expected {
                            let bytes = replacement.to_le_bytes();
                            for (k, &b) in bytes.iter().enumerate() {
                                ta.buffer.set(off + k, b);
                            }
                        }
                        return Ok(JsValue::from_int32(old));
                    }
                    crate::object::TypedArrayKind::Uint8 => {
                        let res = doms_parallel::Atomics::compare_exchange(&ta.buffer, idx, expected as u8, replacement as u8)
                            .map_err(|e| VmError::RuntimeError(e.to_string()))?;
                        let old = match res {
                            Ok(v) => v,
                            Err(v) => v,
                        };
                        return Ok(JsValue::from_int32(old as i32));
                    }
                }
            }
        }
        Err(VmError::TypeError("Atomics.compareExchange requires a TypedArray".into()))
    });

    let atomics_lockfree = vm.alloc_native_function("isLockFree", 1, |_vm, _this, args| {
        let size = args.first().and_then(|v| v.to_int32()).unwrap_or(0);
        Ok(JsValue::from_boolean(size == 1 || size == 2 || size == 4))
    });

    vm.set_property(atomics_obj, "load", atomics_load);
    vm.set_property(atomics_obj, "store", atomics_store);
    vm.set_property(atomics_obj, "add", atomics_add);
    vm.set_property(atomics_obj, "sub", atomics_sub);
    vm.set_property(atomics_obj, "and", atomics_and);
    vm.set_property(atomics_obj, "or", atomics_or);
    vm.set_property(atomics_obj, "xor", atomics_xor);
    vm.set_property(atomics_obj, "compareExchange", atomics_cmpxchg);
    vm.set_property(atomics_obj, "isLockFree", atomics_lockfree);
    vm.globals.insert("Atomics".to_string(), atomics_obj);

    let worker_ctor = vm.alloc_native_function("Worker", 1, |vm, _this, args| {
        let script = args.first().map(|v| vm.to_string_value(*v)).unwrap_or_default();
        let channel = doms_parallel::MessageChannel::new();
        let child_port = channel.port2;

        std::thread::spawn(move || {
            let mut worker_vm = Vm::new();
            let parent_port = Arc::new(std::sync::Mutex::new(child_port));
            let post_msg = worker_vm.alloc_closure_native_function("postMessage", 1, Arc::new({
                let p = Arc::clone(&parent_port);
                move |vm: &mut Vm, _this: JsValue, args: &[JsValue]| -> Result<JsValue, VmError> {
                    let msg = args.first().copied().unwrap_or_default();
                    let json = vm.to_json_string(msg);
                    let guard = p.lock().unwrap();
                    guard.post_message(json.into_bytes()).map_err(|e| VmError::RuntimeError(e.to_string()))?;
                    Ok(JsValue::undefined())
                }
            }));
            worker_vm.globals.insert("postMessage".to_string(), post_msg);

            let code = if std::path::Path::new(&script).exists() {
                std::fs::read_to_string(&script).unwrap_or(script)
            } else {
                script
            };

            if let Ok(prog) = doms_parser::parse(&code) {
                if let Ok(chunk) = doms_compiler::compile(&prog) {
                    let _ = worker_vm.run(&chunk);
                }
            }
        });

        let port_arc = Arc::new(std::sync::Mutex::new(channel.port1));
        Ok(vm.alloc_worker(port_arc))
    });

    vm.globals.insert("Worker".to_string(), worker_ctor);
}

fn register_phase3_builtins(vm: &mut Vm) {
    // ── Map & Set ──
    let map_ctor = vm.alloc_native_function("Map", 0, |vm, _this, args| {
        let map_val = vm.alloc_map();
        if let Some(arg) = args.first() {
            if let Some(ptr) = arg.as_object_ptr() {
                let obj = unsafe { &*(ptr as *mut JsObject) };
                if let ObjectKind::Array(elements) = &obj.kind {
                    for el in elements {
                        if let Some(el_ptr) = el.as_object_ptr() {
                            let el_obj = unsafe { &*(el_ptr as *mut JsObject) };
                            if let ObjectKind::Array(pair) = &el_obj.kind {
                                let k = pair.first().copied().unwrap_or(JsValue::undefined());
                                let v = pair.get(1).copied().unwrap_or(JsValue::undefined());
                                if let Some(m_ptr) = map_val.as_object_ptr() {
                                    let m_obj = unsafe { &mut *(m_ptr as *mut JsObject) };
                                    if let ObjectKind::Map(entries) = &mut m_obj.kind {
                                        entries.push((k, v));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(map_val)
    });
    vm.globals.insert("Map".to_string(), map_ctor);

    let set_ctor = vm.alloc_native_function("Set", 0, |vm, _this, args| {
        let set_val = vm.alloc_set();
        if let Some(arg) = args.first() {
            if let Some(ptr) = arg.as_object_ptr() {
                let obj = unsafe { &*(ptr as *mut JsObject) };
                if let ObjectKind::Array(elements) = &obj.kind {
                    if let Some(s_ptr) = set_val.as_object_ptr() {
                        let s_obj = unsafe { &mut *(s_ptr as *mut JsObject) };
                        if let ObjectKind::Set(set_elements) = &mut s_obj.kind {
                            for el in elements {
                                if !set_elements.contains(el) {
                                    set_elements.push(*el);
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(set_val)
    });
    vm.globals.insert("Set".to_string(), set_ctor);

    // ── WeakRef & FinalizationRegistry ──
    let weakref_ctor = vm.alloc_native_function("WeakRef", 1, |vm, _this, args| {
        let target = args.first().copied().unwrap_or(JsValue::undefined());
        Ok(vm.alloc_weak_ref(target))
    });
    vm.globals.insert("WeakRef".to_string(), weakref_ctor);

    let fr_ctor = vm.alloc_native_function("FinalizationRegistry", 1, |vm, _this, args| {
        let cb = args.first().copied().unwrap_or(JsValue::undefined());
        Ok(vm.alloc_finalization_registry(cb))
    });
    vm.globals.insert("FinalizationRegistry".to_string(), fr_ctor);

    // ── RegExp ──
    let regexp_ctor = vm.alloc_native_function("RegExp", 2, |vm, _this, args| {
        let pattern = args.first().map(|v| vm.to_string_value(*v)).unwrap_or_default();
        let flags = args.get(1).map(|v| vm.to_string_value(*v)).unwrap_or_default();
        Ok(vm.alloc_regexp(pattern, flags))
    });
    vm.globals.insert("RegExp".to_string(), regexp_ctor);

    // ── Intl ──
    let intl_obj = vm.alloc_object();

    let num_format_ctor = vm.alloc_native_function("NumberFormat", 0, |vm, _this, _args| {
        let nf_obj = vm.alloc_object();
        let format_fn = vm.alloc_native_function("format", 1, |vm, _this, args| {
            let n = args.first().and_then(|v| v.to_number()).unwrap_or(0.0);
            let s = if n.fract() == 0.0 && n.abs() < 1e16 {
                let int_val = n as i64;
                let sign = if int_val < 0 { "-" } else { "" };
                let abs_str = int_val.abs().to_string();
                let mut formatted = String::new();
                for (i, ch) in abs_str.chars().rev().enumerate() {
                    if i > 0 && i % 3 == 0 {
                        formatted.push(',');
                    }
                    formatted.push(ch);
                }
                let rev_res: String = formatted.chars().rev().collect();
                format!("{}{}", sign, rev_res)
            } else {
                format!("{:.3}", n)
            };
            Ok(vm.alloc_string(s))
        });
        vm.set_property(nf_obj, "format", format_fn);
        Ok(nf_obj)
    });
    vm.set_property(intl_obj, "NumberFormat", num_format_ctor);

    let date_format_ctor = vm.alloc_native_function("DateTimeFormat", 0, |vm, _this, _args| {
        let df_obj = vm.alloc_object();
        let format_fn = vm.alloc_native_function("format", 1, |vm, _this, args| {
            let s = args.first().map(|v| vm.to_string_value(*v)).unwrap_or_else(|| "2026-10-06T00:00:00Z".to_string());
            Ok(vm.alloc_string(s))
        });
        vm.set_property(df_obj, "format", format_fn);
        Ok(df_obj)
    });
    vm.set_property(intl_obj, "DateTimeFormat", date_format_ctor);

    let collator_ctor = vm.alloc_native_function("Collator", 0, |vm, _this, _args| {
        let coll_obj = vm.alloc_object();
        let compare_fn = vm.alloc_native_function("compare", 2, |vm, _this, args| {
            let a = args.first().map(|v| vm.to_string_value(*v)).unwrap_or_default();
            let b = args.get(1).map(|v| vm.to_string_value(*v)).unwrap_or_default();
            let cmp = match a.cmp(&b) {
                std::cmp::Ordering::Less => -1,
                std::cmp::Ordering::Equal => 0,
                std::cmp::Ordering::Greater => 1,
            };
            Ok(JsValue::from_int32(cmp))
        });
        vm.set_property(coll_obj, "compare", compare_fn);
        Ok(coll_obj)
    });
    vm.set_property(intl_obj, "Collator", collator_ctor);

    vm.globals.insert("Intl".to_string(), intl_obj);

    // ── Temporal ──
    let temporal_obj = vm.alloc_object();

    let instant_ctor = vm.alloc_native_function("Instant", 1, |vm, _this, args| {
        let ms = args.first().and_then(|v| v.to_number()).unwrap_or(0.0);
        let inst = vm.alloc_object();
        vm.set_property(inst, "epochMilliseconds", JsValue::from_double(ms));
        vm.set_property(inst, "epochNanoseconds", JsValue::from_double(ms * 1_000_000.0));
        let to_str = vm.alloc_native_function("toString", 0, |vm, this, _args| {
            let ms_val = vm.get_property(this, "epochMilliseconds")
                .to_number()
                .unwrap_or(0.0);
            Ok(vm.alloc_string(format!("{}Z", ms_val)))
        });
        vm.set_property(inst, "toString", to_str);
        Ok(inst)
    });
    vm.set_property(temporal_obj, "Instant", instant_ctor);

    let now_obj = vm.alloc_object();
    let instant_fn = vm.alloc_native_function("instant", 0, |vm, _this, _args| {
        let ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as f64)
            .unwrap_or(0.0);
        let inst = vm.alloc_object();
        vm.set_property(inst, "epochMilliseconds", JsValue::from_double(ms));
        vm.set_property(inst, "epochNanoseconds", JsValue::from_double(ms * 1_000_000.0));
        let to_str = vm.alloc_native_function("toString", 0, |vm, this, _args| {
            let ms_val = vm.get_property(this, "epochMilliseconds")
                .to_number()
                .unwrap_or(0.0);
            Ok(vm.alloc_string(format!("{}Z", ms_val)))
        });
        vm.set_property(inst, "toString", to_str);
        Ok(inst)
    });
    vm.set_property(now_obj, "instant", instant_fn);
    vm.set_property(temporal_obj, "Now", now_obj);

    let plain_date_ctor = vm.alloc_native_function("PlainDate", 3, |vm, _this, args| {
        let y = args.first().and_then(|v| v.to_int32()).unwrap_or(1970);
        let m = args.get(1).and_then(|v| v.to_int32()).unwrap_or(1);
        let d = args.get(2).and_then(|v| v.to_int32()).unwrap_or(1);
        let date_obj = vm.alloc_object();
        vm.set_property(date_obj, "year", JsValue::from_int32(y));
        vm.set_property(date_obj, "month", JsValue::from_int32(m));
        vm.set_property(date_obj, "day", JsValue::from_int32(d));
        let to_str = vm.alloc_native_function("toString", 0, |vm, this, _args| {
            let y_val = vm.get_property(this, "year").to_int32().unwrap_or(0);
            let m_val = vm.get_property(this, "month").to_int32().unwrap_or(0);
            let d_val = vm.get_property(this, "day").to_int32().unwrap_or(0);
            Ok(vm.alloc_string(format!("{:04}-{:02}-{:02}", y_val, m_val, d_val)))
        });
        vm.set_property(date_obj, "toString", to_str);
        Ok(date_obj)
    });
    vm.set_property(temporal_obj, "PlainDate", plain_date_ctor);

    vm.globals.insert("Temporal".to_string(), temporal_obj);
}
