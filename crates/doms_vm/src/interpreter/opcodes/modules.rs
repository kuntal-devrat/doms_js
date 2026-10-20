//! Module export/import opcodes.

use super::*;

impl Vm {
    pub(crate) fn op_export(
        &mut self,
        name_idx: u16,
        chunk: &BytecodeChunk,
    ) -> Result<(), VmError> {
        let name = match &chunk.constants[name_idx as usize] {
            Constant::String(s) => s.clone(),
            _ => return Err(VmError::TypeError("Export name must be string".into())),
        };
        let val = self.pop().unwrap_or(JsValue::undefined());
        if name == "default" {
            if let Some(ptr) = val.as_object_ptr() {
                let obj = unsafe { &*(ptr as *mut JsObject) };
                if matches!(obj.kind, ObjectKind::Function(_) | ObjectKind::NativeFunction(_)) {
                    let curr_name = self.get_property(val, "name");
                    if curr_name.is_string() && self.to_string_value(curr_name).is_empty() {
                        let dflt_str = self.alloc_string("default".to_string());
                        self.set_property_direct(val, "name", dflt_str);
                    }
                }
            }
        }
        if let Some((mod_obj, exp_name)) = self.imported_bindings.get(&name).cloned() {
            if let Some(cur_mod) = self.current_module_obj {
                self.module_reexports.insert((cur_mod, name.clone()), (mod_obj, exp_name.clone()));
            }
        }

        self.current_exports.insert(name.clone(), val);
        if let Some(mod_obj) = self.current_module_obj {
            self.set_property_direct(mod_obj, &name, val);
            self.set_property_non_configurable(mod_obj, &name);
        }
        Ok(())
    }

