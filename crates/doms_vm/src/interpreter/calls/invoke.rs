use super::*;

impl Vm {
    pub fn execute_call_from_stack_at(
        &mut self,
        callee: JsValue,
        this_val: JsValue,
        stack_args_start: usize,
        argc: usize,
    ) -> Result<Option<JsValue>, VmError> {
        if self.frames.len() >= 1024 {
            return Err(VmError::RangeError("Maximum call stack size exceeded".to_string()));
        }
        let prev_temp = self.temp_roots.len();
        self.temp_roots.push(callee);
        self.temp_roots.push(this_val);
        let args_start = self.temp_roots.len();
        self.temp_roots.extend_from_slice(&self.stack[stack_args_start..stack_args_start + argc]);
        let args_len = argc;
        self.execute_call_common(prev_temp, args_start, args_len)
    }

    pub fn execute_call_internal(
        &mut self,
        callee: JsValue,
        this_val: JsValue,
        args: &[JsValue],
    ) -> Result<Option<JsValue>, VmError> {
        if self.frames.len() >= 1024 {
            return Err(VmError::RangeError("Maximum call stack size exceeded".to_string()));
        }
        let prev_temp = self.temp_roots.len();
        self.temp_roots.push(callee);
        self.temp_roots.push(this_val);
        let args_start = self.temp_roots.len();
        self.temp_roots.extend_from_slice(args);
        let args_len = args.len();
        self.execute_call_common(prev_temp, args_start, args_len)
    }

