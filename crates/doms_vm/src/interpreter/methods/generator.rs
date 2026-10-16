//! Generator object protocol (`next`/`return`/`throw`).

use super::*;

impl Vm {
    pub(crate) fn get_generator_next(&mut self, gen_val: JsValue) -> Option<JsValue> {
        Some(self.alloc_closure_native_function("next", 1, Arc::new(move |vm: &mut Vm, _this: JsValue, args: &[JsValue]| -> Result<JsValue, VmError> {
            let send_val = args.first().copied().unwrap_or_default();
            vm.generator_next(gen_val, send_val)
        })))
    }

    pub fn make_iter_result(&mut self, value: JsValue, done: bool) -> JsValue {
        let obj = self.alloc_object();
        self.set_property(obj, "value", value);
        self.set_property(obj, "done", JsValue::from_boolean(done));
        obj
    }

    pub fn generator_next(&mut self, gen_val: JsValue, send_val: JsValue) -> Result<JsValue, VmError> {
        let ptr = gen_val.as_object_ptr().ok_or(VmError::TypeError("Not a generator".into()))?;
        let (gen_fn, ip, stack, state, this_val, args_obj, mapped_args) = {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            match &obj.kind {
                ObjectKind::Generator(g) => (g.function.clone(), g.ip, g.stack.clone(), g.state, g.this_val, g.arguments, g.mapped_arguments.clone()),
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
        let base_sp = self.stack.len();
        for val in &current_stack {
            self.push(*val);
        }
        self.frames.push(CallFrame {
            chunk,
            ip,
            base_sp,
            this_val,
            gen_ptr: Some(ptr as *mut JsObject),
            locals: current_stack.clone(),
            local_names: gen_fn.local_names.clone(),
            lexical_local_names: gen_fn.lexical_local_names.clone(),
            scopes: gen_fn.upvalues.clone(),
            upvalues_count: gen_fn.upvalues.len(),
            env_obj: None,
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
            mapped_arguments: mapped_args,
        });

        let res = self.run_until_depth(target_depth);
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
