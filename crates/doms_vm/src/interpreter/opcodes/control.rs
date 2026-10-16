//! Function return, `await` suspension, generators, jumps and exception/catch stack control.

use super::*;

impl Vm {
    pub(crate) fn op_return(
        &mut self,
        target_depth: usize,
    ) -> Result<Option<JsValue>, VmError> {
        let ret_val = self.pop().unwrap_or_default();
        let finished = self.frames.pop().unwrap();
        while let Some(top) = self.catch_stack.last() {
            if top.frame_idx >= self.frames.len() {
                self.catch_stack.pop();
            } else {
                break;
            }
        }
        if let Some(g_ptr) = finished.gen_ptr {
            let g_obj = unsafe { &mut *(g_ptr) };
            if let ObjectKind::Generator(ref mut g) = g_obj.kind {
                g.state = crate::object::GeneratorState::Completed;
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

    pub(crate) fn op_await(
        &mut self,
        target_depth: usize,
    ) -> Result<(), VmError> {
        let val = self.pop()?;
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
                            self.handle_exception_at_depth(VmError::Thrown(p.result), target_depth)?;
                            return Ok(());
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
                    let on_resolve = self.alloc_closure_native_function("resolve", 1, Arc::new(move |_vm, _this, args| {
                        let v = args.first().copied().unwrap_or_else(JsValue::undefined);
                        let mut l = res_clone.lock().unwrap();
                        *l = Some(v);
                        Ok(JsValue::undefined())
                    }));
                    let on_reject = self.alloc_closure_native_function("reject", 1, Arc::new(move |_vm, _this, args| {
                        let e = args.first().copied().unwrap_or_else(JsValue::undefined);
                        let mut l = rej_clone.lock().unwrap();
                        *l = Some(e);
                        Ok(JsValue::undefined())
                    }));
                    match self.call(then_fn, val, &[on_resolve, on_reject]) {
                        Ok(_) => {
                            self.drain_microtasks()?;
                            if let Some(e) = error_val.lock().unwrap().take() {
                                self.handle_exception_at_depth(VmError::Thrown(e), target_depth)?;
                                return Ok(());
                            }
                            let final_res = result_val.lock().unwrap().take();
                            final_res.unwrap_or(val)
                        }
                        Err(err) => {
                            self.handle_exception_at_depth(err, target_depth)?;
                            return Ok(());
                        }
                    }
                } else {
                    val
                }
            }
        } else {
            val
        };
        self.push(resolved);
        Ok(())
    }

    pub(crate) fn op_generator_start(&mut self) -> Result<JsValue, VmError> {
        let frame = self.frames.pop().unwrap();
        let gen_func = frame.callee_func.clone().unwrap();
        let generator_instance = JsGenerator {
            function: gen_func,
            ip: frame.ip,
            stack: frame.locals,
            state: crate::object::GeneratorState::SuspendedStart,
            this_val: frame.this_val,
            arguments: frame.arguments,
            mapped_arguments: frame.mapped_arguments,
        };
        let gen_val = self.alloc_generator(generator_instance);
        self.stack.truncate(frame.base_sp);
        Ok(gen_val)
    }

    pub(crate) fn op_yield(&mut self, target_depth: usize) -> Result<Option<JsValue>, VmError> {
        let yielded_val = self.pop().unwrap_or_default();
        let frame = self.frames.last_mut().unwrap();
        if let Some(g_ptr) = frame.gen_ptr {
            let g_obj = unsafe { &mut *(g_ptr) };
            if let ObjectKind::Generator(ref mut g) = g_obj.kind {
                g.ip = frame.ip;
                g.stack = self.stack[frame.base_sp..].to_vec();
                g.state = crate::object::GeneratorState::SuspendedYield;
            }
        }
        let finished = self.frames.pop().unwrap();
        let iter_res = self.make_iter_result(yielded_val, false);
        self.stack.truncate(finished.base_sp);
        if self.frames.len() == target_depth {
            return Ok(Some(iter_res));
        }
        self.push(iter_res);
        Ok(None)
    }

    pub(crate) fn op_jump_if_false(&mut self, target: i32) -> Result<(), VmError> {
        let cond = self.pop()?;
        if !cond.is_truthy() {
            let frame = self.frames.last_mut().unwrap();
            frame.ip = target as usize;
        }
        Ok(())
    }

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
        });
    }

    pub(crate) fn op_push_catch_scope(&mut self, name_idx: u16, chunk: &BytecodeChunk) -> Result<(), VmError> {
        let name = match &chunk.constants[name_idx as usize] {
            Constant::String(s) => s.clone(),
            _ => return Err(VmError::TypeError("Catch param name must be string".into())),
        };
        let err = self.pop()?;
        let obj = self.alloc_object();
        self.set_property(obj, &name, err);
        let frame = self.frames.last_mut().unwrap();
        frame.scopes.push(obj);
        Ok(())
    }
}
