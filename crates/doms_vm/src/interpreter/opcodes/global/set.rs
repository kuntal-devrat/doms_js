use super::*;

impl Vm {
    pub(crate) fn op_set_global(
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
        let mut assigned = false;
        let num_frames = self.frames.len();
        let is_indirect = self.frames.last().map(|f| f.is_indirect_eval).unwrap_or(false);
        let is_func = self.frames.last().map(|f| f.is_function).unwrap_or(false);
        let min_frame_idx = if is_indirect || is_func { num_frames.saturating_sub(1) } else { 0 };
        for f_idx in (min_frame_idx..num_frames).rev() {
            let num_scopes = self.frames[f_idx].scopes.len();
            for s_idx in (0..num_scopes).rev() {
                let scope_val = self.frames[f_idx].scopes[s_idx];
                if scope_val.is_object() {
                    let has = match self.has_property(scope_val, &name) {
                        Ok(h) => h,
                        Err(e) => {
                            self.handle_exception_at_depth(e, target_depth)?;
                            return Ok(None);
                        }
                    };
                    if has {
                        let mut blocked = false;
                        if let Some(sym_unscopables) = self.symbol_unscopables {
                            match self.get_symbol_property_result(scope_val, sym_unscopables) {
                                Ok(unscopables) if unscopables.is_object() => {
                                    match self.get_property_result(unscopables, &name) {
                                        Ok(b) => {
                                            blocked = b.is_truthy();
                                        }
                                        Err(e) => {
                                            self.handle_exception_at_depth(e, target_depth)?;
                                            return Ok(None);
                                        }
                                    }
                                }
                                Err(e) => {
                                    self.handle_exception_at_depth(e, target_depth)?;
                                    return Ok(None);
                                }
                                _ => {}
                            }
                        }
                        if blocked {
                            continue;
                        }

                        let cur_val = match self.get_property_result(scope_val, &name) {
                            Ok(v) => v,
                            Err(e) => {
                                self.handle_exception_at_depth(e, target_depth)?;
                                return Ok(None);
                            }
                        };
                        if cur_val.is_uninitialized() {
                            self.handle_exception_at_depth(
                                VmError::ReferenceError(format!(
                                    "Cannot access '{}' before initialization",
                                    name
                                )),
                                target_depth,
                            )?;
                            return Ok(None);
                        }
                        if let Err(e) = self.set_property_result(scope_val, &name, val) {
                            self.handle_exception_at_depth(e, target_depth)?;
                            return Ok(None);
                        }
                        for f in &mut self.frames {
                            if f.env_obj == Some(scope_val) {
                                for (i, l_name) in f.local_names.iter().enumerate() {
                                    if l_name == &name && i < f.locals.len() {
                                        f.locals[i] = val;
                                    }
                                }
                            }
                        }
                        assigned = true;
                        break;
                    }
                }
            }
            let is_current_frame_with = f_idx == num_frames.saturating_sub(1)
                && self.frames[f_idx].scopes.len() > self.frames[f_idx].upvalues_count;
            if self.in_eval || is_current_frame_with {
                let env_to_sync = {
                    let frame = &mut self.frames[f_idx];
                    let mut found_env = None;
                    for (i, local_name) in frame.local_names.iter().enumerate().rev() {
                        if *local_name == name {
                            if i < frame.locals.len() {
                                if frame.locals[i].is_uninitialized() {
                                    self.handle_exception_at_depth(
                                        VmError::ReferenceError(format!(
                                            "Cannot access '{}' before initialization",
                                            name
                                        )),
                                        target_depth,
                                    )?;
                                    return Ok(None);
                                }
                                frame.locals[i] = val;
                            }
                            if let Some(env) = frame.env_obj {
                                found_env = Some(env);
                            }
                            assigned = true;
                            break;
                        }
                    }
                    found_env
                };
                if let Some(env) = env_to_sync {
                    self.set_property(env, &name, val);
                }
            }
            if assigned {
                break;
            }
        }
        if !assigned {
            if let Some(frame) = self.frames.last() {
                if let Some(ref callee_f) = frame.callee_func {
                    if let Some(ref fn_name) = callee_f.name {
                        if fn_name == &name {
                            if frame.is_strict {
                                self.handle_exception_at_depth(
                                    VmError::TypeError(format!(
                                        "Assignment to constant variable '{}'",
                                        name
                                    )),
                                    target_depth,
                                )?;
                                return Ok(None);
                            } else {
                                return Ok(None);
                            }
                        }
                    }
                }
            }
        }
        if assigned {
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
        }
        if !assigned {
            let is_chunk_import = self.imported_bindings.contains_key(&name)
                || chunk.imported_bindings.contains(&name);
            if is_chunk_import {
                self.handle_exception_at_depth(
                    VmError::TypeError(format!("Assignment to constant variable {}", name)),
                    target_depth,
                )?;
                return Ok(None);
            }
            let mut found_in_module = false;
            let mod_vals: Vec<JsValue> = self.modules.values().copied().collect();
            for m in mod_vals {
                if let Some(ptr) = m.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    if self.shapes.has_property(obj.shape_id, &name) {
                        self.set_property_direct(m, &name, val);
                        found_in_module = true;
                    }
                }
            }
            if found_in_module {
                if self.globals.contains_key(&name) {
                    self.globals.insert(name.clone(), val);
                }
                if self.global_lexicals.contains_key(&name) {
                    self.global_lexicals.insert(name.clone(), val);
                }
                assigned = true;
            }
            if self.const_bindings.contains(&name) {
                self.handle_exception_at_depth(
                    VmError::TypeError(format!("Assignment to constant variable {}", name)),
                    target_depth,
                )?;
                return Ok(None);
            }
            if !self.is_own_property_writable(self.global_object, &name) {
                let is_strict = self.frames.last().map(|f| f.is_strict).unwrap_or(self.is_strict);
                if is_strict {
                    self.handle_exception_at_depth(
                        VmError::TypeError(format!(
                            "Cannot assign to read only property '{}'",
                            name
                        )),
                        target_depth,
                    )?;
                    return Ok(None);
                }
                return Ok(None);
            }
            if let Some(cur_val) = self.global_lexicals.get(&name).copied() {
                if cur_val.is_uninitialized() {
                    self.handle_exception_at_depth(
                        VmError::ReferenceError(format!(
                            "Cannot access '{}' before initialization",
                            name
                        )),
                        target_depth,
                    )?;
                    return Ok(None);
                }
                self.global_lexicals.insert(name.clone(), val);
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
                assigned = true;
            }
        }
        if !assigned {
            let is_current_strict =
                self.frames.last().map(|f| f.is_strict).unwrap_or(self.is_strict);
            if self.globals.contains_key(&name) {
                self.globals.insert(name.clone(), val);
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
                if !self.is_module && self.global_object.is_object() {
                    self.set_property(self.global_object, &name, val);
                }
            } else if is_current_strict {
                self.handle_exception_at_depth(
                    VmError::ReferenceError(format!("Assignment to undeclared variable {}", name)),
                    target_depth,
                )?;
            } else if self.global_object.is_object() && {
                let ptr = self.global_object.as_object_ptr().unwrap();
                let obj = unsafe { &*(ptr as *mut JsObject) };
                self.shapes.has_property(obj.shape_id, &name)
            } {
                self.set_property(self.global_object, &name, val);
                self.globals.insert(name.clone(), val);
            } else {
                self.globals.insert(name.clone(), val);
                if self.global_object.is_object() {
                    self.set_property(self.global_object, &name, val);
                }
            }
        }
        for f in &mut self.frames {
            if !f.is_function {
                for (i, l_name) in f.local_names.iter().enumerate() {
                    if l_name == &name && i < f.locals.len() {
                        f.locals[i] = val;
                    }
                }
            }
        }
        Ok(None)
    }
}
