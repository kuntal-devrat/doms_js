//! Shared array-like iterator used by arrays and typed arrays.

use super::*;
pub(crate) use crate::object::ArrayIterationKind as IterKind;

pub(crate) fn create_array_iterator(vm: &mut Vm, target: JsValue, kind: IterKind) -> Result<JsValue, VmError> {
    let iter_obj = vm.alloc_object_with_exact_proto(vm.array_iterator_proto);
    if let Some(ptr) = iter_obj.as_object_ptr() {
        let obj = unsafe { &mut *(ptr as *mut JsObject) };
        obj.kind = ObjectKind::ArrayIterator {
            target,
            index: 0,
            kind,
        };
    }
    Ok(iter_obj)
}
