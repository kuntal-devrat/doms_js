//! Function return, `await` suspension, generators, jumps and exception/catch stack control.

use super::*;

impl Vm {
    pub(crate) fn op_return(&mut self, target_depth: usize) -> Result<Option<JsValue>, VmError> {
        let ret_val = self.pop().unwrap_or_default();
        let finished = self.frames.pop().unwrap();
        while let Some(top) = self.catch_stack.last() {
            if top.frame_idx >= self.frames.len() {
                self.catch_stack.pop();
            } else {
                break;
            }
        }
        let ret_val = if finished.callee_func.as_ref().map_or(false, |f| f.is_derived_constructor) {
            if ret_val.is_object() {
                ret_val
            } else if !ret_val.is_undefined() {
                return Err(VmError::TypeError(
                    "Derived constructor may only return object or undefined".into(),
                ));
            } else if finished.this_val == JsValue::uninitialized() {
                return Err(VmError::ReferenceError(
                    "Must call super constructor in derived class before returning".into(),
                ));
            } else {
                finished.this_val
            }
        } else {
            ret_val
        };
        if let Some(g_val) = finished.gen_val {
            if let Some(g_ptr) = g_val.as_object_ptr() {
                let g_obj = unsafe { &mut *(g_ptr as *mut JsObject) };
                if let ObjectKind::Generator(ref mut g) = g_obj.kind {
                    g.state = crate::object::GeneratorState::Completed;
                }
            }
            let iter_res = self.make_iter_result(ret_val, true);
            self.stack.truncate(finished.base_sp);
            if self.frames.len() == target_depth {
                return Ok(Some(iter_res));
            }
            self.push(iter_res);
        } else {
            self.stack.truncate(finished.base_sp);
            if self.frames.len() == target_depth {
                return Ok(Some(ret_val));
            }
            self.push(ret_val);
        }
        Ok(None)
    }

