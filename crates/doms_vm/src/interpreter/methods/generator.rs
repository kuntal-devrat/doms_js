//! Generator object protocol (`next`/`return`/`throw`).

use super::*;

impl Vm {
    pub(crate) fn get_generator_next(&mut self, gen_val: JsValue) -> Option<JsValue> {
        Some(self.alloc_closure_native_function("next", 1, Arc::new(move |vm: &mut Vm, _this: JsValue, args: &[JsValue]| -> Result<JsValue, VmError> {
            let send_val = args.first().copied().unwrap_or_default();
            vm.generator_next(gen_val, send_val)
        })))
    }

    pub(crate) fn get_generator_return(&mut self, gen_val: JsValue) -> Option<JsValue> {
        Some(self.alloc_closure_native_function("return", 1, Arc::new(move |vm: &mut Vm, _this: JsValue, args: &[JsValue]| -> Result<JsValue, VmError> {
            let ret_val = args.first().copied().unwrap_or_default();
            vm.generator_return(gen_val, ret_val)
        })))
    }

    pub(crate) fn get_generator_throw(&mut self, gen_val: JsValue) -> Option<JsValue> {
        Some(self.alloc_closure_native_function("throw", 1, Arc::new(move |vm: &mut Vm, _this: JsValue, args: &[JsValue]| -> Result<JsValue, VmError> {
            let err_val = args.first().copied().unwrap_or_default();
            vm.generator_throw(gen_val, err_val)
        })))
    }

    pub fn generator_return(&mut self, gen_val: JsValue, ret_val: JsValue) -> Result<JsValue, VmError> {
        let ptr = gen_val.as_object_ptr().ok_or(VmError::TypeError("Not a generator".into()))?;
        let (gen_fn, _ip, stack, locals, scopes, env_obj, catch_handlers, state, this_val, args_obj, mapped_args) = {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            match &obj.kind {
                ObjectKind::Generator(g) => (
                    g.function.clone(),
                    g.ip,
                    g.stack.clone(),
                    g.locals.clone(),
                    g.scopes.clone(),
                    g.env_obj,
                    g.catch_handlers.clone(),
                    g.state,
                    g.this_val,
                    g.arguments,
                    g.mapped_arguments.clone(),
                ),
                _ => return Err(VmError::TypeError("Not a generator".into())),
            }
        };

        if state == crate::object::GeneratorState::Completed || state == crate::object::GeneratorState::SuspendedStart {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::Generator(ref mut g) = obj.kind {
                g.state = crate::object::GeneratorState::Completed;
                g.catch_handlers.clear();
            }
            return Ok(self.make_iter_result(ret_val, true));
        }

        let finally_handler = catch_handlers.iter().rev().find(|h| h.is_finally).cloned();
        if let Some(h) = finally_handler {
            {
                let obj = unsafe { &mut *(ptr as *mut JsObject) };
                if let ObjectKind::Generator(ref mut g) = obj.kind {
                    g.state = crate::object::GeneratorState::Executing;
                }
            }

            let mut locals = locals;
            if locals.len() <= (h.kind_slot + 1) as usize {
                locals.resize((h.kind_slot + 2) as usize, JsValue::undefined());
            }
            locals[h.kind_slot as usize] = JsValue::from_double(2.0);
            locals[(h.kind_slot + 1) as usize] = ret_val;

            let chunk = Arc::clone(&gen_fn.chunk);
            let target_depth = self.frames.len();
            let gen_frame_idx = self.frames.len();
            let base_sp = self.stack.len();
            for val in &stack {
                self.push(*val);
            }
            self.frames.push(CallFrame {
                chunk,
                ip: h.finally_start,
                base_sp,
                this_val,
                gen_val: Some(gen_val),
                locals,
                local_names: gen_fn.local_names.clone(),
                lexical_local_names: gen_fn.lexical_local_names.clone(),
                scopes,
                upvalues_count: gen_fn.upvalues.len(),
                env_obj,
                arguments: args_obj,
                is_strict: gen_fn.is_strict,
                new_target: None,
                is_function: true,
                is_method: gen_fn.is_method,
                is_arrow: gen_fn.is_arrow,
                has_param_exprs: gen_fn.has_param_exprs,
                in_param_init: false,
                is_indirect_eval: false,
                callee_func: Some(gen_fn.clone()),
                callee_val: None,
                mapped_arguments: mapped_args,
                home_object: gen_fn.home_object,
            });

            let mut found_h = false;
            for handler in catch_handlers.iter().rev() {
                if !found_h {
                    if handler.target_ip == h.target_ip {
                        found_h = true;
                    }
                    continue;
                }
                self.catch_stack.push(CatchHandler {
                    frame_idx: gen_frame_idx + handler.frame_offset,
                    target_ip: handler.target_ip,
                    stack_len: base_sp + handler.stack_len_offset,
                    scopes_len: handler.scopes_len,
                    is_finally: handler.is_finally,
                    finally_start: handler.finally_start,
                    kind_slot: handler.kind_slot,
                });
            }

            let prev_temp = self.temp_roots.len();
            self.temp_roots.push(gen_val);
            let res = self.run_until_depth(target_depth);
            let cur_gen = self.temp_roots.pop().unwrap_or(gen_val);
            self.temp_roots.truncate(prev_temp);
            if res.is_err() {
                if let Some(ptr) = cur_gen.as_object_ptr() {
                    let obj = unsafe { &mut *(ptr as *mut JsObject) };
                    if let ObjectKind::Generator(ref mut g) = obj.kind {
                        g.state = crate::object::GeneratorState::Completed;
                        g.catch_handlers.clear();
                    }
                }
            }
            res
        } else {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::Generator(ref mut g) = obj.kind {
                g.state = crate::object::GeneratorState::Completed;
                g.catch_handlers.clear();
            }
            Ok(self.make_iter_result(ret_val, true))
        }
    }

