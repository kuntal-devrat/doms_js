//! The `delete` operator opcodes (`DeleteProp`, `DeleteElem`, `DeleteIdent`).

use super::*;

impl Vm {
    pub(crate) fn op_delete_prop(
        &mut self,
        name_idx: u16,
        chunk: &BytecodeChunk,
        target_depth: usize,
    ) -> Result<Option<JsValue>, VmError> {
        let prop = match &chunk.constants[name_idx as usize] {
            Constant::String(s) => s.clone(),
            _ => return Err(VmError::TypeError("Property name must be string".into())),
        };
        let target = self.pop()?;
        if target.is_null() || target.is_undefined() {
            self.handle_exception_at_depth(
                VmError::TypeError(format!(
                    "Cannot delete property '{}' of null or undefined",
                    prop
                )),
                target_depth,
            )?;
            return Ok(None);
        }
        let is_strict = self.frames.last().map(|f| f.is_strict).unwrap_or(self.is_strict);
        match self.delete_property(target, &prop) {
            Ok(true) => {
                self.push(JsValue::from_boolean(true));
                Ok(None)
            }
            Ok(false) => {
                if is_strict {
                    self.handle_exception_at_depth(
                        VmError::TypeError(format!("Cannot delete property '{}'", prop)),
                        target_depth,
                    )?;
                    Ok(None)
                } else {
                    self.push(JsValue::from_boolean(false));
                    Ok(None)
                }
            }
            Err(err) => {
                self.handle_exception_at_depth(err, target_depth)?;
                Ok(None)
            }
        }
    }

    pub(crate) fn op_delete_elem(
        &mut self,
        target_depth: usize,
    ) -> Result<Option<JsValue>, VmError> {
        let key_val = self.pop()?;
        let target = self.pop()?;
        if target.is_null() || target.is_undefined() {
            self.handle_exception_at_depth(
                VmError::TypeError("Cannot delete property of null or undefined".into()),
                target_depth,
            )?;
            return Ok(None);
        }
        let is_strict = self.frames.last().map(|f| f.is_strict).unwrap_or(self.is_strict);
        let prop_key = match self.to_property_key(key_val) {
            Ok(k) => k,
            Err(err) => {
                self.handle_exception_at_depth(err, target_depth)?;
                return Ok(None);
            }
        };
        match self.delete_property_key(target, &prop_key) {
            Ok(true) => {
                self.push(JsValue::from_boolean(true));
                Ok(None)
            }
            Ok(false) => {
                if is_strict {
                    self.handle_exception_at_depth(
                        VmError::TypeError("Cannot delete property".into()),
                        target_depth,
                    )?;
                    Ok(None)
                } else {
                    self.push(JsValue::from_boolean(false));
                    Ok(None)
                }
            }
            Err(err) => {
                self.handle_exception_at_depth(err, target_depth)?;
                Ok(None)
            }
        }
    }

    pub(crate) fn op_delete_ident(
        &mut self,
        name_idx: u16,
        chunk: &BytecodeChunk,
        target_depth: usize,
    ) -> Result<Option<JsValue>, VmError> {
        let name = match &chunk.constants[name_idx as usize] {
            Constant::String(s) => s.clone(),
            _ => return Err(VmError::TypeError("Identifier name must be string".into())),
        };
        if name == "arguments" {
            self.push(JsValue::from_boolean(false));
            return Ok(None);
        }
        let is_strict = self.frames.last().map(|f| f.is_strict).unwrap_or(self.is_strict);
        let mut scope_found = false;
        let num_frames = self.frames.len();
        for f_idx in (0..num_frames).rev() {
            let scopes = self.frames[f_idx].scopes.clone();
            for scope_obj in scopes.into_iter().rev() {
                if scope_obj.is_object() && self.has_property(scope_obj, &name).unwrap_or(false) {
                    if !self.is_own_property_configurable(scope_obj, &name) {
                        if is_strict {
                            self.handle_exception_at_depth(
                                VmError::TypeError(format!("Cannot delete property '{}'", name)),
                                target_depth,
                            )?;
                            return Ok(None);
                        } else {
                            self.push(JsValue::from_boolean(false));
                        }
                    } else {
                        let _ = self.delete_property(scope_obj, &name);
                        self.push(JsValue::from_boolean(true));
                    }
                    scope_found = true;
                    break;
                }
            }
            if scope_found {
                break;
            }
        }
        if scope_found {
            return Ok(None);
        }

        let is_local = self.frames.iter().any(|f| f.local_names.iter().any(|n| n == &name));
        if is_local || self.global_lexicals.contains_key(&name) || self.var_names.contains(&name) {
            if is_strict {
                self.handle_exception_at_depth(
                    VmError::SyntaxError(format!(
                        "Delete of unqualified identifier in strict mode: {}",
                        name
                    )),
                    target_depth,
                )?;
                return Ok(None);
            } else {
                self.push(JsValue::from_boolean(false));
            }
        } else if let Some(ptr) = self.global_object.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if self.shapes.has_property(obj.shape_id, &name) {
                if !self.is_own_property_configurable(self.global_object, &name) {
                    if is_strict {
                        self.handle_exception_at_depth(
                            VmError::TypeError(format!("Cannot delete property '{}'", name)),
                            target_depth,
                        )?;
                        return Ok(None);
                    } else {
                        self.push(JsValue::from_boolean(false));
                    }
                } else {
                    let new_shape_id = self.shapes.remove_property(obj.shape_id, &name);
                    obj.shape_id = new_shape_id;
                    self.globals.remove(&name);
                    self.push(JsValue::from_boolean(true));
                }
            } else {
                self.push(JsValue::from_boolean(true));
            }
        } else {
            self.push(JsValue::from_boolean(true));
        }
        Ok(None)
    }
}
