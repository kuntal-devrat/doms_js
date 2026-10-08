//! Built-in functions, objects, and prototype methods.

use crate::interpreter::{PropertyKeyVal, Vm, VmError};
use crate::object::{Accessor, JsObject, JsTypedArray, NativeFn, ObjectKind, TypedArrayKind};
use crate::value::JsValue;
use std::sync::Arc;

pub fn register_builtins(vm: &mut Vm) {
    register_symbol(vm);
    register_object_builtins(vm);
    register_function_builtins(vm);
    register_array_builtins(vm);
    register_console(vm);
    register_math(vm);
    register_json(vm);
    register_global_helpers(vm);
    register_promise(vm);
    register_parallel_and_typed_arrays(vm);
    register_phase3_builtins(vm);
    register_reflect(vm);
    register_proxy(vm);
    register_test262_harness(vm);
    crate::web_apis::register_web_apis(vm);

    // Ensure all built-in properties on global_object are reflected and non-enumerable
    for (name, val) in &vm.globals.clone() {
        if vm.global_object.is_object() {
            vm.set_property(vm.global_object, name, *val);
            vm.non_enumerable_props.insert((vm.global_object, name.clone()));
            if name == "undefined" || name == "NaN" || name == "Infinity" {
                vm.non_configurable_props.insert((vm.global_object, name.clone()));
                vm.non_writable_props.insert((vm.global_object, name.clone()));
            }
        }
    }
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

    let math_constants = [
        ("E", std::f64::consts::E),
        ("LN10", std::f64::consts::LN_10),
        ("LN2", std::f64::consts::LN_2),
        ("LOG2E", std::f64::consts::LOG2_E),
        ("LOG10E", std::f64::consts::LOG10_E),
        ("PI", std::f64::consts::PI),
        ("SQRT1_2", std::f64::consts::FRAC_1_SQRT_2),
        ("SQRT2", std::f64::consts::SQRT_2),
    ];
    for (name, val) in math_constants {
        vm.set_property(math_obj, name, JsValue::from_double(val));
        vm.non_writable_props.insert((math_obj, name.to_string()));
        vm.non_enumerable_props.insert((math_obj, name.to_string()));
        vm.non_configurable_props.insert((math_obj, name.to_string()));
    }

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

fn get_enumerable_keys(vm: &Vm, arg: JsValue) -> Vec<String> {
    let mut raw_names: Vec<String> = Vec::new();
    if let Some(ptr) = arg.as_object_ptr() {
        let obj = unsafe { &*(ptr as *mut JsObject) };
        if let Some(accs) = &obj.accessors {
            for k in accs.keys() {
                if !vm.non_enumerable_props.contains(&(arg, k.clone())) && !raw_names.contains(k) {
                    raw_names.push(k.clone());
                }
            }
        }
        for prop in vm.shapes.all_properties(obj.shape_id) {
            if !vm.non_enumerable_props.contains(&(arg, prop.clone())) && !raw_names.contains(&prop) {
                raw_names.push(prop);
            }
        }
    }
    let mut int_keys: Vec<(u32, String)> = Vec::new();
    let mut str_keys: Vec<String> = Vec::new();
    for k in raw_names {
        if let Ok(idx) = k.parse::<u32>() {
            if idx < u32::MAX && idx.to_string() == k {
                int_keys.push((idx, k));
                continue;
            }
        }
        str_keys.push(k);
    }
    int_keys.sort_by_key(|(idx, _)| *idx);
    let mut ordered = Vec::new();
    for (_, k) in int_keys {
        ordered.push(k);
    }
    for k in str_keys {
        ordered.push(k);
    }
    ordered
}

fn register_symbol(vm: &mut Vm) {
    let symbol_proto = vm.alloc_object();
    vm.symbol_proto = Some(symbol_proto);

    let sym_value_of = vm.alloc_native_function("valueOf", 0, |_vm, this, _args| {
        if this.is_symbol() {
            return Ok(this);
        }
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if matches!(obj.kind, ObjectKind::Symbol(_)) {
                return Ok(unsafe { JsValue::from_symbol_ptr(ptr) });
            }
        }
        Err(VmError::TypeError("Symbol.prototype.valueOf requires that 'this' be a Symbol".into()))
    });
    let sym_to_string = vm.alloc_native_function("toString", 0, |vm, this, _args| {
        if this.is_symbol() {
            return Ok(vm.alloc_string(vm.to_string_value(this)));
        }
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::Symbol(desc) = &obj.kind {
                return Ok(vm.alloc_string(format!("Symbol({})", desc.as_deref().unwrap_or(""))));
            }
        }
        Err(VmError::TypeError("Symbol.prototype.toString requires that 'this' be a Symbol".into()))
    });
    vm.set_property(symbol_proto, "valueOf", sym_value_of);
    vm.set_property(symbol_proto, "toString", sym_to_string);

    let symbol_ctor = vm.alloc_native_function("Symbol", 1, |vm, _this, args| {
        if vm.is_constructing {
            return Err(VmError::TypeError("Symbol is not a constructor".into()));
        }
        let desc = args.first().map(|v| vm.to_string_value(*v));
        Ok(vm.alloc_symbol(desc))
    });
    vm.set_property(symbol_proto, "constructor", symbol_ctor);
    vm.non_enumerable_props.insert((symbol_proto, "constructor".to_string()));
    vm.set_property(symbol_ctor, "prototype", symbol_proto);
    vm.non_enumerable_props.insert((symbol_ctor, "prototype".to_string()));

    let symbol_for = vm.alloc_native_function("for", 1, |vm, _this, args| {
        let key = args.first().map(|v| vm.to_string_value(*v)).unwrap_or_default();
        if let Some(existing) = vm.symbol_registry.get(&key) {
            return Ok(*existing);
        }
        let sym = vm.alloc_symbol(Some(key.clone()));
        vm.symbol_registry.insert(key, sym);
        Ok(sym)
    });
    vm.set_property(symbol_ctor, "for", symbol_for);

    let symbol_key_for = vm.alloc_native_function("keyFor", 1, |vm, _this, args| {
        let sym = args.first().copied().unwrap_or(JsValue::undefined());
        if !sym.is_symbol() {
            return Err(VmError::TypeError("Symbol.keyFor requires that argument be a Symbol".into()));
        }
        for (k, v) in &vm.symbol_registry {
            if *v == sym {
                return Ok(vm.alloc_string(k.clone()));
            }
        }
        Ok(JsValue::undefined())
    });
    vm.set_property(symbol_ctor, "keyFor", symbol_key_for);

    // Standard well-known symbols
    let sym_to_prim = vm.alloc_symbol(Some("Symbol.toPrimitive".into()));
    let sym_iter = vm.alloc_symbol(Some("Symbol.iterator".into()));
    let sym_async_iter = vm.alloc_symbol(Some("Symbol.asyncIterator".into()));
    let sym_has_instance = vm.alloc_symbol(Some("Symbol.hasInstance".into()));
    let sym_concat_spread = vm.alloc_symbol(Some("Symbol.isConcatSpreadable".into()));
    let sym_match = vm.alloc_symbol(Some("Symbol.match".into()));
    let sym_match_all = vm.alloc_symbol(Some("Symbol.matchAll".into()));
    let sym_replace = vm.alloc_symbol(Some("Symbol.replace".into()));
    let sym_search = vm.alloc_symbol(Some("Symbol.search".into()));
    let sym_species = vm.alloc_symbol(Some("Symbol.species".into()));
    let sym_split = vm.alloc_symbol(Some("Symbol.split".into()));
    let sym_tag = vm.alloc_symbol(Some("Symbol.toStringTag".into()));
    let sym_unscopables = vm.alloc_symbol(Some("Symbol.unscopables".into()));

    vm.set_property(symbol_ctor, "toPrimitive", sym_to_prim);
    vm.set_property(symbol_ctor, "iterator", sym_iter);
    vm.set_property(symbol_ctor, "asyncIterator", sym_async_iter);
    vm.set_property(symbol_ctor, "hasInstance", sym_has_instance);
    vm.set_property(symbol_ctor, "isConcatSpreadable", sym_concat_spread);
    vm.set_property(symbol_ctor, "match", sym_match);
    vm.set_property(symbol_ctor, "matchAll", sym_match_all);
    vm.set_property(symbol_ctor, "replace", sym_replace);
    vm.set_property(symbol_ctor, "search", sym_search);
    vm.set_property(symbol_ctor, "species", sym_species);
    vm.set_property(symbol_ctor, "split", sym_split);
    vm.set_property(symbol_ctor, "toStringTag", sym_tag);
    vm.set_property(symbol_ctor, "unscopables", sym_unscopables);

    vm.symbol_iterator = Some(sym_iter);
    vm.symbol_to_string_tag = Some(sym_tag);
    vm.symbol_is_concat_spreadable = Some(sym_concat_spread);

    vm.globals.insert("Symbol".to_string(), symbol_ctor);
    if vm.global_object.is_object() {
        vm.set_property(vm.global_object, "Symbol", symbol_ctor);
    }
}

