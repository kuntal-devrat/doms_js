//! Function/closure creation and direct `eval` calls.

use super::*;

impl Vm {
    pub(crate) fn capture_lexical_state(&self, is_arrow: bool) -> (Vec<JsValue>, bool) {
        if let Some(frame) = self.frames.last() {
            let mut brands = Vec::new();
            if let Some(ref cf) = frame.callee_func {
                brands.extend(cf.lexical_brands.iter().copied());
            }
            if let Some(h) = frame.home_object {
                if !brands.contains(&h) {
                    brands.push(h);
                }
            }
            if frame.is_class_constructor {
                if let Some(c) = frame.callee_val {
                    if !brands.contains(&c) {
                        brands.push(c);
                    }
                    if let Some(c_ptr) = c.as_object_ptr() {
                        let c_obj = unsafe { &*(c_ptr as *mut JsObject) };
                        if let ObjectKind::Function(ref f) = c_obj.kind {
                            if let Some(proto) = f.prototype {
                                if !brands.contains(&proto) {
                                    brands.push(proto);
                                }
                            }
                        }
                    }
                }
            }
            let in_field = if is_arrow {
                frame.in_field_init
                    || frame
                        .callee_func
                        .as_ref()
                        .map_or(false, |func| func.lexical_in_field_init)
            } else {
                false
            };
            (brands, in_field)
        } else {
            (Vec::new(), false)
        }
    }

    pub(crate) fn op_create_function(
        &mut self,
        const_idx: u16,
        chunk: &BytecodeChunk,
    ) -> Result<(), VmError> {
        let def = match &chunk.constants[const_idx as usize] {
            Constant::Function(f) => Arc::clone(f),
            _ => return Err(VmError::TypeError("Expected function def".into())),
        };
        let upvalues = self.capture_upvalues();
        let home_object =
            if def.is_arrow { self.frames.last().and_then(|f| f.home_object) } else { None };
        let (lexical_this, lexical_new_target, lexical_callee) = if def.is_arrow {
            let frame = self.frames.last();
            (
                frame.map(|f| f.this_val),
                frame.and_then(|f| f.new_target),
                frame.and_then(|f| {
                    if f.is_arrow {
                        f.callee_func.as_ref().and_then(|func| func.lexical_callee)
                    } else {
                        f.callee_val
                    }
                }),
            )
        } else {
            (None, None, None)
        };
        let (lexical_brands, lexical_in_field_init) = self.capture_lexical_state(def.is_arrow);
        let cur_mod = self.frames.last().and_then(|f| f.module).or(self.current_module_obj);
        let js_fn = JsFunction {
            name: def.name.clone(),
            params: def.params.clone(),
            length: def.length,
            self_slot: def.self_slot,
            local_names: def.local_names.clone(),
            lexical_local_names: def.lexical_local_names.clone(),
            chunk: Arc::new(def.chunk.clone()),
            upvalues,
            is_async: false,
            is_generator: false,
            call_count: 0,
            jit_entry: None,
            jit_attempted: false,
            is_strict: def.is_strict,
            has_param_exprs: def.has_param_exprs,
            is_method: def.is_method,
            is_arrow: def.is_arrow,
            home_object,
            lexical_this,
            lexical_new_target,
            lexical_callee,
            prototype: None,
            initial_shape: None,
            is_class_constructor: def.is_class_constructor,
            is_derived_constructor: def.is_derived_constructor,
            module: cur_mod,
            lexical_brands,
            lexical_in_field_init,
        };
        let func_val = self.alloc_function(js_fn);
        if def.is_class_constructor {
            if let Some(f_ptr) = func_val.as_object_ptr() {
                let f_obj = unsafe { &mut *(f_ptr as *mut JsObject) };
                if let ObjectKind::Function(ref mut f) = f_obj.kind {
                    if !f.lexical_brands.contains(&func_val) {
                        f.lexical_brands.push(func_val);
                    }
                }
            }
        }
        self.push(func_val);
        Ok(())
    }

