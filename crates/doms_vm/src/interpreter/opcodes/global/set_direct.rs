use super::*;

impl Vm {
    pub(crate) fn op_set_global_direct(
        &mut self,
        name_idx: u16,
        chunk: &BytecodeChunk,
        target_depth: usize,
    ) -> Result<Option<JsValue>, VmError> {
        let name = match &chunk.constants[name_idx as usize] {
            Constant::String(s) => s.clone(),
            _ => return Err(VmError::TypeError("Global name must be string".into())),
        };
        let val = *self.peek()?;
        let is_chunk_import =
            self.imported_bindings.contains_key(&name) || chunk.imported_bindings.contains(&name);
        if is_chunk_import {
            self.handle_exception_at_depth(
                VmError::TypeError(format!("Assignment to constant variable {}", name)),
                target_depth,
            )?;
            return Ok(None);
        }
        let aliases_opt =
            self.frames.iter().rev().find_map(|f| f.chunk.export_aliases.get(&name).cloned());
        if let Some(aliases) = aliases_opt {
            for alias in &aliases {
                if let Some(mod_obj) = self.current_module_obj {
                    self.set_property_direct(mod_obj, alias, val);
                }
                let mod_vals: Vec<JsValue> = self.modules.values().copied().collect();
                for m in mod_vals {
                    if let Some(ptr) = m.as_object_ptr() {
                        let obj = unsafe { &*(ptr as *mut JsObject) };
                        if self.shapes.has_property(obj.shape_id, alias) {
                            self.set_property_direct(m, alias, val);
                        }
                    }
                }
            }
        }
        if let Some(mod_obj) = self.current_module_obj {
            if let Some(ptr) = mod_obj.as_object_ptr() {
                let obj = unsafe { &*(ptr as *mut JsObject) };
                if self.shapes.has_property(obj.shape_id, &name) {
                    self.set_property_direct(mod_obj, &name, val);
                }
            }
        }
        let mod_values: Vec<JsValue> = self.modules.values().copied().collect();
        for m in mod_values {
            if let Some(ptr) = m.as_object_ptr() {
                let obj = unsafe { &*(ptr as *mut JsObject) };
                if self.shapes.has_property(obj.shape_id, &name) {
                    self.set_property_direct(m, &name, val);
                }
            }
        }
        if let Some(target_opt) = self.touched_targets.remove(&name) {
            if let Some(target_scope) = target_opt {
                let still_exists = match self.has_property(target_scope, &name) {
                    Ok(h) => h,
                    Err(e) => {
                        self.handle_exception_at_depth(e, target_depth)?;
                        return Ok(None);
                    }
                };
                let is_strict = self.is_strict
                    || self.frames.last().map(|f| f.is_strict).unwrap_or(false);
                if !still_exists && is_strict {
                    self.handle_exception_at_depth(
                        VmError::ReferenceError(format!("{} is not defined", name)),
                        target_depth,
                    )?;
                    return Ok(None);
                }
                if let Err(e) = self.set_property_result(target_scope, &name, val) {
                    self.handle_exception_at_depth(e, target_depth)?;
                    return Ok(None);
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
                return Ok(None);
            } else {
                return self.op_set_global(name_idx, chunk, target_depth);
            }
        }
        let mut in_scope = false;
        if !name.starts_with('@') {
            let scopes = self.frames.last().map(|f| f.scopes.clone()).unwrap_or_default();
            for scope_val in scopes.iter().rev() {
                if scope_val.is_object() && self.has_property(*scope_val, &name).unwrap_or(false) {
                    self.set_property(*scope_val, &name, val);
                    in_scope = true;
                    break;
                }
            }
        }
        if !in_scope {
            if !self.is_strict && !self.global_lexicals.contains_key(&name) {
                if self.globals.contains_key(&name) {
                    self.globals.insert(name.clone(), val);
                }
                if !self.is_module {
                    self.set_property(self.global_object, &name, val);
                }
            } else if let Some(lex) = self.global_lexicals.get_mut(&name) {
                *lex = val;
            } else {
                if self.globals.contains_key(&name) {
                    self.globals.insert(name.clone(), val);
                }
                if !self.is_module {
                    self.set_property(self.global_object, &name, val);
                }
            }
        }
        Ok(None)
    }
}