    fn execute_call_common(
        &mut self,
        prev_temp: usize,
        args_start: usize,
        args_len: usize,
    ) -> Result<Option<JsValue>, VmError> {
        let cur_callee = self.temp_roots[prev_temp];
        if let Some(ptr) = cur_callee.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            match &mut obj.kind {
                ObjectKind::NativeFunction(native) => {
                    let native = native.clone();
                    let cur_this = self.temp_roots[prev_temp + 1];
                    let res = if args_len <= 8 {
                        let mut stack_args = [JsValue::undefined(); 8];
                        stack_args[..args_len]
                            .copy_from_slice(&self.temp_roots[args_start..args_start + args_len]);
                        let r = native.call(self, cur_this, &stack_args[..args_len]);
                        stack_args = [JsValue::undefined(); 8];
                        std::hint::black_box(&stack_args);
                        r
                    } else {
                        let cur_args = self.temp_roots[args_start..args_start + args_len].to_vec();
                        native.call(self, cur_this, &cur_args)
                    };
                    self.temp_roots.truncate(prev_temp);
                    Ok(Some(res?))
                }
                ObjectKind::Proxy { .. } => {
                    let cur_callee = self.temp_roots[prev_temp];
                    let cur_this = self.temp_roots[prev_temp + 1];
                    let cur_args = self.temp_roots[args_start..args_start + args_len].to_vec();
                    let res = self.proxy_call(cur_callee, cur_this, &cur_args);
                    self.temp_roots.truncate(prev_temp);
                    Ok(Some(res?))
                }
                ObjectKind::Function(f) => {
                    if f.is_class_constructor && !self.is_constructing {
                        self.temp_roots.truncate(prev_temp);
                        return Err(VmError::TypeError(
                            "Class constructor cannot be invoked without 'new'".into(),
                        ));
                    }
                    let func = f.clone();
                    let is_strict_func = func.is_strict;
                    let cur_this = self.temp_roots[prev_temp + 1];
                    let effective_this = if func.is_arrow {
                        func.lexical_this.unwrap_or(self.global_object)
                    } else if !is_strict_func {
                        if cur_this.is_undefined() || cur_this.is_null() {
                            self.global_object
                        } else {
                            let is_primitive = cur_this.is_number()
                                || cur_this.is_boolean()
                                || cur_this.is_symbol()
                                || cur_this.is_string()
                                || cur_this.as_object_ptr().is_some_and(|ptr| {
                                    let obj = unsafe { &*(ptr as *mut JsObject) };
                                    matches!(
                                        obj.kind,
                                        ObjectKind::String(_)
                                            | ObjectKind::Symbol(_)
                                            | ObjectKind::BigInt(_)
                                    )
                                });
                            if is_primitive {
                                self.to_object_value(cur_this).unwrap_or(cur_this)
                            } else {
                                cur_this
                            }
                        }
                    } else {
                        cur_this
                    };
                    self.temp_roots[prev_temp + 1] = effective_this;

                    if !f.is_generator && !f.is_async && !self.is_constructing {
                        f.call_count = f.call_count.saturating_add(1);
                        if f.call_count >= crate::jit::JIT_HOT_THRESHOLD && !f.jit_attempted {
                            f.jit_attempted = true;
                            f.jit_entry = crate::jit::JitCompiler::compile_native(&f.chunk);
                        }
                        if let Some(jit) = &f.jit_entry {
                            if let crate::jit::JitCode::Native { func_ptr, num_locals, .. } =
                                &jit.code
                            {
                                let locals_count = *num_locals;
                                let self_slot = f.self_slot;
                                let func_fn = *func_ptr;
                                let cur_callee = self.temp_roots[prev_temp];

                                let jit_has_rest =
                                    f.params.last().map(|p| p.starts_with("...")).unwrap_or(false);
                                let jit_rest_idx =
                                    if jit_has_rest { f.params.len() - 1 } else { usize::MAX };
                                let rest_arr = if jit_has_rest {
                                    let rest_args = if args_len > jit_rest_idx {
                                        self.temp_roots[args_start + jit_rest_idx..args_start + args_len]
                                            .to_vec()
                                    } else {
                                        Vec::new()
                                    };
                                    Some(self.alloc_array(rest_args))
                                } else {
                                    None
                                };

                                let res = if locals_count <= 16 {
                                    let mut stack_args = [crate::value::TAG_UNDEFINED; 16];
                                    let to_copy = if jit_has_rest {
                                        args_len.min(jit_rest_idx).min(locals_count)
                                    } else {
                                        args_len.min(locals_count)
                                    };
                                    for i in 0..to_copy {
                                        stack_args[i] = self.temp_roots[args_start + i].0;
                                    }
                                    if let Some(arr) = rest_arr {
                                        if jit_rest_idx < locals_count {
                                            stack_args[jit_rest_idx] = arr.0;
                                        }
                                    }
                                    if let Some(slot) = self_slot {
                                        if (slot as usize) < locals_count {
                                            stack_args[slot as usize] = cur_callee.0;
                                        }
                                    }
                                    unsafe {
                                        (func_fn)(
                                            stack_args.as_mut_ptr(),
                                            locals_count,
                                            effective_this.0,
                                            0,
                                        )
                                    }
                                } else {
                                    let mut raw_args =
                                        vec![crate::value::TAG_UNDEFINED; locals_count];
                                    let to_copy = if jit_has_rest {
                                        args_len.min(jit_rest_idx).min(locals_count)
                                    } else {
                                        args_len.min(locals_count)
                                    };
                                    for i in 0..to_copy {
                                        raw_args[i] = self.temp_roots[args_start + i].0;
                                    }
                                    if let Some(arr) = rest_arr {
                                        if jit_rest_idx < locals_count {
                                            raw_args[jit_rest_idx] = arr.0;
                                        }
                                    }
                                    if let Some(slot) = self_slot {
                                        if (slot as usize) < raw_args.len() {
                                            raw_args[slot as usize] = cur_callee.0;
                                        }
                                    }
                                    unsafe {
                                        (func_fn)(
                                            raw_args.as_mut_ptr(),
                                            raw_args.len(),
                                            effective_this.0,
                                            0,
                                        )
                                    }
                                };

                                if res != crate::jit::JIT_DEOPT_SENTINEL {
                                    self.temp_roots.truncate(prev_temp);
                                    return Ok(Some(JsValue(res)));
                                } else {
                                    // Deoptimization: speculative guard failed, fall back to interpreter
                                    f.jit_entry = None;
                                }
                            }
                        }
                    }

                    let has_rest =
                        func.params.last().map(|p| p.starts_with("...")).unwrap_or(false);
                    let rest_idx = if has_rest { func.params.len() - 1 } else { usize::MAX };

                    let locals_start = self.temp_roots.len();
                    for i in 0..func.local_names.len() {
                        if i < func.params.len() {
                            if i == rest_idx {
                                let rest_args = if args_len > rest_idx {
                                    self.temp_roots[args_start + rest_idx..args_start + args_len]
                                        .to_vec()
                                } else {
                                    Vec::new()
                                };
                                let rest_arr = self.alloc_array(rest_args);
                                self.temp_roots.push(rest_arr);
                            } else if func.has_param_exprs && i >= func.length {
                                self.temp_roots.push(JsValue::uninitialized());
                            } else if i < args_len {
                                let arg_val = self.temp_roots[args_start + i];
                                self.temp_roots.push(arg_val);
                            } else {
                                self.temp_roots.push(JsValue::undefined());
                            }
                        } else {
                            if !func.is_arrow
                                && func.local_names.get(i).map(|n| n == "arguments").unwrap_or(false)
                                && !func.params.iter().any(|p| p == "arguments")
                            {
                                let cur_callee = self.temp_roots[prev_temp];
                                let raw = self.temp_roots[args_start..args_start + args_len].to_vec();
                                let a_obj = self.alloc_arguments(cur_callee, raw, is_strict_func);
                                self.temp_roots.push(a_obj);
                            } else if let Some(name) = func.local_names.get(i) {
                                if func.lexical_local_names.contains(name) {
                                    self.temp_roots.push(JsValue::uninitialized());
                                } else {
                                    self.temp_roots.push(JsValue::undefined());
                                }
                            } else {
                                self.temp_roots.push(JsValue::undefined());
                            }
                        }
                    }
                    if !has_rest && args_len > func.local_names.len() {
                        for i in func.local_names.len()..args_len {
                            let arg_val = self.temp_roots[args_start + i];
                            self.temp_roots.push(arg_val);
                        }
                    }
                    let locals_len = self.temp_roots.len() - locals_start;
                    if let Some(slot) = func.self_slot {
                        let cur_callee = self.temp_roots[prev_temp];
                        if (slot as usize) < locals_len {
                            self.temp_roots[locals_start + slot as usize] = cur_callee;
                        }
                    }

                    let new_target = if func.is_arrow {
                        func.lexical_new_target
                    } else if self.is_constructing {
                        self.current_new_target.or(Some(self.temp_roots[prev_temp]))
                    } else {
                        None
                    };

                    let mapped_args = if func.is_arrow {
                        None
                    } else if !is_strict_func && !func.has_param_exprs && !has_rest {
                        Some(vec![true; func.params.len().min(args_len)])
                    } else {
                        None
                    };

                    let raw_args = if func.is_arrow && !func.has_param_exprs {
                        None
                    } else {
                        Some(self.temp_roots[args_start..args_start + args_len].to_vec())
                    };

                    if func.is_generator {
                        let cur_callee = self.temp_roots[prev_temp];
                        let cur_this = self.temp_roots[prev_temp + 1];
                        let locals =
                            self.temp_roots[locals_start..locals_start + locals_len].to_vec();
                        let (fresh_upvalues, fresh_home_object) =
                            if let Some(ptr) = cur_callee.as_object_ptr() {
                                let obj = unsafe { &*(ptr as *mut JsObject) };
                                if let ObjectKind::Function(ref f) = obj.kind {
                                    (f.upvalues.clone(), f.home_object)
                                } else {
                                    (func.upvalues.clone(), func.home_object)
                                }
                            } else {
                                (func.upvalues.clone(), func.home_object)
                            };
                        self.temp_roots.truncate(prev_temp);

                        let mut callee_func = func.clone();
                        callee_func.upvalues = fresh_upvalues.clone();
                        callee_func.home_object = fresh_home_object;

                        // Tiered native Cranelift execution for generator start
                        if !callee_func.is_async && callee_func.jit_entry.is_none() && !callee_func.jit_attempted {
                            let can_jit = crate::jit::JitCompiler::can_compile_native(&callee_func.chunk);
                            if can_jit {
                                callee_func.jit_attempted = true;
                                callee_func.jit_entry =
                                    crate::jit::JitCompiler::compile_native(&callee_func.chunk);
                            }
                        }
                        if !callee_func.is_async {
                            if let Some(jit) = &callee_func.jit_entry {
                                if let crate::jit::JitCode::Native { func_ptr, num_locals, .. } =
                                    &jit.code
                                {
                                let locals_count = *num_locals;
                                let mut raw_locals =
                                    vec![crate::value::TAG_UNDEFINED; locals_count];
                                for (i, v) in locals.iter().enumerate() {
                                    if i < locals_count {
                                        raw_locals[i] = v.0;
                                    }
                                }
                                let prev_vm = crate::jit::CURRENT_VM.with(|c| c.get());
                                let prev_gen = crate::jit::CURRENT_GENERATOR.with(|c| c.get());
                                crate::jit::CURRENT_VM.with(|c| c.set(self as *mut Vm));
                                crate::jit::CURRENT_GENERATOR.with(|c| c.set(0));
                                crate::jit::CURRENT_CALLEE_FUNC
                                    .with(|c| c.borrow_mut().replace(callee_func.clone()));
                                crate::jit::CURRENT_RAW_ARGS
                                    .with(|c| *c.borrow_mut() = raw_args.clone());
                                crate::jit::CURRENT_CALLEE_VAL.with(|c| c.set(cur_callee.0));

                                crate::jit::NURSERY_PTRS.with(|cell| {
                                    let ptrs = unsafe { &mut *cell.get() };
                                    ptrs.bump = self.heap.nursery.cursor();
                                    ptrs.limit = self.heap.nursery.limit();
                                });

                                let native_res = unsafe {
                                    (func_ptr)(
                                        raw_locals.as_mut_ptr(),
                                        raw_locals.len(),
                                        cur_this.0,
                                        0,
                                    )
                                };

                                crate::jit::NURSERY_PTRS.with(|cell| {
                                    let ptrs = unsafe { &mut *cell.get() };
                                    if !ptrs.bump.is_null()
                                        && ptrs.bump > self.heap.nursery.cursor()
                                    {
                                        self.heap.nursery.set_cursor(ptrs.bump);
                                    }
                                });
                                crate::jit::CURRENT_VM.with(|c| c.set(prev_vm));
                                crate::jit::CURRENT_GENERATOR.with(|c| c.set(prev_gen));
                                crate::jit::CURRENT_CALLEE_FUNC.with(|c| c.borrow_mut().take());
                                crate::jit::CURRENT_RAW_ARGS.with(|c| c.borrow_mut().take());
                                crate::jit::CURRENT_CALLEE_VAL.with(|c| c.set(0));

                                if native_res != crate::jit::JIT_DEOPT_SENTINEL {
                                    return Ok(Some(JsValue(native_res)));
                                }
                            }
                        }
                    }

                        let (param_env_obj, scopes, upvalues_count) = if callee_func.has_param_exprs {
                            let prev_temp = self.temp_roots.len();
                            let param_env = self.alloc_object();
                            self.temp_roots.push(param_env);
                            for (i, p) in callee_func.params.iter().enumerate() {
                                let clean_name = p.split('=').next().unwrap().trim();
                                let clean_name =
                                    clean_name.strip_prefix("...").unwrap_or(clean_name);
                                if !clean_name.is_empty()
                                    && !clean_name.starts_with('@')
                                    && !clean_name.starts_with('[')
                                    && !clean_name.starts_with('{')
                                {
                                    let val = raw_args
                                        .as_ref()
                                        .and_then(|a| a.get(i).copied())
                                        .unwrap_or(JsValue::undefined());
                                    self.set_property(param_env, clean_name, val);
                                }
                            }
                            self.temp_roots.truncate(prev_temp);
                            let mut sc = fresh_upvalues.clone();
                            sc.push(param_env);
                            let uv_len = sc.len();
                            (Some(param_env), sc, uv_len)
                        } else {
                            (None, fresh_upvalues.clone(), fresh_upvalues.len())
                        };

                        let chunk = Arc::clone(&callee_func.chunk);
                        let target_depth = self.frames.len();
                        let base_sp = self.stack.len();
                        self.frames.push(CallFrame {
                            chunk,
                            ip: 0,
                            base_sp,
                            this_val: cur_this,
                            gen_val: None,
                            locals,
                            local_names: callee_func.local_names.clone(),
                            lexical_local_names: callee_func.lexical_local_names.clone(),
                            scopes,
                            upvalues_count,
                            env_obj: None,
                            param_env_obj,
                            arguments: None,
                            raw_args: raw_args.clone(),
                            is_strict: is_strict_func,
                            new_target,
                            is_function: true,
                            is_method: callee_func.is_method,
                            is_arrow: callee_func.is_arrow,
                            has_param_exprs: callee_func.has_param_exprs,
                            in_param_init: callee_func.has_param_exprs,
                            is_indirect_eval: false,
                            module: callee_func.module,
                            callee_func: Some(callee_func.clone()),
                            callee_val: Some(cur_callee),
                            mapped_arguments: mapped_args.clone(),
                            home_object: fresh_home_object,
                            in_field_init: if callee_func.is_arrow {
                                callee_func.lexical_in_field_init || self.frames.last().map(|f| f.in_field_init).unwrap_or(false)
                            } else {
                                false
                            },
                        });
                        let gen_val = self.run_until_depth(target_depth)?;
                        Ok(Some(gen_val))
                    } else if func.is_async {
                        let promise_val =
                            self.alloc_promise(PromiseState::Pending, JsValue::undefined());
                        self.temp_roots.push(promise_val);
                        let promise_idx = self.temp_roots.len() - 1;

                        let cur_callee = self.temp_roots[prev_temp];
                        let cur_this = self.temp_roots[prev_temp + 1];
                        let locals =
                            self.temp_roots[locals_start..locals_start + locals_len].to_vec();
                        let (fresh_upvalues, fresh_home_object) =
                            if let Some(ptr) = cur_callee.as_object_ptr() {
                                let obj = unsafe { &*(ptr as *mut JsObject) };
                                if let ObjectKind::Function(ref f) = obj.kind {
                                    (f.upvalues.clone(), f.home_object)
                                } else {
                                    (func.upvalues.clone(), func.home_object)
                                }
                            } else {
                                (func.upvalues.clone(), func.home_object)
                            };

                        let mut callee_func = func.clone();
                        callee_func.upvalues = fresh_upvalues.clone();
                        callee_func.home_object = fresh_home_object;

                        // Tiered native Cranelift execution for generator start
                        if !callee_func.is_async && callee_func.jit_entry.is_none() && !callee_func.jit_attempted {
                            let can_jit = crate::jit::JitCompiler::can_compile_native(&callee_func.chunk);
                            if can_jit {
                                callee_func.jit_attempted = true;
                                callee_func.jit_entry =
                                    crate::jit::JitCompiler::compile_native(&callee_func.chunk);
                            }
                        }
                        if !callee_func.is_async {
                            if let Some(jit) = &callee_func.jit_entry {
                                if let crate::jit::JitCode::Native { func_ptr, num_locals, .. } =
                                    &jit.code
                                {
                                let locals_count = *num_locals;
                                let mut raw_locals =
                                    vec![crate::value::TAG_UNDEFINED; locals_count];
                                for (i, v) in locals.iter().enumerate() {
                                    if i < locals_count {
                                        raw_locals[i] = v.0;
                                    }
                                }
                                let prev_vm = crate::jit::CURRENT_VM.with(|c| c.get());
                                let prev_gen = crate::jit::CURRENT_GENERATOR.with(|c| c.get());
                                crate::jit::CURRENT_VM.with(|c| c.set(self as *mut Vm));
                                crate::jit::CURRENT_GENERATOR.with(|c| c.set(0));
                                crate::jit::CURRENT_CALLEE_FUNC
                                    .with(|c| c.borrow_mut().replace(callee_func.clone()));
                                crate::jit::CURRENT_RAW_ARGS
                                    .with(|c| *c.borrow_mut() = raw_args.clone());
                                crate::jit::CURRENT_CALLEE_VAL.with(|c| c.set(cur_callee.0));

                                crate::jit::NURSERY_PTRS.with(|cell| {
                                    let ptrs = unsafe { &mut *cell.get() };
                                    ptrs.bump = self.heap.nursery.cursor();
                                    ptrs.limit = self.heap.nursery.limit();
                                });

                                let native_res = unsafe {
                                    (func_ptr)(
                                        raw_locals.as_mut_ptr(),
                                        raw_locals.len(),
                                        cur_this.0,
                                        0,
                                    )
                                };

                                crate::jit::NURSERY_PTRS.with(|cell| {
                                    let ptrs = unsafe { &mut *cell.get() };
                                    if !ptrs.bump.is_null()
                                        && ptrs.bump > self.heap.nursery.cursor()
                                    {
                                        self.heap.nursery.set_cursor(ptrs.bump);
                                    }
                                });
                                crate::jit::CURRENT_VM.with(|c| c.set(prev_vm));
                                crate::jit::CURRENT_GENERATOR.with(|c| c.set(prev_gen));
                                crate::jit::CURRENT_CALLEE_FUNC.with(|c| c.borrow_mut().take());
                                crate::jit::CURRENT_RAW_ARGS.with(|c| c.borrow_mut().take());
                                crate::jit::CURRENT_CALLEE_VAL.with(|c| c.set(0));

                                if native_res != crate::jit::JIT_DEOPT_SENTINEL {
                                    let cur_p = self.temp_roots[promise_idx];
                                    self.resolve_promise(cur_p, JsValue(native_res));
                                    let _ = self.drain_microtasks();
                                    let res_promise = self.temp_roots[promise_idx];
                                    self.temp_roots.truncate(prev_temp);
                                    return Ok(Some(res_promise));
                                }
                            }
                        }
                    }

                        let (param_env_obj, scopes, upvalues_count) = if callee_func.has_param_exprs {
                            let prev_temp = self.temp_roots.len();
                            let param_env = self.alloc_object();
                            self.temp_roots.push(param_env);
                            for (i, p) in callee_func.params.iter().enumerate() {
                                let clean_name = p.split('=').next().unwrap().trim();
                                let clean_name =
                                    clean_name.strip_prefix("...").unwrap_or(clean_name);
                                if !clean_name.is_empty()
                                    && !clean_name.starts_with('@')
                                    && !clean_name.starts_with('[')
                                    && !clean_name.starts_with('{')
                                {
                                    let val = raw_args
                                        .as_ref()
                                        .and_then(|a| a.get(i).copied())
                                        .unwrap_or(JsValue::undefined());
                                    self.set_property(param_env, clean_name, val);
                                }
                            }
                            self.temp_roots.truncate(prev_temp);
                            let mut sc = fresh_upvalues.clone();
                            sc.push(param_env);
                            let uv_len = sc.len();
                            (Some(param_env), sc, uv_len)
                        } else {
                            (None, fresh_upvalues.clone(), fresh_upvalues.len())
                        };

                        let chunk = Arc::clone(&callee_func.chunk);
                        let target_depth = self.frames.len();
                        let base_sp = self.stack.len();
                        self.frames.push(CallFrame {
                            chunk,
                            ip: 0,
                            base_sp,
                            this_val: cur_this,
                            gen_val: None,
                            locals,
                            local_names: callee_func.local_names.clone(),
                            lexical_local_names: callee_func.lexical_local_names.clone(),
                            scopes,
                            upvalues_count,
                            env_obj: None,
                            param_env_obj,
                            arguments: None,
                            raw_args: raw_args.clone(),
                            is_strict: is_strict_func,
                            new_target,
                            is_function: true,
                            is_method: callee_func.is_method,
                            is_arrow: callee_func.is_arrow,
                            has_param_exprs: callee_func.has_param_exprs,
                            in_param_init: callee_func.has_param_exprs,
                            is_indirect_eval: false,
                            module: callee_func.module,
                            callee_func: Some(callee_func.clone()),
                            callee_val: Some(cur_callee),
                            mapped_arguments: mapped_args.clone(),
                            home_object: fresh_home_object,
                            in_field_init: if callee_func.is_arrow {
                                callee_func.lexical_in_field_init || self.frames.last().map(|f| f.in_field_init).unwrap_or(false)
                            } else {
                                false
                            },
                        });
                        match self.run_until_depth(target_depth) {
                            Ok(res) => {
                                let cur_p = self.temp_roots[promise_idx];
                                self.resolve_promise(cur_p, res);
                            }
                            Err(e) => {
                                let cur_p = self.temp_roots[promise_idx];
                                let err_val = self.vm_error_to_js_value(&e);
                                self.reject_promise(cur_p, err_val);
                            }
                        }
                        let _ = self.drain_microtasks();
                        let res_promise = self.temp_roots[promise_idx];
                        self.temp_roots.truncate(prev_temp);
                        Ok(Some(res_promise))
                    } else {
                        let cur_callee = self.temp_roots[prev_temp];
                        let cur_this = self.temp_roots[prev_temp + 1];
                        let locals =
                            self.temp_roots[locals_start..locals_start + locals_len].to_vec();
                        let (fresh_upvalues, fresh_home_object) =
                            if let Some(ptr) = cur_callee.as_object_ptr() {
                                let obj = unsafe { &*(ptr as *mut JsObject) };
                                if let ObjectKind::Function(ref f) = obj.kind {
                                    (f.upvalues.clone(), f.home_object)
                                } else {
                                    (func.upvalues.clone(), func.home_object)
                                }
                            } else {
                                (func.upvalues.clone(), func.home_object)
                            };
                        self.temp_roots.truncate(prev_temp);

                        let mut callee_func = func;
                        callee_func.upvalues = fresh_upvalues.clone();
                        callee_func.home_object = fresh_home_object;

                        let (param_env_obj, scopes, upvalues_count) = if callee_func.has_param_exprs {
                            let prev_temp = self.temp_roots.len();
                            let param_env = self.alloc_object();
                            self.temp_roots.push(param_env);
                            for (i, p) in callee_func.params.iter().enumerate() {
                                let clean_name = p.split('=').next().unwrap().trim();
                                let clean_name =
                                    clean_name.strip_prefix("...").unwrap_or(clean_name);
                                if !clean_name.is_empty()
                                    && !clean_name.starts_with('@')
                                    && !clean_name.starts_with('[')
                                    && !clean_name.starts_with('{')
                                {
                                    let val = raw_args
                                        .as_ref()
                                        .and_then(|a| a.get(i).copied())
                                        .unwrap_or(JsValue::undefined());
                                    self.set_property(param_env, clean_name, val);
                                }
                            }
                            self.temp_roots.truncate(prev_temp);
                            let mut sc = fresh_upvalues.clone();
                            sc.push(param_env);
                            let uv_len = sc.len();
                            (Some(param_env), sc, uv_len)
                        } else {
                            (None, fresh_upvalues.clone(), fresh_upvalues.len())
                        };

                        let new_frame = CallFrame {
                            chunk: Arc::clone(&callee_func.chunk),
                            ip: 0,
                            base_sp: self.stack.len(),
                            this_val: cur_this,
                            gen_val: None,
                            locals,
                            local_names: callee_func.local_names.clone(),
                            lexical_local_names: callee_func.lexical_local_names.clone(),
                            scopes,
                            upvalues_count,
                            env_obj: None,
                            param_env_obj,
                            arguments: None,
                            raw_args,
                            is_strict: is_strict_func,
                            new_target,
                            is_function: true,
                            is_method: callee_func.is_method,
                            is_arrow: callee_func.is_arrow,
                            has_param_exprs: callee_func.has_param_exprs,
                            in_param_init: callee_func.has_param_exprs,
                            is_indirect_eval: false,
                            home_object: fresh_home_object,
                            module: callee_func.module,
                            callee_func: Some(callee_func.clone()),
                            callee_val: Some(cur_callee),
                            mapped_arguments: mapped_args,
                            in_field_init: if callee_func.is_arrow {
                                callee_func.lexical_in_field_init || self.frames.last().map(|f| f.in_field_init).unwrap_or(false)
                            } else {
                                false
                            },
                        };
                        self.frames.push(new_frame);
                        Ok(None)
                    }
                }
                _ => {
                    let cur_callee = self.temp_roots[prev_temp];
                    self.temp_roots.truncate(prev_temp);
                    Err(VmError::TypeError(format!(
                        "{} is not a function",
                        self.to_string_value(cur_callee)
                    )))
                }
            }
        } else {
            let cur_callee = self.temp_roots[prev_temp];
            self.temp_roots.truncate(prev_temp);
            Err(VmError::TypeError(format!(
                "{} is not a function",
                self.to_string_value(cur_callee)
            )))
        }
    }

    pub fn call(
        &mut self,
        callee: JsValue,
        this_val: JsValue,
        args: &[JsValue],
    ) -> Result<JsValue, VmError> {
        crate::interpreter::gc::register_stack_base();
        let res_opt = self.execute_call_internal(callee, this_val, args)?;
        if let Some(res) = res_opt {
            Ok(res)
        } else {
            let target_depth = self.frames.len() - 1;
            self.run_until_depth(target_depth)
        }
    }

    pub fn call_method(
        &mut self,
        target: JsValue,
        method_name: &str,
        args: &[JsValue],
    ) -> Result<JsValue, VmError> {
        let method = self.get_property_result(target, method_name)?;
        self.call(method, target, args)
    }
}
