//! The `Math` global object.

use super::*;

#[inline]
fn to_uint32(n: f64) -> u32 {
    if n.is_nan() || n.is_infinite() || n == 0.0 { 0 } else { n.rem_euclid(4294967296.0) as u32 }
}

pub(super) fn register_math(vm: &mut Vm) {
    let math_obj = vm.alloc_object();

    let abs_fn = vm.alloc_native_function("abs", 1, |vm, _this, args| {
        let n = match args.first() {
            Some(v) => vm.to_number_value(*v)?,
            None => f64::NAN,
        };
        Ok(JsValue::from_double(n.abs()))
    });
    let floor_fn = vm.alloc_native_function("floor", 1, |vm, _this, args| {
        let n = match args.first() {
            Some(v) => vm.to_number_value(*v)?,
            None => f64::NAN,
        };
        Ok(JsValue::from_double(n.floor()))
    });
    let ceil_fn = vm.alloc_native_function("ceil", 1, |vm, _this, args| {
        let n = match args.first() {
            Some(v) => vm.to_number_value(*v)?,
            None => f64::NAN,
        };
        Ok(JsValue::from_double(n.ceil()))
    });
    let round_fn = vm.alloc_native_function("round", 1, |vm, _this, args| {
        let n = match args.first() {
            Some(v) => vm.to_number_value(*v)?,
            None => f64::NAN,
        };
        if n.is_nan() || n.is_infinite() {
            return Ok(JsValue::from_double(n));
        }
        if n == 0.0 {
            return Ok(JsValue::from_double(n));
        }
        if n < 0.0 && n >= -0.5 {
            return Ok(JsValue::from_double(-0.0));
        }
        let floor = (n + 0.5).floor();
        if floor == 0.0 && n < 0.0 {
            return Ok(JsValue::from_double(-0.0));
        }
        Ok(JsValue::from_double(floor))
    });
    let sqrt_fn = vm.alloc_native_function("sqrt", 1, |vm, _this, args| {
        let n = match args.first() {
            Some(v) => vm.to_number_value(*v)?,
            None => f64::NAN,
        };
        Ok(JsValue::from_double(n.sqrt()))
    });
    let pow_fn = vm.alloc_native_function("pow", 2, |vm, _this, args| {
        let base = match args.first() {
            Some(v) => vm.to_number_value(*v)?,
            None => f64::NAN,
        };
        let exp = match args.get(1) {
            Some(v) => vm.to_number_value(*v)?,
            None => f64::NAN,
        };
        Ok(JsValue::from_double(crate::interpreter::js_exponentiate(base, exp)))
    });
    let min_fn = vm.alloc_native_function("min", 2, |vm, _this, args| {
        let mut numbers = Vec::with_capacity(args.len());
        for a in args {
            numbers.push(vm.to_number_value(*a)?);
        }
        let mut min = f64::INFINITY;
        for n in numbers {
            if n.is_nan() {
                return Ok(JsValue::from_double(f64::NAN));
            }
            if n == 0.0 && min == 0.0 {
                if (1.0 / n) < (1.0 / min) {
                    min = n;
                }
            } else if n < min {
                min = n;
            }
        }
        Ok(JsValue::from_double(min))
    });
    let max_fn = vm.alloc_native_function("max", 2, |vm, _this, args| {
        let mut numbers = Vec::with_capacity(args.len());
        for a in args {
            numbers.push(vm.to_number_value(*a)?);
        }
        let mut max = f64::NEG_INFINITY;
        for n in numbers {
            if n.is_nan() {
                return Ok(JsValue::from_double(f64::NAN));
            }
            if n == 0.0 && max == 0.0 {
                if (1.0 / n) > (1.0 / max) {
                    max = n;
                }
            } else if n > max {
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
        let n = match args.first() {
            Some(v) => vm.to_number_value(*v)?,
            None => f64::NAN,
        };
        Ok(JsValue::from_double(n.sin()))
    });
    let cos_fn = vm.alloc_native_function("cos", 1, |vm, _this, args| {
        let n = match args.first() {
            Some(v) => vm.to_number_value(*v)?,
            None => f64::NAN,
        };
        Ok(JsValue::from_double(n.cos()))
    });
    let tan_fn = vm.alloc_native_function("tan", 1, |vm, _this, args| {
        let n = match args.first() {
            Some(v) => vm.to_number_value(*v)?,
            None => f64::NAN,
        };
        Ok(JsValue::from_double(n.tan()))
    });
    let asin_fn = vm.alloc_native_function("asin", 1, |vm, _this, args| {
        let n = match args.first() {
            Some(v) => vm.to_number_value(*v)?,
            None => f64::NAN,
        };
        Ok(JsValue::from_double(n.asin()))
    });
    let acos_fn = vm.alloc_native_function("acos", 1, |vm, _this, args| {
        let n = match args.first() {
            Some(v) => vm.to_number_value(*v)?,
            None => f64::NAN,
        };
        Ok(JsValue::from_double(n.acos()))
    });
    let atan_fn = vm.alloc_native_function("atan", 1, |vm, _this, args| {
        let n = match args.first() {
            Some(v) => vm.to_number_value(*v)?,
            None => f64::NAN,
        };
        Ok(JsValue::from_double(n.atan()))
    });
    let atan2_fn = vm.alloc_native_function("atan2", 2, |vm, _this, args| {
        let y = match args.first() {
            Some(v) => vm.to_number_value(*v)?,
            None => f64::NAN,
        };
        let x = match args.get(1) {
            Some(v) => vm.to_number_value(*v)?,
            None => f64::NAN,
        };
        Ok(JsValue::from_double(y.atan2(x)))
    });

    let log_fn = vm.alloc_native_function("log", 1, |vm, _this, args| {
        let n = match args.first() {
            Some(v) => vm.to_number_value(*v)?,
            None => f64::NAN,
        };
        Ok(JsValue::from_double(n.ln()))
    });
    let log10_fn = vm.alloc_native_function("log10", 1, |vm, _this, args| {
        let n = match args.first() {
            Some(v) => vm.to_number_value(*v)?,
            None => f64::NAN,
        };
        Ok(JsValue::from_double(n.log10()))
    });
    let log2_fn = vm.alloc_native_function("log2", 1, |vm, _this, args| {
        let n = match args.first() {
            Some(v) => vm.to_number_value(*v)?,
            None => f64::NAN,
        };
        Ok(JsValue::from_double(n.log2()))
    });
    let log1p_fn = vm.alloc_native_function("log1p", 1, |vm, _this, args| {
        let n = match args.first() {
            Some(v) => vm.to_number_value(*v)?,
            None => f64::NAN,
        };
        Ok(JsValue::from_double(n.ln_1p()))
    });
    let exp_fn = vm.alloc_native_function("exp", 1, |vm, _this, args| {
        let n = match args.first() {
            Some(v) => vm.to_number_value(*v)?,
            None => f64::NAN,
        };
        Ok(JsValue::from_double(n.exp()))
    });
    let expm1_fn = vm.alloc_native_function("expm1", 1, |vm, _this, args| {
        let n = match args.first() {
            Some(v) => vm.to_number_value(*v)?,
            None => f64::NAN,
        };
        Ok(JsValue::from_double(n.exp_m1()))
    });

    let trunc_fn = vm.alloc_native_function("trunc", 1, |vm, _this, args| {
        let n = match args.first() {
            Some(v) => vm.to_number_value(*v)?,
            None => f64::NAN,
        };
        Ok(JsValue::from_double(n.trunc()))
    });
    let sign_fn = vm.alloc_native_function("sign", 1, |vm, _this, args| {
        let n = match args.first() {
            Some(v) => vm.to_number_value(*v)?,
            None => f64::NAN,
        };
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
        let n = match args.first() {
            Some(v) => vm.to_number_value(*v)?,
            None => f64::NAN,
        };
        Ok(JsValue::from_double(n.cbrt()))
    });
    let hypot_fn = vm.alloc_native_function("hypot", 2, |vm, _this, args| {
        let mut numbers = Vec::with_capacity(args.len());
        for a in args {
            numbers.push(vm.to_number_value(*a)?);
        }
        if numbers.iter().any(|n| n.is_infinite()) {
            return Ok(JsValue::from_double(f64::INFINITY));
        }
        if numbers.iter().any(|n| n.is_nan()) {
            return Ok(JsValue::from_double(f64::NAN));
        }
        let mut sum_sq = 0.0;
        for n in numbers {
            sum_sq += n * n;
        }
        Ok(JsValue::from_double(sum_sq.sqrt()))
    });
    let clz32_fn = vm.alloc_native_function("clz32", 1, |vm, _this, args| {
        let n = match args.first() {
            Some(v) => vm.to_number_value(*v)?,
            None => f64::NAN,
        };
        let u = to_uint32(n);
        Ok(JsValue::from_int32(u.leading_zeros() as i32))
    });
    let imul_fn = vm.alloc_native_function("imul", 2, |vm, _this, args| {
        let a_num = match args.first() {
            Some(v) => vm.to_number_value(*v)?,
            None => 0.0,
        };
        let b_num = match args.get(1) {
            Some(v) => vm.to_number_value(*v)?,
            None => 0.0,
        };
        let a = to_uint32(a_num);
        let b = to_uint32(b_num);
        let res = a.wrapping_mul(b) as i32;
        Ok(JsValue::from_int32(res))
    });
    let fround_fn = vm.alloc_native_function("fround", 1, |vm, _this, args| {
        let n = match args.first() {
            Some(v) => vm.to_number_value(*v)?,
            None => f64::NAN,
        };
        Ok(JsValue::from_double((n as f32) as f64))
    });

    // Hyperbolic functions
    let acosh_fn = vm.alloc_native_function("acosh", 1, |vm, _this, args| {
        let n = match args.first() {
            Some(v) => vm.to_number_value(*v)?,
            None => f64::NAN,
        };
        Ok(JsValue::from_double(n.acosh()))
    });
    let asinh_fn = vm.alloc_native_function("asinh", 1, |vm, _this, args| {
        let n = match args.first() {
            Some(v) => vm.to_number_value(*v)?,
            None => f64::NAN,
        };
        Ok(JsValue::from_double(n.asinh()))
    });
    let atanh_fn = vm.alloc_native_function("atanh", 1, |vm, _this, args| {
        let n = match args.first() {
            Some(v) => vm.to_number_value(*v)?,
            None => f64::NAN,
        };
        Ok(JsValue::from_double(n.atanh()))
    });
    let cosh_fn = vm.alloc_native_function("cosh", 1, |vm, _this, args| {
        let n = match args.first() {
            Some(v) => vm.to_number_value(*v)?,
            None => f64::NAN,
        };
        Ok(JsValue::from_double(n.cosh()))
    });
    let sinh_fn = vm.alloc_native_function("sinh", 1, |vm, _this, args| {
        let n = match args.first() {
            Some(v) => vm.to_number_value(*v)?,
            None => f64::NAN,
        };
        Ok(JsValue::from_double(n.sinh()))
    });
    let tanh_fn = vm.alloc_native_function("tanh", 1, |vm, _this, args| {
        let n = match args.first() {
            Some(v) => vm.to_number_value(*v)?,
            None => f64::NAN,
        };
        Ok(JsValue::from_double(n.tanh()))
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
        ("acosh", acosh_fn),
        ("asinh", asinh_fn),
        ("atanh", atanh_fn),
        ("cosh", cosh_fn),
        ("sinh", sinh_fn),
        ("tanh", tanh_fn),
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
        if let Some(ptr) = math_obj.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            obj.set_symbol_attributes(
                tag_sym,
                PropertyAttributes { writable: false, enumerable: false, configurable: true },
            );
        }
    }

    vm.globals.insert("Math".to_string(), math_obj);
    if vm.global_object.is_object() {
        vm.set_property(vm.global_object, "Math", math_obj);
    }
}