fn register_object_builtins(vm: &mut Vm) {
    let object_constructor = vm.alloc_native_function("Object", 1, |vm, _this, args| {
        if let Some(arg) = args.first() {
            if arg.is_object() {
                return Ok(*arg);
            }
            if arg.is_string() {
                let s = vm.to_string_value(*arg);
                return Ok(vm.alloc_string_object(s, vm.string_proto));
            }
            if arg.is_number() {
                let n = arg.to_number().unwrap_or(0.0);
                return Ok(vm.alloc_number_object(n, vm.number_proto));
            }
            if arg.is_boolean() {
                let b = arg.as_boolean().unwrap_or(false);
                return Ok(vm.alloc_boolean_object(b, vm.boolean_proto));
            }
        }
        Ok(vm.alloc_object())
    });

    let keys_fn = vm.alloc_native_function("keys", 1, |vm, _this, args| {
        let arg = match args.first() {
            Some(a) if a.is_object() => *a,
            _ => return Err(VmError::TypeError("Object.keys called on non-object".into())),
        };
        let keys_to_alloc = get_enumerable_keys(vm, arg);
        let keys_vec: Vec<JsValue> = keys_to_alloc.into_iter().map(|k| vm.alloc_string(k)).collect();
        Ok(vm.alloc_array(keys_vec))
    });

    let values_fn = vm.alloc_native_function("values", 1, |vm, _this, args| {
        let arg = match args.first() {
            Some(a) if a.is_object() => *a,
            _ => return Err(VmError::TypeError("Object.values called on non-object".into())),
        };
        let keys_to_alloc = get_enumerable_keys(vm, arg);
        let vals_vec: Vec<JsValue> = keys_to_alloc.into_iter().map(|k| vm.get_property(arg, &k)).collect();
        Ok(vm.alloc_array(vals_vec))
    });

    let entries_fn = vm.alloc_native_function("entries", 1, |vm, _this, args| {
        let arg = match args.first() {
            Some(a) if a.is_object() => *a,
            _ => return Err(VmError::TypeError("Object.entries called on non-object".into())),
        };
        let keys_to_alloc = get_enumerable_keys(vm, arg);
        let entries_vec: Vec<JsValue> = keys_to_alloc
            .into_iter()
            .map(|k| {
                let v = vm.get_property(arg, &k);
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

    let object_proto = vm.alloc_object();
    vm.object_proto = Some(object_proto);
    if let Some(ptr) = vm.global_object.as_object_ptr() {
        let obj = unsafe { &mut *(ptr as *mut JsObject) };
        obj.prototype = Some(object_proto);
    }

    vm.set_property(object_proto, "constructor", object_constructor);
    vm.non_enumerable_props.insert((object_proto, "constructor".to_string()));

    let is_prototype_of_fn = vm.alloc_native_function("isPrototypeOf", 1, |vm, this, args| {
        if !this.is_object() {
            return Ok(JsValue::from_boolean(false));
        }
        let target = match args.first() {
            Some(t) if t.is_object() => *t,
            _ => return Ok(JsValue::from_boolean(false)),
        };
        let mut current = vm.get_prototype_of(target);
        while let Some(proto) = current {
            if proto == this {
                return Ok(JsValue::from_boolean(true));
            }
            current = vm.get_prototype_of(proto);
        }
        Ok(JsValue::from_boolean(false))
    });
    vm.set_property(object_proto, "isPrototypeOf", is_prototype_of_fn);

    let value_of_fn = vm.alloc_native_function("valueOf", 0, |_vm, this, _args| {
        Ok(this)
    });
    vm.set_property(object_proto, "valueOf", value_of_fn);

    // Create Function.prototype early so all native functions get Function.prototype
    let native = crate::object::NativeFunction::new_fn("".to_string(), 0, |_vm, _this, _args| Ok(JsValue::undefined()));
    let fn_proto_obj = Box::new(JsObject::new_native_function(crate::shape::ShapeRegistry::ROOT_SHAPE_ID, Some(object_proto), native));
    let fn_proto_ptr = Box::into_raw(fn_proto_obj);
    vm.track_ptr(fn_proto_ptr);
    let function_proto = unsafe { JsValue::from_object_ptr(fn_proto_ptr as *mut u8) };
    vm.function_proto = Some(function_proto);
    let to_string_fn = vm.alloc_native_function("toString", 0, |vm, this, _args| {
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            let tag = match &obj.kind {
                ObjectKind::Array(_) => "Array",
                ObjectKind::Function(_) | ObjectKind::NativeFunction(_) => "Function",
                ObjectKind::Error { .. } => "Error",
                ObjectKind::RegExp { .. } => "RegExp",
                ObjectKind::Map(_) => "Map",
                ObjectKind::Set(_) => "Set",
                _ => "Object",
            };
            Ok(vm.alloc_string(format!("[object {}]", tag)))
        } else if this.is_null() {
            Ok(vm.alloc_string("[object Null]".into()))
        } else if this.is_undefined() {
            Ok(vm.alloc_string("[object Undefined]".into()))
        } else {
            Ok(vm.alloc_string("[object Object]".into()))
        }
    });
    vm.set_property(object_proto, "toString", to_string_fn);

    let has_own_prop = vm.alloc_native_function("hasOwnProperty", 1, |vm, this, args| {
        let key = args.first().map(|v| vm.to_string_value(*v)).unwrap_or_default();
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            let has = vm.shapes.has_property(obj.shape_id, &key)
                || obj.accessors.as_ref().map(|a| a.contains_key(&key)).unwrap_or(false);
            Ok(JsValue::from_boolean(has))
        } else {
            Ok(JsValue::from_boolean(false))
        }
    });
    vm.set_property(object_proto, "hasOwnProperty", has_own_prop);

    let prop_is_enumerable = vm.alloc_native_function("propertyIsEnumerable", 1, |vm, this, args| {
        let key = args.first().map(|v| vm.to_string_value(*v)).unwrap_or_default();
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            let has = vm.shapes.has_property(obj.shape_id, &key)
                || obj.accessors.as_ref().map(|a| a.contains_key(&key)).unwrap_or(false);
            if has {
                let is_enum = !vm.non_enumerable_props.contains(&(this, key));
                Ok(JsValue::from_boolean(is_enum))
            } else {
                Ok(JsValue::from_boolean(false))
            }
        } else {
            Ok(JsValue::from_boolean(false))
        }
    });
    vm.set_property(object_proto, "propertyIsEnumerable", prop_is_enumerable);

    let group_by_fn = vm.alloc_native_function("groupBy", 2, |vm, _this, args| {
        let items = args.first().copied().unwrap_or(JsValue::undefined());
        let callback = args.get(1).copied().unwrap_or(JsValue::undefined());
        if !vm.is_callable(callback) {
            return Err(VmError::TypeError("Object.groupBy callback must be callable".into()));
        }
        let out_obj = vm.alloc_object();
        let len_val = vm.get_property(items, "length");
        let raw_len = len_val.to_number().unwrap_or(0.0);
        if raw_len > 4_294_967_295.0 || raw_len < 0.0 || raw_len.is_infinite() {
            return Err(VmError::RangeError("Invalid array length".into()));
        }
        let len = raw_len as usize;
        if len > 100_000 {
            return Err(VmError::RangeError("Invalid array length".into()));
        }
        for i in 0..len {
            let item = vm.get_element(items, i);
            let key_val = vm.call(callback, JsValue::undefined(), &[item, JsValue::from_int32(i as i32)])?;
            let key_str = vm.to_string_value(key_val);
            let existing = vm.get_property(out_obj, &key_str);
            if existing.is_undefined() {
                let arr = vm.alloc_array(vec![item]);
                vm.set_property(out_obj, &key_str, arr);
            } else if let Some(ptr) = existing.as_object_ptr() {
                let arr_obj = unsafe { &mut *(ptr as *mut JsObject) };
                if let ObjectKind::Array(ref mut elems) = arr_obj.kind {
                    elems.push(item);
                }
            }
        }
        Ok(out_obj)
    });

    let define_property_fn = vm.alloc_native_function("defineProperty", 3, |vm, _this, args| {
        let target = match args.first() {
            Some(t) if t.is_object() => *t,
            _ => return Err(VmError::TypeError("Object.defineProperty called on non-object".into())),
        };
        let prop_key = match args.get(1) {
            Some(p) => vm.to_property_key(*p)?,
            None => PropertyKeyVal::String("undefined".to_string()),
        };
        let desc = match args.get(2) {
            Some(d) if d.is_object() => *d,
            _ => return Err(VmError::TypeError("Property description must be an object".into())),
        };

        let get_val = vm.get_property(desc, "get");
        let set_val = vm.get_property(desc, "set");
        let has_get = !get_val.is_undefined();
        let has_set = !set_val.is_undefined();

        match prop_key {
            PropertyKeyVal::Symbol(sym) => {
                if has_get || has_set {
                    if let Some(ptr) = target.as_object_ptr() {
                        let obj = unsafe { &mut *(ptr as *mut JsObject) };
                        let accs = obj.symbol_accessors.get_or_insert_with(|| Box::new(std::collections::HashMap::new()));
                        let entry = accs.entry(sym).or_insert(Accessor { getter: None, setter: None });
                        if has_get {
                            if !vm.is_callable(get_val) {
                                return Err(VmError::TypeError("Getter must be a function".into()));
                            }
                            entry.getter = Some(get_val);
                        }
                        if has_set {
                            if !vm.is_callable(set_val) {
                                return Err(VmError::TypeError("Setter must be a function".into()));
                            }
                            entry.setter = Some(set_val);
                        }
                        let order = obj.symbol_order.get_or_insert_with(Vec::new);
                        if !order.contains(&sym) {
                            order.push(sym);
                        }
                    }
                } else {
                    let val = vm.get_property(desc, "value");
                    if let Some(ptr) = target.as_object_ptr() {
                        let obj = unsafe { &mut *(ptr as *mut JsObject) };
                        obj.insert_symbol_property(sym, val);
                    }
                }
            }
            PropertyKeyVal::String(prop_name) => {
                if has_get || has_set {
                    if let Some(ptr) = target.as_object_ptr() {
                        let obj = unsafe { &mut *(ptr as *mut JsObject) };
                        if has_get {
                            if !vm.is_callable(get_val) {
                                return Err(VmError::TypeError("Getter must be a function".into()));
                            }
                            obj.define_getter(prop_name.clone(), get_val);
                        }
                        if has_set {
                            if !vm.is_callable(set_val) {
                                return Err(VmError::TypeError("Setter must be a function".into()));
                            }
                            obj.define_setter(prop_name.clone(), set_val);
                        }
                    }
                } else {
                    let val = vm.get_property(desc, "value");
                    vm.set_property(target, &prop_name, val);
                    if target == vm.global_object {
                        vm.globals.insert(prop_name.clone(), val);
                    }
                }

                let conf_val = vm.get_property(desc, "configurable");
                if !conf_val.is_undefined() {
                    let is_conf = conf_val.is_truthy();
                    if !is_conf {
                        vm.non_configurable_props.insert((target, prop_name.clone()));
                    } else {
                        vm.non_configurable_props.remove(&(target, prop_name.clone()));
                    }
                }

                let enum_val = vm.get_property(desc, "enumerable");
                if !enum_val.is_undefined() {
                    let is_enum = enum_val.is_truthy();
                    if !is_enum {
                        vm.non_enumerable_props.insert((target, prop_name.clone()));
                    } else {
                        vm.non_enumerable_props.remove(&(target, prop_name.clone()));
                    }
                }

                let writ_val = vm.get_property(desc, "writable");
                if !writ_val.is_undefined() {
                    let is_writ = writ_val.is_truthy();
                    if !is_writ {
                        vm.non_writable_props.insert((target, prop_name.clone()));
                    } else {
                        vm.non_writable_props.remove(&(target, prop_name.clone()));
                    }
                }
            }
        }

        Ok(target)
    });

    let get_own_prop_desc_fn = vm.alloc_native_function("getOwnPropertyDescriptor", 2, |vm, _this, args| {
        let target = match args.first() {
            Some(t) if t.is_object() => *t,
            Some(t) if !t.is_nullish() => *t,
            _ => return Err(VmError::TypeError("Object.getOwnPropertyDescriptor called on non-object".into())),
        };
        let prop_key = match args.get(1) {
            Some(p) => vm.to_property_key(*p)?,
            None => PropertyKeyVal::String("undefined".to_string()),
        };
        if let Some(ptr) = target.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            match prop_key {
                PropertyKeyVal::Symbol(sym) => {
                    if let Some(accs) = &obj.symbol_accessors {
                        if let Some(acc) = accs.get(&sym) {
                            let desc = vm.alloc_object();
                            vm.set_property(desc, "get", acc.getter.unwrap_or(JsValue::undefined()));
                            vm.set_property(desc, "set", acc.setter.unwrap_or(JsValue::undefined()));
                            vm.set_property(desc, "enumerable", JsValue::from_boolean(true));
                            vm.set_property(desc, "configurable", JsValue::from_boolean(true));
                            return Ok(desc);
                        }
                    }
                    if let Some(syms) = &obj.symbol_properties {
                        if let Some(val) = syms.get(&sym) {
                            let desc = vm.alloc_object();
                            vm.set_property(desc, "value", *val);
                            vm.set_property(desc, "writable", JsValue::from_boolean(true));
                            vm.set_property(desc, "enumerable", JsValue::from_boolean(true));
                            vm.set_property(desc, "configurable", JsValue::from_boolean(true));
                            return Ok(desc);
                        }
                    }
                }
                PropertyKeyVal::String(prop_name) => {
                    if let Some(accs) = &obj.accessors {
                        if let Some(acc) = accs.get(&prop_name) {
                            let desc = vm.alloc_object();
                            vm.set_property(desc, "get", acc.getter.unwrap_or(JsValue::undefined()));
                            vm.set_property(desc, "set", acc.setter.unwrap_or(JsValue::undefined()));
                            let is_enum = !vm.non_enumerable_props.contains(&(target, prop_name.clone()));
                            vm.set_property(desc, "enumerable", JsValue::from_boolean(is_enum));
                            let is_conf = !vm.non_configurable_props.contains(&(target, prop_name.clone()));
                            vm.set_property(desc, "configurable", JsValue::from_boolean(is_conf));
                            return Ok(desc);
                        }
                    }
                    if vm.shapes.has_property(obj.shape_id, &prop_name) {
                        let val = vm.get_property(target, &prop_name);
                        let desc = vm.alloc_object();
                        vm.set_property(desc, "value", val);
                        let is_writ = !vm.non_writable_props.contains(&(target, prop_name.clone()));
                        let is_enum = !vm.non_enumerable_props.contains(&(target, prop_name.clone()));
                        let is_conf = !vm.non_configurable_props.contains(&(target, prop_name.clone()));
                        vm.set_property(desc, "writable", JsValue::from_boolean(is_writ));
                        vm.set_property(desc, "enumerable", JsValue::from_boolean(is_enum));
                        vm.set_property(desc, "configurable", JsValue::from_boolean(is_conf));
                        return Ok(desc);
                    }
                }
            }
        }
        Ok(JsValue::undefined())
    });

    let get_own_prop_names_fn = vm.alloc_native_function("getOwnPropertyNames", 1, |vm, _this, args| {
        let target = match args.first() {
            Some(t) if t.is_object() => *t,
            _ => return Err(VmError::TypeError("Object.getOwnPropertyNames called on non-object".into())),
        };
        let mut raw_names: Vec<String> = Vec::new();
        if let Some(ptr) = target.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let Some(accs) = &obj.accessors {
                for k in accs.keys() {
                    if !raw_names.contains(k) {
                        raw_names.push(k.clone());
                    }
                }
            }
            for prop in vm.shapes.all_properties(obj.shape_id) {
                if !raw_names.contains(&prop) {
                    raw_names.push(prop);
                }
            }
        }
        let mut int_keys: Vec<(u32, String)> = Vec::new();
        let mut str_keys: Vec<String> = Vec::new();
        for k in raw_names {
            if let Ok(idx) = k.parse::<u32>() {
                if idx < u32::MAX && idx.to_string() == k {
                    int_keys.push((idx, k));
                    continue;
                }
            }
            str_keys.push(k);
        }
        int_keys.sort_by_key(|(idx, _)| *idx);
        let mut names = Vec::new();
        for (_, k) in int_keys {
            names.push(vm.alloc_string(k));
        }
        for k in str_keys {
            names.push(vm.alloc_string(k));
        }
        Ok(vm.alloc_array(names))
    });

    let get_own_prop_symbols_fn = vm.alloc_native_function("getOwnPropertySymbols", 1, |vm, _this, args| {
        let target = match args.first() {
            Some(t) if t.is_object() => *t,
            _ => return Err(VmError::TypeError("Object.getOwnPropertySymbols called on non-object".into())),
        };
        let mut symbols = Vec::new();
        if let Some(ptr) = target.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let Some(order) = &obj.symbol_order {
                for k in order {
                    if !symbols.contains(k) {
                        symbols.push(*k);
                    }
                }
            } else {
                if let Some(sym_accs) = &obj.symbol_accessors {
                    for k in sym_accs.keys() {
                        if !symbols.contains(k) {
                            symbols.push(*k);
                        }
                    }
                }
                if let Some(syms) = &obj.symbol_properties {
                    for k in syms.keys() {
                        if !symbols.contains(k) {
                            symbols.push(*k);
                        }
                    }
                }
            }
        }
        Ok(vm.alloc_array(symbols))
    });

    let prevent_extensions_fn = vm.alloc_native_function("preventExtensions", 1, |vm, _this, args| {
        let target = match args.first() {
            Some(t) if t.is_object() => *t,
            Some(t) if !t.is_nullish() => return Ok(*t),
            _ => return Err(VmError::TypeError("Object.preventExtensions called on non-object".into())),
        };
        vm.non_extensible_objects.insert(target);
        Ok(target)
    });

    let is_extensible_fn = vm.alloc_native_function("isExtensible", 1, |vm, _this, args| {
        let target = match args.first() {
            Some(t) if t.is_object() => *t,
            _ => return Ok(JsValue::from_boolean(false)),
        };
        let ext = !vm.non_extensible_objects.contains(&target);
        Ok(JsValue::from_boolean(ext))
    });

    let get_prototype_of_fn = vm.alloc_native_function("getPrototypeOf", 1, |vm, _this, args| {
        let target = match args.first() {
            Some(t) if !t.is_nullish() => *t,
            _ => return Err(VmError::TypeError("Cannot convert undefined or null to object".into())),
        };
        if let Some(ptr) = target.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            Ok(obj.prototype.unwrap_or(JsValue::null()))
        } else if target.is_string() {
            let string_ctor = vm.globals.get("String").copied().unwrap_or(JsValue::undefined());
            Ok(vm.get_property(string_ctor, "prototype"))
        } else if target.is_number() {
            let num_ctor = vm.globals.get("Number").copied().unwrap_or(JsValue::undefined());
            Ok(vm.get_property(num_ctor, "prototype"))
        } else if target.is_boolean() {
            let bool_ctor = vm.globals.get("Boolean").copied().unwrap_or(JsValue::undefined());
            Ok(vm.get_property(bool_ctor, "prototype"))
        } else {
            Ok(JsValue::null())
        }
    });

    let set_prototype_of_fn = vm.alloc_native_function("setPrototypeOf", 2, |vm, _this, args| {
        let target = match args.first() {
            Some(t) if !t.is_nullish() => *t,
            _ => return Err(VmError::TypeError("Cannot convert undefined or null to object".into())),
        };
        let proto = args.get(1).copied().unwrap_or(JsValue::undefined());
        if !proto.is_object() && !proto.is_null() {
            return Err(VmError::TypeError("Object prototype may only be an Object or null".into()));
        }
        if let Some(ptr) = target.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if vm.non_extensible_objects.contains(&target) {
                return Err(VmError::TypeError("Cannot set prototype of non-extensible object".into()));
            }
            obj.prototype = if proto.is_null() { None } else { Some(proto) };
        }
        Ok(target)
    });

    vm.set_property(object_constructor, "defineProperty", define_property_fn);
    vm.set_property(object_constructor, "getOwnPropertyDescriptor", get_own_prop_desc_fn);
    vm.set_property(object_constructor, "getOwnPropertyNames", get_own_prop_names_fn);
    vm.set_property(object_constructor, "getOwnPropertySymbols", get_own_prop_symbols_fn);
    vm.set_property(object_constructor, "preventExtensions", prevent_extensions_fn);
    vm.set_property(object_constructor, "isExtensible", is_extensible_fn);
    vm.set_property(object_constructor, "getPrototypeOf", get_prototype_of_fn);
    vm.set_property(object_constructor, "setPrototypeOf", set_prototype_of_fn);
    vm.set_property(object_constructor, "keys", keys_fn);
    vm.set_property(object_constructor, "values", values_fn);
    vm.set_property(object_constructor, "entries", entries_fn);
    vm.set_property(object_constructor, "assign", assign_fn);
    vm.set_property(object_constructor, "groupBy", group_by_fn);
    vm.set_property(object_constructor, "prototype", object_proto);
    vm.non_enumerable_props.insert((object_constructor, "prototype".to_string()));

    vm.globals.insert("Object".to_string(), object_constructor);
    if vm.global_object.is_object() {
        vm.set_property(vm.global_object, "Object", object_constructor);
    }
}

