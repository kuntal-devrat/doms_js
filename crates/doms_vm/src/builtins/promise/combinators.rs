//! `Promise.all` and `Promise.race`.

use super::*;

pub(super) fn create_combinator_statics(vm: &mut Vm) -> Vec<(&'static str, JsValue)> {
    let all_static = vm.alloc_native_function("all", 1, |vm, _this, args| {
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

                    let resolve_item = vm.alloc_closure_native_function("resolve_item", 1, Arc::new(move |vm: &mut Vm, _this: JsValue, args: &[JsValue]| -> Result<JsValue, VmError> {
                        let v = args.first().copied().unwrap_or(JsValue::undefined());
                        {
                            let mut lock = res_store.lock().unwrap();
                            lock[idx] = v;
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

                    let reject_item = vm.alloc_closure_native_function("reject_item", 1, Arc::new(move |vm: &mut Vm, _this: JsValue, args: &[JsValue]| -> Result<JsValue, VmError> {
                        let err = args.first().copied().unwrap_or(JsValue::undefined());
                        vm.reject_promise(r_p, err);
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

    let race_static = vm.alloc_native_function("race", 1, |vm, _this, args| {
        let iterable = args.first().copied().unwrap_or(JsValue::undefined());
        let result_promise = vm.alloc_promise(crate::object::PromiseState::Pending, JsValue::undefined());
        if let Some(ptr) = iterable.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::Array(elements) = &obj.kind {
                let elements_clone = elements.clone();
                for elem in elements_clone {
                    let r_p = result_promise;
                    let resolve_item = vm.alloc_closure_native_function("race_res", 1, Arc::new(move |vm: &mut Vm, _this: JsValue, args: &[JsValue]| -> Result<JsValue, VmError> {
                        let v = args.first().copied().unwrap_or(JsValue::undefined());
                        vm.resolve_promise(r_p, v);
                        Ok(JsValue::undefined())
                    }));
                    let reject_item = vm.alloc_closure_native_function("race_rej", 1, Arc::new(move |vm: &mut Vm, _this: JsValue, args: &[JsValue]| -> Result<JsValue, VmError> {
                        let err = args.first().copied().unwrap_or(JsValue::undefined());
                        vm.reject_promise(r_p, err);
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
            }
        }
        Ok(result_promise)
    });
    vec![("all", all_static), ("race", race_static)]
}
