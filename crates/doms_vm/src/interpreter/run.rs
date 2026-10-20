//! Execution entry points and the main bytecode dispatch loop.

use super::*;

impl Vm {
    pub fn run(&mut self, chunk: &BytecodeChunk) -> Result<JsValue, VmError> {
        self.instantiate_global_declarations(chunk)?;
        let root_chunk = Arc::new(chunk.clone());
        let target_depth = self.frames.len();
        let local_names = Arc::clone(&root_chunk.local_names);
        let lexical_local_names = Arc::clone(&root_chunk.lexical_local_names);
        self.frames.push(CallFrame {
            chunk: root_chunk,
            ip: 0,
            base_sp: self.stack.len(),
            this_val: if self.is_module { JsValue::undefined() } else { self.global_object },
            gen_val: None,
            locals: vec![JsValue::undefined(); local_names.len()],
            local_names,
            lexical_local_names,
            scopes: Vec::new(),
            upvalues_count: 0,
            env_obj: None,
            arguments: None,
            raw_args: None,
            is_strict: self.is_strict,
            new_target: None,
            is_function: false,
            is_method: false,
            is_arrow: false,
            has_param_exprs: false,
            in_param_init: false,
            is_indirect_eval: false,
            callee_func: None,
            callee_val: None,
            mapped_arguments: None,
            home_object: None,
        });
        let result = self.run_until_depth(target_depth)?;
        if let Some(frame) = self.frames.get(target_depth) {
            for (idx, name) in frame.local_names.iter().enumerate() {
                if let Some(val) = frame.locals.get(idx) {
                    if self.global_lexicals.contains_key(name) {
                        self.global_lexicals.insert(name.clone(), *val);
                    }
                }
            }
        }
        self.drain_microtasks()?;
        Ok(result)
    }

    pub fn run_until_depth(&mut self, target_depth: usize) -> Result<JsValue, VmError> {
        loop {
            match self.run_until_depth_internal(target_depth) {
                Ok(val) => return Ok(val),
                Err(err) => {
                    if self.handle_exception_at_depth(err.clone(), target_depth).is_ok() {
                        continue;
                    }
                    while let Some(top) = self.catch_stack.last() {
                        if top.frame_idx >= target_depth {
                            self.catch_stack.pop();
                        } else {
                            break;
                        }
                    }
                    if let Some(target_frame) = self.frames.get(target_depth) {
                        let base_sp = target_frame.base_sp;
                        self.stack.truncate(base_sp);
                    }
                    self.frames.truncate(target_depth);
                    return Err(err);
                }
            }
        }
    }