fn register_array_builtins(vm: &mut Vm) {
    let array_constructor = vm.alloc_native_function("Array", 1, |vm, _this, args| {
        if args.len() == 1 {
            if let Some(n) = args[0].to_number() {
                if n < 0.0 || n > 4_294_967_295.0 || n.fract() != 0.0 || n.is_nan() {
                    return Err(VmError::RangeError("Invalid array length".into()));
                }
                let len = n as usize;
                if len > 100_000 {
                    return Err(VmError::RangeError("Invalid array length".into()));
                }
                let mut v = Vec::with_capacity(len.min(1024));
                for _ in 0..len {
                    v.push(JsValue::undefined());
                }
                return Ok(vm.alloc_array(v));
            }
        }
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

    let values_fn = vm.alloc_native_function("values", 0, |vm, this, _args| {
        create_array_iterator(vm, this, IterKind::Values)
    });
    let keys_fn = vm.alloc_native_function("keys", 0, |vm, this, _args| {
        create_array_iterator(vm, this, IterKind::Keys)
    });
    let entries_fn = vm.alloc_native_function("entries", 0, |vm, this, _args| {
        create_array_iterator(vm, this, IterKind::Entries)
    });
    vm.set_property(array_proto, "values", values_fn);
    vm.set_property(array_proto, "keys", keys_fn);
    vm.set_property(array_proto, "entries", entries_fn);
    if let Some(sym_iter) = vm.symbol_iterator {
        vm.set_symbol_property(array_proto, sym_iter, values_fn);
    }

    let of_fn = vm.alloc_native_function("of", 0, |vm, _this, args| {
        Ok(vm.alloc_array(args.to_vec()))
    });
    vm.set_property(array_constructor, "of", of_fn);

    let from_fn = vm.alloc_native_function("from", 1, |vm, _this, args| {
        let items = args.first().copied().unwrap_or(JsValue::undefined());
        let map_fn = args.get(1).copied().unwrap_or(JsValue::undefined());
        let this_arg = args.get(2).copied().unwrap_or(JsValue::undefined());
        let has_map = vm.is_callable(map_fn);

        let mut result_elements = Vec::new();
        let mut handled = false;

        if let Some(sym_iter) = vm.symbol_iterator {
            let iter_method = vm.get_symbol_property_result(items, sym_iter).unwrap_or(JsValue::undefined());
            if vm.is_callable(iter_method) {
                handled = true;
                let iterator = vm.call(iter_method, items, &[])?;
                let next_fn = vm.get_property(iterator, "next");
                let mut idx = 0;
                while vm.is_callable(next_fn) {
                    let step = vm.call(next_fn, iterator, &[])?;
                    let done = vm.get_property(step, "done").is_truthy();
                    if done {
                        break;
                    }
                    let val = vm.get_property(step, "value");
                    let mapped = if has_map {
                        vm.call(map_fn, this_arg, &[val, JsValue::from_int32(idx)])?
                    } else {
                        val
                    };
                    result_elements.push(mapped);
                    idx += 1;
                }
            }
        }

        if !handled {
            let len_val = vm.get_property(items, "length");
            if let Some(n) = len_val.to_number() {
                let len = (n as usize).min(100_000);
                for i in 0..len {
                    let el = vm.get_element(items, i);
                    let mapped = if has_map {
                        vm.call(map_fn, this_arg, &[el, JsValue::from_int32(i as i32)])?
                    } else {
                        el
                    };
                    result_elements.push(mapped);
                }
            }
        }

        Ok(vm.alloc_array(result_elements))
    });
    vm.set_property(array_constructor, "from", from_fn);

    vm.set_property(array_proto, "constructor", array_constructor);

    vm.set_property(array_constructor, "isArray", is_array_fn);
    vm.set_property(array_constructor, "prototype", array_proto);
    vm.globals.insert("Array".to_string(), array_constructor);
    if vm.global_object.is_object() {
        vm.set_property(vm.global_object, "Array", array_constructor);
    }
}

fn register_function_builtins(vm: &mut Vm) {
    let call_fn = vm.alloc_native_function("call", 1, |vm, this, args| {
        if !vm.is_callable(this) {
            return Err(VmError::TypeError("Function.prototype.call called on non-function".into()));
        }
        let this_arg = args.first().copied().unwrap_or(JsValue::undefined());
        let rest_args = if args.len() > 1 { &args[1..] } else { &[] };
        vm.call(this, this_arg, rest_args)
    });

    let apply_fn = vm.alloc_native_function("apply", 2, |vm, this, args| {
        if !vm.is_callable(this) {
            return Err(VmError::TypeError("Function.prototype.apply called on non-function".into()));
        }
        let this_arg = args.first().copied().unwrap_or(JsValue::undefined());
        let mut call_args = Vec::new();
        if let Some(arg_array) = args.get(1) {
            if !arg_array.is_nullish() {
                if let Some(ptr) = arg_array.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    if let ObjectKind::Array(elems) = &obj.kind {
                        call_args.extend_from_slice(elems);
                    } else {
                        let len_val = vm.get_property(*arg_array, "length");
                        let raw_len = len_val.to_number().unwrap_or(0.0);
                        if raw_len > 65536.0 || raw_len < 0.0 || raw_len.is_infinite() {
                            return Err(VmError::RangeError("Maximum call stack / argument limit exceeded".into()));
                        }
                        let len = raw_len as usize;
                        for i in 0..len {
                            call_args.push(vm.get_property(*arg_array, &i.to_string()));
                        }
                    }
                } else {
                    return Err(VmError::TypeError("CreateListFromArrayLike called on non-object".into()));
                }
            }
        }
        vm.call(this, this_arg, &call_args)
    });

    let bind_fn = vm.alloc_native_function("bind", 1, |vm, this, args| {
        if !vm.is_callable(this) {
            return Err(VmError::TypeError("Function.prototype.bind called on non-function".into()));
        }
        let target_fn = this;
        let bound_this = args.first().copied().unwrap_or(JsValue::undefined());
        let bound_args: Vec<JsValue> = if args.len() > 1 { args[1..].to_vec() } else { Vec::new() };

        let bound_closure = vm.alloc_closure_native_function(
            "bound",
            0,
            Arc::new(move |vm, _call_this, call_args| {
                let mut combined_args = bound_args.clone();
                combined_args.extend_from_slice(call_args);
                vm.call(target_fn, bound_this, &combined_args)
            }),
        );
        Ok(bound_closure)
    });

    let to_string_fn = vm.alloc_native_function("toString", 0, |vm, _this, _args| {
        Ok(vm.alloc_string("function () { [native code] }".into()))
    });

    let function_proto = vm.function_proto.unwrap();

    vm.set_property(function_proto, "call", call_fn);
    vm.set_property(function_proto, "apply", apply_fn);
    vm.set_property(function_proto, "bind", bind_fn);
    vm.set_property(function_proto, "toString", to_string_fn);

    let function_constructor = vm.alloc_native_function("Function", 1, |vm, _this, args| {
        let (params_str, body_str) = if args.is_empty() {
            (String::new(), String::new())
        } else if args.len() == 1 {
            (String::new(), vm.to_string_value(args[0]))
        } else {
            let param_parts: Vec<String> = args[..args.len() - 1].iter().map(|a| vm.to_string_value(*a)).collect();
            let p = param_parts.join(", ");
            let b = vm.to_string_value(args[args.len() - 1]);
            (p, b)
        };
        let code = format!("(function({}) {{\n{}\n}})", params_str, body_str);
        let program = doms_parser::parse(&code)
            .map_err(|e| VmError::SyntaxError(e.to_string()))?;
        let chunk = doms_compiler::compile(&program)
            .map_err(|e| VmError::SyntaxError(e.to_string()))?;
        vm.run(&chunk)
    });

    vm.set_property(function_proto, "constructor", function_constructor);
    vm.set_property(function_constructor, "prototype", function_proto);
    if let Some(ptr) = function_constructor.as_object_ptr() {
        let obj = unsafe { &mut *(ptr as *mut JsObject) };
        obj.prototype = Some(function_proto);
    }

    vm.globals.insert("Function".to_string(), function_constructor);
    if vm.global_object.is_object() {
        vm.set_property(vm.global_object, "Function", function_constructor);
    }
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

    let string_proto = vm.alloc_object();
    vm.string_proto = Some(string_proto);

    let str_value_of = vm.alloc_native_function("valueOf", 0, |vm, this, _args| {
        if this.is_string() {
            return Ok(this);
        }
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::StringObject(ref s) = obj.kind {
                return Ok(vm.alloc_string(s.clone()));
            }
        }
        Err(VmError::TypeError("String.prototype.valueOf requires that 'this' be a String".into()))
    });
    let str_to_string = vm.alloc_native_function("toString", 0, |vm, this, _args| {
        if this.is_string() {
            return Ok(this);
        }
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::StringObject(ref s) = obj.kind {
                return Ok(vm.alloc_string(s.clone()));
            }
        }
        Err(VmError::TypeError("String.prototype.toString requires that 'this' be a String".into()))
    });
    vm.set_property(string_proto, "valueOf", str_value_of);
    vm.set_property(string_proto, "toString", str_to_string);

    let str_ctor = vm.alloc_native_function("String", 1, |vm, _this, args| {
        let s = args.first().map(|v| vm.to_string_value(*v)).unwrap_or_default();
        if vm.is_constructing {
            let proto = vm.string_proto;
            Ok(vm.alloc_string_object(s, proto))
        } else {
            Ok(vm.alloc_string(s))
        }
    });

    vm.set_property(string_proto, "constructor", str_ctor);
    vm.non_enumerable_props.insert((string_proto, "constructor".to_string()));
    vm.set_property(str_ctor, "prototype", string_proto);
    vm.non_enumerable_props.insert((str_ctor, "prototype".to_string()));

    if let Some(sym_iter) = vm.symbol_iterator {
        let str_iter_fn = vm.alloc_native_function("[Symbol.iterator]", 0, |vm, this, _args| {
            let s = vm.to_string_value(this);
            let chars: Vec<JsValue> = s.chars().map(|c| vm.alloc_string(c.to_string())).collect();
            Ok(vm.alloc_iterator(chars))
        });
        vm.set_symbol_property(string_proto, sym_iter, str_iter_fn);
    }

    let from_char_code_fn = vm.alloc_native_function("fromCharCode", 1, |vm, _this, args| {
        let mut s = String::new();
        for arg in args {
            let n = vm.to_number_value(*arg).unwrap_or(0.0) as u32 as u16;
            if let Some(ch) = char::decode_utf16([n]).next().and_then(|r| r.ok()) {
                s.push(ch);
            } else {
                s.push('\u{FFFD}');
            }
        }
        Ok(vm.alloc_string(s))
    });
    vm.set_property(str_ctor, "fromCharCode", from_char_code_fn);

    let from_code_point_fn = vm.alloc_native_function("fromCodePoint", 1, |vm, _this, args| {
        let mut s = String::new();
        for arg in args {
            let n = vm.to_number_value(*arg).unwrap_or(0.0) as u32;
            if let Some(ch) = char::from_u32(n) {
                s.push(ch);
            } else {
                return Err(VmError::RangeError(format!("Invalid code point {}", n)));
            }
        }
        Ok(vm.alloc_string(s))
    });
    vm.set_property(str_ctor, "fromCodePoint", from_code_point_fn);

    vm.globals.insert("String".to_string(), str_ctor);
    if vm.global_object.is_object() {
        vm.set_property(vm.global_object, "String", str_ctor);
    }

    let number_proto = vm.alloc_object();
    vm.number_proto = Some(number_proto);

    let num_value_of = vm.alloc_native_function("valueOf", 0, |_vm, this, _args| {
        if this.is_number() {
            return Ok(this);
        }
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::NumberObject(n) = obj.kind {
                return Ok(JsValue::from_double(n));
            }
        }
        Err(VmError::TypeError("Number.prototype.valueOf requires that 'this' be a Number".into()))
    });
    let num_to_string = vm.alloc_native_function("toString", 1, |vm, this, args| {
        let n = if this.is_number() {
            this.to_number().unwrap_or(0.0)
        } else if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::NumberObject(num) = obj.kind {
                num
            } else {
                return Err(VmError::TypeError("Number.prototype.toString requires that 'this' be a Number".into()));
            }
        } else {
            return Err(VmError::TypeError("Number.prototype.toString requires that 'this' be a Number".into()));
        };
        let radix = args.first().and_then(|v| v.to_int32()).unwrap_or(10);
        if !(2..=36).contains(&radix) {
            return Err(VmError::RangeError("radix must be between 2 and 36".into()));
        }
        if radix == 10 {
            Ok(vm.alloc_string(vm.to_string_value(JsValue::from_double(n))))
        } else {
            let int_val = n as i64;
            let s = match radix {
                2 => format!("{:b}", int_val),
                8 => format!("{:o}", int_val),
                16 => format!("{:x}", int_val),
                _ => format!("{}", int_val),
            };
            Ok(vm.alloc_string(s))
        }
    });
    vm.set_property(number_proto, "valueOf", num_value_of);
    vm.set_property(number_proto, "toString", num_to_string);

    let num_ctor = vm.alloc_native_function("Number", 1, |vm, _this, args| {
        let n = if let Some(arg) = args.first() {
            if let Some(num) = arg.to_number() {
                num
            } else {
                let s = vm.to_string_value(*arg);
                let trimmed = s.trim();
                if trimmed.is_empty() {
                    0.0
                } else if trimmed == "Infinity" || trimmed == "+Infinity" {
                    f64::INFINITY
                } else if trimmed == "-Infinity" {
                    f64::NEG_INFINITY
                } else {
                    trimmed.parse::<f64>().unwrap_or(f64::NAN)
                }
            }
        } else {
            0.0
        };
        if vm.is_constructing {
            let proto = vm.number_proto;
            Ok(vm.alloc_number_object(n, proto))
        } else {
            Ok(JsValue::from_double(n))
        }
    });
    vm.set_property(number_proto, "constructor", num_ctor);
    vm.non_enumerable_props.insert((number_proto, "constructor".to_string()));
    vm.set_property(num_ctor, "prototype", number_proto);
    vm.non_enumerable_props.insert((num_ctor, "prototype".to_string()));

    let num_constants: [(&str, f64); 8] = [
        ("NaN", f64::NAN),
        ("POSITIVE_INFINITY", f64::INFINITY),
        ("NEGATIVE_INFINITY", f64::NEG_INFINITY),
        ("MAX_VALUE", 1.7976931348623157e+308),
        ("MIN_VALUE", 5e-324),
        ("EPSILON", 2.220446049250313e-16),
        ("MAX_SAFE_INTEGER", 9007199254740991.0),
        ("MIN_SAFE_INTEGER", -9007199254740991.0),
    ];
    for (name, val) in num_constants {
        vm.set_property(num_ctor, name, JsValue::from_double(val));
        vm.non_writable_props.insert((num_ctor, name.to_string()));
        vm.non_enumerable_props.insert((num_ctor, name.to_string()));
        vm.non_configurable_props.insert((num_ctor, name.to_string()));
    }
    vm.globals.insert("Number".to_string(), num_ctor);
    if vm.global_object.is_object() {
        vm.set_property(vm.global_object, "Number", num_ctor);
    }

    let boolean_proto = vm.alloc_object();
    vm.boolean_proto = Some(boolean_proto);

    let bool_value_of = vm.alloc_native_function("valueOf", 0, |_vm, this, _args| {
        if this.is_boolean() {
            return Ok(this);
        }
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::BooleanObject(b) = obj.kind {
                return Ok(JsValue::from_boolean(b));
            }
        }
        Err(VmError::TypeError("Boolean.prototype.valueOf requires that 'this' be a Boolean".into()))
    });
    let bool_to_string = vm.alloc_native_function("toString", 0, |vm, this, _args| {
        let b = if this.is_boolean() {
            this.as_boolean().unwrap()
        } else if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::BooleanObject(b) = obj.kind {
                b
            } else {
                return Err(VmError::TypeError("Boolean.prototype.toString requires that 'this' be a Boolean".into()));
            }
        } else {
            return Err(VmError::TypeError("Boolean.prototype.toString requires that 'this' be a Boolean".into()));
        };
        Ok(vm.alloc_string(if b { "true".into() } else { "false".into() }))
    });
    vm.set_property(boolean_proto, "valueOf", bool_value_of);
    vm.set_property(boolean_proto, "toString", bool_to_string);

    let bool_ctor = vm.alloc_native_function("Boolean", 1, |vm, _this, args| {
        let b = args.first().map(|v| v.is_truthy()).unwrap_or(false);
        if vm.is_constructing {
            let proto = vm.boolean_proto;
            Ok(vm.alloc_boolean_object(b, proto))
        } else {
            Ok(JsValue::from_boolean(b))
        }
    });
    vm.set_property(boolean_proto, "constructor", bool_ctor);
    vm.non_enumerable_props.insert((boolean_proto, "constructor".to_string()));
    vm.set_property(bool_ctor, "prototype", boolean_proto);
    vm.non_enumerable_props.insert((bool_ctor, "prototype".to_string()));
    vm.globals.insert("Boolean".to_string(), bool_ctor);
    if vm.global_object.is_object() {
        vm.set_property(vm.global_object, "Boolean", bool_ctor);
    }

    let error_types = ["Error", "TypeError", "RangeError", "ReferenceError", "SyntaxError", "URIError", "EvalError"];
    for err_type in error_types {
        let err_name = err_type.to_string();
        let proto = vm.alloc_object();
        let name_val = vm.alloc_string(err_name.clone());
        vm.set_property(proto, "name", name_val);

        let err_name_c = err_name.clone();
        let ctor = vm.alloc_closure_native_function(err_type, 1, Arc::new(move |vm, _this, args| {
            let msg = args.first().map(|v| vm.to_string_value(*v)).unwrap_or_default();
            Ok(vm.alloc_error(err_name_c.clone(), msg))
        }));
        vm.set_property(ctor, "prototype", proto);
        vm.globals.insert(err_type.to_string(), ctor);
    }

    vm.globals.insert("parseInt".to_string(), parse_int);
    vm.globals.insert("parseFloat".to_string(), parse_float);
    vm.globals.insert("isNaN".to_string(), is_nan);
    vm.globals.insert("isFinite".to_string(), is_finite);
    vm.globals.insert("NaN".to_string(), JsValue::from_double(f64::NAN));
    vm.globals.insert("Infinity".to_string(), JsValue::from_double(f64::INFINITY));
    vm.globals.insert("undefined".to_string(), JsValue::undefined());

    // eval
    let eval_fn = vm.alloc_native_function("eval", 1, |vm, _this, args| {
        if args.is_empty() {
            return Ok(JsValue::undefined());
        }
        let code_val = args[0];
        let code_str = if let Some(ptr) = code_val.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::String(ref s) = obj.kind {
                s.clone()
            } else {
                return Ok(code_val);
            }
        } else {
            return Ok(code_val);
        };

        if let Some(max) = vm.max_evals {
            vm.eval_count += 1;
            if vm.eval_count > max {
                return Err(VmError::RangeError(
                    "Execution limit exceeded in eval (too many calls)".to_string(),
                ));
            }
        }
        if let Some(deadline) = vm.deadline {
            if std::time::Instant::now() > deadline {
                return Err(VmError::RuntimeError(
                    "Execution timeout exceeded in eval".to_string(),
                ));
            }
        }
        if vm.oom {
            return Err(VmError::RangeError("Out of memory in eval".to_string()));
        }

        let caller_strict = vm.frames.last().map(|f| f.is_strict).unwrap_or(vm.is_strict);
        let program = if caller_strict {
            doms_parser::parse_strict(&code_str)
        } else {
            doms_parser::parse(&code_str)
        }.map_err(|e| VmError::SyntaxError(e.to_string()))?;

        let is_eval_strict = caller_strict || program.body.iter()
            .take_while(|s| matches!(s, doms_parser::ast::Statement::Expression(doms_parser::ast::Expression::Literal(doms_parser::ast::Literal::String { .. }))))
            .any(|s| matches!(s, doms_parser::ast::Statement::Expression(doms_parser::ast::Expression::Literal(doms_parser::ast::Literal::String { value, has_escape: false })) if value == "use strict"));

        let chunk = doms_compiler::compile(&program)
            .map_err(|e| VmError::SyntaxError(e.to_string()))?;

        vm.in_eval = true;
        let res = (|| {
            vm.instantiate_global_declarations(&chunk)?;
            let target_depth = vm.frames.len();
            let base_sp = vm.stack.len();
            vm.frames.push(crate::interpreter::CallFrame {
                chunk: Arc::new(chunk),
                ip: 0,
                base_sp,
                this_val: vm.global_object,
                gen_ptr: None,
                locals: Vec::new(),
                local_names: Vec::new(),
                scopes: Vec::new(),
                env_obj: None,
                arguments: None,
                is_strict: is_eval_strict,
            });
            vm.run_until_depth(target_depth)
        })();
        vm.in_eval = false;
        res
    });
    vm.globals.insert("eval".to_string(), eval_fn);
    if vm.global_object.is_object() {
        vm.set_property(vm.global_object, "eval", eval_fn);
    }

    // Date
    let date_proto = vm.alloc_object();
    let date_to_string = vm.alloc_native_function("toString", 0, |vm, this, _args| {
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::Date(time_ms) = obj.kind {
                let s = format!("Date({})", time_ms);
                return Ok(vm.alloc_string(s));
            }
        }
        Ok(vm.alloc_string("Date(0)".into()))
    });
    let date_value_of = vm.alloc_native_function("valueOf", 0, |_vm, this, _args| {
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::Date(time_ms) = obj.kind {
                return Ok(JsValue::from_double(time_ms));
            }
        }
        Ok(JsValue::from_double(0.0))
    });
    let date_get_time = vm.alloc_native_function("getTime", 0, |_vm, this, _args| {
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::Date(time_ms) = obj.kind {
                return Ok(JsValue::from_double(time_ms));
            }
        }
        Ok(JsValue::from_double(0.0))
    });
    vm.set_property(date_proto, "toString", date_to_string);
    vm.set_property(date_proto, "valueOf", date_value_of);
    vm.set_property(date_proto, "getTime", date_get_time);

    let date_ctor = vm.alloc_closure_native_function("Date", 1, Arc::new(move |vm, _this, args| {
        let time_ms = args.first().and_then(|v| v.to_number()).unwrap_or(0.0);
        Ok(vm.alloc_date(Some(date_proto), time_ms))
    }));
    vm.set_property(date_ctor, "prototype", date_proto);
    vm.globals.insert("Date".to_string(), date_ctor);

    // Symbol registration moved to register_symbol(vm)
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

    let with_resolvers_static = vm.alloc_native_function("withResolvers", 0, |vm, _this, _args| {
        let promise = vm.alloc_promise(crate::object::PromiseState::Pending, JsValue::undefined());
        let resolve_fn = {
            let p = promise;
            vm.alloc_closure_native_function("resolve", 1, Arc::new(move |vm: &mut Vm, _this, args| {
                let val = args.first().copied().unwrap_or(JsValue::undefined());
                vm.resolve_promise(p, val);
                Ok(JsValue::undefined())
            }))
        };
        let reject_fn = {
            let p = promise;
            vm.alloc_closure_native_function("reject", 1, Arc::new(move |vm: &mut Vm, _this, args| {
                let val = args.first().copied().unwrap_or(JsValue::undefined());
                vm.reject_promise(p, val);
                Ok(JsValue::undefined())
            }))
        };
        let res_obj = vm.alloc_object();
        vm.set_property(res_obj, "promise", promise);
        vm.set_property(res_obj, "resolve", resolve_fn);
        vm.set_property(res_obj, "reject", reject_fn);
        Ok(res_obj)
    });

    let all_settled_static = vm.alloc_native_function("allSettled", 1, |vm, _this, args| {
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

                    let resolve_item = vm.alloc_closure_native_function("settled_res", 1, Arc::new(move |vm: &mut Vm, _this: JsValue, args: &[JsValue]| -> Result<JsValue, VmError> {
                        let v = args.first().copied().unwrap_or(JsValue::undefined());
                        let item_obj = vm.alloc_object();
                        let status_str = vm.alloc_string("fulfilled".into());
                        vm.set_property(item_obj, "status", status_str);
                        vm.set_property(item_obj, "value", v);
                        {
                            let mut lock = res_store.lock().unwrap();
                            lock[idx] = item_obj;
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

                    let rem_rej = Arc::clone(&remaining);
                    let res_store_rej = Arc::clone(&results);
                    let reject_item = vm.alloc_closure_native_function("settled_rej", 1, Arc::new(move |vm: &mut Vm, _this: JsValue, args: &[JsValue]| -> Result<JsValue, VmError> {
                        let err = args.first().copied().unwrap_or(JsValue::undefined());
                        let item_obj = vm.alloc_object();
                        let status_str = vm.alloc_string("rejected".into());
                        vm.set_property(item_obj, "status", status_str);
                        vm.set_property(item_obj, "reason", err);
                        {
                            let mut lock = res_store_rej.lock().unwrap();
                            lock[idx] = item_obj;
                        }
                        if rem_rej.fetch_sub(1, std::sync::atomic::Ordering::SeqCst) == 1 {
                            let final_res = {
                                let lock = res_store_rej.lock().unwrap();
                                lock.clone()
                            };
                            let arr = vm.alloc_array(final_res);
                            vm.resolve_promise(r_p, arr);
                        }
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

    let any_static = vm.alloc_native_function("any", 1, |vm, _this, args| {
        let iterable = args.first().copied().unwrap_or(JsValue::undefined());
        if let Some(ptr) = iterable.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::Array(elements) = &obj.kind {
                let elements_clone = elements.clone();
                let result_promise = vm.alloc_promise(crate::object::PromiseState::Pending, JsValue::undefined());
                if elements_clone.is_empty() {
                    let err_str = vm.alloc_string("All promises were rejected".into());
                    vm.reject_promise(result_promise, err_str);
                    return Ok(result_promise);
                }
                let count = elements_clone.len();
                let remaining = Arc::new(std::sync::atomic::AtomicUsize::new(count));
                let errors = Arc::new(std::sync::Mutex::new(vec![JsValue::undefined(); count]));

                for (idx, elem) in elements_clone.into_iter().enumerate() {
                    let r_p = result_promise;
                    let resolve_item = vm.alloc_closure_native_function("any_res", 1, Arc::new(move |vm: &mut Vm, _this: JsValue, args: &[JsValue]| -> Result<JsValue, VmError> {
                        let v = args.first().copied().unwrap_or(JsValue::undefined());
                        vm.resolve_promise(r_p, v);
                        Ok(JsValue::undefined())
                    }));

                    let rem = Arc::clone(&remaining);
                    let err_store = Arc::clone(&errors);
                    let reject_item = vm.alloc_closure_native_function("any_rej", 1, Arc::new(move |vm: &mut Vm, _this: JsValue, args: &[JsValue]| -> Result<JsValue, VmError> {
                        let err = args.first().copied().unwrap_or(JsValue::undefined());
                        {
                            let mut lock = err_store.lock().unwrap();
                            lock[idx] = err;
                        }
                        if rem.fetch_sub(1, std::sync::atomic::Ordering::SeqCst) == 1 {
                            let final_errs = {
                                let lock = err_store.lock().unwrap();
                                lock.clone()
                            };
                            let err_arr = vm.alloc_array(final_errs);
                            vm.reject_promise(r_p, err_arr);
                        }
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
        let err_str = vm.alloc_string("All promises were rejected".into());
        Ok(vm.alloc_promise(crate::object::PromiseState::Rejected, err_str))
    });

    vm.set_property(promise_ctor, "resolve", resolve_static);
    vm.set_property(promise_ctor, "reject", reject_static);
    vm.set_property(promise_ctor, "all", all_static);
    vm.set_property(promise_ctor, "race", race_static);
    vm.set_property(promise_ctor, "allSettled", all_settled_static);
    vm.set_property(promise_ctor, "any", any_static);
    vm.set_property(promise_ctor, "withResolvers", with_resolvers_static);

    vm.globals.insert("Promise".to_string(), promise_ctor);
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum IterKind {
    Keys,
    Values,
    Entries,
}

fn create_array_iterator(vm: &mut Vm, target: JsValue, kind: IterKind) -> Result<JsValue, VmError> {
    let iter_obj = vm.alloc_object();
    let idx = Arc::new(std::sync::Mutex::new(0usize));
    let next_fn = vm.alloc_closure_native_function("next", 0, Arc::new(move |vm, _this, _args| {
        let mut i = idx.lock().unwrap();
        let len = vm.get_property(target, "length").to_number().unwrap_or(0.0) as usize;
        let res = vm.alloc_object();
        if *i < len {
            let curr = *i;
            *i += 1;
            let val = match kind {
                IterKind::Keys => JsValue::from_int32(curr as i32),
                IterKind::Values => vm.get_element(target, curr),
                IterKind::Entries => {
                    let el = vm.get_element(target, curr);
                    vm.alloc_array(vec![JsValue::from_int32(curr as i32), el])
                }
            };
            vm.set_property(res, "value", val);
            vm.set_property(res, "done", JsValue::from_boolean(false));
        } else {
            vm.set_property(res, "value", JsValue::undefined());
            vm.set_property(res, "done", JsValue::from_boolean(true));
        }
        Ok(res)
    }));
    vm.set_property(iter_obj, "next", next_fn);
    if let Some(sym_iter) = vm.symbol_iterator {
        let self_val = iter_obj;
        let self_fn = vm.alloc_closure_native_function("[Symbol.iterator]", 0, Arc::new(move |_vm, _this, _args| Ok(self_val)));
        vm.set_symbol_property(iter_obj, sym_iter, self_fn);
    }
    Ok(iter_obj)
}

fn construct_typed_array_kind(vm: &mut Vm, args: &[JsValue], kind: TypedArrayKind) -> Result<JsValue, VmError> {
    let bpe = kind.bytes_per_element();
    let arg0 = args.first().copied().unwrap_or(JsValue::undefined());
    // 1. ArrayBuffer or SharedArrayBuffer
    if let Some(ptr) = arg0.as_object_ptr() {
        let obj = unsafe { &*(ptr as *mut JsObject) };
        if let ObjectKind::ArrayBuffer(sab) | ObjectKind::SharedArrayBuffer(sab) = &obj.kind {
            let buf_len = sab.byte_length();
            let byte_offset = args.get(1).and_then(|v| v.to_int32()).unwrap_or(0).max(0) as usize;
            if byte_offset % bpe != 0 {
                return Err(VmError::RangeError("Start offset must be a multiple of the element size".into()));
            }
            if byte_offset > buf_len {
                return Err(VmError::RangeError("Start offset is outside the bounds of the buffer".into()));
            }
            let length = if let Some(len_arg) = args.get(2) {
                let l = len_arg.to_int32().unwrap_or(0).max(0) as usize;
                if byte_offset + l * bpe > buf_len {
                    return Err(VmError::RangeError("Invalid typed array length".into()));
                }
                l
            } else {
                (buf_len - byte_offset) / bpe
            };
            return Ok(vm.alloc_typed_array(JsTypedArray {
                buffer: Arc::clone(sab),
                kind,
                length,
                byte_offset,
            }));
        } else if let ObjectKind::TypedArray(src_ta) = &obj.kind {
            // Another TypedArray
            let len = src_ta.length;
            let new_sab = doms_parallel::SharedArrayBuffer::try_new(len * bpe)
                .map_err(|e| VmError::RuntimeError(e.into()))?;
            let mut dst_ta = JsTypedArray {
                buffer: Arc::new(new_sab),
                kind,
                length: len,
                byte_offset: 0,
            };
            for i in 0..len {
                if let Some(val) = Vm::read_typed_array_element(src_ta, i) {
                    Vm::write_typed_array_element(&mut dst_ta, i, val);
                }
            }
            return Ok(vm.alloc_typed_array(dst_ta));
        } else if let ObjectKind::Array(elements) = &obj.kind {
            // Array
            let len = elements.len();
            let new_sab = doms_parallel::SharedArrayBuffer::try_new(len * bpe)
                .map_err(|e| VmError::RuntimeError(e.into()))?;
            let mut dst_ta = JsTypedArray {
                buffer: Arc::new(new_sab),
                kind,
                length: len,
                byte_offset: 0,
            };
            for (i, el) in elements.iter().enumerate() {
                Vm::write_typed_array_element(&mut dst_ta, i, *el);
            }
            return Ok(vm.alloc_typed_array(dst_ta));
        }
    }

    // 2. Length or number argument
    let len = arg0.to_int32().unwrap_or(0).max(0) as usize;
    if len * bpe > 64 * 1024 * 1024 {
        return Err(VmError::RangeError("Invalid typed array length".into()));
    }
    let sab = doms_parallel::SharedArrayBuffer::try_new(len * bpe)
        .map_err(|e| VmError::RuntimeError(e.into()))?;
    Ok(vm.alloc_typed_array(JsTypedArray {
        buffer: Arc::new(sab),
        kind,
        length: len,
        byte_offset: 0,
    }))
}

fn construct_int8_array(vm: &mut Vm, _this: JsValue, args: &[JsValue]) -> Result<JsValue, VmError> {
    construct_typed_array_kind(vm, args, TypedArrayKind::Int8)
}
fn construct_uint8_array(vm: &mut Vm, _this: JsValue, args: &[JsValue]) -> Result<JsValue, VmError> {
    construct_typed_array_kind(vm, args, TypedArrayKind::Uint8)
}
fn construct_uint8_clamped_array(vm: &mut Vm, _this: JsValue, args: &[JsValue]) -> Result<JsValue, VmError> {
    construct_typed_array_kind(vm, args, TypedArrayKind::Uint8Clamped)
}
fn construct_int16_array(vm: &mut Vm, _this: JsValue, args: &[JsValue]) -> Result<JsValue, VmError> {
    construct_typed_array_kind(vm, args, TypedArrayKind::Int16)
}
fn construct_uint16_array(vm: &mut Vm, _this: JsValue, args: &[JsValue]) -> Result<JsValue, VmError> {
    construct_typed_array_kind(vm, args, TypedArrayKind::Uint16)
}
fn construct_int32_array(vm: &mut Vm, _this: JsValue, args: &[JsValue]) -> Result<JsValue, VmError> {
    construct_typed_array_kind(vm, args, TypedArrayKind::Int32)
}
fn construct_uint32_array(vm: &mut Vm, _this: JsValue, args: &[JsValue]) -> Result<JsValue, VmError> {
    construct_typed_array_kind(vm, args, TypedArrayKind::Uint32)
}
fn construct_float32_array(vm: &mut Vm, _this: JsValue, args: &[JsValue]) -> Result<JsValue, VmError> {
    construct_typed_array_kind(vm, args, TypedArrayKind::Float32)
}
fn construct_float64_array(vm: &mut Vm, _this: JsValue, args: &[JsValue]) -> Result<JsValue, VmError> {
    construct_typed_array_kind(vm, args, TypedArrayKind::Float64)
}
fn construct_bigint64_array(vm: &mut Vm, _this: JsValue, args: &[JsValue]) -> Result<JsValue, VmError> {
    construct_typed_array_kind(vm, args, TypedArrayKind::BigInt64)
}
fn construct_biguint64_array(vm: &mut Vm, _this: JsValue, args: &[JsValue]) -> Result<JsValue, VmError> {
    construct_typed_array_kind(vm, args, TypedArrayKind::BigUint64)
}

fn register_single_typed_array(vm: &mut Vm, kind: TypedArrayKind) {
    let name = kind.name();
    let bpe = kind.bytes_per_element();

    let ctor_fn: NativeFn = match kind {
        TypedArrayKind::Int8 => construct_int8_array,
        TypedArrayKind::Uint8 => construct_uint8_array,
        TypedArrayKind::Uint8Clamped => construct_uint8_clamped_array,
        TypedArrayKind::Int16 => construct_int16_array,
        TypedArrayKind::Uint16 => construct_uint16_array,
        TypedArrayKind::Int32 => construct_int32_array,
        TypedArrayKind::Uint32 => construct_uint32_array,
        TypedArrayKind::Float32 => construct_float32_array,
        TypedArrayKind::Float64 => construct_float64_array,
        TypedArrayKind::BigInt64 => construct_bigint64_array,
        TypedArrayKind::BigUint64 => construct_biguint64_array,
    };

    let ctor = vm.alloc_native_function(name, 1, ctor_fn);

    let proto = vm.alloc_object();

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

    vm.set_property(ctor, "BYTES_PER_ELEMENT", JsValue::from_int32(bpe as i32));
    vm.set_property(proto, "BYTES_PER_ELEMENT", JsValue::from_int32(bpe as i32));
    vm.set_property(proto, "constructor", ctor);
    vm.set_property(ctor, "prototype", proto);

    vm.globals.insert(name.to_string(), ctor);
    if vm.global_object.is_object() {
        vm.set_property(vm.global_object, name, ctor);
    }
}

fn register_parallel_and_typed_arrays(vm: &mut Vm) {
    let sab_ctor = vm.alloc_native_function("SharedArrayBuffer", 1, |vm, _this, args| {
        let byte_len = args.first().and_then(|v| v.to_int32()).unwrap_or(0);
        if !(0..=64 * 1024 * 1024).contains(&byte_len) {
            return Err(VmError::RangeError("Invalid SharedArrayBuffer length".into()));
        }
        let sab = doms_parallel::SharedArrayBuffer::try_new(byte_len as usize)
            .map_err(|e| VmError::RuntimeError(e.into()))?;
        Ok(vm.alloc_shared_array_buffer(Arc::new(sab)))
    });
    let sab_proto = vm.alloc_object();
    vm.set_property(sab_ctor, "prototype", sab_proto);
    vm.globals.insert("SharedArrayBuffer".to_string(), sab_ctor);

    let ab_ctor = vm.alloc_native_function("ArrayBuffer", 1, |vm, _this, args| {
        let byte_len = args.first().and_then(|v| v.to_int32()).unwrap_or(0);
        if !(0..=64 * 1024 * 1024).contains(&byte_len) {
            return Err(VmError::RangeError("Invalid ArrayBuffer length".into()));
        }
        let sab = doms_parallel::SharedArrayBuffer::try_new(byte_len as usize)
            .map_err(|e| VmError::RuntimeError(e.into()))?;
        Ok(vm.alloc_array_buffer(Arc::new(sab)))
    });
    let ab_is_view = vm.alloc_native_function("isView", 1, |_vm, _this, args| {
        if let Some(arg) = args.first() {
            if let Some(ptr) = arg.as_object_ptr() {
                let obj = unsafe { &*(ptr as *mut JsObject) };
                if matches!(obj.kind, ObjectKind::TypedArray(_) | ObjectKind::DataView { .. }) {
                    return Ok(JsValue::from_boolean(true));
                }
            }
        }
        Ok(JsValue::from_boolean(false))
    });
    vm.set_property(ab_ctor, "isView", ab_is_view);

    let ab_proto = vm.alloc_object();
    let ab_slice = vm.alloc_native_function("slice", 2, |vm, this, args| {
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::ArrayBuffer(buf) = &obj.kind {
                let len = buf.byte_length() as i32;
                let start = args.first().and_then(|v| v.to_int32()).unwrap_or(0);
                let end = args.get(1).and_then(|v| v.to_int32()).unwrap_or(len);
                let s = if start < 0 { (len + start).max(0) } else { start.min(len) } as usize;
                let e = if end < 0 { (len + end).max(0) } else { end.min(len) } as usize;
                let count = if e > s { e - s } else { 0 };
                let new_buf = doms_parallel::SharedArrayBuffer::try_new(count)
                    .map_err(|e| VmError::RuntimeError(e.into()))?;
                for i in 0..count {
                    if let Some(b) = buf.get(s + i) {
                        new_buf.set(i, b);
                    }
                }
                return Ok(vm.alloc_array_buffer(Arc::new(new_buf)));
            }
        }
        Err(VmError::TypeError("ArrayBuffer.prototype.slice called on non-ArrayBuffer".into()))
    });
    vm.set_property(ab_proto, "slice", ab_slice);
    vm.set_property(ab_ctor, "prototype", ab_proto);
    vm.globals.insert("ArrayBuffer".to_string(), ab_ctor);

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

    let kinds = [
        TypedArrayKind::Int8,
        TypedArrayKind::Uint8,
        TypedArrayKind::Uint8Clamped,
        TypedArrayKind::Int16,
        TypedArrayKind::Uint16,
        TypedArrayKind::Int32,
        TypedArrayKind::Uint32,
        TypedArrayKind::Float32,
        TypedArrayKind::Float64,
        TypedArrayKind::BigInt64,
        TypedArrayKind::BigUint64,
    ];
    for kind in kinds {
        register_single_typed_array(vm, kind);
    }

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
    let map_group_by = vm.alloc_native_function("groupBy", 2, |vm, _this, args| {
        let items = args.first().copied().unwrap_or(JsValue::undefined());
        let callback = args.get(1).copied().unwrap_or(JsValue::undefined());
        if !vm.is_callable(callback) {
            return Err(VmError::TypeError("Map.groupBy callback must be callable".into()));
        }
        let map_val = vm.alloc_map();
        let len_val = vm.get_property(items, "length");
        let raw_len = len_val.to_number().unwrap_or(0.0);
        if raw_len > 4_294_967_295.0 || raw_len < 0.0 || raw_len.is_infinite() {
            return Err(VmError::RangeError("Invalid array length".into()));
        }
        let len = raw_len as usize;
        if len > 100_000 {
            return Err(VmError::RangeError("Invalid array length".into()));
        }
        for i in 0..len {
            let item = vm.get_element(items, i);
            let key_val = vm.call(callback, JsValue::undefined(), &[item, JsValue::from_int32(i as i32)])?;
            let has_fn = vm.get_property(map_val, "has");
            let has_res = vm.call(has_fn, map_val, &[key_val]).unwrap_or_else(|_| JsValue::from_boolean(false));
            if has_res.is_truthy() {
                let get_fn = vm.get_property(map_val, "get");
                let arr = vm.call(get_fn, map_val, &[key_val])?;
                if let Some(ptr) = arr.as_object_ptr() {
                    let arr_obj = unsafe { &mut *(ptr as *mut JsObject) };
                    if let ObjectKind::Array(ref mut elems) = arr_obj.kind {
                        elems.push(item);
                    }
                }
            } else {
                let arr = vm.alloc_array(vec![item]);
                let set_fn = vm.get_property(map_val, "set");
                let _ = vm.call(set_fn, map_val, &[key_val, arr]);
            }
        }
        Ok(map_val)
    });
    vm.set_property(map_ctor, "groupBy", map_group_by);
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

    // ── WeakMap & WeakSet ──
    let weakmap_ctor = vm.alloc_native_function("WeakMap", 0, |vm, _this, args| {
        let wm_val = vm.alloc_map();
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
                                if !k.is_object() {
                                    return Err(crate::interpreter::VmError::TypeError("Invalid value used as weak map key".into()));
                                }
                                if let Some(m_ptr) = wm_val.as_object_ptr() {
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
        Ok(wm_val)
    });
    vm.globals.insert("WeakMap".to_string(), weakmap_ctor);

    let weakset_ctor = vm.alloc_native_function("WeakSet", 0, |vm, _this, args| {
        let ws_val = vm.alloc_set();
        if let Some(arg) = args.first() {
            if let Some(ptr) = arg.as_object_ptr() {
                let obj = unsafe { &*(ptr as *mut JsObject) };
                if let ObjectKind::Array(elements) = &obj.kind {
                    if let Some(s_ptr) = ws_val.as_object_ptr() {
                        let s_obj = unsafe { &mut *(s_ptr as *mut JsObject) };
                        if let ObjectKind::Set(set_elements) = &mut s_obj.kind {
                            for el in elements {
                                if !el.is_object() {
                                    return Err(crate::interpreter::VmError::TypeError("Invalid value used in weak set".into()));
                                }
                                if !set_elements.contains(el) {
                                    set_elements.push(*el);
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(ws_val)
    });
    vm.globals.insert("WeakSet".to_string(), weakset_ctor);

    // ── RegExp ──
    let regexp_proto = vm.alloc_object();
    vm.regexp_proto = Some(regexp_proto);

    let regexp_ctor = vm.alloc_native_function("RegExp", 2, |vm, _this, args| {
        let pattern = args.first().map(|v| vm.to_string_value(*v)).unwrap_or_default();
        let flags = args.get(1).map(|v| vm.to_string_value(*v)).unwrap_or_default();
        Ok(vm.alloc_regexp(pattern, flags))
    });
    vm.set_property(regexp_ctor, "prototype", regexp_proto);
    vm.set_property(regexp_proto, "constructor", regexp_ctor);

    let re_exec_fn = vm.alloc_native_function("exec", 1, |vm, this, args| {
        let text = args.first().map(|v| vm.to_string_value(*v)).unwrap_or_default();
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::RegExp { pattern, .. } = &obj.kind {
                if let Some(idx) = text.find(pattern) {
                    let match_str = vm.alloc_string(pattern.clone());
                    let arr_val = vm.alloc_array(vec![match_str]);
                    vm.set_property(arr_val, "index", JsValue::from_int32(idx as i32));
                    let text_val = vm.alloc_string(text);
                    vm.set_property(arr_val, "input", text_val);
                    return Ok(arr_val);
                }
            }
        }
        Ok(JsValue::null())
    });

    let re_test_fn = vm.alloc_native_function("test", 1, |vm, this, args| {
        let text = args.first().map(|v| vm.to_string_value(*v)).unwrap_or_default();
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::RegExp { pattern, .. } = &obj.kind {
                return Ok(JsValue::from_boolean(text.contains(pattern)));
            }
        }
        Ok(JsValue::from_boolean(false))
    });

    let re_to_string_fn = vm.alloc_native_function("toString", 0, |vm, this, _args| {
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::RegExp { pattern, flags } = &obj.kind {
                return Ok(vm.alloc_string(format!("/{}/{}", pattern, flags)));
            }
        }
        Ok(vm.alloc_string("/(?:)/".to_string()))
    });

    vm.set_property(regexp_proto, "exec", re_exec_fn);
    vm.set_property(regexp_proto, "test", re_test_fn);
    vm.set_property(regexp_proto, "toString", re_to_string_fn);

    vm.globals.insert("RegExp".to_string(), regexp_ctor);
    if vm.global_object.is_object() {
        vm.set_property(vm.global_object, "RegExp", regexp_ctor);
    }

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
        let resolved_opts = vm.alloc_native_function("resolvedOptions", 0, |vm, _this, _args| {
            let opts = vm.alloc_object();
            let loc = vm.alloc_string("en-US".to_string());
            vm.set_property(opts, "locale", loc);
            Ok(opts)
        });
        vm.set_property(nf_obj, "format", format_fn);
        vm.set_property(nf_obj, "resolvedOptions", resolved_opts);
        Ok(nf_obj)
    });
    let empty_locales_fn = vm.alloc_native_function("supportedLocalesOf", 1, |vm, _this, _args| {
        Ok(vm.alloc_array(Vec::new()))
    });
    vm.set_property(num_format_ctor, "supportedLocalesOf", empty_locales_fn);
    vm.set_property(intl_obj, "NumberFormat", num_format_ctor);

    let date_format_ctor = vm.alloc_native_function("DateTimeFormat", 0, |vm, _this, _args| {
        let df_obj = vm.alloc_object();
        let format_fn = vm.alloc_native_function("format", 1, |vm, _this, args| {
            let s = args.first().map(|v| vm.to_string_value(*v)).unwrap_or_else(|| "2026-10-06T00:00:00Z".to_string());
            Ok(vm.alloc_string(s))
        });
        let resolved_opts = vm.alloc_native_function("resolvedOptions", 0, |vm, _this, _args| {
            let opts = vm.alloc_object();
            let loc = vm.alloc_string("en-US".to_string());
            vm.set_property(opts, "locale", loc);
            Ok(opts)
        });
        vm.set_property(df_obj, "format", format_fn);
        vm.set_property(df_obj, "resolvedOptions", resolved_opts);
        Ok(df_obj)
    });
    let empty_locales_fn2 = vm.alloc_native_function("supportedLocalesOf", 1, |vm, _this, _args| {
        Ok(vm.alloc_array(Vec::new()))
    });
    vm.set_property(date_format_ctor, "supportedLocalesOf", empty_locales_fn2);
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
        let resolved_opts = vm.alloc_native_function("resolvedOptions", 0, |vm, _this, _args| {
            let opts = vm.alloc_object();
            let loc = vm.alloc_string("en-US".to_string());
            vm.set_property(opts, "locale", loc);
            Ok(opts)
        });
        vm.set_property(coll_obj, "compare", compare_fn);
        vm.set_property(coll_obj, "resolvedOptions", resolved_opts);
        Ok(coll_obj)
    });
    let empty_locales_fn3 = vm.alloc_native_function("supportedLocalesOf", 1, |vm, _this, _args| {
        Ok(vm.alloc_array(Vec::new()))
    });
    vm.set_property(collator_ctor, "supportedLocalesOf", empty_locales_fn3);
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

fn register_test262_harness(vm: &mut Vm) {
    let print_fn = vm.alloc_native_function("print", 1, native_console_log);
    vm.globals.insert("print".to_string(), print_fn);

    let test262_obj = vm.alloc_object();
    vm.set_property(test262_obj, "global", vm.global_object);

    let eval_script_fn = vm.alloc_native_function("evalScript", 1, |vm, _this, args| {
        let code = args.first().map(|v| vm.to_string_value(*v)).unwrap_or_default();
        let program = doms_parser::parse(&code).map_err(|e| VmError::SyntaxError(e.to_string()))?;
        let chunk = doms_compiler::compile(&program).map_err(|e| VmError::SyntaxError(e.to_string()))?;
        vm.in_eval = false;
        vm.run(&chunk)
    });
    vm.set_property(test262_obj, "evalScript", eval_script_fn);

    let gc_fn = vm.alloc_native_function("gc", 0, |vm, _this, _args| {
        vm.heap.collect_nursery();
        let _ = vm.heap.collect_old_gen_compact();
        Ok(JsValue::undefined())
    });
    vm.set_property(test262_obj, "gc", gc_fn);

    let detach_fn = vm.alloc_native_function("detachArrayBuffer", 1, |_vm, _this, _args| {
        Ok(JsValue::undefined())
    });
    vm.set_property(test262_obj, "detachArrayBuffer", detach_fn);

    let create_realm_fn = vm.alloc_native_function("createRealm", 0, |vm, _this, _args| {
        let realm_obj = vm.alloc_object();
        let glob = vm.global_object;
        vm.set_property(realm_obj, "global", glob);
        let eval_fn = vm.get_property(glob, "eval");
        vm.set_property(realm_obj, "evalScript", eval_fn);
        Ok(realm_obj)
    });
    vm.set_property(test262_obj, "createRealm", create_realm_fn);
    vm.set_property(test262_obj, "global", vm.global_object);
    let agent_obj = vm.alloc_object();
    vm.set_property(test262_obj, "agent", agent_obj);

    vm.globals.insert("$262".to_string(), test262_obj);

    let do_not_eval = vm.alloc_native_function("$DONOTEVALUATE", 0, |_vm, _this, _args| {
        Err(VmError::ReferenceError("Test262: This statement should not be evaluated.".into()))
    });
    vm.globals.insert("$DONOTEVALUATE".to_string(), do_not_eval);

    let t262_err = vm.alloc_native_function("Test262Error", 1, |vm, this, args| {
        let msg = args.first().map(|v| vm.to_string_value(*v)).unwrap_or_default();
        let target = if this.is_object() { this } else { vm.alloc_object() };
        let name_val = vm.alloc_string("Test262Error".into());
        let msg_val = vm.alloc_string(msg);
        vm.set_property(target, "name", name_val);
        vm.set_property(target, "message", msg_val);
        Ok(target)
    });
    vm.globals.insert("Test262Error".to_string(), t262_err);
}

fn register_reflect(vm: &mut Vm) {
    let reflect_obj = vm.alloc_object();

    let construct_fn = vm.alloc_native_function("construct", 2, |vm, _this, args| {
        let target = args.first().copied().unwrap_or(JsValue::undefined());
        let args_list = if let Some(arg_arr) = args.get(1) {
            if let Some(ptr) = arg_arr.as_object_ptr() {
                let obj = unsafe { &*(ptr as *mut JsObject) };
                if let ObjectKind::Array(elements) = &obj.kind {
                    elements.clone()
                } else {
                    Vec::new()
                }
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        let new_target = args.get(2).copied().unwrap_or(target);
        if !target.is_object() {
            return Err(VmError::TypeError("Target is not a constructor".into()));
        }
        let this_obj = vm.alloc_object();
        let proto = vm.get_property(new_target, "prototype");
        if proto.is_object() {
            let this_ptr = this_obj.as_object_ptr().unwrap();
            let jsobj = unsafe { &mut *(this_ptr as *mut JsObject) };
            jsobj.prototype = Some(proto);
        }
        let res = vm.call(target, this_obj, &args_list)?;
        if res.is_object() {
            Ok(res)
        } else {
            Ok(this_obj)
        }
    });
    vm.set_property(reflect_obj, "construct", construct_fn);

    let apply_fn = vm.alloc_native_function("apply", 3, |vm, _this, args| {
        let target = args.first().copied().unwrap_or(JsValue::undefined());
        let this_arg = args.get(1).copied().unwrap_or(JsValue::undefined());
        let args_list = if let Some(arg_arr) = args.get(2) {
            if let Some(ptr) = arg_arr.as_object_ptr() {
                let obj = unsafe { &*(ptr as *mut JsObject) };
                if let ObjectKind::Array(elements) = &obj.kind {
                    elements.clone()
                } else {
                    Vec::new()
                }
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };
        vm.call(target, this_arg, &args_list)
    });
    vm.set_property(reflect_obj, "apply", apply_fn);

    let get_fn = vm.alloc_native_function("get", 2, |vm, _this, args| {
        let target = args.first().copied().unwrap_or(JsValue::undefined());
        let key = args.get(1).map(|v| vm.to_string_value(*v)).unwrap_or_default();
        Ok(vm.get_property(target, &key))
    });
    vm.set_property(reflect_obj, "get", get_fn);

    let set_fn = vm.alloc_native_function("set", 3, |vm, _this, args| {
        let target = args.first().copied().unwrap_or(JsValue::undefined());
        let key = args.get(1).map(|v| vm.to_string_value(*v)).unwrap_or_default();
        let val = args.get(2).copied().unwrap_or(JsValue::undefined());
        vm.set_property(target, &key, val);
        Ok(JsValue::from_boolean(true))
    });
    vm.set_property(reflect_obj, "set", set_fn);

    let has_fn = vm.alloc_native_function("has", 2, |vm, _this, args| {
        let target = args.first().copied().unwrap_or(JsValue::undefined());
        let key = args.get(1).map(|v| vm.to_string_value(*v)).unwrap_or_default();
        if let Some(ptr) = target.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            Ok(JsValue::from_boolean(vm.shapes.has_property(obj.shape_id, &key)))
        } else {
            Ok(JsValue::from_boolean(false))
        }
    });
    vm.set_property(reflect_obj, "has", has_fn);

    vm.globals.insert("Reflect".to_string(), reflect_obj);
}

fn register_proxy(vm: &mut Vm) {
    let proxy_ctor = vm.alloc_native_function("Proxy", 2, |vm, _this, args| {
        if !vm.is_constructing {
            return Err(VmError::TypeError("Constructor Proxy requires 'new'".into()));
        }
        let target = args.first().copied().unwrap_or(JsValue::undefined());
        let handler = args.get(1).copied().unwrap_or(JsValue::undefined());
        if !target.is_object() || !handler.is_object() {
            return Err(VmError::TypeError("Cannot create proxy with a non-object target or handler".into()));
        }
        Ok(vm.alloc_proxy(target, handler))
    });
    let revocable_fn = vm.alloc_native_function("revocable", 2, |vm, _this, args| {
        let target = args.first().copied().unwrap_or(JsValue::undefined());
        let handler = args.get(1).copied().unwrap_or(JsValue::undefined());
        if !target.is_object() || !handler.is_object() {
            return Err(VmError::TypeError("Cannot create proxy with a non-object target or handler".into()));
        }
        let proxy = vm.alloc_proxy(target, handler);
        let revoke = vm.alloc_native_function("revoke", 0, |_vm, _this, _args| {
            Ok(JsValue::undefined())
        });
        let result = vm.alloc_object();
        vm.set_property(result, "proxy", proxy);
        vm.set_property(result, "revoke", revoke);
        Ok(result)
    });
    vm.set_property(proxy_ctor, "revocable", revocable_fn);
    vm.globals.insert("Proxy".to_string(), proxy_ctor);
    if vm.global_object.is_object() {
        vm.set_property(vm.global_object, "Proxy", proxy_ctor);
    }
}

