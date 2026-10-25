//! Constant loading and local slot writes.

use super::*;

impl Vm {
    pub(crate) fn op_load_const(&mut self, idx: u16, chunk: &BytecodeChunk) -> Result<(), VmError> {
        let c = &chunk.constants[idx as usize];
        let val = match c {
            Constant::Number(n) => {
                if n.fract() == 0.0
                    && *n >= i32::MIN as f64
                    && *n <= i32::MAX as f64
                    && (*n != 0.0 || n.is_sign_positive())
                {
                    JsValue::from_int32(*n as i32)
                } else {
                    JsValue::from_double(*n)
                }
            }
            Constant::BigInt(bi) => self.alloc_bigint(bi.clone()),
            Constant::String(s) => self.alloc_string(s.clone()),
            Constant::Boolean(b) => JsValue::from_boolean(*b),
            Constant::Function(f) => {
                let upvalues = self.capture_upvalues();
                let home_object = if f.is_arrow {
                    self.frames.last().and_then(|fr| fr.home_object)
                } else {
                    None
                };
                let (lexical_this, lexical_new_target, lexical_callee) = if f.is_arrow {
                    let frame = self.frames.last();
                    (
                        frame.map(|fr| fr.this_val),
                        frame.and_then(|fr| fr.new_target),
                        frame.and_then(|fr| {
                            if fr.is_arrow {
                                fr.callee_func.as_ref().and_then(|func| func.lexical_callee)
                            } else {
                                fr.callee_val
                            }
                        }),
                    )
                } else {
                    (None, None, None)
                };
                let cur_mod = self.frames.last().and_then(|f| f.module).or(self.current_module_obj);
                let js_fn = JsFunction {
                    name: f.name.clone(),
                    params: f.params.clone(),
                    length: f.length,
                    self_slot: f.self_slot,
                    local_names: f.local_names.clone(),
                    lexical_local_names: f.lexical_local_names.clone(),
                    chunk: Arc::new(f.chunk.clone()),
                    upvalues,
                    is_async: false,
                    is_generator: f.is_generator,
                    call_count: 0,
                    jit_entry: None,
                    jit_attempted: false,
                    is_strict: f.is_strict,
                    has_param_exprs: f.has_param_exprs,
                    is_method: f.is_method,
                    is_arrow: f.is_arrow,
                    home_object,
                    lexical_this,
                    lexical_new_target,
                    lexical_callee,
                    prototype: None,
                    initial_shape: None,
                    is_class_constructor: f.is_class_constructor,
                    is_derived_constructor: f.is_derived_constructor,
                    module: cur_mod,
                };
                self.alloc_function(js_fn)
            }
            Constant::TemplateObject { .. } => {
                return Err(VmError::RuntimeError(
                    "TemplateObject constant cannot be loaded via LoadConst".into(),
                ));
            }
        };
        self.push(val);
        Ok(())
    }

    pub(crate) fn op_set_local(&mut self, slot: u16) -> Result<(), VmError> {
        let val = *self.peek()?;
        let slot_idx = slot as usize;
        let (name_opt, args_and_idx, sync_to_scopes) = {
            let frame = self.frames.last_mut().unwrap();
            if slot_idx < frame.locals.len() {
                frame.locals[slot_idx] = val;
            } else {
                frame.locals.resize(slot_idx + 1, JsValue::undefined());
                frame.locals[slot_idx] = val;
            }
            let name = frame.local_names.get(slot_idx).cloned();
            let args_and_idx = if let Some(mapped) = &frame.mapped_arguments {
                if mapped.get(slot_idx).copied().unwrap_or(false) {
                    frame.arguments.map(|a| (a, slot_idx))
                } else {
                    None
                }
            } else {
                None
            };
            let sync = !frame.has_param_exprs || frame.in_param_init;
            (name, args_and_idx, sync)
        };
        if let Some(ref name) = name_opt {
            if !name.starts_with('@') {
                let mut updated_scope = false;
                if sync_to_scopes {
                    let (scopes, uv_count) = self
                        .frames
                        .last()
                        .map(|f| (f.scopes.clone(), f.upvalues_count))
                        .unwrap_or_default();
                    let min_idx = uv_count.min(scopes.len());
                    for scope in scopes[min_idx..].iter().rev() {
                        if scope.is_object() && self.has_property(*scope, name).unwrap_or(false) {
                            self.set_property(*scope, name, val);
                            updated_scope = true;
                            break;
                        }
                    }
                }
                if !updated_scope {
                    let is_top = self
                        .frames
                        .last()
                        .and_then(|f| f.chunk.is_top_local.get(slot_idx).copied())
                        .unwrap_or(true);
                    if is_top {
                        if let Some(env) = self
                            .frames
                            .last()
                            .and_then(|f| if f.in_param_init { f.param_env_obj } else { f.env_obj })
                        {
                            self.set_property(env, name, val);
                        }
                    }
                    let is_fn = self.frames.last().map(|f| f.is_function).unwrap_or(false);
                    if !is_fn && self.global_lexicals.contains_key(name) {
                        self.global_lexicals.insert(name.clone(), val);
                    }
                    let cur_mod = self.frames.last().and_then(|f| f.module).or(self.current_module_obj);
                    if let Some(mod_obj) = cur_mod {
                        if let Some(env) = self.module_envs.get_mut(&mod_obj) {
                            if env.lexicals.contains_key(name) {
                                env.lexicals.insert(name.clone(), val);
                            } else if env.vars.contains_key(name) {
                                env.vars.insert(name.clone(), val);
                            }
                        }
                        self.update_module_export(mod_obj, name, val);
                    }
                }
            }
        }
        if let Some((args_obj, idx)) = args_and_idx {
            self.set_own_property(args_obj, &idx.to_string(), val);
        }
        Ok(())
    }
}