    pub fn generator_throw(&mut self, gen_val: JsValue, err_val: JsValue) -> Result<JsValue, VmError> {
        let ptr = gen_val.as_object_ptr().ok_or(VmError::TypeError("Not a generator".into()))?;
        let (gen_fn, ip, stack, locals, scopes, env_obj, catch_handlers, state, this_val, args_obj, mapped_args) = {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            match &obj.kind {
                ObjectKind::Generator(g) => (
                    g.function.clone(),
                    g.ip,
                    g.stack.clone(),
                    g.locals.clone(),
                    g.scopes.clone(),
                    g.env_obj,
                    g.catch_handlers.clone(),
                    g.state,
                    g.this_val,
                    g.arguments,
                    g.mapped_arguments.clone(),
                ),
                _ => return Err(VmError::TypeError("Not a generator".into())),
            }
        };

        if state == crate::object::GeneratorState::Completed || state == crate::object::GeneratorState::SuspendedStart {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::Generator(ref mut g) = obj.kind {
                g.state = crate::object::GeneratorState::Completed;
                g.catch_handlers.clear();
            }
            return Err(VmError::Thrown(err_val));
        }

        {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::Generator(ref mut g) = obj.kind {
                g.state = crate::object::GeneratorState::Executing;
            }
        }

        let chunk = Arc::clone(&gen_fn.chunk);
        let target_depth = self.frames.len();
        let gen_frame_idx = self.frames.len();
        let base_sp = self.stack.len();
        for val in &stack {
            self.push(*val);
        }
        self.frames.push(CallFrame {
            chunk,
            ip,
            base_sp,
            this_val,
            gen_val: Some(gen_val),
            locals,
            local_names: gen_fn.local_names.clone(),
            lexical_local_names: gen_fn.lexical_local_names.clone(),
            scopes,
            upvalues_count: gen_fn.upvalues.len(),
            env_obj,
            arguments: args_obj,
            is_strict: gen_fn.is_strict,
            new_target: None,
            is_function: true,
            is_method: gen_fn.is_method,
            is_arrow: gen_fn.is_arrow,
            has_param_exprs: gen_fn.has_param_exprs,
            in_param_init: false,
            is_indirect_eval: false,
            callee_func: Some(gen_fn.clone()),
            callee_val: None,
            mapped_arguments: mapped_args,
            home_object: gen_fn.home_object,
        });

        for h in catch_handlers {
            self.catch_stack.push(CatchHandler {
                frame_idx: gen_frame_idx + h.frame_offset,
                target_ip: h.target_ip,
                stack_len: base_sp + h.stack_len_offset,
                scopes_len: h.scopes_len,
                is_finally: h.is_finally,
                finally_start: h.finally_start,
                kind_slot: h.kind_slot,
            });
        }

        if let Err(unhandled) = self.handle_exception_at_depth(VmError::Thrown(err_val), target_depth) {
            if let Some(ptr) = gen_val.as_object_ptr() {
                let obj = unsafe { &mut *(ptr as *mut JsObject) };
                if let ObjectKind::Generator(ref mut g) = obj.kind {
                    g.state = crate::object::GeneratorState::Completed;
                    g.catch_handlers.clear();
                }
            }
            self.frames.pop();
            self.stack.truncate(base_sp);
            return Err(unhandled);
        }

        let prev_temp = self.temp_roots.len();
        self.temp_roots.push(gen_val);
        let res = self.run_until_depth(target_depth);
        let cur_gen = self.temp_roots.pop().unwrap_or(gen_val);
        self.temp_roots.truncate(prev_temp);
        if res.is_err() {
            if let Some(ptr) = cur_gen.as_object_ptr() {
                let obj = unsafe { &mut *(ptr as *mut JsObject) };
                if let ObjectKind::Generator(ref mut g) = obj.kind {
                    g.state = crate::object::GeneratorState::Completed;
                    g.catch_handlers.clear();
                }
            }
        }
        res
    }

