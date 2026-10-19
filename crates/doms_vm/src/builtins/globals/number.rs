//! The `Number` global object and its constants.

use super::*;

fn this_number_value(this: JsValue) -> Result<f64, VmError> {
    if this.is_number() {
        return Ok(this.to_number().unwrap_or(0.0));
    }
    if let Some(ptr) = this.as_object_ptr() {
        let obj = unsafe { &*(ptr as *mut JsObject) };
        if let ObjectKind::NumberObject(n) = obj.kind {
            return Ok(n);
        }
    }
    Err(VmError::TypeError("Number method called on non-number".into()))
}

fn format_exponential(n: f64, fraction_digits: Option<usize>) -> String {
    if n.is_nan() {
        return "NaN".to_string();
    }
    if n.is_infinite() {
        return if n.is_sign_negative() { "-Infinity".to_string() } else { "Infinity".to_string() };
    }
    // ECMAScript: If x is -0, let x be +0.
    let is_neg = n.is_sign_negative() && n != 0.0;
    let abs_n = n.abs();

    if abs_n == 0.0 {
        let frac_str = match fraction_digits {
            Some(f) if f > 0 => format!(".{}", "0".repeat(f)),
            _ => String::new(),
        };
        return format!("0{}e+0", frac_str);
    }

    if let Some(f) = fraction_digits {
        let mut exp = abs_n.log10().floor() as i32;
        let mut scaled = abs_n / 10f64.powi(exp);
        if scaled >= 10.0 {
            scaled /= 10.0;
            exp += 1;
        } else if scaled < 1.0 {
            scaled *= 10.0;
            exp -= 1;
        }
        let factor = 10f64.powi(f as i32);
        let mut rounded = (scaled * factor).round() / factor;
        if rounded >= 10.0 {
            rounded /= 10.0;
            exp += 1;
        }
        let prefix = if is_neg { "-" } else { "" };
        let sign = if exp >= 0 { "+" } else { "-" };
        format!("{}{:.*}e{}{}", prefix, f, rounded, sign, exp.abs())
    } else {
        let raw = format!("{:e}", abs_n);
        let prefix = if is_neg { "-" } else { "" };
        if let Some((mantissa, exp_str)) = raw.split_once('e') {
            let exp: i32 = exp_str.parse().unwrap_or(0);
            let sign = if exp >= 0 { "+" } else { "-" };
            format!("{}{}{}e{}{}", prefix, mantissa, "", sign, exp.abs())
        } else {
            format!("{}{}", prefix, raw)
        }
    }
}

