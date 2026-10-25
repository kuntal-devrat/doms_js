//! Execution entry points and the main bytecode dispatch loop.

use super::*;

impl Vm {
    pub fn run(&mut self, chunk: &BytecodeChunk) -> Result<JsValue, VmError> {
        self.instantiate_global_declarations(chunk)?;
        let root_chunk = Arc::new(chunk.clone());
        let target_depth = self.frames.len();
        let local_names = Arc::clone(&root_chunk.local_names);
        let lexical_local_names = Arc::clone(&root_chunk.lexical_local_names);
        let mut initial_locals = vec![JsValue::undefined(); local_names.len()];
        for (i, name) in local_names.iter().enumerate() {
            if root_chunk.top_lex_names.contains(name) {
                initial_locals[i] = JsValue::uninitialized();
            }
        }
        for name in &root_chunk.top_const_names {
            self.const_bindings.insert(name.clone());
        }
        self.frames.push(CallFrame {
            chunk: root_chunk,
            ip: 0,
            base_sp: self.stack.len(),
            this_val: if self.is_module { JsValue::undefined() } else { self.global_object },
            gen_val: None,
            locals: initial_locals,
            local_names,
            lexical_local_names,
            scopes: Vec::new(),
            upvalues_count: 0,
            env_obj: None,
            param_env_obj: None,
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
            module: self.current_module_obj,
            in_field_init: false,
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
        if target_depth == 0 {
            self.drain_microtasks()?;
        }
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
                    if this_val == JsValue::uninitialized() {
                        self.handle_exception_at_depth(
                            VmError::ReferenceError(
                                "Must call super constructor in derived class before accessing 'this'"
                                    .into(),
                            ),
                            target_depth,
                        )?;
                        continue;
                    }
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
                    if frame.this_val == JsValue::uninitialized() {
                        self.handle_exception_at_depth(
                            VmError::ReferenceError(
                                "Must call super constructor in derived class before accessing 'this'"
                                    .into(),
                            ),
                            target_depth,
                        )?;
                        continue;
                    }
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
                OpCode::GetTemplateObject(idx) => {
                    self.op_get_template_object(idx, &chunk)?;
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
                OpCode::DefineDataProp(prop_idx) => {
                    let name = match &chunk.constants[prop_idx as usize] {
                        Constant::String(s) => s.clone(),
                        _ => return Err(VmError::TypeError("Property name must be string".into())),
                    };
                    let val = self.pop()?;
                    let obj = *self.stack.last().ok_or(VmError::StackUnderflow)?;
                    if let Some(val_ptr) = val.as_object_ptr() {
                        let val_obj = unsafe { &mut *(val_ptr as *mut JsObject) };
                        if let ObjectKind::Function(ref mut f) = val_obj.kind {
                            if f.name.as_deref().unwrap_or("").is_empty() {
                                f.name = Some(name.clone());
                                let name_val = self.alloc_string(name.clone());
                                self.define_builtin_property(
                                    val,
                                    "name",
                                    name_val,
                                    crate::shape::PropertyAttributes {
                                        writable: false,
                                        enumerable: false,
                                        configurable: true,
                                    },
                                );
                            }
                        }
                    }
                    self.create_data_property_or_throw(obj, &name, val)?;
                }
                OpCode::DefineDataPropComputed => {
                    let val = self.pop()?;
                    let key_val = self.pop()?;
                    let obj = *self.stack.last().ok_or(VmError::StackUnderflow)?;
                    let prop_key = self.to_property_key(key_val)?;
                    match prop_key {
                        PropertyKeyVal::String(s) => {
                            if let Some(val_ptr) = val.as_object_ptr() {
                                let val_obj = unsafe { &mut *(val_ptr as *mut JsObject) };
                                if let ObjectKind::Function(ref mut f) = val_obj.kind {
                                    if f.name.as_deref().unwrap_or("").is_empty() {
                                        f.name = Some(s.clone());
                                    }
                                }
                            }
                            self.create_data_property_or_throw(obj, &s, val)?;
                        }
                        PropertyKeyVal::Symbol(sym) => {
                            if let Some(ptr) = obj.as_object_ptr() {
                                let jsobj = unsafe { &mut *(ptr as *mut JsObject) };
                                jsobj.insert_symbol_property(sym, val);
                            }
                            if let Some(val_ptr) = val.as_object_ptr() {
                                let val_obj = unsafe { &mut *(val_ptr as *mut JsObject) };
                                if let ObjectKind::Function(ref mut f) = val_obj.kind {
                                    if f.is_method && f.home_object.is_none() {
                                        f.home_object = Some(obj);
                                    }
                                }
                            }
                        }
                    }
                }
                OpCode::EnterFieldInit => {
                    if let Some(frame) = self.frames.last_mut() {
                        frame.in_field_init = true;
                    }
                }
                OpCode::LeaveFieldInit => {
                    if let Some(frame) = self.frames.last_mut() {
                        frame.in_field_init = false;
                    }
                }
                OpCode::SetProtoOrNoop => {
                    let val = self.pop()?;
                    let obj = *self.stack.last().ok_or(VmError::StackUnderflow)?;
                    if val.is_object() || val.is_null() {
                        if let Some(ptr) = obj.as_object_ptr() {
                            let jsobj = unsafe { &mut *(ptr as *mut JsObject) };
                            jsobj.prototype = if val.is_null() { None } else { Some(val) };
                        }
                    }
                }
                OpCode::ToPropertyKey => {
                    let key = self.pop()?;
                    let prop_key = self.to_property_key(key)?;
                    match prop_key {
                        PropertyKeyVal::String(s) => {
                            let s_val = self.alloc_string(s);
                            self.push(s_val);
                        }
                        PropertyKeyVal::Symbol(sym) => {
                            self.push(sym);
                        }
                    }
                }
                OpCode::SetFunctionNameFromKey(prefix_kind) => {
                    self.op_set_function_name_from_key(prefix_kind, target_depth)?;
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
                                                && !self.array_proto_tainted
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
                        if !jsobj.is_extensible() {
                            return Err(VmError::TypeError(
                                "Cannot add private field to non-extensible object".into(),
                            ));
                        }
                        if jsobj.private_elements.as_ref().map_or(false, |p| p.contains_key(&name)) {
                            return Err(VmError::TypeError(
                                format!("Cannot add private field {} twice", name),
                            ));
                        }
                        if let Some(val_ptr) = val.as_object_ptr() {
                            let val_obj = unsafe { &mut *(val_ptr as *mut JsObject) };
                            if let ObjectKind::Function(ref mut f) = val_obj.kind {
                                if f.name.as_deref().unwrap_or("").is_empty() {
                                    let display_name = name.split('@').next().unwrap_or(&name);
                                    f.name = Some(display_name.to_string());
                                    let name_val = self.alloc_string(display_name.to_string());
                                    self.define_builtin_property(
                                        val,
                                        "name",
                                        name_val,
                                        crate::shape::PropertyAttributes {
                                            writable: false,
                                            enumerable: false,
                                            configurable: true,
                                        },
                                    );
                                }
                            }
                        }
                        let brand = self.current_class_brand();
                        jsobj.insert_private_element(name, val, brand);
                        self.push(val);
                    } else {
                        return Err(VmError::TypeError(format!(
                            "Cannot install private field {} on non-object",
                            name
                        )));
                    }
                }
                OpCode::DefinePrivateMethod(name_idx) => {
                    let name = match &chunk.constants[name_idx as usize] {
                        Constant::String(s) => s.clone(),
                        _ => {
                            return Err(VmError::TypeError(
                                "Private method name must be string".into(),
                            ));
                        }
                    };
                    let val = self.pop()?;
                    let obj = self.pop()?;
                    if let Some(ptr) = obj.as_object_ptr() {
                        let jsobj = unsafe { &mut *(ptr as *mut JsObject) };
                        if !jsobj.is_extensible() {
                            return Err(VmError::TypeError(
                                "Cannot add private method to non-extensible object".into(),
                            ));
                        }
                        if jsobj.private_elements.as_ref().map_or(false, |p| p.contains_key(&name)) {
                            return Err(VmError::TypeError(
                                format!("Cannot add private method {} twice", name),
                            ));
                        }
                        if let Some(val_ptr) = val.as_object_ptr() {
                            let val_obj = unsafe { &mut *(val_ptr as *mut JsObject) };
                            if let ObjectKind::Function(ref mut f) = val_obj.kind {
                                if f.name.as_deref().unwrap_or("").is_empty() {
                                    let display_name = name.split('@').next().unwrap_or(&name);
                                    f.name = Some(display_name.to_string());
                                }
                                if f.is_method && f.home_object.is_none() {
                                    f.home_object = Some(obj);
                                }
                            }
                        }
                        jsobj.insert_private_method(name, val);
                        self.push(val);
                    } else {
                        return Err(VmError::TypeError(format!(
                            "Cannot install private method {} on non-object",
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
                        if !jsobj.is_extensible() {
                            return Err(VmError::TypeError(
                                "Cannot add private getter to non-extensible object".into(),
                            ));
                        }
                        if let Some(p) = jsobj.private_elements.as_ref() {
                            if let Some(entry) = p.get(&name) {
                                match entry {
                                    crate::object::PrivateElement::Accessor(acc)
                                        if acc.getter.is_none() => {}
                                    _ => {
                                        return Err(VmError::TypeError(
                                            format!("Cannot define private getter {} twice", name),
                                        ));
                                    }
                                }
                            }
                        }
                        if let Some(val_ptr) = getter.as_object_ptr() {
                            let val_obj = unsafe { &mut *(val_ptr as *mut JsObject) };
                            if let ObjectKind::Function(ref mut f) = val_obj.kind {
                                if f.name.as_deref().unwrap_or("").is_empty() {
                                    let display_name = name.split('@').next().unwrap_or(&name);
                                    f.name = Some(format!("get {}", display_name));
                                }
                                if f.is_method && f.home_object.is_none() {
                                    f.home_object = Some(obj);
                                }
                            }
                        }
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
                        if !jsobj.is_extensible() {
                            return Err(VmError::TypeError(
                                "Cannot add private setter to non-extensible object".into(),
                            ));
                        }
                        if let Some(p) = jsobj.private_elements.as_ref() {
                            if let Some(entry) = p.get(&name) {
                                match entry {
                                    crate::object::PrivateElement::Accessor(acc)
                                        if acc.setter.is_none() => {}
                                    _ => {
                                        return Err(VmError::TypeError(
                                            format!("Cannot define private setter {} twice", name),
                                        ));
                                    }
                                }
                            }
                        }
                        if let Some(val_ptr) = setter.as_object_ptr() {
                            let val_obj = unsafe { &mut *(val_ptr as *mut JsObject) };
                            if let ObjectKind::Function(ref mut f) = val_obj.kind {
                                if f.name.as_deref().unwrap_or("").is_empty() {
                                    let display_name = name.split('@').next().unwrap_or(&name);
                                    f.name = Some(format!("set {}", display_name));
                                }
                                if f.is_method && f.home_object.is_none() {
                                    f.home_object = Some(obj);
                                }
                            }
                        }
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
                OpCode::SuperCall(argc) => {
                    let argc_usize = argc as usize;
                    let stack_len = self.stack.len();
                    if stack_len < argc_usize {
                        return Err(VmError::TypeError("Stack underflow in op_super_call".into()));
                    }
                    let args_start = stack_len - argc_usize;
                    let mut args = Vec::with_capacity(argc_usize);
                    args.extend_from_slice(&self.stack[args_start..stack_len]);
                    self.stack.truncate(args_start);
                    self.op_super_call(&args, target_depth)?;
                }
                OpCode::SuperCallSpread => {
                    let args_val = self.pop()?;
                    let args = self.array_to_vec_spread(args_val);
                    self.op_super_call(&args, target_depth)?;
                }
                OpCode::SuperCallForward => {
                    let raw =
                        self.frames.last().and_then(|f| f.raw_args.clone()).unwrap_or_default();
                    self.op_super_call(&raw, target_depth)?;
                }

                OpCode::Return => {
                    if let Some(v) = self.op_return(target_depth)? {
                        return Ok(v);
                    }
                }
                OpCode::Await => {
                    let val = self.pop()?;
                    let is_rejected = if let Some(ptr) = val.as_object_ptr() {
                        let obj = unsafe { &*(ptr as *mut JsObject) };
                        if let ObjectKind::Promise(p) = &obj.kind {
                            if p.state == PromiseState::Rejected {
                                Some(p.result)
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    };

                    if let Some(err_val) = is_rejected {
                        return Err(VmError::Thrown(err_val));
                    }

                    let is_top_level_module = self.frames.last().map(|f| !f.is_function).unwrap_or(false)
                        && target_depth > 0
                        && self.is_module;
                    if is_top_level_module {
                        let finished = self.frames.pop().unwrap();
                        let next_ip = finished.ip;
                        let chunk_to_resume = Arc::clone(&finished.chunk);
                        let saved_locals = finished.locals.clone();
                        let saved_mod = self.current_module_obj;
                        let saved_base = self.base_dir.clone();
                        let saved_is_strict = finished.is_strict;

                        if let Some(mod_val) = saved_mod {
                            self.module_eval_status.insert(mod_val, ModuleStatus::EvaluatingAsync);
                        }

                        let mut resolved_val = val;
                        if let Some(ptr) = val.as_object_ptr() {
                            let obj = unsafe { &*(ptr as *mut JsObject) };
                            if let ObjectKind::Promise(p) = &obj.kind {
                                if p.state == PromiseState::Fulfilled {
                                    resolved_val = p.result;
                                }
                            }
                        }

                        let continuation = self.alloc_closure_native_function(
                            "tla_continuation",
                            1,
                            Arc::new(move |vm, _this, args| {
                                let arg_val = args.first().copied().unwrap_or(resolved_val);
                                let old_mod = vm.current_module_obj;
                                let old_base = vm.base_dir.clone();
                                let old_is_mod = vm.is_module;
                                vm.is_module = true;
                                vm.current_module_obj = saved_mod;
                                vm.base_dir = saved_base.clone();
                                if let Some(mod_val) = saved_mod {
                                    vm.module_eval_status.insert(mod_val, ModuleStatus::Evaluating);
                                }

                                let target_depth = vm.frames.len();
                                let base_sp = vm.stack.len();
                                vm.push(arg_val);
                                vm.frames.push(crate::interpreter::CallFrame {
                                    chunk: Arc::clone(&chunk_to_resume),
                                    ip: next_ip,
                                    base_sp,
                                    this_val: JsValue::undefined(),
                                    gen_val: None,
                                    locals: saved_locals.clone(),
                                    local_names: Arc::clone(&chunk_to_resume.local_names),
                                    lexical_local_names: Arc::clone(&chunk_to_resume.lexical_local_names),
                                    scopes: Vec::new(),
                                    upvalues_count: 0,
                                    env_obj: None,
                                    param_env_obj: None,
                                    arguments: None,
                                    raw_args: None,
                                    is_strict: saved_is_strict,
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
                                    module: saved_mod,
                                    in_field_init: false,
                                });

                                let run_res = vm.run_until_depth(target_depth);

                                vm.is_module = old_is_mod;
                                vm.current_module_obj = old_mod;
                                vm.base_dir = old_base;

                                if let Some(mod_val) = saved_mod {
                                    match &run_res {
                                        Ok(_) => {
                                            let current_st = vm
                                                .module_eval_status
                                                .get(&mod_val)
                                                .copied()
                                                .unwrap_or(ModuleStatus::Evaluating);
                                            if current_st != ModuleStatus::EvaluatingAsync {
                                                vm.prevent_extensions_object(mod_val);
                                                vm.module_eval_status.insert(mod_val, ModuleStatus::Evaluated);
                                                if let Some(eval_p) = vm.module_eval_promises.get(&mod_val).copied() {
                                                    vm.resolve_promise(eval_p, mod_val);
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            vm.prevent_extensions_object(mod_val);
                                            let err_val = vm.vm_error_to_js_value(e);
                                            vm.module_eval_errors.insert(mod_val, err_val);
                                            vm.module_eval_status.insert(mod_val, ModuleStatus::Evaluated);
                                            if let Some(eval_p) = vm.module_eval_promises.get(&mod_val).copied() {
                                                vm.reject_promise(eval_p, err_val);
                                            }
                                        }
                                    }
                                }

                                run_res
                            }),
                        );

                        let is_pending = if let Some(ptr) = val.as_object_ptr() {
                            let obj = unsafe { &*(ptr as *mut JsObject) };
                            if let ObjectKind::Promise(p) = &obj.kind {
                                p.state == PromiseState::Pending
                            } else {
                                false
                            }
                        } else {
                            false
                        };

                        if is_pending {
                            let cont_c = continuation;
                            let on_fulfill = self.alloc_closure_native_function(
                                "tla_on_fulfill",
                                1,
                                Arc::new(move |vm, _this, args| {
                                    let resolved = args.first().copied().unwrap_or(JsValue::undefined());
                                    vm.call(cont_c, JsValue::undefined(), &[resolved])
                                }),
                            );
                            let on_reject = self.alloc_closure_native_function(
                                "tla_on_reject",
                                1,
                                Arc::new(move |vm, _this, args| {
                                    let reason = args.first().copied().unwrap_or(JsValue::undefined());
                                    if let Some(mod_val) = saved_mod {
                                        vm.module_eval_errors.insert(mod_val, reason);
                                        vm.module_eval_status.insert(mod_val, ModuleStatus::Evaluated);
                                        if let Some(eval_p) = vm.module_eval_promises.get(&mod_val).copied() {
                                            vm.reject_promise(eval_p, reason);
                                        }
                                    }
                                    Ok(JsValue::undefined())
                                }),
                            );
                            let _ = self.promise_then(val, on_fulfill, on_reject);
                        } else {
                            self.enqueue_microtask(continuation, Vec::new());
                        }

                        return Ok(JsValue::undefined());
                    } else {
                        self.push(val);
                        self.op_await(target_depth)?;
                    }
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
                OpCode::AwaitImports => {
                    self.op_await_imports()?;
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
                OpCode::Inc => {
                    self.op_inc()?;
                }
                OpCode::Dec => {
                    self.op_dec()?;
                }
                OpCode::ToNumeric => {
                    self.op_to_numeric()?;
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
                OpCode::DeleteSuper => {
                    self.handle_exception_at_depth(
                        VmError::ReferenceError("Unsupported reference to 'super'".into()),
                        target_depth,
                    )?;
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
                OpCode::JumpIfLocalDefined(slot, target) => {
                    let (arg, param_env_opt, name_opt) = {
                        let frame = self.frames.last_mut().unwrap();
                        let slot_idx = slot as usize;
                        let arg = frame
                            .raw_args
                            .as_ref()
                            .and_then(|args| args.get(slot_idx).copied())
                            .unwrap_or(JsValue::undefined());
                        if !arg.is_undefined() {
                            frame.locals[slot_idx] = arg;
                            (arg, frame.param_env_obj, frame.local_names.get(slot_idx).cloned())
                        } else {
                            (JsValue::undefined(), None, None)
                        }
                    };
                    if !arg.is_undefined() {
                        if let (Some(param_env), Some(name)) = (param_env_opt, name_opt) {
                            if !name.starts_with('@') && !name.starts_with('[') && !name.starts_with('{') {
                                self.set_property(param_env, &name, arg);
                            }
                        }
                        let frame = self.frames.last_mut().unwrap();
                        frame.ip = target as usize;
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
                    if val.is_uninitialized() {
                        let name = frame
                            .local_names
                            .get(slot as usize)
                            .cloned()
                            .unwrap_or_else(|| "variable".to_string());
                        self.handle_exception_at_depth(
                            VmError::ReferenceError(format!(
                                "Cannot access '{}' before initialization",
                                name
                            )),
                            target_depth,
                        )?;
                        continue;
                    }
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
                OpCode::PushClassScope(name_idx) => {
                    let name = match &chunk.constants[name_idx as usize] {
                        Constant::String(s) => s.clone(),
                        _ => return Err(VmError::TypeError("Class name must be string".into())),
                    };
                    let scope_obj = self.alloc_object();
                    self.temp_roots.push(scope_obj);
                    self.set_own_property(scope_obj, &name, JsValue::uninitialized());
                    self.temp_roots.pop();
                    let frame = self.frames.last_mut().unwrap();
                    frame.scopes.push(scope_obj);
                }
                OpCode::InitClassBinding(name_idx) => {
                    let name = match &chunk.constants[name_idx as usize] {
                        Constant::String(s) => s.clone(),
                        _ => return Err(VmError::TypeError("Class name must be string".into())),
                    };
                    let ctor = *self.peek()?;
                    let frame = self.frames.last().unwrap();
                    if let Some(&scope_obj) = frame.scopes.last() {
                        self.set_own_property(scope_obj, &name, ctor);
                        self.set_property_non_writable(scope_obj, &name);
                    }
                }
                OpCode::StoreScopeBinding(name_idx) => {
                    let name = match &chunk.constants[name_idx as usize] {
                        Constant::String(s) => s.clone(),
                        _ => return Err(VmError::TypeError("Binding name must be string".into())),
                    };
                    let val = self.pop()?;
                    let frame = self.frames.last().unwrap();
                    if let Some(&scope_obj) = frame.scopes.last() {
                        self.set_own_property(scope_obj, &name, val);
                    }
                }
                OpCode::EnterFunctionBody => {
                    let frame = self.frames.last_mut().unwrap();
                    frame.in_param_init = false;
                    for (i, val) in frame.locals.iter_mut().enumerate() {
                        if val.is_uninitialized() {
                            let arg_val = frame
                                .raw_args
                                .as_ref()
                                .and_then(|args| args.get(i).copied())
                                .unwrap_or(JsValue::undefined());
                            *val =
                                if arg_val.is_undefined() { JsValue::undefined() } else { arg_val };
                        }
                    }
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
                OpCode::TouchBinding(name_idx) => {
                    let name = match &chunk.constants[name_idx as usize] {
                        Constant::String(s) => s.as_str(),
                        _ => return Err(VmError::TypeError("Binding name must be string".into())),
                    };
                    self.touch_identifier_reference(name)?;
                }
                OpCode::ThrowConstAssignment(name_idx) => {
                    let name = match &chunk.constants[name_idx as usize] {
                        Constant::String(s) => s.as_str(),
                        _ => "variable",
                    };
                    self.handle_exception_at_depth(
                        VmError::TypeError(format!("Assignment to constant variable '{}'", name)),
                        target_depth,
                    )?;
                    continue;
                }
                OpCode::CheckLRefStrict(name_idx) => {
                    let name = match &chunk.constants[name_idx as usize] {
                        Constant::String(s) => s.as_str(),
                        _ => return Err(VmError::TypeError("Binding name must be string".into())),
                    };
                    let resolvable = self.is_identifier_resolvable(name)?;
                    self.push(JsValue::from_boolean(resolvable));
                }
                OpCode::SetGlobalStrictResolved(name_idx) => {
                    let val = self.pop()?;
                    let is_resolvable = self.pop()?.as_boolean().unwrap_or(false);
                    let name = match &chunk.constants[name_idx as usize] {
                        Constant::String(s) => s.clone(),
                        _ => return Err(VmError::TypeError("Binding name must be string".into())),
                    };
                    if !is_resolvable {
                        self.handle_exception_at_depth(
                            VmError::ReferenceError(format!("Assignment to undeclared variable {}", name)),
                            target_depth,
                        )?;
                        continue;
                    }
                    if let Some(target_opt) = self.touched_targets.remove(&name) {
                        if let Some(target_scope) = target_opt {
                            let still_exists = match self.has_property(target_scope, &name) {
                                Ok(h) => h,
                                Err(e) => {
                                    self.handle_exception_at_depth(e, target_depth)?;
                                    continue;
                                }
                            };
                            let is_strict = self.is_strict
                                || self.frames.last().map(|f| f.is_strict).unwrap_or(false);
                            if !still_exists && is_strict {
                                self.handle_exception_at_depth(
                                    VmError::ReferenceError(format!("{} is not defined", name)),
                                    target_depth,
                                )?;
                                continue;
                            }
                            self.push(val);
                            if let Err(e) = self.set_property_result(target_scope, &name, val) {
                                self.handle_exception_at_depth(e, target_depth)?;
                            }
                            for f in &mut self.frames {
                                if f.env_obj == Some(target_scope) {
                                    for (i, l_name) in f.local_names.iter().enumerate() {
                                        if l_name == &name && i < f.locals.len() {
                                            f.locals[i] = val;
                                        }
                                    }
                                }
                            }
                            continue;
                        }
                    }
                    self.push(val);
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
                OpCode::SetMethodHomeObject => {
                    let func_val = self.pop()?;
                    let home_obj = self.pop()?;
                    if let Some(f_ptr) = func_val.as_object_ptr() {
                        let f_obj = unsafe { &mut *(f_ptr as *mut JsObject) };
                        if let ObjectKind::Function(ref mut f) = f_obj.kind {
                            f.home_object = Some(home_obj);
                        }
                    }
                    self.push(func_val);
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

    pub(crate) fn current_class_brand(&self) -> Option<JsValue> {
        self.frames.iter().rev().find_map(|f| f.home_object.or(f.callee_val))
    }

    pub(crate) fn check_private_brand(&mut self, brand: JsValue) -> bool {
        let caller_brand = self.current_class_brand();
        let Some(home) = caller_brand else { return false; };
        if home == brand {
            return true;
        }
        if let Some(f_ptr) = home.as_object_ptr() {
            let f_obj = unsafe { &*(f_ptr as *mut JsObject) };
            if let ObjectKind::Function(ref f) = f_obj.kind {
                if f.prototype == Some(brand) || f.home_object == Some(brand) {
                    return true;
                }
            }
        }
        if let Some(b_ptr) = brand.as_object_ptr() {
            let b_obj = unsafe { &*(b_ptr as *mut JsObject) };
            if let ObjectKind::Function(ref f) = b_obj.kind {
                if f.prototype == Some(home) || f.home_object == Some(home) {
                    return true;
                }
            }
        }
        false
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
                let res = unsafe { (func_ptr)(locals_ptr, locals_len, cur_this, 0) };

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
