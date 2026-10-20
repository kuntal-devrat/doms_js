//! Function/closure creation and direct `eval` calls.

use super::*;

impl Vm {
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
        let js_fn = JsFunction {
            name: def.name.clone(),
            params: def.params.clone(),
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
            prototype: None,
            initial_shape: None,
        };
        let func_val = self.alloc_function(js_fn);
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
        let js_fn = JsFunction {
            name: def.name.clone(),
            params: def.params.clone(),
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
            prototype: None,
            initial_shape: None,
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
        let js_fn = JsFunction {
            name: def.name.clone(),
            params: def.params.clone(),
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
            prototype: None,
            initial_shape: None,
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
        let js_fn = JsFunction {
            name: def.name.clone(),
            params: def.params.clone(),
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
            prototype: None,
            initial_shape: None,
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
