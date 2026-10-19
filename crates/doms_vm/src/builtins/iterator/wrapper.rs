use crate::interpreter::{Vm, VmError};
use crate::object::{JsObject, ObjectKind};
use crate::value::JsValue;

pub fn register_wrap_for_valid_iterator_proto(vm: &mut Vm, iter_proto: JsValue) -> JsValue {
    let wrap_proto = vm.alloc_object_with_exact_proto(Some(iter_proto));
    vm.wrap_for_valid_iterator_proto = Some(wrap_proto);

    let next_fn = vm.alloc_native_function("next", 0, |vm, this, args| {
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::WrapForValidIterator { iterator, next_method, done } = &mut obj.kind {
                if *done {
                    return vm.alloc_iter_result(JsValue::undefined(), true);
                }
                let cur_iter = *iterator;
                let cur_next = *next_method;
                if !vm.is_callable(cur_next) {
                    *done = true;
                    return Err(VmError::TypeError("next method is not callable".into()));
                }
                let res = vm.call(cur_next, cur_iter, args);
                match res {
                    Ok(val) => {
                        if !val.is_object() {
                            *done = true;
                            return Err(VmError::TypeError("Iterator result must be an object".into()));
                        }
                        return Ok(val);
                    }
                    Err(e) => {
                        *done = true;
                        return Err(e);
                    }
                }
            }
        }
        Err(VmError::TypeError("%WrapForValidIteratorPrototype%.next requires [[Iterated]] internal slot".into()))
    });
    vm.set_property(wrap_proto, "next", next_fn);
    vm.set_property_non_enumerable(wrap_proto, "next".to_string());

    let return_fn = vm.alloc_native_function("return", 0, |vm, this, args| {
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::WrapForValidIterator { iterator, done, .. } = &mut obj.kind {
                if *done {
                    let v = args.first().copied().unwrap_or(JsValue::undefined());
                    return vm.alloc_iter_result(v, true);
                }
                *done = true;
                let cur_iter = *iterator;
                let ret_method = vm.get_property_result(cur_iter, "return")?;
                if ret_method.is_undefined() || ret_method.is_null() {
                    let v = args.first().copied().unwrap_or(JsValue::undefined());
                    return vm.alloc_iter_result(v, true);
                }
                if !vm.is_callable(ret_method) {
                    return Err(VmError::TypeError("return method is not callable".into()));
                }
                let res = vm.call(ret_method, cur_iter, args)?;
                if !res.is_object() {
                    return Err(VmError::TypeError("Iterator return method must return an object".into()));
                }
                return Ok(res);
            }
        }
        Err(VmError::TypeError("%WrapForValidIteratorPrototype%.return requires [[Iterated]] internal slot".into()))
    });
    vm.set_property(wrap_proto, "return", return_fn);
    vm.set_property_non_enumerable(wrap_proto, "return".to_string());

    wrap_proto
}
