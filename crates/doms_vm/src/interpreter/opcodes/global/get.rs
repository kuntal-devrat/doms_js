//! Global variable read opcodes (`GetGlobal`, `GetGlobalSafe`).

use super::*;

impl Vm {
    pub(crate) fn op_get_global_safe(
        &mut self,
        name_idx: u16,
        chunk: &BytecodeChunk,
        target_depth: usize,
    ) -> Result<Option<JsValue>, VmError> {
        let name = match &chunk.constants[name_idx as usize] {
            Constant::String(s) => s.as_str(),
            _ => return Err(VmError::TypeError("Global name must be string".into())),
        };
        if let Some(val) = self.resolve_identifier(name) {
            if val.is_uninitialized() {
                self.handle_exception_at_depth(
                    VmError::ReferenceError(format!("Cannot access '{}' before initialization", name)),
                    target_depth,
                )?;
                return Ok(None);
            }
            self.push(val);
        } else {
            self.push(JsValue::undefined());
        }
        Ok(None)
    }

    pub(crate) fn op_get_global(
        &mut self,
        name_idx: u16,
        chunk: &BytecodeChunk,
        target_depth: usize,
    ) -> Result<Option<JsValue>, VmError> {
        let name = match &chunk.constants[name_idx as usize] {
            Constant::String(s) => s.as_str(),
            _ => return Err(VmError::TypeError("Global name must be string".into())),
        };
        if let Some(val) = self.resolve_identifier(name) {
            if val.is_uninitialized() {
                self.handle_exception_at_depth(
                    VmError::ReferenceError(format!("Cannot access '{}' before initialization", name)),
                    target_depth,
                )?;
                return Ok(None);
            }
            self.push(val);
        } else {
            self.handle_exception_at_depth(VmError::ReferenceError(format!("{} is not defined", name)), target_depth)?;
        }
        Ok(None)
    }
}
