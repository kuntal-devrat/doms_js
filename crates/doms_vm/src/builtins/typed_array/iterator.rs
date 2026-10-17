//! Shared array-like iterator used by arrays and typed arrays.

use super::*;
pub(crate) use crate::object::ArrayIterationKind as IterKind;

pub(crate) fn create_array_iterator(vm: &mut Vm, target: JsValue, kind: IterKind) -> Result<JsValue, VmError> {
    let prev_temp = vm.temp_roots.len();
    vm.temp_roots.push(target);
    let iter_obj = vm.alloc_object();
    vm.temp_roots.push(iter_obj);

    if let Some(ptr) = vm.temp_roots[prev_temp + 1].as_object_ptr() {
        let obj = unsafe { &mut *(ptr as *mut JsObject) };
        obj.kind = ObjectKind::ArrayIterator {
            target: vm.temp_roots[prev_temp],
            index: 0,
            kind,
        };
    }

    let next_fn = vm.alloc_native_function("next", 0, |vm, this, _args| {
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::ArrayIterator { target, index, kind } = &mut obj.kind {
                let cur_target = *target;
                let cur_kind = *kind;
                let curr_idx = *index;
                let len = vm.get_property(cur_target, "length").to_number().unwrap_or(0.0) as usize;
                if curr_idx < len {
                    *index += 1;
                    let val = match cur_kind {
                        IterKind::Keys => JsValue::from_int32(curr_idx as i32),
                        IterKind::Values => vm.get_element_result(cur_target, curr_idx)?,
                        IterKind::Entries => {
                            let el = vm.get_element_result(cur_target, curr_idx)?;
                            let prev = vm.temp_roots.len();
                            vm.temp_roots.push(el);
                            let arr = vm.alloc_array(vec![JsValue::from_int32(curr_idx as i32), vm.temp_roots[prev]]);
                            vm.temp_roots.truncate(prev);
                            arr
                        }
                    };
                    let prev = vm.temp_roots.len();
                    vm.temp_roots.push(val);
                    let res = vm.alloc_object();
                    vm.temp_roots.push(res);
                    let cur_val = vm.temp_roots[prev];
                    let cur_res = vm.temp_roots[prev + 1];
                    vm.set_property(cur_res, "value", cur_val);
                    vm.set_property(cur_res, "done", JsValue::from_boolean(false));
                    let out = vm.temp_roots[prev + 1];
                    vm.temp_roots.truncate(prev);
                    return Ok(out);
                } else {
                    let prev = vm.temp_roots.len();
                    let res = vm.alloc_object();
                    vm.temp_roots.push(res);
                    let cur_res = vm.temp_roots[prev];
                    vm.set_property(cur_res, "value", JsValue::undefined());
                    vm.set_property(cur_res, "done", JsValue::from_boolean(true));
                    let out = vm.temp_roots[prev];
                    vm.temp_roots.truncate(prev);
                    return Ok(out);
                }
            }
        }
        Err(VmError::TypeError("%ArrayIteratorPrototype%.next called on incompatible receiver".into()))
    });

    let cur_iter = vm.temp_roots[prev_temp + 1];
    vm.set_property(cur_iter, "next", next_fn);
    if let Some(sym_iter) = vm.symbol_iterator {
        let self_fn = vm.alloc_native_function("[Symbol.iterator]", 0, |_vm, this, _args| Ok(this));
        let cur_iter = vm.temp_roots[prev_temp + 1];
        vm.set_symbol_property(cur_iter, sym_iter, self_fn);
    }
    if let Some(sym_tag) = vm.symbol_to_string_tag {
        let tag_str = vm.alloc_string("Array Iterator".into());
        let cur_iter = vm.temp_roots[prev_temp + 1];
        vm.set_symbol_property(cur_iter, sym_tag, tag_str);
    }

    let res = vm.temp_roots[prev_temp + 1];
    vm.temp_roots.truncate(prev_temp);
    Ok(res)
}