    pub(crate) fn op_create_async_function(
        &mut self,
        const_idx: u16,
        chunk: &BytecodeChunk,
    ) -> Result<(), VmError> {
        let def = match &chunk.constants[const_idx as usize] {
            Constant::Function(f) => Arc::clone(f),
            _ => return Err(VmError::TypeError("Expected function def".into())),
        };
        let upvalues = self.capture_upvalues();
        let home_object =
            if def.is_arrow { self.frames.last().and_then(|f| f.home_object) } else { None };
        let (lexical_this, lexical_new_target, lexical_callee) = if def.is_arrow {
            let frame = self.frames.last();
            (
                frame.map(|f| f.this_val),
                frame.and_then(|f| f.new_target),
                frame.and_then(|f| {
                    if f.is_arrow {
                        f.callee_func.as_ref().and_then(|func| func.lexical_callee)
                    } else {
                        f.callee_val
                    }
                }),
            )
        } else {
            (None, None, None)
        };
        let cur_mod = self.frames.last().and_then(|f| f.module).or(self.current_module_obj);
        let js_fn = JsFunction {
            name: def.name.clone(),
            params: def.params.clone(),
            length: def.length,
            self_slot: def.self_slot,
            local_names: def.local_names.clone(),
            lexical_local_names: def.lexical_local_names.clone(),
            chunk: Arc::new(def.chunk.clone()),
            upvalues,
            is_async: true,
            is_generator: false,
            call_count: 0,
            jit_entry: None,
            jit_attempted: false,
            is_strict: def.is_strict,
            has_param_exprs: def.has_param_exprs,
            is_method: def.is_method,
            is_arrow: def.is_arrow,
            home_object,
            lexical_this,
            lexical_new_target,
            lexical_callee,
            prototype: None,
            initial_shape: None,
            is_class_constructor: false,
            is_derived_constructor: false,
            module: cur_mod,
        };
        let func_val = self.alloc_function(js_fn);
        self.push(func_val);
        Ok(())
    }

    pub(crate) fn op_create_generator(
        &mut self,
        const_idx: u16,
        chunk: &BytecodeChunk,
    ) -> Result<(), VmError> {
        let def = match &chunk.constants[const_idx as usize] {
            Constant::Function(f) => Arc::clone(f),
            _ => return Err(VmError::TypeError("Expected function def".into())),
        };
        let upvalues = self.capture_upvalues();
        let home_object =
            if def.is_arrow { self.frames.last().and_then(|f| f.home_object) } else { None };
        let (lexical_this, lexical_new_target, lexical_callee) = if def.is_arrow {
            let frame = self.frames.last();
            (
                frame.map(|f| f.this_val),
                frame.and_then(|f| f.new_target),
                frame.and_then(|f| {
                    if f.is_arrow {
                        f.callee_func.as_ref().and_then(|func| func.lexical_callee)
                    } else {
                        f.callee_val
                    }
                }),
            )
        } else {
            (None, None, None)
        };
        let cur_mod = self.frames.last().and_then(|f| f.module).or(self.current_module_obj);
        let js_fn = JsFunction {
            name: def.name.clone(),
            params: def.params.clone(),
            length: def.length,
            self_slot: def.self_slot,
            local_names: def.local_names.clone(),
            lexical_local_names: def.lexical_local_names.clone(),
            chunk: Arc::new(def.chunk.clone()),
            upvalues,
            is_async: false,
            is_generator: true,
            call_count: 0,
            jit_entry: None,
            jit_attempted: false,
            is_strict: def.is_strict,
            has_param_exprs: def.has_param_exprs,
            is_method: def.is_method,
            is_arrow: def.is_arrow,
            home_object,
            lexical_this,
            lexical_new_target,
            lexical_callee,
            prototype: None,
            initial_shape: None,
            is_class_constructor: false,
            is_derived_constructor: false,
            module: cur_mod,
        };
        let func_val = self.alloc_function(js_fn);
        self.push(func_val);
        Ok(())
    }

