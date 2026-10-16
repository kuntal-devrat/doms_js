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
    vm.set_property(number_proto, "valueOf", num_value_of);
    vm.set_property(number_proto, "toString", num_to_string);

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
}
