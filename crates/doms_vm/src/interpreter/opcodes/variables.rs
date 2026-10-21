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
                let (lexical_this, lexical_new_target) = if f.is_arrow {
                    let frame = self.frames.last();
                    (frame.map(|fr| fr.this_val), frame.and_then(|fr| fr.new_target))
                } else {
                    (None, None)
                };
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
                    prototype: None,
                    initial_shape: None,
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
        let (name_opt, args_and_idx) = {
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
            (name, args_and_idx)
        };
        if let Some(ref name) = name_opt {
            let mut updated_scope = false;
            let scopes = self.frames.last().map(|f| f.scopes.clone()).unwrap_or_default();
            for scope in scopes.iter().rev() {
                if scope.is_object() && self.has_property(*scope, name).unwrap_or(false) {
                    self.set_property(*scope, name, val);
                    updated_scope = true;
                    break;
                }
            }
            if !updated_scope {
                if let Some(env) = self.frames.last().and_then(|f| f.env_obj) {
                    self.set_property(env, name, val);
                }
                let is_fn = self.frames.last().map(|f| f.is_function).unwrap_or(false);
                if !is_fn && self.global_lexicals.contains_key(name) {
                    self.global_lexicals.insert(name.clone(), val);
                }
            }
        }
        if let Some((args_obj, idx)) = args_and_idx {
            self.set_own_property(args_obj, &idx.to_string(), val);
        }
        Ok(())
    }
}
