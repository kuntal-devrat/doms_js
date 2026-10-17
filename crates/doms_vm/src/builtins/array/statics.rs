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

        let prev_temp = vm.temp_roots.len();
        vm.temp_roots.push(items);
        vm.temp_roots.push(map_fn);
        vm.temp_roots.push(this_arg);
        let result_start = vm.temp_roots.len();

        let cur_items = vm.temp_roots[prev_temp];
        if let Some(sym_iter) = vm.symbol_iterator {
            let iter_method = vm.get_symbol_property_result(cur_items, sym_iter).unwrap_or(JsValue::undefined());
            if vm.is_callable(iter_method) {
                let cur_items = vm.temp_roots[prev_temp];
                let iterator = match vm.call(iter_method, cur_items, &[]) {
                    Ok(it) => it,
                    Err(e) => {
                        vm.temp_roots.truncate(prev_temp);
                        return Err(e);
                    }
                };
                let iter_root_idx = vm.temp_roots.len();
                vm.temp_roots.push(iterator);
                let next_fn = vm.get_property(iterator, "next");
                let next_root_idx = vm.temp_roots.len();
                vm.temp_roots.push(next_fn);

                let mut idx = 0;
                while vm.is_callable(vm.temp_roots[next_root_idx]) {
                    let cur_next = vm.temp_roots[next_root_idx];
                    let cur_iter = vm.temp_roots[iter_root_idx];
                    let step = match vm.call(cur_next, cur_iter, &[]) {
                        Ok(st) => st,
                        Err(e) => {
                            vm.temp_roots.truncate(prev_temp);
                            return Err(e);
                        }
                    };
                    let done = match vm.get_property_result(step, "done") {
                        Ok(d) => d.is_truthy(),
                        Err(e) => {
                            vm.temp_roots.truncate(prev_temp);
                            return Err(e);
                        }
                    };
                    if done {
                        break;
                    }
                    let val = match vm.get_property_result(step, "value") {
                        Ok(v) => v,
                        Err(e) => {
                            vm.temp_roots.truncate(prev_temp);
                            return Err(e);
                        }
                    };
                    let cur_map = vm.temp_roots[prev_temp + 1];
                    let cur_this_arg = vm.temp_roots[prev_temp + 2];
                    let mapped = if has_map {
                        match vm.call(cur_map, cur_this_arg, &[val, JsValue::from_int32(idx)]) {
                            Ok(m) => m,
                            Err(e) => {
                                vm.temp_roots.truncate(prev_temp);
                                return Err(e);
                            }
                        }
                    } else {
                        val
                    };
                    vm.temp_roots.push(mapped);
                    idx += 1;
                }
                // Elements were pushed after iterator and next_fn.
                // Move elements to result_start.
                let num_elements = vm.temp_roots.len() - (next_root_idx + 1);
                let mut elements = Vec::with_capacity(num_elements);
                for k in (next_root_idx + 1)..vm.temp_roots.len() {
                    elements.push(vm.temp_roots[k]);
                }
                vm.temp_roots.truncate(prev_temp);
                return Ok(vm.alloc_array(elements));
            }
        }

        let cur_items = vm.temp_roots[prev_temp];
        let len_val = vm.get_property(cur_items, "length");
        if let Some(n) = len_val.to_number() {
            let len = (n as usize).min(100_000);
            for i in 0..len {
                let cur_items = vm.temp_roots[prev_temp];
                let el = vm.get_element(cur_items, i);
                let cur_map = vm.temp_roots[prev_temp + 1];
                let cur_this_arg = vm.temp_roots[prev_temp + 2];
                let mapped = if has_map {
                    match vm.call(cur_map, cur_this_arg, &[el, JsValue::from_int32(i as i32)]) {
                        Ok(m) => m,
                        Err(e) => {
                            vm.temp_roots.truncate(prev_temp);
                            return Err(e);
                        }
                    }
                } else {
                    el
                };
                vm.temp_roots.push(mapped);
            }
        }

        let num_elements = vm.temp_roots.len() - result_start;
        let mut result_elements = Vec::with_capacity(num_elements);
        for k in result_start..vm.temp_roots.len() {
            result_elements.push(vm.temp_roots[k]);
        }
        vm.temp_roots.truncate(prev_temp);
        Ok(vm.alloc_array(result_elements))
    });
    vm.set_property(array_constructor, "from", from_fn);
}
