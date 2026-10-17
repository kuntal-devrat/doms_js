//! Identifier resolution across frames, scopes and globals.

use super::*;

impl Vm {
    pub fn resolve_identifier(&mut self, name: &str) -> Result<Option<JsValue>, VmError> {
        if let Some(frame) = self.frames.last() {
            if name == "arguments" && !frame.is_arrow {
                if let Some(args_val) = frame.arguments {
                    let has_local_arg = frame.local_names.iter().any(|n| n == "arguments");
                    if !has_local_arg {
                        return Ok(Some(args_val));
                    }
                }
            }

            let scopes = frame.scopes.clone();
            for scope_val in scopes.into_iter().rev() {
                if scope_val.is_object() {
                    let has = self.has_property(scope_val, name)?;
                    if has {
                        if let Some(sym_unscopables) = self.symbol_unscopables {
                            let unscopables = self.get_symbol_property_result(scope_val, sym_unscopables)?;
                            if unscopables.is_object() {
                                let blocked = self.get_property_result(unscopables, name)?.is_truthy();
                                if blocked {
                                    continue;
                                }
                            }
                        }
                        return Ok(Some(self.get_property_result(scope_val, name)?));
                    }
                }
            }
            if let Some(frame) = self.frames.last() {
                for (i, local_name) in frame.local_names.iter().enumerate().rev() {
                    if local_name == name {
                        return Ok(Some(frame.locals.get(i).copied().unwrap_or(JsValue::undefined())));
                    }
                }
                if name == "arguments" && !frame.is_arrow {
                    if let Some(args_val) = frame.arguments {
                        return Ok(Some(args_val));
                    }
                }
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
