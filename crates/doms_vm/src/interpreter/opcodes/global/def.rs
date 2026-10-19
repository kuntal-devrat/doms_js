use super::*;

impl Vm {
    pub(crate) fn op_def_global(
        &mut self,
        name_idx: u16,
        chunk: &BytecodeChunk,
        target_depth: usize,
    ) -> Result<Option<JsValue>, VmError> {
                    let name = match &chunk.constants[name_idx as usize] {
                        Constant::String(s) => s.clone(),
                        _ => return Err(VmError::TypeError("Global name must be string".into())),
                    };
                    if self.global_lexicals.contains_key(&name) {
                        self.handle_exception_at_depth(
                            VmError::SyntaxError(format!("Identifier '{}' has already been declared", name)),
                            target_depth,
                        )?;
                        return Ok(None);
                    }
                    self.var_names.insert(name.clone());
                    if !self.globals.contains_key(&name) {
                        if !self.is_extensible_object(self.global_object) {
                            self.handle_exception_at_depth(
                                VmError::TypeError(format!("Cannot define property '{}' on non-extensible global object", name)),
                                target_depth,
                            )?;
                            return Ok(None);
                        }
                        self.globals.insert(name.clone(), JsValue::undefined());
                        if self.global_object.is_object() {
                            self.define_builtin_property(
                                self.global_object,
                                &name,
                                JsValue::undefined(),
                                crate::shape::PropertyAttributes {
                                    writable: true,
                                    enumerable: true,
                                    configurable: false,
                                },
                            );
                        }
                    }
        Ok(None)
    }
}