    pub(crate) fn op_export_all(&mut self) -> Result<(), VmError> {
        let mod_obj = self.pop()?;
        if let Some(ptr) = mod_obj.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            let mut names = self.shapes.all_properties(obj.shape_id);
            for ((re_mod, re_name), _) in &self.module_reexports {
                if *re_mod == mod_obj && !names.contains(re_name) {
                    names.push(re_name.clone());
                }
            }
            for name in names {
                if name != "default" && !name.starts_with("Symbol(") {
                    let val = self.get_property(mod_obj, &name);
                    if let Some(existing_mod) = self.star_exports.get(&name).copied() {
                        if existing_mod != mod_obj {
                            // Ambiguous export - omit from module namespace object
                            self.current_exports.remove(&name);
                            if let Some(cur) = self.current_module_obj {
                                self.module_reexports.remove(&(cur, name.clone()));
                                if let Some(cur_ptr) = cur.as_object_ptr() {
                                    let cur_obj = unsafe { &mut *(cur_ptr as *mut JsObject) };
                                    cur_obj.shape_id =
                                        self.shapes.remove_property(cur_obj.shape_id, &name);
                                }
                            }
                            continue;
                        }
                    } else {
                        self.star_exports.insert(name.clone(), mod_obj);
                        self.current_exports.insert(name.clone(), val);
                        if let Some(cur) = self.current_module_obj {
                            self.set_property_direct(cur, &name, val);
                            self.set_property_non_configurable(cur, &name);
                            self.module_reexports
                                .insert((cur, name.clone()), (mod_obj, name.clone()));
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub(crate) fn op_import_module(
        &mut self,
        src_idx: u16,
        chunk: &BytecodeChunk,
    ) -> Result<(), VmError> {
        let src = match &chunk.constants[src_idx as usize] {
            Constant::String(s) => s.clone(),
            _ => return Err(VmError::TypeError("Import source must be string".into())),
        };
        let file_path = if let Some(ref base) = self.base_dir {
            base.join(&src)
        } else {
            std::path::PathBuf::from(&src)
        };
        let canonical_key = file_path
            .canonicalize()
            .unwrap_or_else(|_| file_path.clone())
            .to_string_lossy()
            .to_string();
        let mod_obj = if let Some(m) = self.modules.get(&canonical_key).copied() {
            m
        } else if let Some(m) = self.modules.get(&src).copied() {
            m
        } else {
            let m = self.alloc_object();
            self.set_prototype(m, JsValue::null());
            if let Some(tag_sym) = self.symbol_to_string_tag {
                let tag_val = self.alloc_string("Module".to_string());
                self.set_symbol_property(m, tag_sym, tag_val);
            }
            self.module_namespace_objects.insert(m);
            self.modules.insert(canonical_key.clone(), m);
            self.modules.insert(src.clone(), m);
            if let Some(name) = file_path.file_name() {
                self.modules.insert(format!("./{}", name.to_string_lossy()), m);
                self.modules.insert(name.to_string_lossy().to_string(), m);
            }

            if let Ok(content) = std::fs::read_to_string(&file_path) {
                if let Ok(prog) = doms_parser::parse_module(&content) {
                    if let Ok(c) = doms_compiler::compile_strict(&prog) {
                        let old_exports = std::mem::take(&mut self.current_exports);
                        let old_is_mod = self.is_module;
                        let old_base = self.base_dir.clone();
                        let old_mod_obj = self.current_module_obj;
                        let old_lexicals = std::mem::take(&mut self.global_lexicals);
                        let old_globals = self.globals.clone();
                        let old_microtasks = std::mem::take(&mut self.microtasks);
                        let old_consts = std::mem::take(&mut self.const_bindings);
                        let old_imported = std::mem::take(&mut self.imported_bindings);
                        let old_star_exports = std::mem::take(&mut self.star_exports);
                        let old_var_names = std::mem::take(&mut self.var_names);
                        let old_script_vars = std::mem::take(&mut self.script_var_names);

                        self.is_module = true;
                        self.base_dir = file_path.parent().map(|p| p.to_path_buf());
                        self.current_module_obj = Some(m);

                        let _ = self.run(&c);

                        self.is_module = old_is_mod;
                        self.base_dir = old_base;
                        self.current_module_obj = old_mod_obj;
                        self.global_lexicals = old_lexicals;
                        for (k, v) in old_globals {
                            self.globals.insert(k, v);
                        }
                        self.microtasks = old_microtasks;
                        self.const_bindings = old_consts;
                        self.imported_bindings = old_imported;
                        self.star_exports = old_star_exports;
                        self.var_names = old_var_names;
                        self.script_var_names = old_script_vars;
                        let exports = std::mem::replace(&mut self.current_exports, old_exports);
                        for (k, v) in exports {
                            self.set_property_direct(m, &k, v);
                            self.set_property_non_configurable(m, &k);
                        }
                        self.prevent_extensions_object(m);
                    }
                }
            }
            self.prevent_extensions_object(m);
            m
        };
        self.push(mod_obj);
        Ok(())
    }

    pub(crate) fn op_dynamic_import(&mut self) -> Result<(), VmError> {
        let specifier_val = self.pop()?;
        let src = self.to_string_value(specifier_val);
        let file_path = if let Some(ref base) = self.base_dir {
            base.join(&src)
        } else {
            std::path::PathBuf::from(&src)
        };
        let canonical_key = file_path
            .canonicalize()
            .unwrap_or_else(|_| file_path.clone())
            .to_string_lossy()
            .to_string();
        let mod_obj = if let Some(m) = self.modules.get(&canonical_key).copied() {
            m
        } else if let Some(m) = self.modules.get(&src).copied() {
            m
        } else {
            let m = self.alloc_object();
            self.set_prototype(m, JsValue::null());
            if let Some(tag_sym) = self.symbol_to_string_tag {
                let tag_val = self.alloc_string("Module".to_string());
                self.set_symbol_property(m, tag_sym, tag_val);
            }
            self.module_namespace_objects.insert(m);
            self.modules.insert(canonical_key.clone(), m);
            self.modules.insert(src.clone(), m);
            if let Some(name) = file_path.file_name() {
                self.modules.insert(format!("./{}", name.to_string_lossy()), m);
                self.modules.insert(name.to_string_lossy().to_string(), m);
            }

            if let Ok(content) = std::fs::read_to_string(&file_path) {
                if let Ok(prog) =
                    doms_parser::parse_module(&content).or_else(|_| doms_parser::parse(&content))
                {
                    if let Ok(c) = doms_compiler::compile_strict(&prog) {
                        let old_exports = std::mem::take(&mut self.current_exports);
                        let old_is_mod = self.is_module;
                        let old_base = self.base_dir.clone();
                        let old_mod_obj = self.current_module_obj;
                        let old_lexicals = std::mem::take(&mut self.global_lexicals);
                        let old_globals = self.globals.clone();
                        let old_microtasks = std::mem::take(&mut self.microtasks);
                        let old_consts = std::mem::take(&mut self.const_bindings);
                        let old_imported = std::mem::take(&mut self.imported_bindings);
                        let old_star_exports = std::mem::take(&mut self.star_exports);
                        let old_var_names = std::mem::take(&mut self.var_names);
                        let old_script_vars = std::mem::take(&mut self.script_var_names);

                        self.is_module = true;
                        self.base_dir = file_path.parent().map(|p| p.to_path_buf());
                        self.current_module_obj = Some(m);

                        let _ = self.run(&c);

                        self.is_module = old_is_mod;
                        self.base_dir = old_base;
                        self.current_module_obj = old_mod_obj;
                        self.global_lexicals = old_lexicals;
                        for (k, v) in old_globals {
                            self.globals.insert(k, v);
                        }
                        self.microtasks = old_microtasks;
                        self.const_bindings = old_consts;
                        self.imported_bindings = old_imported;
                        self.star_exports = old_star_exports;
                        self.var_names = old_var_names;
                        self.script_var_names = old_script_vars;
                        let exports = std::mem::replace(&mut self.current_exports, old_exports);
                        for (k, v) in exports {
                            self.set_property(m, &k, v);
                            self.set_property_non_configurable(m, &k);
                        }
                        self.prevent_extensions_object(m);
                    }
                }
            }
            self.prevent_extensions_object(m);
            m
        };
        let promise_val = self.alloc_promise(PromiseState::Fulfilled, mod_obj);
        self.push(promise_val);
        Ok(())
    }

    pub(crate) fn op_bind_import(
        &mut self,
        name_idx: u16,
        exp_idx: u16,
        chunk: &BytecodeChunk,
    ) -> Result<(), VmError> {
        let bind_name = match &chunk.constants[name_idx as usize] {
            Constant::String(s) => s.clone(),
            _ => return Err(VmError::TypeError("Import bind name must be string".into())),
        };
        let exp_name = match &chunk.constants[exp_idx as usize] {
            Constant::String(s) => s.clone(),
            _ => return Err(VmError::TypeError("Import export name must be string".into())),
        };
        let mod_obj = *self.peek()?;
        self.imported_bindings.insert(bind_name.clone(), (mod_obj, exp_name.clone()));
        let val = if exp_name == "*" { mod_obj } else { self.get_property(mod_obj, &exp_name) };
        self.global_lexicals.insert(bind_name.clone(), val);
        if self.globals.contains_key(&bind_name) {
            self.globals.insert(bind_name.clone(), val);
        }
        Ok(())
    }

    pub(crate) fn op_reexport(
        &mut self,
        target_name_idx: u16,
        exp_name_idx: u16,
        chunk: &BytecodeChunk,
    ) -> Result<(), VmError> {
        let target_name = match &chunk.constants[target_name_idx as usize] {
            Constant::String(s) => s.clone(),
            _ => return Err(VmError::TypeError("ReExport target name must be string".into())),
        };
        let exp_name = match &chunk.constants[exp_name_idx as usize] {
            Constant::String(s) => s.clone(),
            _ => return Err(VmError::TypeError("ReExport export name must be string".into())),
        };
        let tgt_mod = *self.peek()?;
        if let Some(cur_mod) = self.current_module_obj {
            self.module_reexports
                .insert((cur_mod, exp_name.clone()), (tgt_mod, target_name.clone()));
            let init_val = self.get_property(tgt_mod, &target_name);
            self.set_property_direct(cur_mod, &exp_name, init_val);
            self.set_property_non_configurable(cur_mod, &exp_name);
            self.current_exports.insert(exp_name, init_val);
        }
        Ok(())
    }
}
