//! The `Math` global object.

use super::*;

pub(super) fn register_math(vm: &mut Vm) {
    let math_obj = vm.alloc_object();

    let abs_fn = vm.alloc_native_function("abs", 1, |vm, _this, args| {
        let n = args.first().and_then(|v| vm.to_number_value(*v).ok()).unwrap_or(f64::NAN);
        Ok(JsValue::from_double(n.abs()))
    });
    let floor_fn = vm.alloc_native_function("floor", 1, |vm, _this, args| {
        let n = args.first().and_then(|v| vm.to_number_value(*v).ok()).unwrap_or(f64::NAN);
        Ok(JsValue::from_double(n.floor()))
    });
    let ceil_fn = vm.alloc_native_function("ceil", 1, |vm, _this, args| {
        let n = args.first().and_then(|v| vm.to_number_value(*v).ok()).unwrap_or(f64::NAN);
        Ok(JsValue::from_double(n.ceil()))
    });
    let round_fn = vm.alloc_native_function("round", 1, |vm, _this, args| {
        let n = args.first().and_then(|v| vm.to_number_value(*v).ok()).unwrap_or(f64::NAN);
        Ok(JsValue::from_double(n.round()))
    });
    let sqrt_fn = vm.alloc_native_function("sqrt", 1, |vm, _this, args| {
        let n = args.first().and_then(|v| vm.to_number_value(*v).ok()).unwrap_or(f64::NAN);
        Ok(JsValue::from_double(n.sqrt()))
    });
    let pow_fn = vm.alloc_native_function("pow", 2, |vm, _this, args| {
        let base = args.first().and_then(|v| vm.to_number_value(*v).ok()).unwrap_or(f64::NAN);
        let exp = args.get(1).and_then(|v| vm.to_number_value(*v).ok()).unwrap_or(f64::NAN);
        Ok(JsValue::from_double(crate::interpreter::js_exponentiate(base, exp)))
    });
    let min_fn = vm.alloc_native_function("min", 2, |vm, _this, args| {
        let mut min = f64::INFINITY;
        for a in args {
            let n = vm.to_number_value(*a).unwrap_or(f64::NAN);
            if n.is_nan() {
                return Ok(JsValue::from_double(f64::NAN));
            }
            if n < min {
                min = n;
            }
        }
        Ok(JsValue::from_double(min))
    });
    let max_fn = vm.alloc_native_function("max", 2, |vm, _this, args| {
        let mut max = f64::NEG_INFINITY;
        for a in args {
            let n = vm.to_number_value(*a).unwrap_or(f64::NAN);
            if n.is_nan() {
                return Ok(JsValue::from_double(f64::NAN));
            }
            if n > max {
                max = n;
            }
        }
        Ok(JsValue::from_double(max))
    });
    let random_fn = vm.alloc_native_function("random", 0, |_vm, _this, _args| {
        static mut SEED: u64 = 123456789;
        let rand_val = unsafe {
            SEED = SEED.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (SEED >> 11) as f64 / (1u64 << 53) as f64
        };
        Ok(JsValue::from_double(rand_val))
    });

    let sin_fn = vm.alloc_native_function("sin", 1, |vm, _this, args| {
        let n = args.first().and_then(|v| vm.to_number_value(*v).ok()).unwrap_or(f64::NAN);
        Ok(JsValue::from_double(n.sin()))
    });
    let cos_fn = vm.alloc_native_function("cos", 1, |vm, _this, args| {
        let n = args.first().and_then(|v| vm.to_number_value(*v).ok()).unwrap_or(f64::NAN);
        Ok(JsValue::from_double(n.cos()))
    });
    let tan_fn = vm.alloc_native_function("tan", 1, |vm, _this, args| {
        let n = args.first().and_then(|v| vm.to_number_value(*v).ok()).unwrap_or(f64::NAN);
        Ok(JsValue::from_double(n.tan()))
    });
    let asin_fn = vm.alloc_native_function("asin", 1, |vm, _this, args| {
        let n = args.first().and_then(|v| vm.to_number_value(*v).ok()).unwrap_or(f64::NAN);
        Ok(JsValue::from_double(n.asin()))
    });
    let acos_fn = vm.alloc_native_function("acos", 1, |vm, _this, args| {
        let n = args.first().and_then(|v| vm.to_number_value(*v).ok()).unwrap_or(f64::NAN);
        Ok(JsValue::from_double(n.acos()))
    });
    let atan_fn = vm.alloc_native_function("atan", 1, |vm, _this, args| {
        let n = args.first().and_then(|v| vm.to_number_value(*v).ok()).unwrap_or(f64::NAN);
        Ok(JsValue::from_double(n.atan()))
    });
    let atan2_fn = vm.alloc_native_function("atan2", 2, |vm, _this, args| {
        let y = args.first().and_then(|v| vm.to_number_value(*v).ok()).unwrap_or(f64::NAN);
        let x = args.get(1).and_then(|v| vm.to_number_value(*v).ok()).unwrap_or(f64::NAN);
        Ok(JsValue::from_double(y.atan2(x)))
    });

    let log_fn = vm.alloc_native_function("log", 1, |vm, _this, args| {
        let n = args.first().and_then(|v| vm.to_number_value(*v).ok()).unwrap_or(f64::NAN);
        Ok(JsValue::from_double(n.ln()))
    });
    let log10_fn = vm.alloc_native_function("log10", 1, |vm, _this, args| {
        let n = args.first().and_then(|v| vm.to_number_value(*v).ok()).unwrap_or(f64::NAN);
        Ok(JsValue::from_double(n.log10()))
    });
    let log2_fn = vm.alloc_native_function("log2", 1, |vm, _this, args| {
        let n = args.first().and_then(|v| vm.to_number_value(*v).ok()).unwrap_or(f64::NAN);
        Ok(JsValue::from_double(n.log2()))
    });
    let log1p_fn = vm.alloc_native_function("log1p", 1, |vm, _this, args| {
        let n = args.first().and_then(|v| vm.to_number_value(*v).ok()).unwrap_or(f64::NAN);
        Ok(JsValue::from_double(n.ln_1p()))
    });
    let exp_fn = vm.alloc_native_function("exp", 1, |vm, _this, args| {
        let n = args.first().and_then(|v| vm.to_number_value(*v).ok()).unwrap_or(f64::NAN);
        Ok(JsValue::from_double(n.exp()))
    });
    let expm1_fn = vm.alloc_native_function("expm1", 1, |vm, _this, args| {
        let n = args.first().and_then(|v| vm.to_number_value(*v).ok()).unwrap_or(f64::NAN);
        Ok(JsValue::from_double(n.exp_m1()))
    });

    let trunc_fn = vm.alloc_native_function("trunc", 1, |vm, _this, args| {
        let n = args.first().and_then(|v| vm.to_number_value(*v).ok()).unwrap_or(f64::NAN);
        Ok(JsValue::from_double(n.trunc()))
    });
    let sign_fn = vm.alloc_native_function("sign", 1, |vm, _this, args| {
        let n = args.first().and_then(|v| vm.to_number_value(*v).ok()).unwrap_or(f64::NAN);
        if n.is_nan() {
            Ok(JsValue::from_double(f64::NAN))
        } else if n == 0.0 {
            Ok(JsValue::from_double(n))
        } else if n > 0.0 {
            Ok(JsValue::from_double(1.0))
        } else {
            Ok(JsValue::from_double(-1.0))
        }
    });
    let cbrt_fn = vm.alloc_native_function("cbrt", 1, |vm, _this, args| {
        let n = args.first().and_then(|v| vm.to_number_value(*v).ok()).unwrap_or(f64::NAN);
        Ok(JsValue::from_double(n.cbrt()))
    });
    let hypot_fn = vm.alloc_native_function("hypot", 2, |vm, _this, args| {
        let mut sum_sq = 0.0;
        for a in args {
            let n = vm.to_number_value(*a).unwrap_or(f64::NAN);
            if n.is_infinite() {
                return Ok(JsValue::from_double(f64::INFINITY));
            }
            if n.is_nan() {
                return Ok(JsValue::from_double(f64::NAN));
            }
            sum_sq += n * n;
        }
        Ok(JsValue::from_double(sum_sq.sqrt()))
    });
    let clz32_fn = vm.alloc_native_function("clz32", 1, |vm, _this, args| {
        let n = args.first().and_then(|v| vm.to_number_value(*v).ok()).unwrap_or(0.0);
        let u = n as u32;
        Ok(JsValue::from_int32(u.leading_zeros() as i32))
    });
    let imul_fn = vm.alloc_native_function("imul", 2, |vm, _this, args| {
        let a = args.first().and_then(|v| vm.to_number_value(*v).ok()).unwrap_or(0.0) as u32;
        let b = args.get(1).and_then(|v| vm.to_number_value(*v).ok()).unwrap_or(0.0) as u32;
        let res = a.wrapping_mul(b) as i32;
        Ok(JsValue::from_int32(res))
    });
    let fround_fn = vm.alloc_native_function("fround", 1, |vm, _this, args| {
        let n = args.first().and_then(|v| vm.to_number_value(*v).ok()).unwrap_or(f64::NAN);
        Ok(JsValue::from_double((n as f32) as f64))
    });

    let methods = [
        ("abs", abs_fn),
        ("floor", floor_fn),
        ("ceil", ceil_fn),
        ("round", round_fn),
        ("sqrt", sqrt_fn),
        ("pow", pow_fn),
        ("min", min_fn),
        ("max", max_fn),
        ("random", random_fn),
        ("sin", sin_fn),
        ("cos", cos_fn),
        ("tan", tan_fn),
        ("asin", asin_fn),
        ("acos", acos_fn),
        ("atan", atan_fn),
        ("atan2", atan2_fn),
        ("log", log_fn),
        ("log10", log10_fn),
        ("log2", log2_fn),
        ("log1p", log1p_fn),
        ("exp", exp_fn),
        ("expm1", expm1_fn),
        ("trunc", trunc_fn),
        ("sign", sign_fn),
        ("cbrt", cbrt_fn),
        ("hypot", hypot_fn),
        ("clz32", clz32_fn),
        ("imul", imul_fn),
        ("fround", fround_fn),
    ];

    for (name, func) in methods {
        vm.set_property(math_obj, name, func);
        vm.set_property_non_enumerable(math_obj, name.to_string());
    }

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
        vm.set_property_non_writable(math_obj, name.to_string());
        vm.set_property_non_enumerable(math_obj, name.to_string());
        vm.set_property_non_configurable(math_obj, name.to_string());
    }

    if let Some(tag_sym) = vm.symbol_to_string_tag {
        let tag_val = vm.alloc_string("Math".into());
        vm.set_symbol_property(math_obj, tag_sym, tag_val);
        let key_name = format!("[Symbol:{:x}]", tag_sym.raw_bits());
        vm.set_property_non_writable(math_obj, key_name.clone());
        vm.set_property_non_enumerable(math_obj, key_name.clone());
        vm.set_property_non_configurable(math_obj, key_name);
    }

    vm.globals.insert("Math".to_string(), math_obj);
    if vm.global_object.is_object() {
        vm.set_property(vm.global_object, "Math", math_obj);
    }
}