    pub fn make_iter_result(&mut self, value: JsValue, done: bool) -> JsValue {
        let obj = self.alloc_object();
        self.set_property(obj, "value", value);
        self.set_property(obj, "done", JsValue::from_boolean(done));
        obj
    }

    pub fn generator_next(&mut self, gen_val: JsValue, send_val: JsValue) -> Result<JsValue, VmError> {
        let ptr = gen_val.as_object_ptr().ok_or(VmError::TypeError("Not a generator".into()))?;
        let (gen_fn, ip, stack, locals, scopes, env_obj, catch_handlers, state, this_val, args_obj, mapped_args) = {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            match &obj.kind {
                ObjectKind::Generator(g) => (
                    g.function.clone(),
                    g.ip,
                    g.stack.clone(),
                    g.locals.clone(),
                    g.scopes.clone(),
                    g.env_obj,
                    g.catch_handlers.clone(),
                    g.state,
                    g.this_val,
                    g.arguments,
                    g.mapped_arguments.clone(),
                ),
                _ => return Err(VmError::TypeError("Not a generator".into())),
            }
        };

        if state == crate::object::GeneratorState::Completed {
            let res = self.make_iter_result(JsValue::undefined(), true);
            if gen_fn.is_async {
                let promise = self.alloc_promise(crate::object::PromiseState::Pending, JsValue::undefined());
                self.resolve_promise(promise, res);
                let _ = self.drain_microtasks();
                return Ok(promise);
            }
            return Ok(res);
        }

        let mut current_stack = stack;
        if state == crate::object::GeneratorState::SuspendedYield {
            current_stack.push(send_val);
        }

        {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::Generator(ref mut g) = obj.kind {
                g.state = crate::object::GeneratorState::Executing;
            }
        }

        let chunk = Arc::clone(&gen_fn.chunk);
        let target_depth = self.frames.len();
        let gen_frame_idx = self.frames.len();
        let base_sp = self.stack.len();
        for val in &current_stack {
            self.push(*val);
        }
        self.frames.push(CallFrame {
            chunk,
            ip,
            base_sp,
            this_val,
            gen_val: Some(gen_val),
            locals,
            local_names: gen_fn.local_names.clone(),
            lexical_local_names: gen_fn.lexical_local_names.clone(),
            scopes,
            upvalues_count: gen_fn.upvalues.len(),
            env_obj,
            arguments: args_obj,
            is_strict: gen_fn.is_strict,
            new_target: None,
            is_function: true,
            is_method: gen_fn.is_method,
            is_arrow: gen_fn.is_arrow,
            has_param_exprs: gen_fn.has_param_exprs,
            in_param_init: false,
            is_indirect_eval: false,
            callee_func: Some(gen_fn.clone()),
            callee_val: None,
            mapped_arguments: mapped_args,
            home_object: gen_fn.home_object,
        });

        for h in catch_handlers {
            self.catch_stack.push(CatchHandler {
                frame_idx: gen_frame_idx + h.frame_offset,
                target_ip: h.target_ip,
                stack_len: base_sp + h.stack_len_offset,
                scopes_len: h.scopes_len,
                is_finally: h.is_finally,
                finally_start: h.finally_start,
                kind_slot: h.kind_slot,
            });
        }

        let prev_temp = self.temp_roots.len();
        self.temp_roots.push(gen_val);
        let res = self.run_until_depth(target_depth);
        let cur_gen = self.temp_roots.pop().unwrap_or(gen_val);
        self.temp_roots.truncate(prev_temp);
        if res.is_err() {
            if let Some(ptr) = cur_gen.as_object_ptr() {
                let obj = unsafe { &mut *(ptr as *mut JsObject) };
                if let ObjectKind::Generator(ref mut g) = obj.kind {
                    g.state = crate::object::GeneratorState::Completed;
                    g.catch_handlers.clear();
                }
            }
        }
        if gen_fn.is_async {
            match res {
                Ok(val) => {
                    let promise = self.alloc_promise(crate::object::PromiseState::Pending, JsValue::undefined());
                    self.resolve_promise(promise, val);
                    let _ = self.drain_microtasks();
                    Ok(promise)
                }
                Err(e) => {
                    let promise = self.alloc_promise(crate::object::PromiseState::Pending, JsValue::undefined());
                    let err_val = self.alloc_string(e.to_string());
                    self.reject_promise(promise, err_val);
                    let _ = self.drain_microtasks();
                    Ok(promise)
                }
            }
        } else {
            res
        }
    }
}
