//! Shared array-like iterator used by arrays and typed arrays.

use super::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum IterKind {
    Keys,
    Values,
    Entries,
}

pub(crate) fn create_array_iterator(vm: &mut Vm, target: JsValue, kind: IterKind) -> Result<JsValue, VmError> {
    let iter_obj = vm.alloc_object();
    let idx = Arc::new(std::sync::Mutex::new(0usize));
    let next_fn = vm.alloc_closure_native_function("next", 0, Arc::new(move |vm, _this, _args| {
        let mut i = idx.lock().unwrap();
        let len = vm.get_property(target, "length").to_number().unwrap_or(0.0) as usize;
        let res = vm.alloc_object();
        if *i < len {
            let curr = *i;
            *i += 1;
            let val = match kind {
                IterKind::Keys => JsValue::from_int32(curr as i32),
                IterKind::Values => vm.get_element_result(target, curr)?,
                IterKind::Entries => {
                    let el = vm.get_element_result(target, curr)?;
                    vm.alloc_array(vec![JsValue::from_int32(curr as i32), el])
                }
            };
            vm.set_property(res, "value", val);
            vm.set_property(res, "done", JsValue::from_boolean(false));
        } else {
            vm.set_property(res, "value", JsValue::undefined());
            vm.set_property(res, "done", JsValue::from_boolean(true));
        }
        Ok(res)
    }));
    vm.set_property(iter_obj, "next", next_fn);
    if let Some(sym_iter) = vm.symbol_iterator {
        let self_val = iter_obj;
        let self_fn = vm.alloc_closure_native_function("[Symbol.iterator]", 0, Arc::new(move |_vm, _this, _args| Ok(self_val)));
        vm.set_symbol_property(iter_obj, sym_iter, self_fn);
    }
    Ok(iter_obj)
}
