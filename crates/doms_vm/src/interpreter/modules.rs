//! Dynamic `eval` and global declaration instantiation.

use super::*;

impl Vm {
    // Dynamic eval and module instantiation

    pub fn eval(&mut self, source: &str) -> Result<JsValue, VmError> {
        let program = doms_parser::parse(source)
            .map_err(|e| VmError::SyntaxError(e.to_string()))?;
        let chunk = doms_compiler::compile(&program)
            .map_err(|e| VmError::TypeError(e.to_string()))?;
        self.run(&chunk)
    }

    pub fn instantiate_global_declarations(&mut self, chunk: &BytecodeChunk) -> Result<(), VmError> {
        if !self.is_module {
            // Step 1: Validate top_lex_names
            for name in &chunk.top_lex_names {
                if self.global_lexicals.contains_key(name) {
                    return Err(VmError::SyntaxError(format!("Identifier '{}' has already been declared", name)));
                }
                if self.script_var_names.contains(name) {
                    return Err(VmError::SyntaxError(format!("Identifier '{}' has already been declared", name)));
                }
                if !self.is_own_property_configurable(self.global_object, name)
                    || name == "undefined" || name == "NaN" || name == "Infinity"
                {
                    return Err(VmError::SyntaxError(format!("Identifier '{}' has already been declared", name)));
                }
            }

            // Step 2: Validate top_var_names
            for name in &chunk.top_var_names {
                if self.global_lexicals.contains_key(name) {
                    return Err(VmError::SyntaxError(format!("Identifier '{}' has already been declared", name)));
                }
                if !self.is_extensible_object(self.global_object) {
                    let has = if let Some(ptr) = self.global_object.as_object_ptr() {
                        let obj = unsafe { &*(ptr as *mut JsObject) };
                        self.shapes.has_property(obj.shape_id, name)
                    } else {
                        false
                    };
                    if !has {
                        return Err(VmError::TypeError(format!("Cannot define property '{}' on non-extensible global object", name)));
                    }
                }
            }

            // Step 3: Validate top_func_names
            for name in &chunk.top_func_names {
                if self.global_lexicals.contains_key(name) {
                    return Err(VmError::SyntaxError(format!("Identifier '{}' has already been declared", name)));
                }
                if !self.is_own_property_configurable(self.global_object, name) {
                    let is_writable = self.is_own_property_writable(self.global_object, name);
                    let is_enum = self.is_own_property_enumerable(self.global_object, name);
                    let has_accessor = if let Some(ptr) = self.global_object.as_object_ptr() {
                        let obj = unsafe { &*(ptr as *mut JsObject) };
                        obj.accessors.as_ref().map(|a| a.contains_key(name)).unwrap_or(false)
                    } else {
                        false
                    };
                    if has_accessor || !is_writable || !is_enum {
                        return Err(VmError::TypeError(format!("Cannot declare global function '{}': existing property is non-configurable", name)));
                    }
                }
                if !self.is_extensible_object(self.global_object) {
                    let has = if let Some(ptr) = self.global_object.as_object_ptr() {
                        let obj = unsafe { &*(ptr as *mut JsObject) };
                        self.shapes.has_property(obj.shape_id, name)
                    } else {
                        false
                    };
                    if !has {
                        return Err(VmError::TypeError(format!("Cannot define function '{}' on non-extensible global object", name)));
                    }
                }
            }
        }

        // All validations passed! Instantiate bindings:
        for name in &chunk.top_lex_names {
            self.global_lexicals.insert(name.clone(), JsValue::uninitialized());
        }

        let in_eval = self.in_eval;
        for name in &chunk.top_var_names {
            self.var_names.insert(name.clone());
            if !in_eval {
                self.script_var_names.insert(name.clone());
            }
            let has = if let Some(ptr) = self.global_object.as_object_ptr() {
                let obj = unsafe { &*(ptr as *mut JsObject) };
                self.shapes.has_property(obj.shape_id, name)
            } else {
                false
            };
            if !has {
                self.globals.insert(name.clone(), JsValue::undefined());
                if !self.is_module && self.global_object.is_object() {
                    if !in_eval {
                        self.define_builtin_property(self.global_object, name, JsValue::undefined(), crate::shape::PropertyAttributes {
                            writable: true,
                            enumerable: true,
                            configurable: false,
                        });
                    } else {
                        self.set_property(self.global_object, name, JsValue::undefined());
                    }
                }
            } else if !self.globals.contains_key(name) {
                let existing_val = self.get_property(self.global_object, name);
                self.globals.insert(name.clone(), existing_val);
            }
        }

        for name in &chunk.top_func_names {
            self.var_names.insert(name.clone());
            if !in_eval {
                self.script_var_names.insert(name.clone());
                if !self.is_module && self.global_object.is_object() {
                    self.set_property_non_configurable(self.global_object, name);
                }
            }
            if !self.globals.contains_key(name) {
                self.globals.insert(name.clone(), JsValue::undefined());
                if !self.is_module && self.global_object.is_object() {
                    self.set_property(self.global_object, name, JsValue::undefined());
                }
            }
        }

        if self.is_module {
            if let Some(mod_obj) = self.current_module_obj {
                for name in &chunk.export_names {
                    let local_name = chunk.export_aliases.iter()
                        .find_map(|(local, aliases)| if aliases.contains(name) { Some(local) } else { None })
                        .unwrap_or(name);
                    let val = if chunk.top_func_names.contains(local_name) {
                        self.globals.get(local_name).copied().unwrap_or_else(JsValue::undefined)
                    } else if chunk.top_var_names.contains(local_name) {
                        JsValue::undefined()
                    } else {
                        JsValue::uninitialized()
                    };
                    self.set_property_direct(mod_obj, name, val);
                    self.set_property_non_configurable(mod_obj, name);
                }
            }
        }

        Ok(())
    }
}
