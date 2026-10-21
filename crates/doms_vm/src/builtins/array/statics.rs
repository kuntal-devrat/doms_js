//! Static `Array` methods (`Array.of` and `Array.from`).

use super::*;

fn error_to_js_value(vm: &mut Vm, e: VmError) -> JsValue {
    match e {
        VmError::Thrown(v) => v,
        VmError::TypeError(msg) => vm.alloc_error_opt("TypeError".to_string(), Some(msg)),
        VmError::RangeError(msg) => vm.alloc_error_opt("RangeError".to_string(), Some(msg)),
        VmError::ReferenceError(msg) => vm.alloc_error_opt("ReferenceError".to_string(), Some(msg)),
        VmError::SyntaxError(msg) => vm.alloc_error_opt("SyntaxError".to_string(), Some(msg)),
        VmError::UriError(msg) => vm.alloc_error_opt("UriError".to_string(), Some(msg)),
        _ => vm.alloc_error_opt("Error".to_string(), Some(e.to_string())),
    }
}

pub(super) fn register_statics(vm: &mut Vm, array_constructor: JsValue) {
    let of_fn = vm.alloc_native_function("of", 0, |vm, this, args| {
        if vm.is_constructor(this) {
            let target = vm.construct(this, &[JsValue::from_int32(args.len() as i32)])?;
            for (i, &arg) in args.iter().enumerate() {
                vm.set_element(target, i, arg);
            }
            Ok(target)
        } else {
            Ok(vm.alloc_array(args.to_vec()))
        }
    });
    vm.set_property(array_constructor, "of", of_fn);

    let from_fn = vm.alloc_native_function("from", 1, |vm, this, args| {
        let items = args.first().copied().unwrap_or(JsValue::undefined());
        if items.is_null() || items.is_undefined() {
            return Err(VmError::TypeError("Cannot convert undefined or null to object".into()));
        }
        let map_fn = args.get(1).copied().unwrap_or(JsValue::undefined());
        let this_arg = args.get(2).copied().unwrap_or(JsValue::undefined());
        let has_map = !map_fn.is_undefined();
        if has_map && !vm.is_callable(map_fn) {
            return Err(VmError::TypeError(
                "Array.from: when provided, mapFn must be callable".into(),
            ));
        }

        let is_ctor = vm.is_constructor(this);

        let prev_temp = vm.temp_roots.len();
        vm.temp_roots.push(this);
        vm.temp_roots.push(items);
        vm.temp_roots.push(map_fn);
        vm.temp_roots.push(this_arg);
        let result_start = vm.temp_roots.len();

        let cur_items = vm.temp_roots[prev_temp + 1];
        if let Some(sym_iter) = vm.symbol_iterator {
            let iter_method =
                vm.get_symbol_property_result(cur_items, sym_iter).unwrap_or(JsValue::undefined());
            if vm.is_callable(iter_method) {
                let cur_items = vm.temp_roots[prev_temp + 1];
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
                    let cur_map = vm.temp_roots[prev_temp + 2];
                    let cur_this_arg = vm.temp_roots[prev_temp + 3];
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
                let num_elements = vm.temp_roots.len() - (next_root_idx + 1);
                let mut elements = Vec::with_capacity(num_elements);
                for k in (next_root_idx + 1)..vm.temp_roots.len() {
                    elements.push(vm.temp_roots[k]);
                }
                let cur_this = vm.temp_roots[prev_temp];
                vm.temp_roots.truncate(prev_temp);
                if is_ctor {
                    let target = vm.construct(cur_this, &[])?;
                    let prev_t = vm.temp_roots.len();
                    vm.temp_roots.push(target);
                    for (i, el) in elements.into_iter().enumerate() {
                        let cur_target = vm.temp_roots[prev_t];
                        vm.create_data_property_or_throw(cur_target, &i.to_string(), el)?;
                    }
                    let cur_target = vm.temp_roots[prev_t];
                    vm.set_property_throw(
                        cur_target,
                        "length",
                        JsValue::from_int32(num_elements as i32),
                    )?;
                    let res = vm.temp_roots.pop().unwrap();
                    return Ok(res);
                } else {
                    return Ok(vm.alloc_array(elements));
                }
            }
        }

        let cur_items = vm.temp_roots[prev_temp + 1];
        let len = vm.length_of_array_like(cur_items)?;
        for i in 0..len {
            let cur_items = vm.temp_roots[prev_temp + 1];
            let el = vm.get_element_result(cur_items, i)?;
            let cur_map = vm.temp_roots[prev_temp + 2];
            let cur_this_arg = vm.temp_roots[prev_temp + 3];
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

        let num_elements = vm.temp_roots.len() - result_start;
        let mut result_elements = Vec::with_capacity(num_elements);
        for k in result_start..vm.temp_roots.len() {
            result_elements.push(vm.temp_roots[k]);
        }
        let cur_this = vm.temp_roots[prev_temp];
        vm.temp_roots.truncate(prev_temp);
        if is_ctor {
            let target =
                vm.construct(cur_this, &[JsValue::from_int32(result_elements.len() as i32)])?;
            let prev_t = vm.temp_roots.len();
            vm.temp_roots.push(target);
            for (i, el) in result_elements.into_iter().enumerate() {
                let cur_target = vm.temp_roots[prev_t];
                vm.create_data_property_or_throw(cur_target, &i.to_string(), el)?;
            }
            let cur_target = vm.temp_roots[prev_t];
            vm.set_property_throw(cur_target, "length", JsValue::from_int32(num_elements as i32))?;
            let res = vm.temp_roots.pop().unwrap();
            Ok(res)
        } else {
            Ok(vm.alloc_array(result_elements))
        }
    });
    vm.set_property(array_constructor, "from", from_fn);

    let from_async_fn = vm.alloc_native_function("fromAsync", 1, |vm, this, args| {
        if vm.current_new_target.is_some() {
            return Err(VmError::TypeError("Array.fromAsync is not a constructor".into()));
        }
        let async_items = args.first().copied().unwrap_or(JsValue::undefined());
        if async_items.is_null() || async_items.is_undefined() {
            let err = vm.alloc_error_opt(
                "TypeError".to_string(),
                Some("Cannot convert undefined or null to object".into()),
            );
            let p = vm.alloc_promise(crate::object::PromiseState::Pending, JsValue::undefined());
            vm.reject_promise(p, err);
            return Ok(p);
        }
        let map_fn = args.get(1).copied().unwrap_or(JsValue::undefined());
        let has_map = !map_fn.is_undefined();
        if has_map && !vm.is_callable(map_fn) {
            let err = vm.alloc_error_opt(
                "TypeError".to_string(),
                Some("Array.fromAsync: when provided, mapFn must be callable".into()),
            );
            let p = vm.alloc_promise(crate::object::PromiseState::Pending, JsValue::undefined());
            vm.reject_promise(p, err);
            return Ok(p);
        }
        let this_arg = args.get(2).copied().unwrap_or(JsValue::undefined());
        let is_ctor = vm.is_constructor(this);

        let p = vm.alloc_promise(crate::object::PromiseState::Pending, JsValue::undefined());

        let prev_temp = vm.temp_roots.len();
        vm.temp_roots.push(this);
        vm.temp_roots.push(async_items);
        vm.temp_roots.push(map_fn);
        vm.temp_roots.push(this_arg);
        vm.temp_roots.push(p);

        let mut using_async_iter = false;
        let mut iter_method = JsValue::undefined();
        if let Some(sym_async) = vm.symbol_async_iterator {
            let cur_items = vm.temp_roots[prev_temp + 1];
            iter_method =
                vm.get_symbol_property_result(cur_items, sym_async).unwrap_or(JsValue::undefined());
            if !iter_method.is_undefined() && !iter_method.is_null() {
                if !vm.is_callable(iter_method) {
                    let err = vm.alloc_error_opt(
                        "TypeError".to_string(),
                        Some("asyncIterator is not callable".into()),
                    );
                    let cur_p = vm.temp_roots[prev_temp + 4];
                    vm.reject_promise(cur_p, err);
                    vm.temp_roots.truncate(prev_temp);
                    return Ok(cur_p);
                }
                using_async_iter = true;
            }
        }
        if !using_async_iter {
            if let Some(sym_iter) = vm.symbol_iterator {
                let cur_items = vm.temp_roots[prev_temp + 1];
                let m = vm
                    .get_symbol_property_result(cur_items, sym_iter)
                    .unwrap_or(JsValue::undefined());
                if !m.is_undefined() && !m.is_null() {
                    if !vm.is_callable(m) {
                        let err = vm.alloc_error_opt(
                            "TypeError".to_string(),
                            Some("iterator is not callable".into()),
                        );
                        let cur_p = vm.temp_roots[prev_temp + 4];
                        vm.reject_promise(cur_p, err);
                        vm.temp_roots.truncate(prev_temp);
                        return Ok(cur_p);
                    }
                    iter_method = m;
                }
            }
        }

        let mut elements = Vec::new();
        if vm.is_callable(iter_method) {
            let cur_items = vm.temp_roots[prev_temp + 1];
            let iterator = match vm.call(iter_method, cur_items, &[]) {
                Ok(it) => it,
                Err(e) => {
                    let err = error_to_js_value(vm, e);
                    let cur_p = vm.temp_roots[prev_temp + 4];
                    vm.reject_promise(cur_p, err);
                    vm.temp_roots.truncate(prev_temp);
                    return Ok(cur_p);
                }
            };
            let iter_root = vm.temp_roots.len();
            vm.temp_roots.push(iterator);
            let next_fn = vm.get_property(iterator, "next");
            let next_root = vm.temp_roots.len();
            vm.temp_roots.push(next_fn);

            let mut idx = 0;
            while vm.is_callable(vm.temp_roots[next_root]) {
                if idx >= 100_000 {
                    break;
                }
                let cur_next = vm.temp_roots[next_root];
                let cur_it = vm.temp_roots[iter_root];
                let step = match vm.call(cur_next, cur_it, &[]) {
                    Ok(st) => st,
                    Err(e) => {
                        let err = error_to_js_value(vm, e);
                        let cur_p = vm.temp_roots[prev_temp + 4];
                        vm.reject_promise(cur_p, err);
                        vm.temp_roots.truncate(prev_temp);
                        return Ok(cur_p);
                    }
                };
                let unwrapped_step = if let Some(ptr) = step.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    if let ObjectKind::Promise(ref p) = obj.kind {
                        if p.state == crate::object::PromiseState::Fulfilled {
                            p.result
                        } else {
                            step
                        }
                    } else {
                        step
                    }
                } else {
                    step
                };
                let done = match vm.get_property_result(unwrapped_step, "done") {
                    Ok(d) => d.is_truthy(),
                    Err(e) => {
                        let err = error_to_js_value(vm, e);
                        let cur_p = vm.temp_roots[prev_temp + 4];
                        vm.reject_promise(cur_p, err);
                        vm.temp_roots.truncate(prev_temp);
                        return Ok(cur_p);
                    }
                };
                if done {
                    break;
                }
                let mut val = match vm.get_property_result(unwrapped_step, "value") {
                    Ok(v) => v,
                    Err(e) => {
                        let err = error_to_js_value(vm, e);
                        let cur_p = vm.temp_roots[prev_temp + 4];
                        vm.reject_promise(cur_p, err);
                        vm.temp_roots.truncate(prev_temp);
                        return Ok(cur_p);
                    }
                };
                if !using_async_iter {
                    if let Some(ptr) = val.as_object_ptr() {
                        let obj = unsafe { &*(ptr as *mut JsObject) };
                        if let ObjectKind::Promise(ref p) = obj.kind {
                            if p.state == crate::object::PromiseState::Fulfilled {
                                val = p.result;
                            }
                        }
                    }
                }
                let cur_map = vm.temp_roots[prev_temp + 2];
                let cur_this_arg = vm.temp_roots[prev_temp + 3];
                let mapped = if has_map {
                    match vm.call(cur_map, cur_this_arg, &[val, JsValue::from_int32(idx)]) {
                        Ok(m) => m,
                        Err(e) => {
                            let err = error_to_js_value(vm, e);
                            let cur_p = vm.temp_roots[prev_temp + 4];
                            vm.reject_promise(cur_p, err);
                            vm.temp_roots.truncate(prev_temp);
                            return Ok(cur_p);
                        }
                    }
                } else {
                    val
                };
                elements.push(mapped);
                idx += 1;
            }
        } else {
            let cur_items = vm.temp_roots[prev_temp + 1];
            let len = match vm.length_of_array_like(cur_items) {
                Ok(l) => l,
                Err(e) => {
                    let err = error_to_js_value(vm, e);
                    let cur_p = vm.temp_roots[prev_temp + 4];
                    vm.reject_promise(cur_p, err);
                    vm.temp_roots.truncate(prev_temp);
                    return Ok(cur_p);
                }
            };
            let capped_len = len.min(100_000);
            for i in 0..capped_len {
                let cur_items = vm.temp_roots[prev_temp + 1];
                let mut el = match vm.get_element_result(cur_items, i) {
                    Ok(e) => e,
                    Err(e) => {
                        let err = error_to_js_value(vm, e);
                        let cur_p = vm.temp_roots[prev_temp + 4];
                        vm.reject_promise(cur_p, err);
                        vm.temp_roots.truncate(prev_temp);
                        return Ok(cur_p);
                    }
                };
                if let Some(ptr) = el.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    if let ObjectKind::Promise(ref p) = obj.kind {
                        if p.state == crate::object::PromiseState::Fulfilled {
                            el = p.result;
                        }
                    }
                }
                let cur_map = vm.temp_roots[prev_temp + 2];
                let cur_this_arg = vm.temp_roots[prev_temp + 3];
                let mapped = if has_map {
                    match vm.call(cur_map, cur_this_arg, &[el, JsValue::from_int32(i as i32)]) {
                        Ok(m) => m,
                        Err(e) => {
                            let err = error_to_js_value(vm, e);
                            let cur_p = vm.temp_roots[prev_temp + 4];
                            vm.reject_promise(cur_p, err);
                            vm.temp_roots.truncate(prev_temp);
                            return Ok(cur_p);
                        }
                    }
                } else {
                    el
                };
                elements.push(mapped);
            }
        }

        let cur_this = vm.temp_roots[prev_temp];
        let cur_p = vm.temp_roots[prev_temp + 4];
        let num_elems = elements.len();
        let target = if is_ctor {
            let t = match vm.construct(cur_this, &[JsValue::from_int32(num_elems as i32)]) {
                Ok(t) => t,
                Err(e) => {
                    let err = error_to_js_value(vm, e);
                    vm.reject_promise(cur_p, err);
                    vm.temp_roots.truncate(prev_temp);
                    return Ok(cur_p);
                }
            };
            for (i, el) in elements.into_iter().enumerate() {
                if let Err(e) = vm.create_data_property_or_throw(t, &i.to_string(), el) {
                    let err = error_to_js_value(vm, e);
                    vm.reject_promise(cur_p, err);
                    vm.temp_roots.truncate(prev_temp);
                    return Ok(cur_p);
                }
            }
            let _ = vm.set_property_throw(t, "length", JsValue::from_int32(num_elems as i32));
            t
        } else {
            vm.alloc_array(elements)
        };

        vm.resolve_promise(cur_p, target);
        vm.temp_roots.truncate(prev_temp);
        Ok(cur_p)
    });
    vm.set_property(array_constructor, "fromAsync", from_async_fn);
}