    pub(crate) fn await_value(&mut self, val: JsValue) -> Result<JsValue, VmError> {
        let resolved = if let Some(ptr) = val.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::Promise(_) = &obj.kind {
                loop {
                    let current_state = {
                        let o = unsafe { &*(ptr as *mut JsObject) };
                        if let ObjectKind::Promise(p) = &o.kind {
                            p.state
                        } else {
                            break;
                        }
                    };
                    if current_state != PromiseState::Pending {
                        break;
                    }
                    if self.microtasks.is_empty() {
                        break;
                    }
                    self.drain_microtasks()?;
                }
                self.drain_microtasks()?;
                let o_final = unsafe { &*(ptr as *mut JsObject) };
                if let ObjectKind::Promise(p) = &o_final.kind {
                    match p.state {
                        PromiseState::Fulfilled => p.result,
                        PromiseState::Rejected => {
                            return Err(VmError::Thrown(p.result));
                        }
                        PromiseState::Pending => val,
                    }
                } else {
                    val
                }
            } else {
                let then_fn = self.get_property(val, "then");
                if self.is_callable(then_fn) {
                    let result_val = Arc::new(std::sync::Mutex::new(None));
                    let error_val = Arc::new(std::sync::Mutex::new(None));
                    let res_clone = Arc::clone(&result_val);
                    let rej_clone = Arc::clone(&error_val);
                    let on_resolve = self.alloc_closure_native_function(
                        "resolve",
                        1,
                        Arc::new(move |_vm, _this, args| {
                            let v = args.first().copied().unwrap_or_else(JsValue::undefined);
                            let mut l = res_clone.lock().unwrap();
                            *l = Some(v);
                            Ok(JsValue::undefined())
                        }),
                    );
                    let on_reject = self.alloc_closure_native_function(
                        "reject",
                        1,
                        Arc::new(move |_vm, _this, args| {
                            let e = args.first().copied().unwrap_or_else(JsValue::undefined);
                            let mut l = rej_clone.lock().unwrap();
                            *l = Some(e);
                            Ok(JsValue::undefined())
                        }),
                    );
                    match self.call(then_fn, val, &[on_resolve, on_reject]) {
                        Ok(_) => {
                            self.drain_microtasks()?;
                            if let Some(e) = error_val.lock().unwrap().take() {
                                return Err(VmError::Thrown(e));
                            }
                            let final_res = result_val.lock().unwrap().take();
                            final_res.unwrap_or(val)
                        }
                        Err(err) => {
                            return Err(err);
                        }
                    }
                } else {
                    val
                }
            }
        } else {
            val
        };
        Ok(resolved)
    }

    pub(crate) fn op_await(&mut self, target_depth: usize) -> Result<(), VmError> {
        let val = self.pop()?;
        match self.await_value(val) {
            Ok(resolved) => {
                self.push(resolved);
                Ok(())
            }
            Err(err) => {
                self.handle_exception_at_depth(err, target_depth)?;
                Ok(())
            }
        }
    }

    pub(crate) fn op_generator_start(&mut self) -> Result<JsValue, VmError> {
        let frame = self.frames.pop().unwrap();
        let gen_func = frame.callee_func.clone().unwrap();
        let generator_instance = JsGenerator {
            function: gen_func,
            ip: frame.ip,
            stack: Vec::new(),
            locals: frame.locals,
            scopes: frame.scopes,
            env_obj: frame.env_obj,
            catch_handlers: Vec::new(),
            state: crate::object::GeneratorState::SuspendedStart,
            this_val: frame.this_val,
            arguments: frame.arguments,
            mapped_arguments: frame.mapped_arguments,
            async_queue: Vec::new(),
        };
        let gen_val = self.alloc_generator(generator_instance);
        self.stack.truncate(frame.base_sp);
        Ok(gen_val)
    }

    pub(crate) fn op_yield(&mut self, target_depth: usize) -> Result<Option<JsValue>, VmError> {
        let yielded_val = self.pop().unwrap_or_default();
        let prev_temp = self.temp_roots.len();
        self.temp_roots.push(yielded_val);
        let gen_frame_idx = self.frames.len() - 1;
        let frame = self.frames.last_mut().unwrap();
        let base_sp = frame.base_sp;
        if let Some(g_val) = frame.gen_val {
            if let Some(g_ptr) = g_val.as_object_ptr() {
                let g_obj = unsafe { &mut *(g_ptr as *mut JsObject) };
                if let ObjectKind::Generator(ref mut g) = g_obj.kind {
                    g.ip = frame.ip;
                    g.stack = self.stack[base_sp..].to_vec();
                    g.locals = frame.locals.clone();
                    g.scopes = frame.scopes.clone();
                    g.env_obj = frame.env_obj;
                    g.state = crate::object::GeneratorState::SuspendedYield;

                    let mut saved_handlers = Vec::new();
                    while let Some(top) = self.catch_stack.last() {
                        if top.frame_idx >= gen_frame_idx {
                            let h = self.catch_stack.pop().unwrap();
                            saved_handlers.push(SavedCatchHandler {
                                frame_offset: h.frame_idx - gen_frame_idx,
                                target_ip: h.target_ip,
                                stack_len_offset: h.stack_len.saturating_sub(base_sp),
                                scopes_len: h.scopes_len,
                                is_finally: h.is_finally,
                                finally_start: h.finally_start,
                                kind_slot: h.kind_slot,
                            });
                        } else {
                            break;
                        }
                    }
                    saved_handlers.reverse();
                    g.catch_handlers = saved_handlers;
                }
            }
        }
        let finished = self.frames.pop().unwrap();
        let cur_yielded = self.temp_roots[prev_temp];
        let iter_res = self.make_iter_result(cur_yielded, false);
        self.temp_roots.truncate(prev_temp);
        self.stack.truncate(finished.base_sp);
        if self.frames.len() == target_depth {
            return Ok(Some(iter_res));
        }
        self.push(iter_res);
        Ok(None)
    }

    #[allow(dead_code)]
    pub(crate) fn op_jump_if_false(&mut self, target: i32) -> Result<(), VmError> {
        let cond = self.pop()?;
        if !cond.is_truthy() {
            let frame = self.frames.last_mut().unwrap();
            frame.ip = target as usize;
        }
        Ok(())
    }

    #[allow(dead_code)]
    pub(crate) fn op_jump_if_true(&mut self, target: i32) -> Result<(), VmError> {
        let cond = self.pop()?;
        if cond.is_truthy() {
            let frame = self.frames.last_mut().unwrap();
            frame.ip = target as usize;
        }
        Ok(())
    }

    pub(crate) fn op_push_catch(&mut self, target: i32) {
        let scopes_len = self.frames.last().map(|f| f.scopes.len()).unwrap_or(0);
        self.catch_stack.push(CatchHandler {
            frame_idx: self.frames.len().saturating_sub(1),
            target_ip: target as usize,
            stack_len: self.stack.len(),
            scopes_len,
            is_finally: false,
            finally_start: 0,
            kind_slot: 0,
        });
    }

    pub(crate) fn op_push_finally(&mut self, target: i32, finally_start: i32, kind_slot: u16) {
        let scopes_len = self.frames.last().map(|f| f.scopes.len()).unwrap_or(0);
        self.catch_stack.push(CatchHandler {
            frame_idx: self.frames.len().saturating_sub(1),
            target_ip: target as usize,
            stack_len: self.stack.len(),
            scopes_len,
            is_finally: true,
            finally_start: finally_start as usize,
            kind_slot,
        });
    }

    pub(crate) fn op_end_finally(
        &mut self,
        kind_slot: u16,
        target_depth: usize,
    ) -> Result<Option<JsValue>, VmError> {
        let frame = self.frames.last().unwrap();
        let kind =
            frame.locals.get(kind_slot as usize).and_then(|v| v.as_number()).unwrap_or(0.0) as i32;
        let val =
            frame.locals.get((kind_slot + 1) as usize).copied().unwrap_or_else(JsValue::undefined);
        match kind {
            0 => Ok(None),
            1 => {
                self.handle_exception_at_depth(VmError::Thrown(val), target_depth)?;
                Ok(None)
            }
            2 => {
                self.push(val);
                self.op_return(target_depth)
            }
            _ => Ok(None),
        }
    }

    pub(crate) fn op_push_catch_scope(
        &mut self,
        name_idx: u16,
        chunk: &BytecodeChunk,
    ) -> Result<(), VmError> {
        let name = match &chunk.constants[name_idx as usize] {
            Constant::String(s) => s.clone(),
            _ => return Err(VmError::TypeError("Catch param name must be string".into())),
        };
        let err = self.pop()?;
        let prev_temp = self.temp_roots.len();
        self.temp_roots.push(err);
        let obj = self.alloc_object();
        self.temp_roots.push(obj);
        let cur_err = self.temp_roots[prev_temp];
        let cur_obj = self.temp_roots[prev_temp + 1];
        self.define_builtin_property(
            cur_obj,
            &name,
            cur_err,
            crate::shape::PropertyAttributes {
                writable: true,
                enumerable: true,
                configurable: false,
            },
        );
        let cur_obj = self.temp_roots[prev_temp + 1];
        self.temp_roots.truncate(prev_temp);
        let frame = self.frames.last_mut().unwrap();
        frame.scopes.push(cur_obj);
        Ok(())
    }
}
