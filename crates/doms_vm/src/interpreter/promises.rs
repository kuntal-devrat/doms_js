//! Promise resolution and `then`/`catch`/`finally` plumbing.

use super::*;

impl Vm {
    pub fn resolve_promise(&mut self, promise_val: JsValue, resolution: JsValue) {
        if let Some(ptr) = resolution.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::Promise(_) = &obj.kind {
                let then_fn = self.get_property(resolution, "then");
                if then_fn.is_object() {
                    let p_resolve = promise_val;
                    let fulfill_cb = self.alloc_closure_native_function("chain_resolve", 1, Arc::new(move |vm, _, args| {
                        let inner_val = args.first().copied().unwrap_or(JsValue::undefined());
                        vm.resolve_promise(p_resolve, inner_val);
                        Ok(JsValue::undefined())
                    }));
                    let p_reject = promise_val;
                    let reject_cb = self.alloc_closure_native_function("chain_reject", 1, Arc::new(move |vm, _, args| {
                        let inner_err = args.first().copied().unwrap_or(JsValue::undefined());
                        vm.reject_promise(p_reject, inner_err);
                        Ok(JsValue::undefined())
                    }));
                    let _ = self.call(then_fn, resolution, &[fulfill_cb, reject_cb]);
                    return;
                }
            }
        }

