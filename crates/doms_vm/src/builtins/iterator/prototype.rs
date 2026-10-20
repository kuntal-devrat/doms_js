//! %IteratorPrototype% methods and accessors.

use super::helper::{self, iterator_close_normal};
use crate::builtins::promise::iterator::{IteratorRecord, iterator_close};
use crate::interpreter::{PropertyKeyVal, Vm, VmError};
use crate::object::{IteratorHelperKind, JsObject, ObjectKind};
use crate::value::JsValue;

pub fn alloc_iterator_helper(
    vm: &mut Vm,
    iterator: JsValue,
    next_method: JsValue,
    kind: IteratorHelperKind,
) -> JsValue {
    let helper_obj = vm.alloc_object_with_exact_proto(vm.iterator_helper_proto);
    if let Some(ptr) = helper_obj.as_object_ptr() {
        let obj = unsafe { &mut *(ptr as *mut JsObject) };
        obj.kind = ObjectKind::IteratorHelper {
            kind: Box::new(kind),
            iterator,
            next_method,
            done: false,
            index: 0,
            executing: false,
        };
    }
    helper_obj
}

pub fn get_iterator_direct(vm: &mut Vm, this: JsValue) -> Result<(JsValue, JsValue), VmError> {
    if !this.is_object() {
        return Err(VmError::TypeError("Iterator method called on non-object".into()));
    }
    let next = vm.get_property_result(this, "next")?;
    if !vm.is_callable(next) {
        return Err(VmError::TypeError("next method is not callable".into()));
    }
    Ok((this, next))
}

pub fn setter_that_ignores_prototype_properties(
    vm: &mut Vm,
    home: JsValue,
    this: JsValue,
    prop: &str,
    val: JsValue,
) -> Result<JsValue, VmError> {
    if !this.is_object() {
        return Err(VmError::TypeError("Setter called on non-object".into()));
    }
    if this == home {
        return Err(VmError::TypeError(format!(
            "Cannot set property '{}' on prototype directly",
            prop
        )));
    }
    let has_own = vm.has_own_property(this, prop);
    if !has_own {
        vm.set_property_direct(this, prop, val);
    } else {
        vm.set_property(this, prop, val);
    }
    Ok(JsValue::undefined())
}

pub fn setter_that_ignores_prototype_symbol_properties(
    vm: &mut Vm,
    home: JsValue,
    this: JsValue,
    sym: JsValue,
    val: JsValue,
) -> Result<JsValue, VmError> {
    if !this.is_object() {
        return Err(VmError::TypeError("Setter called on non-object".into()));
    }
    if this == home {
        return Err(VmError::TypeError("Cannot set symbol property on prototype directly".into()));
    }
    let has_own = vm.has_own_property_key(this, &PropertyKeyVal::Symbol(sym));
    if !has_own {
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            obj.insert_symbol_property(sym, val);
        }
    } else {
        vm.set_symbol_property(this, sym, val);
    }
    Ok(JsValue::undefined())
}

fn validate_take_drop_limit(
    vm: &mut Vm,
    this: JsValue,
    arg: Option<JsValue>,
) -> Result<f64, VmError> {
    let limit_val = arg.unwrap_or(JsValue::undefined());
    let num_res = vm.to_number_value(limit_val);
    let num = match num_res {
        Ok(n) => n,
        Err(e) => {
            let rec =
                IteratorRecord { iterator: this, next_method: JsValue::undefined(), done: false };
            let _ = helper::iterator_close_normal(vm, &rec);
            return Err(e);
        }
    };
    if num.is_nan() || (num.is_finite() && num > 9007199254740991.0) {
        let rec = IteratorRecord { iterator: this, next_method: JsValue::undefined(), done: false };
        let _ = helper::iterator_close_normal(vm, &rec);
        return Err(VmError::RangeError("Invalid limit".into()));
    }
    let int_limit = num.trunc();
    if int_limit < 0.0 {
        let rec = IteratorRecord { iterator: this, next_method: JsValue::undefined(), done: false };
        let _ = helper::iterator_close_normal(vm, &rec);
        return Err(VmError::RangeError("Limit must be non-negative".into()));
    }
    Ok(num)
}

