//! Lexical scope declarations at the global level (`DefLexical`, `DefConst`, `InitConst`).

use super::*;

impl Vm {
    pub(crate) fn op_def_lexical(
        &mut self,
        name_idx: u16,
        chunk: &BytecodeChunk,
        target_depth: usize,
    ) -> Result<Option<JsValue>, VmError> {
        let name = match &chunk.constants[name_idx as usize] {
            Constant::String(s) => s.clone(),
            _ => return Err(VmError::TypeError("Global name must be string".into())),
        };
        if name == "undefined"
            || name == "NaN"
            || name == "Infinity"
            || !self.is_own_property_configurable(self.global_object, &name)
            || self.global_lexicals.contains_key(&name)
            || self.var_names.contains(&name)
        {
            self.handle_exception_at_depth(
                VmError::SyntaxError(format!("Identifier '{}' has already been declared", name)),
                target_depth,
            )?;
            return Ok(None);
        }
        self.global_lexicals.insert(name, JsValue::undefined());
        Ok(None)
    }

    pub(crate) fn op_def_const(
        &mut self,
        name_idx: u16,
        chunk: &BytecodeChunk,
        target_depth: usize,
    ) -> Result<Option<JsValue>, VmError> {
        let name = match &chunk.constants[name_idx as usize] {
            Constant::String(s) => s.clone(),
            _ => return Err(VmError::TypeError("Global name must be string".into())),
        };
        if name == "undefined"
            || name == "NaN"
            || name == "Infinity"
            || !self.is_own_property_configurable(self.global_object, &name)
            || self.global_lexicals.contains_key(&name)
            || self.var_names.contains(&name)
        {
            self.handle_exception_at_depth(
                VmError::SyntaxError(format!("Identifier '{}' has already been declared", name)),
                target_depth,
            )?;
            return Ok(None);
        }
        self.global_lexicals.insert(name.clone(), JsValue::undefined());
        self.const_bindings.insert(name);
        Ok(None)
    }

    pub(crate) fn op_init_const(
        &mut self,
        name_idx: u16,
        chunk: &BytecodeChunk,
    ) -> Result<(), VmError> {
        let name = match &chunk.constants[name_idx as usize] {
            Constant::String(s) => s.clone(),
            _ => return Err(VmError::TypeError("Global name must be string".into())),
        };
        let val = *self.peek()?;
        let mut in_scope = false;
        let scopes = self.frames.last().map(|f| f.scopes.clone()).unwrap_or_default();
        for scope_val in scopes.iter().rev() {
            if scope_val.is_object() && self.has_property(*scope_val, &name).unwrap_or(false) {
                self.set_property(*scope_val, &name, val);
                in_scope = true;
                break;
            }
        }
        if !in_scope {
            self.global_lexicals.insert(name.clone(), val);
        }
        self.const_bindings.insert(name);
        Ok(())
    }
}
