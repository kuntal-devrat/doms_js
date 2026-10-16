//! `JSON.parse` reviver walk.

use super::*;

pub(super) fn json_walk(vm: &mut Vm, reviver: JsValue, holder: JsValue, name: &str) -> Result<JsValue, VmError> {
    let val = vm.get_property(holder, name);
    if let Some(ptr) = val.as_object_ptr() {
        let obj = unsafe { &*(ptr as *mut JsObject) };
        if let ObjectKind::Array(elements) = &obj.kind {
            let len = elements.len();
            for i in 0..len {
                let prop_str = i.to_string();
                let new_elem = json_walk(vm, reviver, val, &prop_str)?;
                if new_elem.is_undefined() {
                    vm.delete_property(val, &prop_str)?;
                } else {
                    vm.set_property(val, &prop_str, new_elem);
                }
            }
        } else {
            let keys = get_enumerable_keys(vm, val);
            for k in keys {
                let new_val = json_walk(vm, reviver, val, &k)?;
                if new_val.is_undefined() {
                    vm.delete_property(val, &k)?;
                } else {
                    vm.set_property(val, &k, new_val);
                }
            }
        }
    }

    let name_val = vm.alloc_string(name.to_string());
    vm.call(reviver, holder, &[name_val, val])
}