    fn run_until_depth_internal(&mut self, target_depth: usize) -> Result<JsValue, VmError> {
        crate::interpreter::gc::register_stack_base();
        while self.frames.len() > target_depth {
            // Fuel is consumed for every executed instruction, whether or not a
            // limit is configured, so `fuel_consumed`/interrupt checks are exact.
            self.instruction_count += 1;
            if self.objects.len() >= self.gc_threshold {
                self.gc_collect();
                if self.objects.len() * 2 > self.gc_threshold {
                    self.gc_threshold = (self.objects.len() * 2).max(16384);
                }
            }
            if self.oom {
                return Err(VmError::RangeError(
                    "Out of memory: allocation limit reached".to_string(),
                ));
            }
            if let Some(max) = self.max_instructions {
                if self.instruction_count > max {
                    return Err(VmError::RuntimeError(
                        "Execution step limit exceeded (timeout)".to_string(),
                    ));
                }
            }
            if self.instruction_count % 1024 == 0 {
                if let Some(ref flag) = self.interrupt_flag {
                    if flag.load(std::sync::atomic::Ordering::Relaxed) {
                        return Err(VmError::RuntimeError("Execution interrupted".to_string()));
                    }
                }
                if let Some(deadline) = self.deadline {
                    if Instant::now() > deadline {
                        return Err(VmError::RuntimeError(
                            "Execution timeout exceeded".to_string(),
                        ));
                    }
                }
            }
            let (op, chunk) = {
                let frame = self.frames.last_mut().unwrap();
                if frame.ip >= frame.chunk.instructions.len() {
                    let finished = self.frames.pop().unwrap();
                    while let Some(top) = self.catch_stack.last() {
                        if top.frame_idx >= self.frames.len() {
                            self.catch_stack.pop();
                        } else {
                            break;
                        }
                    }
                    let ret_val = if self.stack.len() > finished.base_sp {
                        self.stack.pop().unwrap_or_default()
                    } else {
                        JsValue::undefined()
                    };
                    if let Some(g_val) = finished.gen_val {
                        let prev_temp = self.temp_roots.len();
                        self.temp_roots.push(ret_val);
                        self.temp_roots.push(g_val);
                        if let Some(g_ptr) = self.temp_roots[prev_temp + 1].as_object_ptr() {
                            let g_obj = unsafe { &mut *(g_ptr as *mut JsObject) };
                            if let ObjectKind::Generator(ref mut g) = g_obj.kind {
                                g.state = crate::object::GeneratorState::Completed;
                            }
                        }
                        let cur_ret = self.temp_roots[prev_temp];
                        let iter_res = self.make_iter_result(cur_ret, true);
                        self.temp_roots.truncate(prev_temp);
                        self.stack.truncate(finished.base_sp);
                        if self.frames.len() == target_depth {
                            return Ok(iter_res);
                        }
                        self.push(iter_res);
                        continue;
                    }
                    self.stack.truncate(finished.base_sp);
                    if self.frames.len() == target_depth {
                        return Ok(ret_val);
                    }
                    self.push(ret_val);
                    continue;
                }
                let op = frame.chunk.instructions[frame.ip];
                frame.ip += 1;
                (op, Arc::clone(&frame.chunk))
            };

            match op {
                OpCode::LoadConst(idx) => {
                    self.op_load_const(idx, &chunk)?;
                }
                OpCode::LoadNull => self.push(JsValue::null()),
                OpCode::LoadUndefined => self.push(JsValue::undefined()),
                OpCode::LoadHole => self.push(JsValue::hole()),
                OpCode::LoadUninitialized => self.push(JsValue::uninitialized()),
                OpCode::LoadTrue => self.push(JsValue::from_boolean(true)),
                OpCode::LoadFalse => self.push(JsValue::from_boolean(false)),
                OpCode::This => {
                    let this_val = self.frames.last().unwrap().this_val;
                    self.push(this_val);
                }
                OpCode::SetThis => {
                    let val = *self.peek()?;
                    if let Some(frame) = self.frames.last_mut() {
                        frame.this_val = val;
                    }
                }
                OpCode::LoadSuper => {
                    let frame = self.frames.last().unwrap();
                    let base = if let Some(home_obj) = frame.home_object {
                        if let Some(ptr) = home_obj.as_object_ptr() {
                            let obj = unsafe { &*(ptr as *mut JsObject) };
                            obj.prototype.unwrap_or(JsValue::null())
                        } else {
                            JsValue::null()
                        }
                    } else if let Some(ptr) = frame.this_val.as_object_ptr() {
                        let obj = unsafe { &*(ptr as *mut JsObject) };
                        obj.prototype.unwrap_or(JsValue::null())
                    } else {
                        JsValue::null()
                    };
                    self.push(base);
                }

                OpCode::CreateObject => {
                    let obj = self.alloc_object();
                    self.push(obj);
                }
                OpCode::CreateArray(count) => {
                    self.op_create_array(count)?;
                }
                OpCode::ArrayPush => {
                    self.op_array_push()?;
                }
                OpCode::ArraySpreadPush => {
                    self.op_array_spread_push(target_depth)?;
                }
                OpCode::RequireObjectCoercible => {
                    self.op_require_object_coercible(target_depth)?;
                }
                OpCode::CreateRegExp(p_idx, f_idx) => {
                    self.op_create_regexp(p_idx, f_idx, &chunk)?;
                }
                OpCode::GetKeys => {
                    if let Some(v) = self.op_get_keys(target_depth)? {
                        return Ok(v);
                    }
                }
                OpCode::GetIterator => {
                    if let Some(v) = self.op_get_iterator(target_depth)? {
                        return Ok(v);
                    }
                }
                OpCode::IteratorNext(target) => {
                    if let Some(v) = self.op_iterator_next(target, target_depth)? {
                        return Ok(v);
                    }
                }
                OpCode::IteratorClose(is_abrupt) => {
                    if let Some(v) = self.op_iterator_close(is_abrupt, target_depth)? {
                        return Ok(v);
                    }
                }
                OpCode::GetAsyncIterator => {
                    if let Some(v) = self.op_get_async_iterator(target_depth)? {
                        return Ok(v);
                    }
                }
                OpCode::AsyncIteratorNext(target) => {
                    if let Some(v) = self.op_async_iterator_next(target, target_depth)? {
                        return Ok(v);
                    }
                }
                OpCode::AsyncIteratorClose(is_abrupt) => {
                    if let Some(v) = self.op_async_iterator_close(is_abrupt, target_depth)? {
                        return Ok(v);
                    }
                }
                OpCode::GetProp(name_idx) => {
                    self.op_get_prop(name_idx, &chunk, target_depth)?;
                }
                OpCode::SetProp(name_idx) => {
                    self.op_set_prop(name_idx, &chunk, target_depth)?;
                }
                OpCode::DefineGetter(prop_idx) => {
                    self.op_define_getter(prop_idx, &chunk)?;
                }
                OpCode::DefineSetter(prop_idx) => {
                    self.op_define_setter(prop_idx, &chunk)?;
                }
                OpCode::DefineComputedGetter => {
                    self.op_define_computed_getter(target_depth)?;
                }
                OpCode::DefineComputedSetter => {
                    self.op_define_computed_setter(target_depth)?;
                }
                OpCode::DefineClassElement(flags) => {
                    self.op_define_class_element(flags, target_depth)?;
                }
                OpCode::InheritClass => {
                    self.op_inherit_class()?;
                }
                OpCode::GetElem => {
                    let stack_len = self.stack.len();
                    if stack_len >= 2 {
                        let key = self.stack[stack_len - 1];
                        let obj = self.stack[stack_len - 2];
                        if key.is_int32() {
                            let idx = key.as_int32().unwrap();
                            if idx >= 0 {
                                if let Some(ptr) = obj.as_object_ptr() {
                                    let jsobj = unsafe { &*(ptr as *mut JsObject) };
                                    if !jsobj.has_element_attributes() {
                                        if let ObjectKind::Array(elements) = &jsobj.kind {
                                            let uidx = idx as usize;
                                            if uidx < elements.len() {
                                                let el = elements[uidx];
                                                if !el.is_hole() {
                                                    self.stack[stack_len - 2] = el;
                                                    self.stack.truncate(stack_len - 1);
                                                    continue;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    self.op_get_elem(target_depth)?;
                }
                OpCode::SetElem => {
                    let stack_len = self.stack.len();
                    if stack_len >= 3 {
                        let val = self.stack[stack_len - 1];
                        let key = self.stack[stack_len - 2];
                        let obj = self.stack[stack_len - 3];
                        if key.is_int32() {
                            let idx = key.as_int32().unwrap();
                            if idx >= 0 {
                                if let Some(ptr) = obj.as_object_ptr() {
                                    let jsobj = unsafe { &mut *(ptr as *mut JsObject) };
                                    let is_extensible = jsobj.is_extensible();
                                    if !jsobj.has_element_attributes() {
                                        if let ObjectKind::Array(elements) = &mut jsobj.kind {
                                            let uidx = idx as usize;
                                            if uidx < elements.len() && !elements[uidx].is_hole() {
                                                elements[uidx] = val;
                                                self.stack[stack_len - 3] = val;
                                                self.stack.truncate(stack_len - 2);
                                                continue;
                                            } else if uidx == elements.len()
                                                && (jsobj.prototype.is_none()
                                                    || jsobj.prototype == self.array_proto)
                                                && is_extensible
                                            {
                                                elements.push(val);
                                                self.stack[stack_len - 3] = val;
                                                self.stack.truncate(stack_len - 2);
                                                continue;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    self.op_set_elem(target_depth)?;
                }
                OpCode::SetSuperProp(name_idx) => {
                    self.op_set_super_prop(name_idx, &chunk, target_depth)?;
                }
                OpCode::SetSuperElem => {
                    self.op_set_super_elem(target_depth)?;
                }
                OpCode::ObjectSpread => {
                    let source = self.pop()?;
                    let target = *self.stack.last().ok_or(VmError::StackUnderflow)?;
                    self.copy_data_properties(target, source, &[])?;
                }
                OpCode::ObjectRest(count) => {
                    let mut excluded = Vec::with_capacity(count as usize);
                    for _ in 0..count {
                        excluded.push(self.pop()?);
                    }
                    excluded.reverse();
                    let source = self.pop()?;
                    let target = self.alloc_object();
                    self.copy_data_properties(target, source, &excluded)?;
                    self.push(target);
                }
                OpCode::DefinePrivateField(name_idx) => {
                    let name = match &chunk.constants[name_idx as usize] {
                        Constant::String(s) => s.clone(),
                        _ => {
                            return Err(VmError::TypeError(
                                "Private field name must be string".into(),
                            ));
                        }
                    };
                    let val = self.pop()?;
                    let obj = self.pop()?;
                    if let Some(ptr) = obj.as_object_ptr() {
                        let jsobj = unsafe { &mut *(ptr as *mut JsObject) };
                        jsobj.insert_private_element(name, val);
                        self.push(val);
                    } else {
                        return Err(VmError::TypeError(format!(
                            "Cannot install private field {} on non-object",
                            name
                        )));
                    }
                }
                OpCode::DefinePrivateGetter(name_idx) => {
                    let name = match &chunk.constants[name_idx as usize] {
                        Constant::String(s) => s.clone(),
                        _ => {
                            return Err(VmError::TypeError(
                                "Private getter name must be string".into(),
                            ));
                        }
                    };
                    let getter = self.pop()?;
                    let obj = self.pop()?;
                    if let Some(ptr) = obj.as_object_ptr() {
                        let jsobj = unsafe { &mut *(ptr as *mut JsObject) };
                        jsobj.insert_private_getter(name, getter);
                        self.push(getter);
                    } else {
                        return Err(VmError::TypeError(format!(
                            "Cannot install private getter {} on non-object",
                            name
                        )));
                    }
                }
                OpCode::DefinePrivateSetter(name_idx) => {
                    let name = match &chunk.constants[name_idx as usize] {
                        Constant::String(s) => s.clone(),
                        _ => {
                            return Err(VmError::TypeError(
                                "Private setter name must be string".into(),
                            ));
                        }
                    };
                    let setter = self.pop()?;
                    let obj = self.pop()?;
                    if let Some(ptr) = obj.as_object_ptr() {
                        let jsobj = unsafe { &mut *(ptr as *mut JsObject) };
                        jsobj.insert_private_setter(name, setter);
                        self.push(setter);
                    } else {
                        return Err(VmError::TypeError(format!(
                            "Cannot install private setter {} on non-object",
                            name
                        )));
                    }
                }

                OpCode::CreateFunction(const_idx) => {
                    self.op_create_function(const_idx, &chunk)?;
                }
                OpCode::CreateAsyncFunction(const_idx) => {
                    self.op_create_async_function(const_idx, &chunk)?;
                }
                OpCode::CreateGenerator(const_idx) => {
                    self.op_create_generator(const_idx, &chunk)?;
                }
                OpCode::CreateAsyncGenerator(const_idx) => {
                    self.op_create_async_generator(const_idx, &chunk)?;
                }

                OpCode::GeneratorStart => {
                    return Ok(self.op_generator_start()?);
                }

                OpCode::Call(argc) => {
                    self.op_call(argc, target_depth)?;
                }
                OpCode::CallWithThis(argc) => {
                    self.op_call_with_this(argc, target_depth)?;
                }
                OpCode::CallSpread => {
                    self.op_call_spread(target_depth)?;
                }
                OpCode::CallWithThisSpread => {
                    self.op_call_with_this_spread(target_depth)?;
                }
                OpCode::CallDirectEval(argc) => {
                    self.op_call_direct_eval(argc, target_depth)?;
                }

                OpCode::GetNewTarget => {
                    let target = self
                        .frames
                        .last()
                        .and_then(|f| f.new_target)
                        .unwrap_or(JsValue::undefined());
                    self.push(target);
                }

                OpCode::Construct(argc) => {
                    self.op_construct(argc, target_depth)?;
                }
                OpCode::ConstructSpread => {
                    self.op_construct_spread(target_depth)?;
                }

                OpCode::Return => {
                    if let Some(v) = self.op_return(target_depth)? {
                        return Ok(v);
                    }
                }
                OpCode::Await => {
                    self.op_await(target_depth)?;
                }
                OpCode::Yield => {
                    if let Some(v) = self.op_yield(target_depth)? {
                        return Ok(v);
                    }
                }
                OpCode::SetFunctionName(name_idx) => {
                    self.op_set_function_name(name_idx, &chunk)?;
                }

                OpCode::Export(name_idx) => {
                    self.op_export(name_idx, &chunk)?;
                }
                OpCode::ExportAll => {
                    self.op_export_all()?;
                }
                OpCode::BindImport(name_idx, exp_idx) => {
                    self.op_bind_import(name_idx, exp_idx, &chunk)?;
                }
                OpCode::ReExport(target_name_idx, exp_name_idx) => {
                    self.op_reexport(target_name_idx, exp_name_idx, &chunk)?;
                }
                OpCode::ImportModule(src_idx) => {
                    self.op_import_module(src_idx, &chunk)?;
                }
                OpCode::DynamicImport => {
                    self.op_dynamic_import()?;
                }

                OpCode::Add => {
                    self.op_add()?;
                }
                OpCode::Sub => {
                    self.op_sub()?;
                }
                OpCode::Mul => {
                    self.op_mul()?;
                }
                OpCode::Div => {
                    self.op_div()?;
                }
                OpCode::Mod => {
                    self.op_mod()?;
                }
                OpCode::Exp => {
                    self.op_exp()?;
                }
                OpCode::Pos => {
                    self.op_pos()?;
                }
                OpCode::Neg => {
                    self.op_neg()?;
                }
                OpCode::Void => {
                    let _ = self.pop()?;
                    self.push(JsValue::undefined());
                }
                OpCode::Delete => {
                    let _ = self.pop()?;
                    self.push(JsValue::from_boolean(true));
                }
                OpCode::DeleteProp(name_idx) => {
                    if let Some(v) = self.op_delete_prop(name_idx, &chunk, target_depth)? {
                        return Ok(v);
                    }
                }
                OpCode::DeleteElem => {
                    if let Some(v) = self.op_delete_elem(target_depth)? {
                        return Ok(v);
                    }
                }
                OpCode::DeleteIdent(name_idx) => {
                    if let Some(v) = self.op_delete_ident(name_idx, &chunk, target_depth)? {
                        return Ok(v);
                    }
                }
                OpCode::Not => {
                    self.op_not()?;
                }
                OpCode::BitNot => {
                    self.op_bit_not()?;
                }
                OpCode::Typeof => {
                    self.op_typeof()?;
                }

                OpCode::BitAnd => {
                    self.op_bit_and()?;
                }
                OpCode::BitOr => {
                    self.op_bit_or()?;
                }
                OpCode::BitXor => {
                    self.op_bit_xor()?;
                }
                OpCode::Shl => {
                    self.op_shl()?;
                }
                OpCode::Shr => {
                    self.op_shr()?;
                }
                OpCode::Ushr => {
                    self.op_ushr()?;
                }

                OpCode::StrictEq => {
                    self.op_strict_eq()?;
                }
                OpCode::StrictNotEq => {
                    self.op_strict_not_eq()?;
                }
                OpCode::Eq => {
                    self.op_eq()?;
                }
                OpCode::NotEq => {
                    self.op_not_eq()?;
                }
                OpCode::Lt => {
                    self.op_lt()?;
                }
                OpCode::Lte => {
                    self.op_lte()?;
                }
                OpCode::Gt => {
                    self.op_gt()?;
                }
                OpCode::Gte => {
                    self.op_gte()?;
                }
                OpCode::Instanceof => {
                    self.op_instanceof()?;
                }
                OpCode::In => {
                    self.op_in()?;
                }
                OpCode::PrivateIn(name_idx) => {
                    let name = match &chunk.constants[name_idx as usize] {
                        Constant::String(s) => s.clone(),
                        _ => {
                            return Err(VmError::TypeError(
                                "Private field name must be string".into(),
                            ));
                        }
                    };
                    let rhs = self.pop()?;
                    if !rhs.is_js_object() {
                        return Err(VmError::TypeError(
                            "Cannot use 'in' operator to search for private field in non-object"
                                .into(),
                        ));
                    }
                    let has = if let Some(ptr) = rhs.as_object_ptr() {
                        let obj = unsafe { &*(ptr as *mut JsObject) };
                        obj.contains_private_element(&name)
                    } else {
                        false
                    };
                    self.push(JsValue::from_boolean(has));
                }

                OpCode::Jump(target) => {
                    let t = target as usize;
                    let frame = self.frames.last_mut().unwrap();
                    if t <= frame.ip {
                        match self.maybe_trigger_osr(&chunk, t, target_depth)? {
                            OsrOutcome::Completed(ret_val) => {
                                if self.frames.len() == target_depth {
                                    return Ok(ret_val);
                                }
                                self.push(ret_val);
                                continue;
                            }
                            OsrOutcome::Yielded => {
                                continue;
                            }
                            OsrOutcome::NotTriggered => {}
                        }
                    }
                    let frame = self.frames.last_mut().unwrap();
                    frame.ip = t;
                }
                OpCode::JumpIfFalse(target) => {
                    let cond = self.pop()?;
                    if !cond.is_truthy() {
                        let t = target as usize;
                        let frame = self.frames.last_mut().unwrap();
                        if t <= frame.ip {
                            match self.maybe_trigger_osr(&chunk, t, target_depth)? {
                                OsrOutcome::Completed(ret_val) => {
                                    if self.frames.len() == target_depth {
                                        return Ok(ret_val);
                                    }
                                    self.push(ret_val);
                                    continue;
                                }
                                OsrOutcome::Yielded => {
                                    continue;
                                }
                                OsrOutcome::NotTriggered => {}
                            }
                        }
                        let frame = self.frames.last_mut().unwrap();
                        frame.ip = t;
                    }
                }
                OpCode::JumpIfTrue(target) => {
                    let cond = self.pop()?;
                    if cond.is_truthy() {
                        let t = target as usize;
                        let frame = self.frames.last_mut().unwrap();
                        if t <= frame.ip {
                            match self.maybe_trigger_osr(&chunk, t, target_depth)? {
                                OsrOutcome::Completed(ret_val) => {
                                    if self.frames.len() == target_depth {
                                        return Ok(ret_val);
                                    }
                                    self.push(ret_val);
                                    continue;
                                }
                                OsrOutcome::Yielded => {
                                    continue;
                                }
                                OsrOutcome::NotTriggered => {}
                            }
                        }
                        let frame = self.frames.last_mut().unwrap();
                        frame.ip = t;
                    }
                }

                OpCode::Throw => {
                    let err = self.pop()?;
                    return Err(VmError::Thrown(err));
                }
                OpCode::PushCatch(target) => {
                    self.op_push_catch(target);
                }
                OpCode::PushFinally(target, start, slot) => {
                    self.op_push_finally(target, start, slot);
                }
                OpCode::PopCatch => {
                    self.catch_stack.pop();
                }
                OpCode::EndFinally(slot) => {
                    if let Some(v) = self.op_end_finally(slot, target_depth)? {
                        return Ok(v);
                    }
                }

                OpCode::GetLocal(slot) => {
                    let frame = self.frames.last().unwrap();
                    let val =
                        frame.locals.get(slot as usize).copied().unwrap_or(JsValue::undefined());
                    self.push(val);
                }
                OpCode::SetLocal(slot) => {
                    self.op_set_local(slot)?;
                }
                OpCode::PushScope => {
                    let scope_obj = self.pop()?;
                    let frame = self.frames.last_mut().unwrap();
                    frame.scopes.push(scope_obj);
                }
                OpCode::PopScope => {
                    let frame = self.frames.last_mut().unwrap();
                    frame.scopes.pop();
                }
                OpCode::PushCatchScope(name_idx) => {
                    self.op_push_catch_scope(name_idx, &chunk)?;
                }
                OpCode::EnterFunctionBody => {
                    let frame = self.frames.last_mut().unwrap();
                    frame.in_param_init = false;
                }
                OpCode::DefGlobal(name_idx) => {
                    if let Some(v) = self.op_def_global(name_idx, &chunk, target_depth)? {
                        return Ok(v);
                    }
                }
                OpCode::DefLexical(name_idx) => {
                    if let Some(v) = self.op_def_lexical(name_idx, &chunk, target_depth)? {
                        return Ok(v);
                    }
                }
                OpCode::DefConst(name_idx) => {
                    if let Some(v) = self.op_def_const(name_idx, &chunk, target_depth)? {
                        return Ok(v);
                    }
                }
                OpCode::InitConst(name_idx) => {
                    self.op_init_const(name_idx, &chunk)?;
                }
                OpCode::GetGlobalSafe(name_idx) => {
                    if let Some(v) = self.op_get_global_safe(name_idx, &chunk, target_depth)? {
                        return Ok(v);
                    }
                }
                OpCode::GetGlobal(name_idx) => {
                    if let Some(v) = self.op_get_global(name_idx, &chunk, target_depth)? {
                        return Ok(v);
                    }
                }
                OpCode::SetGlobalDirect(name_idx) => {
                    if let Some(v) = self.op_set_global_direct(name_idx, &chunk, target_depth)? {
                        return Ok(v);
                    }
                }
                OpCode::SetGlobal(name_idx) => {
                    if let Some(v) = self.op_set_global(name_idx, &chunk, target_depth)? {
                        return Ok(v);
                    }
                }

                OpCode::Pop => {
                    self.pop()?;
                }
                OpCode::Dup => {
                    let val = *self.peek()?;
                    self.push(val);
                }
                OpCode::Halt => {
                    let finished = self.frames.pop().unwrap();
                    let ret_val = if self.stack.len() > finished.base_sp {
                        self.stack.pop().unwrap_or_default()
                    } else {
                        JsValue::undefined()
                    };
                    self.stack.truncate(finished.base_sp);
                    return Ok(ret_val);
                }
            }
        }

        Ok(self.pop().unwrap_or_default())
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum OsrOutcome {
    Completed(JsValue),
    Yielded,
    NotTriggered,
}

impl Vm {
    #[inline]
    fn maybe_trigger_osr(
        &mut self,
        chunk: &BytecodeChunk,
        target_ip: usize,
        _target_depth: usize,
    ) -> Result<OsrOutcome, VmError> {
        // Native loops run without per-instruction checks, so they must not be
        // entered while the embedder asked for preemption or fuel metering.
        // Falling back to the interpreter keeps interrupts, deadlines and
        // instruction budgets exact.
        if self.interrupt_flag.is_some()
            || self.deadline.is_some()
            || self.max_instructions.is_some()
        {
            return Ok(OsrOutcome::NotTriggered);
        }
        let count = self.loop_hotness.entry((chunk.id, target_ip)).or_insert(0);
        *count = count.saturating_add(1);
        if *count >= crate::jit::OSR_HOT_THRESHOLD {
            if !self.osr_cache.contains_key(&(chunk.id, target_ip)) {
                let jit_fn = crate::jit::JitCompiler::compile_osr(chunk, target_ip);
                self.osr_cache.insert((chunk.id, target_ip), jit_fn);
            }
            let osr_native = self.osr_cache.get(&(chunk.id, target_ip)).and_then(|opt| {
                opt.as_ref().and_then(|jit| {
                    if let crate::jit::JitCode::Native { func_ptr, num_locals, .. } = &jit.code {
                        Some((*func_ptr, *num_locals))
                    } else {
                        None
                    }
                })
            });
            if let Some((func_ptr, num_locals)) = osr_native {
                let prev_vm = crate::jit::CURRENT_VM.with(|c| c.get());
                crate::jit::CURRENT_VM.with(|c| c.set(self as *mut Vm));
                crate::jit::NURSERY_PTRS.with(|cell| {
                    let ptrs = unsafe { &mut *cell.get() };
                    ptrs.bump = self.heap.nursery.cursor();
                    ptrs.limit = self.heap.nursery.limit();
                });

                let (locals_ptr, locals_len) = {
                    let frame = self.frames.last_mut().unwrap();
                    if frame.locals.len() < num_locals {
                        frame.locals.resize(num_locals, JsValue::undefined());
                    }
                    (frame.locals.as_mut_ptr() as *mut u64, frame.locals.len())
                };

                let cur_this =
                    self.frames.last().map(|f| f.this_val.0).unwrap_or(crate::value::TAG_UNDEFINED);
                let res = unsafe { (func_ptr)(locals_ptr, locals_len, cur_this) };

                crate::jit::NURSERY_PTRS.with(|cell| {
                    let ptrs = unsafe { &mut *cell.get() };
                    if !ptrs.bump.is_null() {
                        if ptrs.bump > self.heap.nursery.cursor() {
                            self.heap.nursery.set_cursor(ptrs.bump);
                        } else {
                            ptrs.bump = self.heap.nursery.cursor();
                        }
                    }
                });
                crate::jit::CURRENT_VM.with(|c| c.set(prev_vm));
                let yielded_ip = crate::jit::OSR_YIELDED_IP.with(|c| c.take());
                if res != crate::jit::JIT_DEOPT_SENTINEL {
                    let finished = self.frames.pop().unwrap();
                    for (idx, name) in finished.local_names.iter().enumerate() {
                        if let Some(val) = finished.locals.get(idx) {
                            if self.global_lexicals.contains_key(name) {
                                self.global_lexicals.insert(name.clone(), *val);
                            }
                        }
                    }
                    while let Some(top) = self.catch_stack.last() {
                        if top.frame_idx >= self.frames.len() {
                            self.catch_stack.pop();
                        } else {
                            break;
                        }
                    }
                    let ret_val = JsValue(res);
                    self.stack.truncate(finished.base_sp);
                    return Ok(OsrOutcome::Completed(ret_val));
                } else if let Some(target) = yielded_ip {
                    if let Some(frame) = self.frames.last_mut() {
                        frame.ip = target;
                    }
                    return Ok(OsrOutcome::Yielded);
                } else {
                    return Ok(OsrOutcome::NotTriggered);
                }
            }
        }
        Ok(OsrOutcome::NotTriggered)
    }
}
