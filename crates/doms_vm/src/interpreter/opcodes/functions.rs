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
}