pub fn register_iterator_prototype_methods(vm: &mut Vm, iter_proto: JsValue, iter_ctor: JsValue) {
    // 1. constructor accessor
    let ctor_getter = vm.alloc_closure_native_function(
        "get constructor",
        0,
        std::sync::Arc::new(move |_vm, _this, _args| Ok(iter_ctor)),
    );
    let ctor_setter = vm.alloc_closure_native_function(
        "set constructor",
        1,
        std::sync::Arc::new(move |vm, this, args| {
            let v = args.first().copied().unwrap_or(JsValue::undefined());
            setter_that_ignores_prototype_properties(vm, iter_proto, this, "constructor", v)
        }),
    );
    if let Some(ptr) = iter_proto.as_object_ptr() {
        let obj = unsafe { &mut *(ptr as *mut JsObject) };
        obj.define_accessor("constructor".to_string(), Some(ctor_getter), Some(ctor_setter));
    }
    vm.set_property_non_enumerable(iter_proto, "constructor".to_string());

    // 2. [Symbol.toStringTag] accessor
    if let Some(sym_tag) = vm.symbol_to_string_tag {
        let tag_getter =
            vm.alloc_native_function("get [Symbol.toStringTag]", 0, |vm, _this, _args| {
                Ok(vm.alloc_string("Iterator".into()))
            });
        let tag_setter = vm.alloc_closure_native_function(
            "set [Symbol.toStringTag]",
            1,
            std::sync::Arc::new(move |vm, this, args| {
                let v = args.first().copied().unwrap_or(JsValue::undefined());
                setter_that_ignores_prototype_symbol_properties(vm, iter_proto, this, sym_tag, v)
            }),
        );
        if let Some(ptr) = iter_proto.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            obj.define_symbol_accessor(sym_tag, Some(tag_getter), Some(tag_setter));
        }
        let key_name = format!("[Symbol:{:x}]", sym_tag.raw_bits());
        vm.set_property_non_enumerable(iter_proto, key_name);
    }

    // 3. [Symbol.iterator]()
    if let Some(sym_iter) = vm.symbol_iterator {
        let iter_self_fn =
            vm.alloc_native_function("[Symbol.iterator]", 0, |_vm, this, _args| Ok(this));
        vm.set_symbol_property(iter_proto, sym_iter, iter_self_fn);
        let key_name = format!("[Symbol:{:x}]", sym_iter.raw_bits());
        vm.set_property_non_enumerable(iter_proto, key_name);
    }

    // 4. toArray()
    let to_array_fn = vm.alloc_native_function("toArray", 0, |vm, this, _args| {
        let (target, next_method) = get_iterator_direct(vm, this)?;
        let mut items = Vec::new();
        loop {
            let step = vm.call(next_method, target, &[])?;
            if !step.is_object() {
                return Err(VmError::TypeError("Iterator step must be an object".into()));
            }
            let done = vm.get_property_result(step, "done")?.is_truthy();
            if done {
                break;
            }
            let val = vm.get_property_result(step, "value")?;
            items.push(val);
        }
        Ok(vm.alloc_array(items))
    });
    vm.set_property(iter_proto, "toArray", to_array_fn);
    vm.set_property_non_enumerable(iter_proto, "toArray".to_string());

    // 5. forEach(fn)
    let for_each_fn = vm.alloc_native_function("forEach", 1, |vm, this, args| {
        if !this.is_object() {
            return Err(VmError::TypeError(
                "Iterator.prototype.forEach called on non-object".into(),
            ));
        }
        let callback = args.first().copied().unwrap_or(JsValue::undefined());
        if !vm.is_callable(callback) {
            return Err(VmError::TypeError("forEach callback is not callable".into()));
        }
        let (target, next_method) = get_iterator_direct(vm, this)?;
        let mut counter = 0usize;
        loop {
            let step = vm.call(next_method, target, &[])?;
            if !step.is_object() {
                return Err(VmError::TypeError("Iterator step must be an object".into()));
            }
            let done = vm.get_property_result(step, "done")?.is_truthy();
            if done {
                break;
            }
            let val = vm.get_property_result(step, "value")?;
            let call_res = vm.call(
                callback,
                JsValue::undefined(),
                &[val, JsValue::from_int32(counter as i32)],
            );
            if let Err(e) = call_res {
                let rec = IteratorRecord { iterator: target, next_method, done: false };
                return Err(iterator_close(vm, &rec, Some(e)));
            }
            counter += 1;
        }
        Ok(JsValue::undefined())
    });
    vm.set_property(iter_proto, "forEach", for_each_fn);
    vm.set_property_non_enumerable(iter_proto, "forEach".to_string());

    // 6. some(predicate)
    let some_fn = vm.alloc_native_function("some", 1, |vm, this, args| {
        if !this.is_object() {
            return Err(VmError::TypeError("Iterator.prototype.some called on non-object".into()));
        }
        let predicate = args.first().copied().unwrap_or(JsValue::undefined());
        if !vm.is_callable(predicate) {
            return Err(VmError::TypeError("some predicate is not callable".into()));
        }
        let (target, next_method) = get_iterator_direct(vm, this)?;
        let mut counter = 0usize;
        loop {
            let step = vm.call(next_method, target, &[])?;
            if !step.is_object() {
                return Err(VmError::TypeError("Iterator step must be an object".into()));
            }
            let done = vm.get_property_result(step, "done")?.is_truthy();
            if done {
                return Ok(JsValue::from_boolean(false));
            }
            let val = vm.get_property_result(step, "value")?;
            let call_res = vm.call(
                predicate,
                JsValue::undefined(),
                &[val, JsValue::from_int32(counter as i32)],
            );
            let res_val = match call_res {
                Ok(v) => v,
                Err(e) => {
                    let rec = IteratorRecord { iterator: target, next_method, done: false };
                    return Err(iterator_close(vm, &rec, Some(e)));
                }
            };
            if res_val.is_truthy() {
                let rec = IteratorRecord { iterator: target, next_method, done: false };
                iterator_close_normal(vm, &rec)?;
                return Ok(JsValue::from_boolean(true));
            }
            counter += 1;
        }
    });
    vm.set_property(iter_proto, "some", some_fn);
    vm.set_property_non_enumerable(iter_proto, "some".to_string());

    // 7. every(predicate)
    let every_fn = vm.alloc_native_function("every", 1, |vm, this, args| {
        if !this.is_object() {
            return Err(VmError::TypeError("Iterator.prototype.every called on non-object".into()));
        }
        let predicate = args.first().copied().unwrap_or(JsValue::undefined());
        if !vm.is_callable(predicate) {
            return Err(VmError::TypeError("every predicate is not callable".into()));
        }
        let (target, next_method) = get_iterator_direct(vm, this)?;
        let mut counter = 0usize;
        loop {
            let step = vm.call(next_method, target, &[])?;
            if !step.is_object() {
                return Err(VmError::TypeError("Iterator step must be an object".into()));
            }
            let done = vm.get_property_result(step, "done")?.is_truthy();
            if done {
                return Ok(JsValue::from_boolean(true));
            }
            let val = vm.get_property_result(step, "value")?;
            let call_res = vm.call(
                predicate,
                JsValue::undefined(),
                &[val, JsValue::from_int32(counter as i32)],
            );
            let res_val = match call_res {
                Ok(v) => v,
                Err(e) => {
                    let rec = IteratorRecord { iterator: target, next_method, done: false };
                    return Err(iterator_close(vm, &rec, Some(e)));
                }
            };
            if !res_val.is_truthy() {
                let rec = IteratorRecord { iterator: target, next_method, done: false };
                iterator_close_normal(vm, &rec)?;
                return Ok(JsValue::from_boolean(false));
            }
            counter += 1;
        }
    });
    vm.set_property(iter_proto, "every", every_fn);
    vm.set_property_non_enumerable(iter_proto, "every".to_string());

    // 8. find(predicate)
    let find_fn = vm.alloc_native_function("find", 1, |vm, this, args| {
        if !this.is_object() {
            return Err(VmError::TypeError("Iterator.prototype.find called on non-object".into()));
        }
        let predicate = args.first().copied().unwrap_or(JsValue::undefined());
        if !vm.is_callable(predicate) {
            return Err(VmError::TypeError("find predicate is not callable".into()));
        }
        let (target, next_method) = get_iterator_direct(vm, this)?;
        let mut counter = 0usize;
        loop {
            let step = vm.call(next_method, target, &[])?;
            if !step.is_object() {
                return Err(VmError::TypeError("Iterator step must be an object".into()));
            }
            let done = vm.get_property_result(step, "done")?.is_truthy();
            if done {
                return Ok(JsValue::undefined());
            }
            let val = vm.get_property_result(step, "value")?;
            let call_res = vm.call(
                predicate,
                JsValue::undefined(),
                &[val, JsValue::from_int32(counter as i32)],
            );
            let res_val = match call_res {
                Ok(v) => v,
                Err(e) => {
                    let rec = IteratorRecord { iterator: target, next_method, done: false };
                    return Err(iterator_close(vm, &rec, Some(e)));
                }
            };
            if res_val.is_truthy() {
                let rec = IteratorRecord { iterator: target, next_method, done: false };
                iterator_close_normal(vm, &rec)?;
                return Ok(val);
            }
            counter += 1;
        }
    });
    vm.set_property(iter_proto, "find", find_fn);
    vm.set_property_non_enumerable(iter_proto, "find".to_string());

    // 9. reduce(reducer [, initialValue])
    let reduce_fn = vm.alloc_native_function("reduce", 1, |vm, this, args| {
        if !this.is_object() {
            return Err(VmError::TypeError(
                "Iterator.prototype.reduce called on non-object".into(),
            ));
        }
        let reducer = args.first().copied().unwrap_or(JsValue::undefined());
        if !vm.is_callable(reducer) {
            return Err(VmError::TypeError("reduce reducer is not callable".into()));
        }
        let (target, next_method) = get_iterator_direct(vm, this)?;
        let (mut accumulator, mut counter) = if args.len() >= 2 {
            (args[1], 0usize)
        } else {
            let step = vm.call(next_method, target, &[])?;
            if !step.is_object() {
                return Err(VmError::TypeError("Iterator step must be an object".into()));
            }
            if vm.get_property_result(step, "done")?.is_truthy() {
                return Err(VmError::TypeError(
                    "Reduce of empty iterator with no initial value".into(),
                ));
            }
            (vm.get_property_result(step, "value")?, 1usize)
        };

        loop {
            let step = vm.call(next_method, target, &[])?;
            if !step.is_object() {
                return Err(VmError::TypeError("Iterator step must be an object".into()));
            }
            let done = vm.get_property_result(step, "done")?.is_truthy();
            if done {
                return Ok(accumulator);
            }
            let val = vm.get_property_result(step, "value")?;
            let call_res = vm.call(
                reducer,
                JsValue::undefined(),
                &[accumulator, val, JsValue::from_int32(counter as i32)],
            );
            accumulator = match call_res {
                Ok(v) => v,
                Err(e) => {
                    let rec = IteratorRecord { iterator: target, next_method, done: false };
                    return Err(iterator_close(vm, &rec, Some(e)));
                }
            };
            counter += 1;
        }
    });
    vm.set_property(iter_proto, "reduce", reduce_fn);
    vm.set_property_non_enumerable(iter_proto, "reduce".to_string());

    // 10. map(mapper)
    let map_fn = vm.alloc_native_function("map", 1, |vm, this, args| {
        if !this.is_object() {
            return Err(VmError::TypeError("Iterator.prototype.map called on non-object".into()));
        }
        let mapper = args.first().copied().unwrap_or(JsValue::undefined());
        if !vm.is_callable(mapper) {
            return Err(VmError::TypeError("map mapper is not callable".into()));
        }
        let (target, next_method) = get_iterator_direct(vm, this)?;
        Ok(alloc_iterator_helper(vm, target, next_method, IteratorHelperKind::Map { mapper }))
    });
    vm.set_property(iter_proto, "map", map_fn);
    vm.set_property_non_enumerable(iter_proto, "map".to_string());

    // 11. filter(predicate)
    let filter_fn = vm.alloc_native_function("filter", 1, |vm, this, args| {
        if !this.is_object() {
            return Err(VmError::TypeError(
                "Iterator.prototype.filter called on non-object".into(),
            ));
        }
        let predicate = args.first().copied().unwrap_or(JsValue::undefined());
        if !vm.is_callable(predicate) {
            return Err(VmError::TypeError("filter predicate is not callable".into()));
        }
        let (target, next_method) = get_iterator_direct(vm, this)?;
        Ok(alloc_iterator_helper(vm, target, next_method, IteratorHelperKind::Filter { predicate }))
    });
    vm.set_property(iter_proto, "filter", filter_fn);
    vm.set_property_non_enumerable(iter_proto, "filter".to_string());

    // 12. take(limit)
    let take_fn = vm.alloc_native_function("take", 1, |vm, this, args| {
        if !this.is_object() {
            return Err(VmError::TypeError("Iterator.prototype.take called on non-object".into()));
        }
        let limit = validate_take_drop_limit(vm, this, args.first().copied())?;
        let (target, next_method) = get_iterator_direct(vm, this)?;
        Ok(alloc_iterator_helper(
            vm,
            target,
            next_method,
            IteratorHelperKind::Take { remaining: limit },
        ))
    });
    vm.set_property(iter_proto, "take", take_fn);
    vm.set_property_non_enumerable(iter_proto, "take".to_string());

    // 13. drop(limit)
    let drop_fn = vm.alloc_native_function("drop", 1, |vm, this, args| {
        if !this.is_object() {
            return Err(VmError::TypeError("Iterator.prototype.drop called on non-object".into()));
        }
        let limit = validate_take_drop_limit(vm, this, args.first().copied())?;
        let (target, next_method) = get_iterator_direct(vm, this)?;
        Ok(alloc_iterator_helper(
            vm,
            target,
            next_method,
            IteratorHelperKind::Drop { remaining: limit },
        ))
    });
    vm.set_property(iter_proto, "drop", drop_fn);
    vm.set_property_non_enumerable(iter_proto, "drop".to_string());

    // 14. flatMap(mapper)
    let flat_map_fn = vm.alloc_native_function("flatMap", 1, |vm, this, args| {
        if !this.is_object() {
            return Err(VmError::TypeError(
                "Iterator.prototype.flatMap called on non-object".into(),
            ));
        }
        let mapper = args.first().copied().unwrap_or(JsValue::undefined());
        if !vm.is_callable(mapper) {
            return Err(VmError::TypeError("flatMap mapper is not callable".into()));
        }
        let (target, next_method) = get_iterator_direct(vm, this)?;
        Ok(alloc_iterator_helper(
            vm,
            target,
            next_method,
            IteratorHelperKind::FlatMap { mapper, inner_iter: None, inner_next: None },
        ))
    });
    vm.set_property(iter_proto, "flatMap", flat_map_fn);
    vm.set_property_non_enumerable(iter_proto, "flatMap".to_string());
}
