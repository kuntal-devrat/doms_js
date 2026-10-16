//! The `Math` global object.

use super::*;

pub(super) fn register_math(vm: &mut Vm) {
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
        Ok(JsValue::from_double(crate::interpreter::js_exponentiate(base, exp)))
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
