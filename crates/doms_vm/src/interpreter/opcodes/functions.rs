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
                    let home_object = if def.is_arrow {
                        self.frames.last().and_then(|f| f.home_object)
                    } else {
                        None
                    };
                    let js_fn = JsFunction {
                        name: def.name.clone(),
                        params: def.params.clone(),
                        local_names: def.local_names.clone(),
                        lexical_local_names: def.lexical_local_names.clone(),
                        chunk: Arc::new(def.chunk.clone()),
                        upvalues,
                        is_async: false,
                        is_generator: false,
                        call_count: 0,
                        jit_entry: None,
                        is_strict: def.is_strict,
                        has_param_exprs: def.has_param_exprs,
                        is_method: def.is_method,
                        is_arrow: def.is_arrow,
                        home_object,
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
                    let home_object = if def.is_arrow {
                        self.frames.last().and_then(|f| f.home_object)
                    } else {
                        None
                    };
                    let js_fn = JsFunction {
                        name: def.name.clone(),
                        params: def.params.clone(),
                        local_names: def.local_names.clone(),
                        lexical_local_names: def.lexical_local_names.clone(),
                        chunk: Arc::new(def.chunk.clone()),
                        upvalues,
                        is_async: true,
                        is_generator: false,
                        call_count: 0,
                        jit_entry: None,
                        is_strict: def.is_strict,
                        has_param_exprs: def.has_param_exprs,
                        is_method: def.is_method,
                        is_arrow: def.is_arrow,
                        home_object,
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
                    let home_object = if def.is_arrow {
                        self.frames.last().and_then(|f| f.home_object)
                    } else {
                        None
                    };
                    let js_fn = JsFunction {
                        name: def.name.clone(),
                        params: def.params.clone(),
                        local_names: def.local_names.clone(),
                        lexical_local_names: def.lexical_local_names.clone(),
                        chunk: Arc::new(def.chunk.clone()),
                        upvalues,
                        is_async: false,
                        is_generator: true,
                        call_count: 0,
                        jit_entry: None,
                        is_strict: def.is_strict,
                        has_param_exprs: def.has_param_exprs,
                        is_method: def.is_method,
                        is_arrow: def.is_arrow,
                        home_object,
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
                    let home_object = if def.is_arrow {
                        self.frames.last().and_then(|f| f.home_object)
                    } else {
                        None
                    };
                    let js_fn = JsFunction {
                        name: def.name.clone(),
                        params: def.params.clone(),
                        local_names: def.local_names.clone(),
                        lexical_local_names: def.lexical_local_names.clone(),
                        chunk: Arc::new(def.chunk.clone()),
                        upvalues,
                        is_async: true,
                        is_generator: true,
                        call_count: 0,
                        jit_entry: None,
                        is_strict: def.is_strict,
                        has_param_exprs: def.has_param_exprs,
                        is_method: def.is_method,
                        is_arrow: def.is_arrow,
                        home_object,
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
                    let is_eval = if let Some(eval_val) = self.globals.get("eval") {
                        *eval_val == callee
                    } else {
                        false
                    };
                    if is_eval {
                        self.in_direct_eval = true;
                    }
                    let res = self.execute_call_internal(callee, JsValue::undefined(), args);
                    self.in_direct_eval = false;
                    match res {
                        Ok(Some(res)) => self.push(res),
                        Ok(None) => {},
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
        if let Some(ptr) = val.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::Function(ref mut func) = obj.kind {
                let is_anon = func.name.as_deref().unwrap_or("").is_empty();
                if is_anon {
                    let has_accessor = if let Some(ref accs) = obj.accessors {
                        accs.contains_key("name")
                    } else {
                        false
                    };
                    if !has_accessor {
                        let cur_name_val = self.get_property(val, "name");
                        if cur_name_val.is_empty_string() || cur_name_val.is_undefined() {
                            func.name = Some(name.clone());
                            let name_val = self.alloc_string(name.clone());
                            self.set_own_property(val, "name", name_val);
                            self.non_enumerable_props.insert((val, "name".to_string()));
                            self.non_writable_props.insert((val, "name".to_string()));
                            self.non_configurable_props.remove(&(val, "name".to_string()));
                        }
                    }
                }
            }
        }
        Ok(())
    }
}
