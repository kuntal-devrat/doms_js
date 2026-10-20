//! Iterator.zip implementation.

use super::prototype::alloc_iterator_helper;
use crate::interpreter::{Vm, VmError};
use crate::object::{IteratorHelperKind, ZipMode};
use crate::value::JsValue;

pub fn iterator_zip(vm: &mut Vm, args: &[JsValue]) -> Result<JsValue, VmError> {
    let iterables_val = args.first().copied().unwrap_or(JsValue::undefined());
    if !iterables_val.is_object() {
        return Err(VmError::TypeError("Iterator.zip requires an iterable of iterables".into()));
    }
    let sym_iter =
        vm.symbol_iterator.ok_or_else(|| VmError::TypeError("Symbol.iterator not found".into()))?;
    let iter_method = vm.get_symbol_property_result(iterables_val, sym_iter)?;
    if iter_method.is_undefined() || iter_method.is_null() || !vm.is_callable(iter_method) {
        return Err(VmError::TypeError("Iterator.zip argument must be iterable".into()));
    }
    let outer_iter = vm.call(iter_method, iterables_val, &[])?;
    if !outer_iter.is_object() {
        return Err(VmError::TypeError("Symbol.iterator must return an object".into()));
    }
    let outer_next = vm.get_property_result(outer_iter, "next")?;
    if !vm.is_callable(outer_next) {
        return Err(VmError::TypeError("next method is not callable".into()));
    }

    let mut others = Vec::new();
    loop {
        let step = vm.call(outer_next, outer_iter, &[])?;
        if !step.is_object() {
            return Err(VmError::TypeError("Iterator step must be an object".into()));
        }
        if vm.get_property(step, "done").is_truthy() {
            break;
        }
        let item = vm.get_property(step, "value");
        others.push(item);
    }

    Ok(alloc_iterator_helper(
        vm,
        outer_iter,
        outer_next,
        IteratorHelperKind::Zip { others, mode: ZipMode::Shortest, padding: None },
    ))
}
