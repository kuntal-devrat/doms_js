//! The `Array` constructor and `Array.isArray`.

use super::*;

pub(super) fn register_constructor(vm: &mut Vm) -> (JsValue, JsValue) {
    let array_constructor = vm.alloc_native_function("Array", 1, |vm, _this, args| {
        if args.len() == 1 {
            if let Some(n) = args[0].to_number() {
                if n < 0.0 || n > 4_294_967_295.0 || n.fract() != 0.0 || n.is_nan() {
                    return Err(VmError::RangeError("Invalid array length".into()));
                }
                let len = n as usize;
                let mut v = Vec::new();
                if len <= 1024 {
                    v.resize(len, JsValue::hole());
                }
                let arr = vm.alloc_array(v);
                if len > 1024 {
                    vm.set_property(arr, "length", JsValue::from_double(n));
                }
                return Ok(arr);
            }
        }
        Ok(vm.alloc_array(args.to_vec()))
    });

    let is_array_fn = vm.alloc_native_function("isArray", 1, |vm, _this, args| {
        let arg = args.first().copied().unwrap_or(JsValue::undefined());
        Ok(JsValue::from_boolean(vm.is_array_result(arg)?))
    });
    (array_constructor, is_array_fn)
}