pub(super) fn register_number_global(vm: &mut Vm) {
    let number_proto = vm.alloc_number_object(0.0, vm.object_proto);
    vm.number_proto = Some(number_proto);

    let num_value_of = vm.alloc_native_function("valueOf", 0, |_vm, this, _args| {
        let n = this_number_value(this)?;
        Ok(JsValue::from_double(n))
    });

    let num_to_string = vm.alloc_native_function("toString", 1, |vm, this, args| {
        let n = this_number_value(this)?;
        let radix_arg = args.first().copied().unwrap_or(JsValue::undefined());
        let radix = if radix_arg.is_undefined() {
            10
        } else {
            let r = vm.to_integer_or_infinity(radix_arg)?;
            r as i32
        };
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
        let n = this_number_value(this)?;
        Ok(vm.alloc_string(vm.to_string_value(JsValue::from_double(n))))
    });

    let num_to_fixed = vm.alloc_native_function("toFixed", 1, |vm, this, args| {
        let n = this_number_value(this)?;
        let frac_val = args.first().copied().unwrap_or(JsValue::undefined());
        let f = vm.to_integer_or_infinity(frac_val)?;
        if f < 0.0 || f > 100.0 {
            return Err(VmError::RangeError("toFixed() digits argument must be between 0 and 100".into()));
        }
        if n.is_nan() || n.is_infinite() {
            return Ok(vm.alloc_string(vm.to_string_value(JsValue::from_double(n))));
        }
        if n.abs() >= 1e21 {
            return Ok(vm.alloc_string(vm.to_string_value(JsValue::from_double(n))));
        }
        let s = format!("{:.*}", f as usize, n);
        Ok(vm.alloc_string(s))
    });

    let num_to_exponential = vm.alloc_native_function("toExponential", 1, |vm, this, args| {
        let n = this_number_value(this)?;
        let frac_arg = args.first().copied().unwrap_or(JsValue::undefined());
        let frac_opt = if frac_arg.is_undefined() {
            None
        } else {
            let f = vm.to_integer_or_infinity(frac_arg)?;
            if f < 0.0 || f > 100.0 {
                return Err(VmError::RangeError("toExponential() argument must be between 0 and 100".into()));
            }
            Some(f as usize)
        };
        if n.is_nan() || n.is_infinite() {
            return Ok(vm.alloc_string(vm.to_string_value(JsValue::from_double(n))));
        }
        let s = format_exponential(n, frac_opt);
        Ok(vm.alloc_string(s))
    });

    let num_to_precision = vm.alloc_native_function("toPrecision", 1, |vm, this, args| {
        let n = this_number_value(this)?;
        let prec_arg = args.first().copied().unwrap_or(JsValue::undefined());
        if prec_arg.is_undefined() {
            return Ok(vm.alloc_string(vm.to_string_value(JsValue::from_double(n))));
        }
        let p = vm.to_integer_or_infinity(prec_arg)?;
        if p < 1.0 || p > 100.0 {
            return Err(VmError::RangeError("toPrecision() argument must be between 1 and 100".into()));
        }
        if n.is_nan() || n.is_infinite() {
            return Ok(vm.alloc_string(vm.to_string_value(JsValue::from_double(n))));
        }
        let p_usize = p as usize;
        let abs_n = n.abs();
        if abs_n == 0.0 {
            let frac = if p_usize > 1 { format!(".{}", "0".repeat(p_usize - 1)) } else { String::new() };
            return Ok(vm.alloc_string(format!("0{}", frac)));
        }
        let e = abs_n.log10().floor() as i32;
        if e < -6 || e >= p as i32 {
            let s = format_exponential(n, Some(p_usize - 1));
            return Ok(vm.alloc_string(s));
        }
        let frac_digits = if e < 0 {
            (p as i32 - 1 - e) as usize
        } else {
            (p_usize).saturating_sub((e + 1) as usize)
        };
        let s = format!("{:.*}", frac_digits, n);
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
            vm.to_number_value(*arg)?
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

    let num_is_finite = vm.alloc_native_function("isFinite", 1, |_vm, _this, args| {
        let arg = args.first().copied().unwrap_or(JsValue::undefined());
        if !arg.is_number() {
            return Ok(JsValue::from_boolean(false));
        }
        let n = arg.to_number().unwrap();
        Ok(JsValue::from_boolean(!n.is_nan() && !n.is_infinite()))
    });
    vm.set_property(num_ctor, "isFinite", num_is_finite);
    vm.set_property_non_enumerable(num_ctor, "isFinite".to_string());

    let num_is_integer = vm.alloc_native_function("isInteger", 1, |_vm, _this, args| {
        let arg = args.first().copied().unwrap_or(JsValue::undefined());
        if !arg.is_number() {
            return Ok(JsValue::from_boolean(false));
        }
        let n = arg.to_number().unwrap();
        if n.is_nan() || n.is_infinite() {
            return Ok(JsValue::from_boolean(false));
        }
        Ok(JsValue::from_boolean(n.fract() == 0.0))
    });
    vm.set_property(num_ctor, "isInteger", num_is_integer);
    vm.set_property_non_enumerable(num_ctor, "isInteger".to_string());

    let num_is_nan = vm.alloc_native_function("isNaN", 1, |_vm, _this, args| {
        let arg = args.first().copied().unwrap_or(JsValue::undefined());
        if !arg.is_number() {
            return Ok(JsValue::from_boolean(false));
        }
        let n = arg.to_number().unwrap();
        Ok(JsValue::from_boolean(n.is_nan()))
    });
    vm.set_property(num_ctor, "isNaN", num_is_nan);
    vm.set_property_non_enumerable(num_ctor, "isNaN".to_string());

    let num_is_safe_integer = vm.alloc_native_function("isSafeInteger", 1, |_vm, _this, args| {
        let arg = args.first().copied().unwrap_or(JsValue::undefined());
        if !arg.is_number() {
            return Ok(JsValue::from_boolean(false));
        }
        let n = arg.to_number().unwrap();
        if n.is_nan() || n.is_infinite() || n.fract() != 0.0 {
            return Ok(JsValue::from_boolean(false));
        }
        Ok(JsValue::from_boolean(n.abs() <= 9007199254740991.0))
    });
    vm.set_property(num_ctor, "isSafeInteger", num_is_safe_integer);
    vm.set_property_non_enumerable(num_ctor, "isSafeInteger".to_string());

    if let Some(&p_int) = vm.globals.get("parseInt") {
        vm.set_property(num_ctor, "parseInt", p_int);
        vm.set_property_non_enumerable(num_ctor, "parseInt".to_string());
    }
    if let Some(&p_float) = vm.globals.get("parseFloat") {
        vm.set_property(num_ctor, "parseFloat", p_float);
        vm.set_property_non_enumerable(num_ctor, "parseFloat".to_string());
    }

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
