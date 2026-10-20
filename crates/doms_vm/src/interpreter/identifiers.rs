//! Identifier resolution across frames, scopes and globals.

use super::*;

impl Vm {
    pub fn ensure_frame_arguments(&mut self) -> Option<JsValue> {
        let (callee, raw_args, is_strict, mapped_args) = {
            let frame = self.frames.last()?;
            if let Some(args_val) = frame.arguments {
                return Some(args_val);
            }
            if frame.is_arrow {
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
        if let Some(frame) = self.frames.last() {
            if !frame.scopes.is_empty() {
                let scopes_len = frame.scopes.len();
                for idx in (0..scopes_len).rev() {
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
                            return Ok(Some(self.get_property_result(scope_val, name)?));
                        }
                    }
                }
            }
        }

        if self.in_eval {
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
            name == "arguments" && !frame.is_arrow
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
                if !self.shapes.has_property(obj.shape_id, &export_name)
                    && !self.module_reexports.contains_key(&(mod_obj, export_name.clone()))
                {
                    return Ok(Some(JsValue::uninitialized()));
                }
            }
            let val = self.get_property_result(mod_obj, &export_name)?;
            return Ok(Some(val));
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
}
