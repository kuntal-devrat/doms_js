//! `Promise.withResolvers`, `Promise.allSettled` and `Promise.any`.

use super::*;

pub(super) fn create_settled_statics(vm: &mut Vm) -> Vec<(&'static str, JsValue)> {
    let with_resolvers_static = vm.alloc_native_function("withResolvers", 0, |vm, _this, _args| {
        let promise = vm.alloc_promise(crate::object::PromiseState::Pending, JsValue::undefined());
        let resolve_fn = {
            let p = promise;
            vm.alloc_closure_native_function("resolve", 1, Arc::new(move |vm: &mut Vm, _this, args| {
                let val = args.first().copied().unwrap_or(JsValue::undefined());
                vm.resolve_promise(p, val);
                Ok(JsValue::undefined())
            }))
        };
        let reject_fn = {
            let p = promise;
            vm.alloc_closure_native_function("reject", 1, Arc::new(move |vm: &mut Vm, _this, args| {
                let val = args.first().copied().unwrap_or(JsValue::undefined());
                vm.reject_promise(p, val);
                Ok(JsValue::undefined())
            }))
        };
        let res_obj = vm.alloc_object();
        vm.set_property(res_obj, "promise", promise);
        vm.set_property(res_obj, "resolve", resolve_fn);
        vm.set_property(res_obj, "reject", reject_fn);
        Ok(res_obj)
    });

    let all_settled_static = vm.alloc_native_function("allSettled", 1, |vm, _this, args| {
        let iterable = args.first().copied().unwrap_or(JsValue::undefined());
        if let Some(ptr) = iterable.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::Array(elements) = &obj.kind {
                let elements_clone = elements.clone();
                let result_promise = vm.alloc_promise(crate::object::PromiseState::Pending, JsValue::undefined());
                if elements_clone.is_empty() {
                    let empty_arr = vm.alloc_array(Vec::new());
                    vm.resolve_promise(result_promise, empty_arr);
                    return Ok(result_promise);
                }
                let count = elements_clone.len();
                let remaining = Arc::new(std::sync::atomic::AtomicUsize::new(count));
                let results = Arc::new(std::sync::Mutex::new(vec![JsValue::undefined(); count]));

                for (idx, elem) in elements_clone.into_iter().enumerate() {
                    let r_p = result_promise;
                    let rem = Arc::clone(&remaining);
                    let res_store = Arc::clone(&results);

                    let resolve_item = vm.alloc_closure_native_function("settled_res", 1, Arc::new(move |vm: &mut Vm, _this: JsValue, args: &[JsValue]| -> Result<JsValue, VmError> {
                        let v = args.first().copied().unwrap_or(JsValue::undefined());
                        let item_obj = vm.alloc_object();
                        let status_str = vm.alloc_string("fulfilled".into());
                        vm.set_property(item_obj, "status", status_str);
                        vm.set_property(item_obj, "value", v);
                        {
                            let mut lock = res_store.lock().unwrap();
                            lock[idx] = item_obj;
                        }
                        if rem.fetch_sub(1, std::sync::atomic::Ordering::SeqCst) == 1 {
                            let final_res = {
                                let lock = res_store.lock().unwrap();
                                lock.clone()
                            };
                            let arr = vm.alloc_array(final_res);
                            vm.resolve_promise(r_p, arr);
                        }
                        Ok(JsValue::undefined())
                    }));

                    let rem_rej = Arc::clone(&remaining);
                    let res_store_rej = Arc::clone(&results);
                    let reject_item = vm.alloc_closure_native_function("settled_rej", 1, Arc::new(move |vm: &mut Vm, _this: JsValue, args: &[JsValue]| -> Result<JsValue, VmError> {
                        let err = args.first().copied().unwrap_or(JsValue::undefined());
                        let item_obj = vm.alloc_object();
                        let status_str = vm.alloc_string("rejected".into());
                        vm.set_property(item_obj, "status", status_str);
                        vm.set_property(item_obj, "reason", err);
                        {
                            let mut lock = res_store_rej.lock().unwrap();
                            lock[idx] = item_obj;
                        }
                        if rem_rej.fetch_sub(1, std::sync::atomic::Ordering::SeqCst) == 1 {
                            let final_res = {
                                let lock = res_store_rej.lock().unwrap();
                                lock.clone()
                            };
                            let arr = vm.alloc_array(final_res);
                            vm.resolve_promise(r_p, arr);
                        }
                        Ok(JsValue::undefined())
                    }));

                    if let Some(p_ptr) = elem.as_object_ptr() {
                        let p_obj = unsafe { &*(p_ptr as *mut JsObject) };
                        if matches!(p_obj.kind, ObjectKind::Promise(_)) {
                            let then_fn = vm.get_property(elem, "then");
                            let _ = vm.call(then_fn, elem, &[resolve_item, reject_item]);
                            continue;
                        }
                    }
                    let _ = vm.call(resolve_item, JsValue::undefined(), &[elem]);
                }

                return Ok(result_promise);
            }
        }
        let empty_arr = vm.alloc_array(Vec::new());
        Ok(vm.alloc_promise(crate::object::PromiseState::Fulfilled, empty_arr))
    });

    let any_static = vm.alloc_native_function("any", 1, |vm, _this, args| {
        let iterable = args.first().copied().unwrap_or(JsValue::undefined());
        if let Some(ptr) = iterable.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::Array(elements) = &obj.kind {
                let elements_clone = elements.clone();
                let result_promise = vm.alloc_promise(crate::object::PromiseState::Pending, JsValue::undefined());
                if elements_clone.is_empty() {
                    let err_str = vm.alloc_string("All promises were rejected".into());
                    vm.reject_promise(result_promise, err_str);
                    return Ok(result_promise);
                }
                let count = elements_clone.len();
                let remaining = Arc::new(std::sync::atomic::AtomicUsize::new(count));
                let errors = Arc::new(std::sync::Mutex::new(vec![JsValue::undefined(); count]));

                for (idx, elem) in elements_clone.into_iter().enumerate() {
                    let r_p = result_promise;
                    let resolve_item = vm.alloc_closure_native_function("any_res", 1, Arc::new(move |vm: &mut Vm, _this: JsValue, args: &[JsValue]| -> Result<JsValue, VmError> {
                        let v = args.first().copied().unwrap_or(JsValue::undefined());
                        vm.resolve_promise(r_p, v);
                        Ok(JsValue::undefined())
                    }));

                    let rem = Arc::clone(&remaining);
                    let err_store = Arc::clone(&errors);
                    let reject_item = vm.alloc_closure_native_function("any_rej", 1, Arc::new(move |vm: &mut Vm, _this: JsValue, args: &[JsValue]| -> Result<JsValue, VmError> {
                        let err = args.first().copied().unwrap_or(JsValue::undefined());
                        {
                            let mut lock = err_store.lock().unwrap();
                            lock[idx] = err;
                        }
                        if rem.fetch_sub(1, std::sync::atomic::Ordering::SeqCst) == 1 {
                            let final_errs = {
                                let lock = err_store.lock().unwrap();
                                lock.clone()
                            };
                            let err_arr = vm.alloc_array(final_errs);
                            vm.reject_promise(r_p, err_arr);
                        }
                        Ok(JsValue::undefined())
                    }));

                    if let Some(p_ptr) = elem.as_object_ptr() {
                        let p_obj = unsafe { &*(p_ptr as *mut JsObject) };
                        if matches!(p_obj.kind, ObjectKind::Promise(_)) {
                            let then_fn = vm.get_property(elem, "then");
                            let _ = vm.call(then_fn, elem, &[resolve_item, reject_item]);
                            continue;
                        }
                    }
                    let _ = vm.call(resolve_item, JsValue::undefined(), &[elem]);
                }
                return Ok(result_promise);
            }
        }
        let err_str = vm.alloc_string("All promises were rejected".into());
        Ok(vm.alloc_promise(crate::object::PromiseState::Rejected, err_str))
    });
    vec![
        ("allSettled", all_settled_static),
        ("any", any_static),
        ("withResolvers", with_resolvers_static),
    ]
}
