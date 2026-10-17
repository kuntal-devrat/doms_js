//! Static `Array` methods (`Array.of` and `Array.from`).

use super::*;

pub(super) fn register_statics(vm: &mut Vm, array_constructor: JsValue) {
    let of_fn = vm.alloc_native_function("of", 0, |vm, _this, args| {
        Ok(vm.alloc_array(args.to_vec()))
    });
    vm.set_property(array_constructor, "of", of_fn);

    let from_fn = vm.alloc_native_function("from", 1, |vm, _this, args| {
        let items = args.first().copied().unwrap_or(JsValue::undefined());
        let map_fn = args.get(1).copied().unwrap_or(JsValue::undefined());
        let this_arg = args.get(2).copied().unwrap_or(JsValue::undefined());
        let has_map = vm.is_callable(map_fn);

        let mut result_elements = Vec::new();
        let mut handled = false;

        if let Some(sym_iter) = vm.symbol_iterator {
            let iter_method = vm.get_symbol_property_result(items, sym_iter).unwrap_or(JsValue::undefined());
            if vm.is_callable(iter_method) {
                handled = true;
                let iterator = vm.call(iter_method, items, &[])?;
                let next_fn = vm.get_property(iterator, "next");
                let mut idx = 0;
                while vm.is_callable(next_fn) {
                    let step = vm.call(next_fn, iterator, &[])?;
                    let done = vm.get_property_result(step, "done")?.is_truthy();
                    if done {
                        break;
                    }
                    let val = vm.get_property_result(step, "value")?;
                    let mapped = if has_map {
                        vm.call(map_fn, this_arg, &[val, JsValue::from_int32(idx)])?
                    } else {
                        val
                    };
                    result_elements.push(mapped);
                    idx += 1;
                }
            }
        }

        if !handled {
            let len_val = vm.get_property(items, "length");
            if let Some(n) = len_val.to_number() {
                let len = (n as usize).min(100_000);
                for i in 0..len {
                    let el = vm.get_element(items, i);
                    let mapped = if has_map {
                        vm.call(map_fn, this_arg, &[el, JsValue::from_int32(i as i32)])?
                    } else {
                        el
                    };
                    result_elements.push(mapped);
                }
            }
        }

        Ok(vm.alloc_array(result_elements))
    });
    vm.set_property(array_constructor, "from", from_fn);
}
