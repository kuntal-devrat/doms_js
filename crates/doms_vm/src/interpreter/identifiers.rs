//! Identifier resolution across frames, scopes and globals.

use super::*;

impl Vm {
    pub fn ensure_frame_arguments(&mut self) -> Option<JsValue> {
        let (callee, raw_args, is_strict, mapped_args) = {
            let frame = self.frames.last()?;
            if let Some(args_val) = frame.arguments {
                return Some(args_val);
            }
            if !frame.is_function || frame.is_arrow {
                return None;
            }
            let callee = frame.callee_val.unwrap_or(JsValue::undefined());
            let mut raw = frame.raw_args.clone().unwrap_or_default();
            if let Some(ref mapped) = frame.mapped_arguments {
                for (i, m) in mapped.iter().enumerate() {
                    if *m {
                        if let Some(local_val) = frame.locals.get(i) {
                            if i < raw.len() {
                                raw[i] = *local_val;
                            }
                        }
                    }
                }
            }
            (callee, raw, frame.is_strict, frame.mapped_arguments.clone())
        };
        let a_obj = self.alloc_arguments(callee, raw_args, is_strict);
        if let Some(frame) = self.frames.last_mut() {
            frame.arguments = Some(a_obj);
            frame.mapped_arguments = mapped_args;
        }
        Some(a_obj)
    }

    pub fn resolve_identifier(&mut self, name: &str) -> Result<Option<JsValue>, VmError> {
        if !name.starts_with('@') {
            let target_opt = self.touched_targets.get(name).copied();
            if let Some(target_scope) = target_opt {
                if let Some(scope_val) = target_scope {
                    let still_exists = self.has_property(scope_val, name)?;
                    if !still_exists {
                        let is_strict = self.is_strict
                            || self.frames.last().map(|f| f.is_strict).unwrap_or(false);
                        if is_strict {
                            return Err(VmError::ReferenceError(format!("{} is not defined", name)));
                        } else {
                            return Ok(Some(JsValue::undefined()));
                        }
                    }
                    return Ok(Some(self.get_property_result(scope_val, name)?));
                }
            } else if let Some(frame) = self.frames.last() {
                if !frame.scopes.is_empty() {
                    let min_scope = if name == "arguments" && frame.is_function && !frame.is_arrow {
                        frame.upvalues_count
                    } else {
                        0
                    };
                    let scopes_len = frame.scopes.len();
                    for idx in (min_scope..scopes_len).rev() {
                        let scope_val = self.frames.last().unwrap().scopes[idx];
                        if scope_val.is_object() {
                            let has = self.has_property(scope_val, name)?;
                            if has {
                                if let Some(sym_unscopables) = self.symbol_unscopables {
                                    let unscopables =
                                        self.get_symbol_property_result(scope_val, sym_unscopables)?;
                                    if unscopables.is_object() {
                                        let blocked =
                                            self.get_property_result(unscopables, name)?.is_truthy();
                                        if blocked {
                                            continue;
                                        }
                                    }
                                }
                                let still_exists = self.has_property(scope_val, name)?;
                                if !still_exists {
                                    let is_strict = self.is_strict
                                        || self.frames.last().map(|f| f.is_strict).unwrap_or(false);
                                    if is_strict {
                                        return Err(VmError::ReferenceError(format!(
                                            "{} is not defined",
                                            name
                                        )));
                                    } else {
                                        return Ok(Some(JsValue::undefined()));
                                    }
                                }
                                return Ok(Some(self.get_property_result(scope_val, name)?));
                            }
                        }
                    }
                }
            }
        }

        if self.in_eval && !self.frames.last().map_or(false, |f| f.is_indirect_eval) {
            for frame in self.frames.iter().rev() {
                for (i, local_name) in frame.local_names.iter().enumerate().rev() {
                    if local_name == name {
                        return Ok(Some(
                            frame.locals.get(i).copied().unwrap_or(JsValue::undefined()),
                        ));
                    }
                }
            }
        } else if let Some(frame) = self.frames.last() {
            if frame.scopes.len() > frame.upvalues_count {
                for (i, local_name) in frame.local_names.iter().enumerate().rev() {
                    if local_name == name {
                        return Ok(Some(
                            frame.locals.get(i).copied().unwrap_or(JsValue::undefined()),
                        ));
                    }
                }
            }
        }

        let is_args = if let Some(frame) = self.frames.last() {
            name == "arguments" && frame.is_function && !frame.is_arrow
        } else {
            false
        };
        if is_args {
            if let Some(args_val) = self.ensure_frame_arguments() {
                return Ok(Some(args_val));
            }
        }

        if let Some(frame) = self.frames.last() {
            if let Some(ref callee_f) = frame.callee_func {
                if let Some(ref fn_name) = callee_f.name {
                    if fn_name == name {
                        if let Some(val) = frame.callee_val {
                            return Ok(Some(val));
                        }
                    }
                }
            }
        }
        if let Some((mod_obj, export_name)) = self.imported_bindings.get(name).cloned() {
            if export_name == "*" {
                return Ok(Some(mod_obj));
            }
            if let Some(ptr) = mod_obj.as_object_ptr() {
                let obj = unsafe { &*(ptr as *mut JsObject) };
                let mut has = self.shapes.has_property(obj.shape_id, &export_name);
                if !has && self.module_eval_status.get(&mod_obj) == Some(&ModuleStatus::EvaluatingAsync) {
                    let _ = self.drain_microtasks();
                    if let Some(err_val) = self.module_eval_errors.get(&mod_obj).copied() {
                        return Err(VmError::Thrown(err_val));
                    }
                    let obj_now = unsafe { &*(ptr as *mut JsObject) };
                    has = self.shapes.has_property(obj_now.shape_id, &export_name);
                }
                if !has
                    && !self.module_reexports.contains_key(&(mod_obj, export_name.clone()))
                {
                    return Ok(Some(JsValue::uninitialized()));
                }
            }
            let val = self.get_property_result(mod_obj, &export_name)?;
            return Ok(Some(val));
        }
        let cur_mod = self.frames.last().and_then(|f| f.module).or(self.current_module_obj);
        if let Some(mod_obj) = cur_mod {
            if let Some(f) = self.frames.iter().rev().find(|f| f.module == Some(mod_obj)) {
                for (i, local_name) in f.local_names.iter().enumerate().rev() {
                    if local_name == name {
                        if let Some(val) = f.locals.get(i).copied() {
                            return Ok(Some(val));
                        }
                    }
                }
            }
            if let Some(env) = self.module_envs.get(&mod_obj) {
                if let Some(val) = env.lexicals.get(name) {
                    return Ok(Some(*val));
                }
                if let Some(val) = env.vars.get(name) {
                    return Ok(Some(*val));
                }
            }
        }
        if let Some(val) = self.global_lexicals.get(name) {
            return Ok(Some(*val));
        }
        if let Some(val) = self.globals.get(name) {
            return Ok(Some(*val));
        }
        if self.global_object.is_object() {
            let val = self.get_property_result(self.global_object, name)?;
            if !val.is_undefined() {
                return Ok(Some(val));
            }
            if let Some(ptr) = self.global_object.as_object_ptr() {
                let obj = unsafe { &*(ptr as *mut JsObject) };
                if self.shapes.has_property(obj.shape_id, name) {
                    return Ok(Some(val));
                }
            }
        }
        Ok(None)
    }

