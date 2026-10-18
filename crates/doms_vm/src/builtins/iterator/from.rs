//! Iterator.from implementation.

use crate::interpreter::{Vm, VmError};
use crate::object::{JsObject, ObjectKind};
use crate::value::JsValue;

pub fn alloc_wrap_for_valid_iterator(vm: &mut Vm, iterator: JsValue, next_method: JsValue) -> JsValue {
    let wrap_obj = vm.alloc_object_with_exact_proto(vm.wrap_for_valid_iterator_proto);
    if let Some(ptr) = wrap_obj.as_object_ptr() {
        let obj = unsafe { &mut *(ptr as *mut JsObject) };
        obj.kind = ObjectKind::WrapForValidIterator {
            iterator,
            next_method,
            done: false,
        };
    }
    wrap_obj
}

pub fn iterator_from(vm: &mut Vm, o: JsValue) -> Result<JsValue, VmError> {
    if !o.is_object() && !o.is_string() {
        return Err(VmError::TypeError("Iterator.from requires an object or string".into()));
    }

    let using_iterator = if let Some(sym_iter) = vm.symbol_iterator {
        let m = vm.get_symbol_property_result(o, sym_iter)?;
        if m.is_undefined() || m.is_null() {
            None
        } else if !vm.is_callable(m) {
            return Err(VmError::TypeError("Symbol.iterator property is not callable".into()));
        } else {
            Some(m)
        }
    } else {
        None
    };

    if let Some(using_iter_fn) = using_iterator {
        let iterator = vm.call(using_iter_fn, o, &[])?;
        if !iterator.is_object() {
            return Err(VmError::TypeError("Symbol.iterator must return an object".into()));
        }
        if vm.is_instance_of_iterator_proto(iterator) {
            return Ok(iterator);
        }
        let next_method = vm.get_property_result(iterator, "next")?;
        return Ok(alloc_wrap_for_valid_iterator(vm, iterator, next_method));
    }

    if !o.is_object() {
        return Err(VmError::TypeError("Iterator.from requires an object".into()));
    }

    if vm.is_instance_of_iterator_proto(o) {
        return Ok(o);
    }

    let next_method = vm.get_property_result(o, "next")?;
    Ok(alloc_wrap_for_valid_iterator(vm, o, next_method))
}
