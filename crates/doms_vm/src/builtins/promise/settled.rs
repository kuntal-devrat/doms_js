//! `Promise.allSettled`, `Promise.any`, and `Promise.withResolvers`.

use super::iterator::*;
use super::*;

pub(super) fn create_settled_statics(vm: &mut Vm) -> Vec<(&'static str, JsValue)> {
    let with_resolvers_static = vm.alloc_native_function("withResolvers", 0, |vm, this, _args| {
        if !vm.is_constructor(this) {
            return Err(VmError::TypeError(
                "Promise.withResolvers called on non-constructor".into(),
            ));
        }
        let capability = super::new_promise_capability(vm, this)?;
        let res_obj = vm.alloc_object();
        vm.set_property(res_obj, "promise", capability.promise);
        vm.set_property(res_obj, "resolve", capability.resolve);
        vm.set_property(res_obj, "reject", capability.reject);
        Ok(res_obj)
    });

    let all_settled_static = vm.alloc_native_function("allSettled", 1, |vm, this, args| {
        if !vm.is_constructor(this) {
            return Err(VmError::TypeError(
                "Promise.allSettled called on non-constructor".into(),
            ));
        }
        let capability = super::new_promise_capability(vm, this)?;
        let resolve_method = vm.get_property(this, "resolve");
        if !vm.is_callable(resolve_method) {
            let err = vm.alloc_error("TypeError".into(), "Promise resolve method is not callable".into());
            let _ = vm.call(capability.reject, JsValue::undefined(), &[err]);
            return Ok(capability.promise);
        }

        let iterable = args.first().copied().unwrap_or(JsValue::undefined());
        let mut iter_record = match get_iterator(vm, iterable) {
            Ok(rec) => rec,
            Err(e) => {
                let err_val = vm.alloc_error("TypeError".into(), e.to_string());
                let _ = vm.call(capability.reject, JsValue::undefined(), &[err_val]);
                return Ok(capability.promise);
            }
        };

        let prev_temp = vm.temp_roots.len();
        vm.temp_roots.push(capability.promise);
        vm.temp_roots.push(capability.resolve);
        vm.temp_roots.push(capability.reject);
        vm.temp_roots.push(resolve_method);
        vm.temp_roots.push(iter_record.iterator);
        vm.temp_roots.push(iter_record.next_method);

        let remaining = Arc::new(std::sync::atomic::AtomicUsize::new(1));
        let results = Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut index = 0usize;

        loop {
            iter_record.iterator = vm.temp_roots[prev_temp + 4];
            iter_record.next_method = vm.temp_roots[prev_temp + 5];

            let step_opt = match iterator_step(vm, &mut iter_record) {
                Ok(s) => s,
                Err(e) => {
                    let final_err = iterator_close(vm, &iter_record, Some(e));
                    let cur_rej = vm.temp_roots[prev_temp + 2];
                    let cur_prom = vm.temp_roots[prev_temp];
                    let err_val = vm.alloc_error("TypeError".into(), final_err.to_string());
                    let _ = vm.call(cur_rej, JsValue::undefined(), &[err_val]);
                    vm.temp_roots.truncate(prev_temp);
                    return Ok(cur_prom);
                }
            };

            let step = match step_opt {
                Some(s) => s,
                None => {
                    if remaining.fetch_sub(1, std::sync::atomic::Ordering::SeqCst) == 1 {
                        let final_res = results.lock().unwrap().clone();
                        let cur_resolve = vm.temp_roots[prev_temp + 1];
                        let cur_reject = vm.temp_roots[prev_temp + 2];
                        let arr = vm.alloc_array(final_res);
                        if let Err(e) = vm.call(cur_resolve, JsValue::undefined(), &[arr]) {
                            let err_val = vm.vm_error_to_js_value(&e);
                            let _ = vm.call(cur_reject, JsValue::undefined(), &[err_val]);
                        }
                    }
                    let cur_prom = vm.temp_roots[prev_temp];
                    vm.temp_roots.truncate(prev_temp);
                    return Ok(cur_prom);
                }
            };

            let next_val = match iterator_value(vm, &mut iter_record, step) {
                Ok(v) => v,
                Err(e) => {
                    let final_err = iterator_close(vm, &iter_record, Some(e));
                    let cur_rej = vm.temp_roots[prev_temp + 2];
                    let cur_prom = vm.temp_roots[prev_temp];
                    let err_val = vm.alloc_error("TypeError".into(), final_err.to_string());
                    let _ = vm.call(cur_rej, JsValue::undefined(), &[err_val]);
                    vm.temp_roots.truncate(prev_temp);
                    return Ok(cur_prom);
                }
            };

            results.lock().unwrap().push(JsValue::undefined());
            remaining.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

            let cap_resolve = vm.temp_roots[prev_temp + 1];
            let cap_reject = vm.temp_roots[prev_temp + 2];
            let rem = Arc::clone(&remaining);
            let res_store = Arc::clone(&results);
            let already_called = Arc::new(std::sync::atomic::AtomicBool::new(false));

            let ac1 = Arc::clone(&already_called);
            let rem1 = Arc::clone(&rem);
            let res_store1 = Arc::clone(&res_store);
            let item_idx = index;
            let resolve_item = vm.alloc_closure_native_function(
                "",
                1,
                Arc::new(move |vm: &mut Vm, _this: JsValue, args: &[JsValue]| -> Result<JsValue, VmError> {
                    if ac1.swap(true, std::sync::atomic::Ordering::SeqCst) {
                        return Ok(JsValue::undefined());
                    }
                    let v = args.first().copied().unwrap_or(JsValue::undefined());
                    let item_obj = vm.alloc_object();
                    let status_str = vm.alloc_string("fulfilled".into());
                    vm.set_property(item_obj, "status", status_str);
                    vm.set_property(item_obj, "value", v);
                    {
                        let mut lock = res_store1.lock().unwrap();
                        if item_idx < lock.len() {
                            lock[item_idx] = item_obj;
                        }
                    }
                    if rem1.fetch_sub(1, std::sync::atomic::Ordering::SeqCst) == 1 {
                        let final_res = res_store1.lock().unwrap().clone();
                        let arr = vm.alloc_array(final_res);
                        if let Err(e) = vm.call(cap_resolve, JsValue::undefined(), &[arr]) {
                            let err_val = vm.vm_error_to_js_value(&e);
                            let _ = vm.call(cap_reject, JsValue::undefined(), &[err_val]);
                        }
                    }
                    Ok(JsValue::undefined())
                }),
            );

            let ac2 = Arc::clone(&already_called);
            let rem2 = Arc::clone(&rem);
            let res_store2 = Arc::clone(&res_store);
            let reject_item = vm.alloc_closure_native_function(
                "",
                1,
                Arc::new(move |vm: &mut Vm, _this: JsValue, args: &[JsValue]| -> Result<JsValue, VmError> {
                    if ac2.swap(true, std::sync::atomic::Ordering::SeqCst) {
                        return Ok(JsValue::undefined());
                    }
                    let err = args.first().copied().unwrap_or(JsValue::undefined());
                    let item_obj = vm.alloc_object();
                    let status_str = vm.alloc_string("rejected".into());
                    vm.set_property(item_obj, "status", status_str);
                    vm.set_property(item_obj, "reason", err);
                    {
                        let mut lock = res_store2.lock().unwrap();
                        if item_idx < lock.len() {
                            lock[item_idx] = item_obj;
                        }
                    }
                    if rem2.fetch_sub(1, std::sync::atomic::Ordering::SeqCst) == 1 {
                        let final_res = res_store2.lock().unwrap().clone();
                        let arr = vm.alloc_array(final_res);
                        if let Err(e) = vm.call(cap_resolve, JsValue::undefined(), &[arr]) {
                            let err_val = vm.vm_error_to_js_value(&e);
                            let _ = vm.call(cap_reject, JsValue::undefined(), &[err_val]);
                        }
                    }
                    Ok(JsValue::undefined())
                }),
            );

            let cur_resolve_method = vm.temp_roots[prev_temp + 3];
            let next_promise = match vm.call(cur_resolve_method, this, &[next_val]) {
                Ok(p) => p,
                Err(e) => {
                    let final_err = iterator_close(vm, &iter_record, Some(e));
                    let cur_rej = vm.temp_roots[prev_temp + 2];
                    let cur_prom = vm.temp_roots[prev_temp];
                    let err_val = vm.alloc_error("TypeError".into(), final_err.to_string());
                    let _ = vm.call(cur_rej, JsValue::undefined(), &[err_val]);
                    vm.temp_roots.truncate(prev_temp);
                    return Ok(cur_prom);
                }
            };

            let then_fn = match vm.get_property_result(next_promise, "then") {
                Ok(tf) => tf,
                Err(e) => {
                    let final_err = iterator_close(vm, &iter_record, Some(e));
                    let cur_rej = vm.temp_roots[prev_temp + 2];
                    let cur_prom = vm.temp_roots[prev_temp];
                    let err_val = vm.alloc_error("TypeError".into(), final_err.to_string());
                    let _ = vm.call(cur_rej, JsValue::undefined(), &[err_val]);
                    vm.temp_roots.truncate(prev_temp);
                    return Ok(cur_prom);
                }
            };

            if vm.is_callable(then_fn) {
                if let Err(e) = vm.call(then_fn, next_promise, &[resolve_item, reject_item]) {
                    let final_err = iterator_close(vm, &iter_record, Some(e));
                    let cur_rej = vm.temp_roots[prev_temp + 2];
                    let cur_prom = vm.temp_roots[prev_temp];
                    let err_val = vm.alloc_error("TypeError".into(), final_err.to_string());
                    let _ = vm.call(cur_rej, JsValue::undefined(), &[err_val]);
                    vm.temp_roots.truncate(prev_temp);
                    return Ok(cur_prom);
                }
            } else {
                let final_err = iterator_close(
                    vm,
                    &iter_record,
                    Some(VmError::TypeError("Promise then is not callable".into())),
                );
                let cur_rej = vm.temp_roots[prev_temp + 2];
                let cur_prom = vm.temp_roots[prev_temp];
                let err_val = vm.alloc_error("TypeError".into(), final_err.to_string());
                let _ = vm.call(cur_rej, JsValue::undefined(), &[err_val]);
                vm.temp_roots.truncate(prev_temp);
                return Ok(cur_prom);
            }

            index += 1;
        }
    });

    let any_static = vm.alloc_native_function("any", 1, |vm, this, args| {
        if !vm.is_constructor(this) {
            return Err(VmError::TypeError(
                "Promise.any called on non-constructor".into(),
            ));
        }
        let capability = super::new_promise_capability(vm, this)?;
        let resolve_method = vm.get_property(this, "resolve");
        if !vm.is_callable(resolve_method) {
            let err = vm.alloc_error("TypeError".into(), "Promise resolve method is not callable".into());
            let _ = vm.call(capability.reject, JsValue::undefined(), &[err]);
            return Ok(capability.promise);
        }

        let iterable = args.first().copied().unwrap_or(JsValue::undefined());
        let mut iter_record = match get_iterator(vm, iterable) {
            Ok(rec) => rec,
            Err(e) => {
                let err_val = vm.vm_error_to_js_value(&e);
                let _ = vm.call(capability.reject, JsValue::undefined(), &[err_val]);
                return Ok(capability.promise);
            }
        };

        let prev_temp = vm.temp_roots.len();
        vm.temp_roots.push(capability.promise);
        vm.temp_roots.push(capability.resolve);
        vm.temp_roots.push(capability.reject);
        vm.temp_roots.push(resolve_method);
        vm.temp_roots.push(iter_record.iterator);
        vm.temp_roots.push(iter_record.next_method);

        let remaining = Arc::new(std::sync::atomic::AtomicUsize::new(1));
        let errors = Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut index = 0usize;

        loop {
            iter_record.iterator = vm.temp_roots[prev_temp + 4];
            iter_record.next_method = vm.temp_roots[prev_temp + 5];

            let step_opt = match iterator_step(vm, &mut iter_record) {
                Ok(s) => s,
                Err(e) => {
                    let final_err = iterator_close(vm, &iter_record, Some(e));
                    let cur_rej = vm.temp_roots[prev_temp + 2];
                    let cur_prom = vm.temp_roots[prev_temp];
                    let err_val = vm.vm_error_to_js_value(&final_err);
                    let _ = vm.call(cur_rej, JsValue::undefined(), &[err_val]);
                    vm.temp_roots.truncate(prev_temp);
                    return Ok(cur_prom);
                }
            };

            let step = match step_opt {
                Some(s) => s,
                None => {
                    if remaining.fetch_sub(1, std::sync::atomic::Ordering::SeqCst) == 1 {
                        let final_errs = errors.lock().unwrap().clone();
                        let cur_reject = vm.temp_roots[prev_temp + 2];
                        let err_obj = vm.alloc_error(
                            "AggregateError".into(),
                            "All promises were rejected".into(),
                        );
                        let err_arr = vm.alloc_array(final_errs);
                        vm.set_property(err_obj, "errors", err_arr);
                        let _ = vm.call(cur_reject, JsValue::undefined(), &[err_obj]);
                    }
                    let cur_prom = vm.temp_roots[prev_temp];
                    vm.temp_roots.truncate(prev_temp);
                    return Ok(cur_prom);
                }
            };

            let next_val = match iterator_value(vm, &mut iter_record, step) {
                Ok(v) => v,
                Err(e) => {
                    let final_err = iterator_close(vm, &iter_record, Some(e));
                    let cur_rej = vm.temp_roots[prev_temp + 2];
                    let cur_prom = vm.temp_roots[prev_temp];
                    let err_val = vm.vm_error_to_js_value(&final_err);
                    let _ = vm.call(cur_rej, JsValue::undefined(), &[err_val]);
                    vm.temp_roots.truncate(prev_temp);
                    return Ok(cur_prom);
                }
            };

            errors.lock().unwrap().push(JsValue::undefined());
            remaining.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

            let cap_resolve = vm.temp_roots[prev_temp + 1];
            let cap_reject = vm.temp_roots[prev_temp + 2];
            let rem2 = Arc::clone(&remaining);
            let err_store2 = Arc::clone(&errors);
            let item_idx = index;
            let already_called = Arc::new(std::sync::atomic::AtomicBool::new(false));
            let ac = Arc::clone(&already_called);
            let reject_item = vm.alloc_closure_native_function(
                "",
                1,
                Arc::new(move |vm: &mut Vm, _this: JsValue, args: &[JsValue]| -> Result<JsValue, VmError> {
                    if ac.swap(true, std::sync::atomic::Ordering::SeqCst) {
                        return Ok(JsValue::undefined());
                    }
                    let err = args.first().copied().unwrap_or(JsValue::undefined());
                    {
                        let mut lock = err_store2.lock().unwrap();
                        if item_idx < lock.len() {
                            lock[item_idx] = err;
                        }
                    }
                    if rem2.fetch_sub(1, std::sync::atomic::Ordering::SeqCst) == 1 {
                        let final_errs = err_store2.lock().unwrap().clone();
                        let err_obj = vm.alloc_error(
                            "AggregateError".into(),
                            "All promises were rejected".into(),
                        );
                        let err_arr = vm.alloc_array(final_errs);
                        vm.set_property(err_obj, "errors", err_arr);
                        let _ = vm.call(cap_reject, JsValue::undefined(), &[err_obj]);
                    }
                    Ok(JsValue::undefined())
                }),
            );

            let cur_resolve_method = vm.temp_roots[prev_temp + 3];
            let next_promise = match vm.call(cur_resolve_method, this, &[next_val]) {
                Ok(p) => p,
                Err(e) => {
                    let final_err = iterator_close(vm, &iter_record, Some(e));
                    let cur_rej = vm.temp_roots[prev_temp + 2];
                    let cur_prom = vm.temp_roots[prev_temp];
                    let err_val = vm.vm_error_to_js_value(&final_err);
                    let _ = vm.call(cur_rej, JsValue::undefined(), &[err_val]);
                    vm.temp_roots.truncate(prev_temp);
                    return Ok(cur_prom);
                }
            };

            let then_fn = match vm.get_property_result(next_promise, "then") {
                Ok(tf) => tf,
                Err(e) => {
                    let final_err = iterator_close(vm, &iter_record, Some(e));
                    let cur_rej = vm.temp_roots[prev_temp + 2];
                    let cur_prom = vm.temp_roots[prev_temp];
                    let err_val = vm.vm_error_to_js_value(&final_err);
                    let _ = vm.call(cur_rej, JsValue::undefined(), &[err_val]);
                    vm.temp_roots.truncate(prev_temp);
                    return Ok(cur_prom);
                }
            };

            if vm.is_callable(then_fn) {
                if let Err(e) = vm.call(then_fn, next_promise, &[cap_resolve, reject_item]) {
                    let final_err = iterator_close(vm, &iter_record, Some(e));
                    let cur_rej = vm.temp_roots[prev_temp + 2];
                    let cur_prom = vm.temp_roots[prev_temp];
                    let err_val = vm.vm_error_to_js_value(&final_err);
                    let _ = vm.call(cur_rej, JsValue::undefined(), &[err_val]);
                    vm.temp_roots.truncate(prev_temp);
                    return Ok(cur_prom);
                }
            } else {
                let final_err = iterator_close(
                    vm,
                    &iter_record,
                    Some(VmError::TypeError("Promise then is not callable".into())),
                );
                let cur_rej = vm.temp_roots[prev_temp + 2];
                let cur_prom = vm.temp_roots[prev_temp];
                let err_val = vm.vm_error_to_js_value(&final_err);
                let _ = vm.call(cur_rej, JsValue::undefined(), &[err_val]);
                vm.temp_roots.truncate(prev_temp);
                return Ok(cur_prom);
            }

            index += 1;
        }
    });

    vec![
        ("allSettled", all_settled_static),
        ("any", any_static),
        ("withResolvers", with_resolvers_static),
    ]
}