    pub(crate) fn op_create_async_generator(
        &mut self,
        const_idx: u16,
        chunk: &BytecodeChunk,
    ) -> Result<(), VmError> {
        let def = match &chunk.constants[const_idx as usize] {
            Constant::Function(f) => Arc::clone(f),
            _ => return Err(VmError::TypeError("Expected function def".into())),
        };
        let upvalues = self.capture_upvalues();
        let home_object =
            if def.is_arrow { self.frames.last().and_then(|f| f.home_object) } else { None };
        let (lexical_this, lexical_new_target, lexical_callee) = if def.is_arrow {
            let frame = self.frames.last();
            (
                frame.map(|f| f.this_val),
                frame.and_then(|f| f.new_target),
                frame.and_then(|f| {
                    if f.is_arrow {
                        f.callee_func.as_ref().and_then(|func| func.lexical_callee)
                    } else {
                        f.callee_val
                    }
                }),
            )
        } else {
            (None, None, None)
        };
        let cur_mod = self.frames.last().and_then(|f| f.module).or(self.current_module_obj);
        let js_fn = JsFunction {
            name: def.name.clone(),
            params: def.params.clone(),
            length: def.length,
            self_slot: def.self_slot,
            local_names: def.local_names.clone(),
            lexical_local_names: def.lexical_local_names.clone(),
            chunk: Arc::new(def.chunk.clone()),
            upvalues,
            is_async: true,
            is_generator: true,
            call_count: 0,
            jit_entry: None,
            jit_attempted: false,
            is_strict: def.is_strict,
            has_param_exprs: def.has_param_exprs,
            is_method: def.is_method,
            is_arrow: def.is_arrow,
            home_object,
            lexical_this,
            lexical_new_target,
            lexical_callee,
            prototype: None,
            initial_shape: None,
            is_class_constructor: false,
            is_derived_constructor: false,
            module: cur_mod,
        };
        let func_val = self.alloc_function(js_fn);
        self.push(func_val);
        Ok(())
    }

    pub(crate) fn op_call_direct_eval(
        &mut self,
        argc: u8,
        target_depth: usize,
    ) -> Result<(), VmError> {
        let mut args = Vec::with_capacity(argc as usize);
        for _ in 0..argc {
            args.push(self.pop()?);
        }
        args.reverse();
        let callee = self.pop()?;
        let is_eval =
            if let Some(eval_val) = self.globals.get("eval") { *eval_val == callee } else { false };
        if is_eval {
            self.in_direct_eval = true;
        }
        let res = self.execute_call_internal(callee, JsValue::undefined(), &args);
        self.in_direct_eval = false;
        match res {
            Ok(Some(res)) => self.push(res),
            Ok(None) => {}
            Err(err) => {
                self.handle_exception_at_depth(err, target_depth)?;
            }
        }
        Ok(())
    }

    pub(crate) fn op_set_function_name(
        &mut self,
        name_idx: u16,
        chunk: &BytecodeChunk,
    ) -> Result<(), VmError> {
        let name = match &chunk.constants[name_idx as usize] {
            Constant::String(s) => s.clone(),
            _ => return Err(VmError::TypeError("Function name must be string".into())),
        };
        let val = self.peek().copied()?;
        let is_anon_fn = if let Some(ptr) = val.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::Function(ref func) = obj.kind {
                let is_anon = func.name.as_deref().unwrap_or("").is_empty();
                let has_accessor = obj.accessors.as_ref().map_or(false, |a| a.contains_key("name"));
                is_anon && !has_accessor
            } else {
                false
            }
        } else {
            false
        };

        if is_anon_fn {
            let prev_temp = self.temp_roots.len();
            self.temp_roots.push(val);
            let cur_val = self.temp_roots[prev_temp];
            let cur_name_val = self.get_property(cur_val, "name");
            if cur_name_val.is_empty_string() || cur_name_val.is_undefined() {
                let name_val = self.alloc_string(name.clone());
                self.temp_roots.push(name_val);
                let cur_val = self.temp_roots[prev_temp];
                let cur_name = self.temp_roots[prev_temp + 1];
                if let Some(ptr) = cur_val.as_object_ptr() {
                    let obj = unsafe { &mut *(ptr as *mut JsObject) };
                    if let ObjectKind::Function(ref mut func) = obj.kind {
                        func.name = Some(name.clone());
                    }
                }
                self.define_builtin_property(
                    cur_val,
                    "name",
                    cur_name,
                    crate::shape::PropertyAttributes {
                        writable: false,
                        enumerable: false,
                        configurable: true,
                    },
                );
            }
            self.temp_roots.truncate(prev_temp);
        }
        Ok(())
    }
}
