//! The `Number` global object and its constants.

use super::*;

pub(super) fn register_number_global(vm: &mut Vm) {
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

    let num_to_locale_string = vm.alloc_native_function("toLocaleString", 0, |vm, this, _args| {
        let n = if this.is_number() {
            this.to_number().unwrap_or(0.0)
        } else if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::NumberObject(num) = obj.kind {
                num
            } else {
                return Err(VmError::TypeError("Number.prototype.toLocaleString requires that 'this' be a Number".into()));
            }
        } else {
            return Err(VmError::TypeError("Number.prototype.toLocaleString requires that 'this' be a Number".into()));
        };
        Ok(vm.alloc_string(vm.to_string_value(JsValue::from_double(n))))
    });

    let num_to_fixed = vm.alloc_native_function("toFixed", 1, |vm, this, args| {
        let n = if this.is_number() {
            this.to_number().unwrap_or(0.0)
        } else if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::NumberObject(num) = obj.kind {
                num
            } else {
                return Err(VmError::TypeError("Number.prototype.toFixed requires that 'this' be a Number".into()));
            }
        } else {
            return Err(VmError::TypeError("Number.prototype.toFixed requires that 'this' be a Number".into()));
        };
        let frac = args.first().and_then(|v| v.to_int32()).unwrap_or(0);
        if !(0..=100).contains(&frac) {
            return Err(VmError::RangeError("toFixed() digits argument must be between 0 and 100".into()));
        }
        if n.is_nan() || n.is_infinite() {
            return Ok(vm.alloc_string(vm.to_string_value(JsValue::from_double(n))));
        }
        let s = format!("{:.*}", frac as usize, n);
        Ok(vm.alloc_string(s))
    });

    let num_to_exponential = vm.alloc_native_function("toExponential", 1, |vm, this, args| {
        let n = if this.is_number() {
            this.to_number().unwrap_or(0.0)
        } else if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::NumberObject(num) = obj.kind {
                num
            } else {
                return Err(VmError::TypeError("Number.prototype.toExponential requires that 'this' be a Number".into()));
            }
        } else {
            return Err(VmError::TypeError("Number.prototype.toExponential requires that 'this' be a Number".into()));
        };
        let frac = args.first().and_then(|v| v.to_int32()).unwrap_or(0);
        if !(0..=100).contains(&frac) {
            return Err(VmError::RangeError("toExponential() argument must be between 0 and 100".into()));
        }
        if n.is_nan() || n.is_infinite() {
            return Ok(vm.alloc_string(vm.to_string_value(JsValue::from_double(n))));
        }
        let s = format!("{:.*e}", frac as usize, n);
        Ok(vm.alloc_string(s))
    });

    let num_to_precision = vm.alloc_native_function("toPrecision", 1, |vm, this, args| {
        let n = if this.is_number() {
            this.to_number().unwrap_or(0.0)
        } else if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::NumberObject(num) = obj.kind {
                num
            } else {
                return Err(VmError::TypeError("Number.prototype.toPrecision requires that 'this' be a Number".into()));
            }
        } else {
            return Err(VmError::TypeError("Number.prototype.toPrecision requires that 'this' be a Number".into()));
        };
        let prec = args.first().and_then(|v| v.to_int32()).unwrap_or(0);
        if prec == 0 {
            return Ok(vm.alloc_string(vm.to_string_value(JsValue::from_double(n))));
        }
        if !(1..=100).contains(&prec) {
            return Err(VmError::RangeError("toPrecision() argument must be between 1 and 100".into()));
        }
        if n.is_nan() || n.is_infinite() {
            return Ok(vm.alloc_string(vm.to_string_value(JsValue::from_double(n))));
        }
        let s = format!("{:.*}", prec as usize, n);
        Ok(vm.alloc_string(s))
    });

    for (m, f) in [
        ("valueOf", num_value_of),
        ("toString", num_to_string),
        ("toLocaleString", num_to_locale_string),
        ("toFixed", num_to_fixed),
        ("toExponential", num_to_exponential),
        ("toPrecision", num_to_precision),
    ] {
        vm.set_property(number_proto, m, f);
        vm.set_property_non_enumerable(number_proto, m.to_string());
    }

    let num_ctor = vm.alloc_native_function("Number", 1, |vm, _this, args| {
        let n = if let Some(arg) = args.first() {
            vm.to_number_value(*arg).unwrap_or(f64::NAN)
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
    vm.set_property_non_enumerable(number_proto, "constructor".to_string());
    vm.set_property(num_ctor, "prototype", number_proto);
    vm.set_property_non_enumerable(num_ctor, "prototype".to_string());
    vm.set_property_non_writable(num_ctor, "prototype".to_string());
    vm.set_property_non_configurable(num_ctor, "prototype".to_string());

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
        vm.set_property_non_writable(num_ctor, name.to_string());
        vm.set_property_non_enumerable(num_ctor, name.to_string());
        vm.set_property_non_configurable(num_ctor, name.to_string());
    }
    vm.globals.insert("Number".to_string(), num_ctor);
    if vm.global_object.is_object() {
        vm.set_property(vm.global_object, "Number", num_ctor);
    }
}
