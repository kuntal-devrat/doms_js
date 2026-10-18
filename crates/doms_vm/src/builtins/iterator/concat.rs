//! Iterator.concat implementation.

use super::prototype::alloc_iterator_helper;
use crate::interpreter::{Vm, VmError};
use crate::object::IteratorHelperKind;
use crate::value::JsValue;

pub fn iterator_concat(vm: &mut Vm, args: &[JsValue]) -> Result<JsValue, VmError> {
    let sym_iter = vm.symbol_iterator.ok_or_else(|| VmError::TypeError("Symbol.iterator not found".into()))?;
    let mut iterables = Vec::new();
    for arg in args {
        if !arg.is_object() {
            return Err(VmError::TypeError("Iterator.concat item must be an object".into()));
        }
        let method = vm.get_symbol_property_result(*arg, sym_iter)?;
        if method.is_undefined() || method.is_null() || !vm.is_callable(method) {
            return Err(VmError::TypeError("Iterator.concat item must be iterable".into()));
        }
        iterables.push(*arg);
    }

    Ok(alloc_iterator_helper(
        vm,
        JsValue::undefined(),
        JsValue::undefined(),
        IteratorHelperKind::Concat {
            iterables,
            current_idx: 0,
            current_iter: None,
            current_next: None,
        },
    ))
}