    pub fn touch_identifier_reference(&mut self, name: &str) -> Result<(), VmError> {
        if !name.starts_with('@') {
            if let Some(frame) = self.frames.last() {
                if !frame.scopes.is_empty() {
                    let scopes_len = frame.scopes.len();
                    for idx in (0..scopes_len).rev() {
                        let scope_val = self.frames.last().unwrap().scopes[idx];
                        if scope_val.is_object() {
                            let has = self.has_property(scope_val, name)?;
                            if has {
                                let mut blocked = false;
                                if let Some(sym_unscopables) = self.symbol_unscopables {
                                    if let Ok(unscopables) = self.get_symbol_property_result(scope_val, sym_unscopables) {
                                        if unscopables.is_object() {
                                            if let Ok(b) = self.get_property_result(unscopables, name) {
                                                blocked = b.is_truthy();
                                            }
                                        }
                                    }
                                }
                                if !blocked {
                                    self.touched_targets.insert(name.to_string(), Some(scope_val));
                                    return Ok(());
                                }
                            }
                        }
                    }
                }
            }
            self.touched_targets.insert(name.to_string(), None);
        }
        Ok(())
    }

    pub fn is_identifier_resolvable(&mut self, name: &str) -> Result<bool, VmError> {
        if !name.starts_with('@') {
            let num_frames = self.frames.len();
            let is_indirect = self.frames.last().map(|f| f.is_indirect_eval).unwrap_or(false);
            let is_func = self.frames.last().map(|f| f.is_function).unwrap_or(false);
            let min_frame_idx = if is_indirect || is_func { num_frames.saturating_sub(1) } else { 0 };
            for f_idx in (min_frame_idx..num_frames).rev() {
                let num_scopes = self.frames[f_idx].scopes.len();
                for s_idx in (0..num_scopes).rev() {
                    let scope_val = self.frames[f_idx].scopes[s_idx];
                    if scope_val.is_object() {
                        let has = self.has_property(scope_val, name)?;
                        if has {
                            let mut blocked = false;
                            if let Some(sym_unscopables) = self.symbol_unscopables {
                                if let Ok(unscopables) = self.get_symbol_property_result(scope_val, sym_unscopables) {
                                    if unscopables.is_object() {
                                        if let Ok(b) = self.get_property_result(unscopables, name) {
                                            blocked = b.is_truthy();
                                        }
                                    }
                                }
                            }
                            if !blocked {
                                self.touched_targets.insert(name.to_string(), Some(scope_val));
                                return Ok(true);
                            }
                        }
                    }
                }
                for local_name in self.frames[f_idx].local_names.iter() {
                    if local_name == name {
                        return Ok(true);
                    }
                }
            }
        }
        if let Some(frame) = self.frames.last() {
            if frame.chunk.export_aliases.contains_key(name) || frame.chunk.top_func_names.iter().any(|n| n == name) {
                return Ok(true);
            }
        }
        let cur_mod = self.frames.last().and_then(|f| f.module).or(self.current_module_obj);
        if let Some(mod_obj) = cur_mod {
            if let Some(env) = self.module_envs.get(&mod_obj) {
                if env.lexicals.contains_key(name) || env.vars.contains_key(name) {
                    return Ok(true);
                }
            }
        }
        if self.global_lexicals.contains_key(name) || self.globals.contains_key(name) {
            return Ok(true);
        }
        if self.imported_bindings.contains_key(name) {
            return Ok(true);
        }
        for m in self.modules.values() {
            if let Some(ptr) = m.as_object_ptr() {
                let obj = unsafe { &*(ptr as *mut JsObject) };
                if self.shapes.has_property(obj.shape_id, name) {
                    return Ok(true);
                }
            }
        }
        if let Some(ptr) = self.global_object.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if self.shapes.has_property(obj.shape_id, name) {
                return Ok(true);
            }
        }
        Ok(false)
    }
}
