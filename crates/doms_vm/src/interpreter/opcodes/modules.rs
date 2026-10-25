//! Module export/import opcodes.

use super::*;

impl Vm {
    fn is_safe_import_path(src: &str) -> bool {
        let path = std::path::Path::new(src);
        !path.is_absolute()
            && !path.components().any(|c| matches!(c, std::path::Component::ParentDir))
    }

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
                if cur_mod != mod_obj {
                    self.module_reexports.insert((cur_mod, name.clone()), (mod_obj, exp_name.clone()));
                }
            }
        } else if self.module_namespace_objects.contains(&val) {
            if let Some(cur_mod) = self.current_module_obj {
                self.module_reexports.insert((cur_mod, name.clone()), (val, "*".to_string()));
            }
        }

        self.current_exports.insert(name.clone(), val);
        if let Some(mod_obj) = self.current_module_obj {
            self.set_property_direct(mod_obj, &name, val);
            self.set_property_non_configurable(mod_obj, &name);
            if let Some(env) = self.module_envs.get_mut(&mod_obj) {
                if env.lexicals.contains_key(&name) {
                    env.lexicals.insert(name.clone(), val);
                } else if env.vars.contains_key(&name) {
                    env.vars.insert(name.clone(), val);
                }
            }
        }
        if self.imported_bindings.contains_key(&name) {
            self.global_lexicals.insert(name.clone(), val);
        }
        Ok(())
    }

    pub fn update_module_export(&mut self, mod_obj: JsValue, name: &str, val: JsValue) {
        if let Some(ptr) = mod_obj.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if self.shapes.has_property(obj.shape_id, name) {
                self.set_property_direct(mod_obj, name, val);
            }
        }
        let aliases_opt = self
            .module_export_aliases
            .get(&mod_obj)
            .and_then(|m| m.get(name).cloned());
        if let Some(aliases) = aliases_opt {
            for alias in aliases {
                self.set_property_direct(mod_obj, &alias, val);
            }
        }
    }

    pub fn resolve_root_export(
        &self,
        mut mod_obj: JsValue,
        name: &str,
    ) -> Option<(JsValue, String)> {
        let mut curr_name = name.to_string();
        let mut visited = HashSet::new();
        if let Some(cur) = self.current_module_obj {
            visited.insert((cur, name.to_string()));
        }
        while let Some((tgt_mod, tgt_key)) =
            self.module_reexports.get(&(mod_obj, curr_name.clone())).cloned()
        {
            if !visited.insert((mod_obj, curr_name.clone())) {
                return None;
            }
            if let Some(cur) = self.current_module_obj {
                if tgt_mod == cur && tgt_key == name {
                    return None;
                }
            }
            if visited.contains(&(tgt_mod, tgt_key.clone())) {
                return None;
            }
            mod_obj = tgt_mod;
            curr_name = tgt_key;
            if curr_name == "*" {
                break;
            }
        }
        if let Some(cur) = self.current_module_obj {
            if mod_obj == cur && curr_name == name {
                return None;
            }
        }
        Some((mod_obj, curr_name))
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
                    let root_new = match self.resolve_root_export(mod_obj, &name) {
                        Some(r) => r,
                        None => continue,
                    };
                    let val = self.get_property(mod_obj, &name);
                    if let Some(existing_mod) = self.star_exports.get(&name).copied() {
                        if existing_mod != mod_obj {
                            let root_existing = self.resolve_root_export(existing_mod, &name);
                            if root_existing != Some(root_new) {
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
                        }
                    } else {
                        self.star_exports.insert(name.clone(), mod_obj);
                    }
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
        if !Self::is_safe_import_path(&src) {
            return Err(VmError::TypeError("Invalid module path".into()));
        }
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

        let (m, eval_promise) = self.load_and_evaluate_module(&file_path, &canonical_key, &src)?;

        if let Some(cur_mod) = self.current_module_obj {
            let is_back_edge = self.frames.iter().any(|f| f.module == Some(m));
            if !is_back_edge {
                let target_mod = if let Some(&root) = self.module_cycle_roots.get(&m) {
                    if !self.frames.iter().any(|f| f.module == Some(root)) {
                        root
                    } else {
                        m
                    }
                } else {
                    m
                };
                let wait_p = self
                    .module_eval_promises
                    .get(&target_mod)
                    .copied()
                    .unwrap_or(eval_promise);
                self.module_async_deps
                    .entry(cur_mod)
                    .or_default()
                    .push((target_mod, wait_p));
            }
        }

        self.push(m);
        Ok(())
    }

    pub(crate) fn op_await_imports(&mut self) -> Result<(), VmError> {
        let cur_mod = self.frames.last().and_then(|f| f.module).or(self.current_module_obj);
        if let Some(mod_obj) = cur_mod {
            let deps = self.module_async_deps.remove(&mod_obj).unwrap_or_default();
            for (m, eval_promise) in deps {
                if self.module_eval_status.get(&m) == Some(&ModuleStatus::EvaluatingAsync) {
                    let _ = self.await_value(eval_promise)?;
                }
                if let Some(err_val) = self.module_eval_errors.get(&m).copied() {
                    return Err(VmError::Thrown(err_val));
                }
                if let Some(ptr) = eval_promise.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    if let ObjectKind::Promise(p) = &obj.kind {
                        if p.state == PromiseState::Rejected {
                            return Err(VmError::Thrown(p.result));
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub(crate) fn load_and_evaluate_module(
        &mut self,
        file_path: &std::path::Path,
        canonical_key: &str,
        src: &str,
    ) -> Result<(JsValue, JsValue), VmError> {
        let existing = self
            .modules
            .get(canonical_key)
            .copied()
            .or_else(|| self.modules.get(src).copied());

        if let Some(m) = existing {
            if self.module_eval_status.get(&m) == Some(&ModuleStatus::Evaluating) {
                if let Some(cur_mod) = self.current_module_obj {
                    let root = self.module_cycle_roots.get(&m).copied().unwrap_or(m);
                    self.module_cycle_roots.insert(cur_mod, root);
                }
            }
            let eval_p = self
                .module_eval_promises
                .get(&m)
                .copied()
                .unwrap_or_else(|| self.alloc_promise(PromiseState::Fulfilled, m));
            return Ok((m, eval_p));
        }

        let m = self.alloc_object();
        self.set_prototype(m, JsValue::null());
        if let Some(tag_sym) = self.symbol_to_string_tag {
            let tag_val = self.alloc_string("Module".to_string());
            self.define_symbol_property(
                m,
                tag_sym,
                tag_val,
                crate::shape::PropertyAttributes::NON_WRITABLE_NON_ENUMERABLE,
            );
        }
        self.module_namespace_objects.insert(m);
        self.prevent_extensions_object(m);
        self.modules.insert(canonical_key.to_string(), m);
        self.modules.insert(src.to_string(), m);
        if let Some(name) = file_path.file_name() {
            self.modules.insert(format!("./{}", name.to_string_lossy()), m);
            self.modules.insert(name.to_string_lossy().to_string(), m);
        }

        let eval_promise = self.alloc_promise(PromiseState::Pending, JsValue::undefined());
        self.module_eval_promises.insert(m, eval_promise);
        self.module_eval_status.insert(m, ModuleStatus::Evaluating);

        let content = std::fs::read_to_string(file_path)
            .map_err(|e| VmError::TypeError(format!("Cannot read module {}: {}", src, e)))?;
        let prog = doms_parser::parse_module(&content)
            .or_else(|_| doms_parser::parse(&content))
            .map_err(|e| VmError::SyntaxError(e.to_string()))?;
        let c = doms_compiler::compile_strict(&prog)
            .map_err(|e| VmError::SyntaxError(e.to_string()))?;

        let old_exports = std::mem::take(&mut self.current_exports);
        let old_is_mod = self.is_module;
        let old_base = self.base_dir.clone();
        let old_mod_obj = self.current_module_obj;
        let old_lexicals = std::mem::take(&mut self.global_lexicals);
        let old_globals = self.globals.clone();
        let old_consts = std::mem::take(&mut self.const_bindings);
        let old_imported = std::mem::take(&mut self.imported_bindings);
        let old_star_exports = std::mem::take(&mut self.star_exports);
        let old_var_names = std::mem::take(&mut self.var_names);
        let old_script_vars = std::mem::take(&mut self.script_var_names);
        for v in &old_var_names {
            if v != "assert" && v != "$262" && v != "$DONE" && v != "Test262Error" {
                self.globals.remove(v);
            }
        }

        self.is_module = true;
        self.base_dir = file_path.parent().map(|p| p.to_path_buf());
        self.current_module_obj = Some(m);

        let run_res = self.run(&c);

        self.is_module = old_is_mod;
        self.base_dir = old_base;
        self.current_module_obj = old_mod_obj;
        self.global_lexicals = old_lexicals;
        self.globals = old_globals;
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

        match run_res {
            Ok(_) => {
                let current_st = self
                    .module_eval_status
                    .get(&m)
                    .copied()
                    .unwrap_or(ModuleStatus::Evaluating);
                if current_st != ModuleStatus::EvaluatingAsync {
                    self.prevent_extensions_object(m);
                    self.module_eval_status.insert(m, ModuleStatus::Evaluated);
                    self.resolve_promise(eval_promise, m);
                }
            }
            Err(e) => {
                self.prevent_extensions_object(m);
                let err_val = self.vm_error_to_js_value(&e);
                self.module_eval_errors.insert(m, err_val);
                self.module_eval_status.insert(m, ModuleStatus::Evaluated);
                self.reject_promise(eval_promise, err_val);
                return Err(e);
            }
        }

        Ok((m, eval_promise))
    }

    pub(crate) fn op_dynamic_import(&mut self) -> Result<(), VmError> {
        let specifier_val = self.pop()?;
        let src = self.to_string_value(specifier_val);
        if !Self::is_safe_import_path(&src) {
            return Err(VmError::TypeError("Invalid module path".into()));
        }
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

        let (m, eval_promise) = match self.load_and_evaluate_module(&file_path, &canonical_key, &src) {
            Ok(pair) => pair,
            Err(e) => {
                let err_val = self.vm_error_to_js_value(&e);
                let rej_p = self.alloc_promise(PromiseState::Rejected, err_val);
                self.push(rej_p);
                return Ok(());
            }
        };

        let on_fulfilled = self.alloc_closure_native_function(
            "dynamic_import_fulfilled",
            1,
            Arc::new(move |_vm, _, _| Ok(m)),
        );
        let dyn_promise = self.promise_then(eval_promise, on_fulfilled, JsValue::undefined())?;
        self.push(dyn_promise);
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