        if let Some(ptr) = promise_val.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::Promise(ref mut p) = obj.kind {
                if p.state == PromiseState::Pending {
                    p.state = PromiseState::Fulfilled;
                    p.result = resolution;
                    let reactions = std::mem::take(&mut p.fulfill_reactions);
                    p.reject_reactions.clear();
                    for cb in reactions {
                        self.enqueue_microtask(cb, vec![resolution]);
                    }
                }
            }
        }
    }

    pub fn reject_promise(&mut self, promise_val: JsValue, reason: JsValue) {
        if let Some(ptr) = promise_val.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::Promise(ref mut p) = obj.kind {
                if p.state == PromiseState::Pending {
                    p.state = PromiseState::Rejected;
                    p.result = reason;
                    let reactions = std::mem::take(&mut p.reject_reactions);
                    p.fulfill_reactions.clear();
                    for cb in reactions {
                        self.enqueue_microtask(cb, vec![reason]);
                    }
                }
            }
        }
    }

    pub fn promise_then(&mut self, promise_val: JsValue, on_fulfilled: JsValue, on_rejected: JsValue) -> Result<JsValue, VmError> {
        let ptr = promise_val.as_object_ptr().ok_or_else(|| VmError::TypeError("Promise.prototype.then called on non-object".into()))?;
        let is_promise = {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            matches!(obj.kind, ObjectKind::Promise(_))
        };
        if !is_promise {
            return Err(VmError::TypeError("Promise.prototype.then called on non-promise".into()));
        }

        let next_promise = self.alloc_promise(PromiseState::Pending, JsValue::undefined());
        let obj = unsafe { &mut *(ptr as *mut JsObject) };
        if let ObjectKind::Promise(ref mut p) = obj.kind {
            match p.state {
                PromiseState::Fulfilled => {
                    let val = p.result;
                    let next_p = next_promise;
                    if on_fulfilled.is_object() {
                        let cb_task = self.alloc_closure_native_function("then_cb", 0, Arc::new(move |vm: &mut Vm, _, _| -> Result<JsValue, VmError> {
                            match vm.call(on_fulfilled, JsValue::undefined(), &[val]) {
                                Ok(res) => vm.resolve_promise(next_p, res),
                                Err(e) => {
                                    let err_val = vm.alloc_string(e.to_string());
                                    vm.reject_promise(next_p, err_val);
                                }
                            }
                            Ok(JsValue::undefined())
                        }));
                        self.enqueue_microtask(cb_task, Vec::new());
                    } else {
                        let cb_task = self.alloc_closure_native_function("fwd_cb", 0, Arc::new(move |vm: &mut Vm, _, _| -> Result<JsValue, VmError> {
                            vm.resolve_promise(next_p, val);
                            Ok(JsValue::undefined())
                        }));
                        self.enqueue_microtask(cb_task, Vec::new());
                    }
                }
                PromiseState::Rejected => {
                    let err = p.result;
                    let next_p = next_promise;
                    if on_rejected.is_object() {
                        let cb_task = self.alloc_closure_native_function("catch_cb", 0, Arc::new(move |vm: &mut Vm, _, _| -> Result<JsValue, VmError> {
                            match vm.call(on_rejected, JsValue::undefined(), &[err]) {
                                Ok(res) => vm.resolve_promise(next_p, res),
                                Err(e) => {
                                    let err_val = vm.alloc_string(e.to_string());
                                    vm.reject_promise(next_p, err_val);
                                }
                            }
                            Ok(JsValue::undefined())
                        }));
                        self.enqueue_microtask(cb_task, Vec::new());
                    } else {
                        let cb_task = self.alloc_closure_native_function("rfwd_cb", 0, Arc::new(move |vm: &mut Vm, _, _| -> Result<JsValue, VmError> {
                            vm.reject_promise(next_p, err);
                            Ok(JsValue::undefined())
                        }));
                        self.enqueue_microtask(cb_task, Vec::new());
                    }
                }
                PromiseState::Pending => {
                    let next_p = next_promise;
                    if on_fulfilled.is_object() {
                        let cb_task = self.alloc_closure_native_function("pending_then", 1, Arc::new(move |vm: &mut Vm, _, args: &[JsValue]| -> Result<JsValue, VmError> {
                            let val = args.first().copied().unwrap_or_default();
                            match vm.call(on_fulfilled, JsValue::undefined(), &[val]) {
                                Ok(res) => vm.resolve_promise(next_p, res),
                                Err(e) => {
                                    let err_val = vm.alloc_string(e.to_string());
                                    vm.reject_promise(next_p, err_val);
                                }
                            }
                            Ok(JsValue::undefined())
                        }));
                        p.fulfill_reactions.push(cb_task);
                    } else {
                        let cb_task = self.alloc_closure_native_function("pending_fwd", 1, Arc::new(move |vm: &mut Vm, _, args: &[JsValue]| -> Result<JsValue, VmError> {
                            let val = args.first().copied().unwrap_or_default();
                            vm.resolve_promise(next_p, val);
                            Ok(JsValue::undefined())
                        }));
                        p.fulfill_reactions.push(cb_task);
                    }

                    if on_rejected.is_object() {
                        let cb_task = self.alloc_closure_native_function("pending_catch", 1, Arc::new(move |vm: &mut Vm, _, args: &[JsValue]| -> Result<JsValue, VmError> {
                            let err = args.first().copied().unwrap_or_default();
                            match vm.call(on_rejected, JsValue::undefined(), &[err]) {
                                Ok(res) => vm.resolve_promise(next_p, res),
                                Err(e) => {
                                    let err_val = vm.alloc_string(e.to_string());
                                    vm.reject_promise(next_p, err_val);
                                }
                            }
                            Ok(JsValue::undefined())
                        }));
                        p.reject_reactions.push(cb_task);
                    } else {
                        let cb_task = self.alloc_closure_native_function("pending_rfwd", 1, Arc::new(move |vm: &mut Vm, _, args: &[JsValue]| -> Result<JsValue, VmError> {
                            let err = args.first().copied().unwrap_or_default();
                            vm.reject_promise(next_p, err);
                            Ok(JsValue::undefined())
                        }));
                        p.reject_reactions.push(cb_task);
                    }
                }
            }
        }

        Ok(next_promise)
    }

    pub(crate) fn get_promise_then(&mut self, promise_val: JsValue) -> Option<JsValue> {
        Some(self.alloc_closure_native_function("then", 2, Arc::new(move |vm: &mut Vm, _this: JsValue, args: &[JsValue]| -> Result<JsValue, VmError> {
            let on_fulfilled = args.first().copied().unwrap_or_default();
            let on_rejected = args.get(1).copied().unwrap_or_default();
            vm.promise_then(promise_val, on_fulfilled, on_rejected)
        })))
    }

    pub(crate) fn get_promise_catch(&mut self, promise_val: JsValue) -> Option<JsValue> {
        Some(self.alloc_closure_native_function("catch", 1, Arc::new(move |vm: &mut Vm, _this: JsValue, args: &[JsValue]| -> Result<JsValue, VmError> {
            let on_rejected = args.first().copied().unwrap_or_default();
            let then_fn = vm.get_property(promise_val, "then");
            vm.call(then_fn, promise_val, &[JsValue::undefined(), on_rejected])
        })))
    }

    pub(crate) fn get_promise_finally(&mut self, promise_val: JsValue) -> Option<JsValue> {
        Some(self.alloc_closure_native_function("finally", 1, Arc::new(move |vm: &mut Vm, _this: JsValue, args: &[JsValue]| -> Result<JsValue, VmError> {
            let on_finally = args.first().copied().unwrap_or_default();
            let then_fn = vm.get_property(promise_val, "then");
            vm.call(then_fn, promise_val, &[on_finally, on_finally])
        })))
    }
}
