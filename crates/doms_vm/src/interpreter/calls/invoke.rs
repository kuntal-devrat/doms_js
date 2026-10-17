use super::*;

impl Vm {
    pub fn execute_call_internal(
        &mut self,
        callee: JsValue,
        this_val: JsValue,
        args: Vec<JsValue>,
    ) -> Result<Option<JsValue>, VmError> {
        if self.frames.len() >= 1024 {
            return Err(VmError::RangeError("Maximum call stack size exceeded".to_string()));
        }
        if let Some(ptr) = callee.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            match &mut obj.kind {
                ObjectKind::NativeFunction(native) => {
                    let prev_temp = self.temp_roots.len();
                    self.temp_roots.push(this_val);
                    self.temp_roots.extend_from_slice(&args);
                    let res = native.call(self, this_val, &args);
                    self.temp_roots.truncate(prev_temp);
                    Ok(Some(res?))
                }
                ObjectKind::Function(f) => {
                    if !f.is_generator && !f.is_async && !self.is_constructing {
                        f.call_count = f.call_count.saturating_add(1);
                        if f.call_count >= crate::jit::JIT_HOT_THRESHOLD && f.jit_entry.is_none() {
                            f.jit_entry = crate::jit::JitCompiler::compile_native(&f.chunk);
                        }
                        if let Some(jit) = &f.jit_entry {
                            if let crate::jit::JitCode::Native { func_ptr, num_locals, .. } = &jit.code {
                                let mut raw_args: Vec<u64> = args.iter().map(|v| v.0).collect();
                                if raw_args.len() < *num_locals {
                                    raw_args.resize(*num_locals, crate::value::TAG_UNDEFINED);
                                }
                                let res = unsafe { (func_ptr)(raw_args.as_ptr(), raw_args.len()) };
                                return Ok(Some(JsValue(res)));
                            }
                        }
                    }

                    let func = f.clone();
                    let is_strict_func = func.is_strict;
                    let effective_this = if !is_strict_func {
                        if this_val.is_undefined() || this_val.is_null() {
                            self.global_object
                        } else {
                            let is_primitive = this_val.is_number()
                                || this_val.is_boolean()
                                || this_val.is_symbol()
                                || this_val.is_string()
                                || this_val.as_object_ptr().is_some_and(|ptr| {
                                    let obj = unsafe { &*(ptr as *mut JsObject) };
                                    matches!(obj.kind, ObjectKind::String(_) | ObjectKind::Symbol(_) | ObjectKind::BigInt(_))
                                });
                            if is_primitive {
                                self.to_object_value(this_val).unwrap_or(this_val)
                            } else {
                                this_val
                            }
                        }
                    } else {
                        this_val
                    };

                    let has_rest = func.params.last().map(|p| p.starts_with("...")).unwrap_or(false);
                    let rest_idx = if has_rest { func.params.len() - 1 } else { usize::MAX };

                    let mut locals = Vec::with_capacity(func.local_names.len().max(args.len()));
                    for i in 0..func.local_names.len() {
                        if i < func.params.len() {
                            if i == rest_idx {
                                let rest_args = if args.len() > rest_idx {
                                    args[rest_idx..].to_vec()
                                } else {
                                    Vec::new()
                                };
                                let rest_arr = self.alloc_array(rest_args);
                                locals.push(rest_arr);
                            } else if i < args.len() {
                                locals.push(args[i]);
                            } else {
                                locals.push(JsValue::undefined());
                            }
                        } else {
                            locals.push(JsValue::undefined());
                        }
                    }
                    if !has_rest && args.len() > func.local_names.len() {
                        for arg in &args[func.local_names.len()..] {
                            locals.push(*arg);
                        }
                    }

                    let new_target = if self.is_constructing { Some(callee) } else { None };

                    let (args_obj, mapped_args) = if func.is_arrow {
                        (None, None)
                    } else {
                        let mapped = if !is_strict_func && !func.has_param_exprs && !has_rest {
                            Some(vec![true; func.params.len().min(args.len())])
                        } else {
                            None
                        };
                        (Some(self.alloc_arguments(callee, args.clone(), is_strict_func)), mapped)
                    };

                    if func.is_generator {
                        let chunk = Arc::clone(&func.chunk);
                        let target_depth = self.frames.len();
                        let base_sp = self.stack.len();
                        self.frames.push(CallFrame {
                            chunk,
                            ip: 0,
                            base_sp,
                            this_val: effective_this,
                            gen_val: None,
                            locals: locals.clone(),
                            local_names: func.local_names.clone(),
                            lexical_local_names: func.lexical_local_names.clone(),
                            scopes: func.upvalues.clone(),
                            upvalues_count: func.upvalues.len(),
                            env_obj: None,
                            arguments: args_obj,
                            is_strict: is_strict_func,
                            new_target,
                            is_function: true,
                            is_method: func.is_method,
                            is_arrow: func.is_arrow,
                            has_param_exprs: func.has_param_exprs,
                            in_param_init: func.has_param_exprs,
                            is_indirect_eval: false,
                            callee_func: Some(func.clone()),
                            callee_val: Some(callee),
                            mapped_arguments: mapped_args.clone(),
                            home_object: func.home_object,
                        });
                        let gen_val = self.run_until_depth(target_depth)?;
                        Ok(Some(gen_val))
                    } else if func.is_async {
                        let promise_val = self.alloc_promise(PromiseState::Pending, JsValue::undefined());
                        let chunk = Arc::clone(&func.chunk);
                        let target_depth = self.frames.len();
                        let base_sp = self.stack.len();
                        self.frames.push(CallFrame {
                            chunk,
                            ip: 0,
                            base_sp,
                            this_val: effective_this,
                            gen_val: None,
                            locals: locals.clone(),
                            local_names: func.local_names.clone(),
                            lexical_local_names: func.lexical_local_names.clone(),
                            scopes: func.upvalues.clone(),
                            upvalues_count: func.upvalues.len(),
                            env_obj: None,
                            arguments: args_obj,
                            is_strict: is_strict_func,
                            new_target,
                            is_function: true,
                            is_method: func.is_method,
                            is_arrow: func.is_arrow,
                            has_param_exprs: func.has_param_exprs,
                            in_param_init: func.has_param_exprs,
                            is_indirect_eval: false,
                            callee_func: Some(func.clone()),
                            callee_val: Some(callee),
                            mapped_arguments: mapped_args.clone(),
                            home_object: func.home_object,
                        });
                        match self.run_until_depth(target_depth) {
                            Ok(res) => self.resolve_promise(promise_val, res),
                            Err(e) => {
                                let err_val = self.alloc_string(e.to_string());
                                self.reject_promise(promise_val, err_val);
                            }
                        }
                        let _ = self.drain_microtasks();
                        Ok(Some(promise_val))
                    } else {
                        let new_frame = CallFrame {
                            chunk: Arc::clone(&func.chunk),
                            ip: 0,
                            base_sp: self.stack.len(),
                            this_val: effective_this,
                            gen_val: None,
                            locals,
                            local_names: func.local_names.clone(),
                            lexical_local_names: func.lexical_local_names.clone(),
                            scopes: func.upvalues.clone(),
                            upvalues_count: func.upvalues.len(),
                            env_obj: None,
                            arguments: args_obj,
                            is_strict: is_strict_func,
                            new_target,
                            is_function: true,
                            is_method: func.is_method,
                            is_arrow: func.is_arrow,
                            has_param_exprs: func.has_param_exprs,
                            in_param_init: func.has_param_exprs,
                            is_indirect_eval: false,
                            home_object: func.home_object,
                            callee_func: Some(func),
                            callee_val: Some(callee),
                            mapped_arguments: mapped_args,
                        };
                        self.frames.push(new_frame);
                        Ok(None)
                    }
                }
                _ => Err(VmError::TypeError(format!("{} is not a function", self.to_string_value(callee)))),
            }
        } else {
            Err(VmError::TypeError(format!("{} is not a function", self.to_string_value(callee))))
        }
    }

    pub fn call(&mut self, callee: JsValue, this_val: JsValue, args: &[JsValue]) -> Result<JsValue, VmError> {
        let res_opt = self.execute_call_internal(callee, this_val, args.to_vec())?;
        if let Some(res) = res_opt {
            Ok(res)
        } else {
            let target_depth = self.frames.len() - 1;
            self.run_until_depth(target_depth)
        }
    }
}
