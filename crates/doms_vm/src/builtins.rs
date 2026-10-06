//! Built-in functions, objects, and prototype methods.

use crate::interpreter::{Vm, VmError};
use crate::object::{JsObject, ObjectKind};
use crate::value::JsValue;

pub fn register_builtins(vm: &mut Vm) {
    register_console(vm);
    register_math(vm);
    register_json(vm);
    register_object_builtins(vm);
    register_array_builtins(vm);
    register_global_helpers(vm);
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
