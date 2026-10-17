//! Constant loading and local slot writes.

use super::*;

impl Vm {

    pub(crate) fn op_load_const(
        &mut self,
        idx: u16,
        chunk: &BytecodeChunk,
    ) -> Result<(), VmError> {
                    let c = &chunk.constants[idx as usize];
                    let val = match c {
                        Constant::Number(n) => JsValue::from_double(*n),
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
                            let js_fn = JsFunction {
                                name: f.name.clone(),
                                params: f.params.clone(),
                                local_names: f.local_names.clone(),
                                lexical_local_names: f.lexical_local_names.clone(),
                                chunk: Arc::new(f.chunk.clone()),
                                upvalues,
                                is_async: false,
                                is_generator: f.is_generator,
                                call_count: 0,
                                jit_entry: None,
                                is_strict: f.is_strict,
                                has_param_exprs: f.has_param_exprs,
                                is_method: f.is_method,
                                is_arrow: f.is_arrow,
                                home_object,
                            };
                            self.alloc_function(js_fn)
                        }
                    };
                    self.push(val);
        Ok(())
    }

    pub(crate) fn op_set_local(
        &mut self,
        slot: u16,
    ) -> Result<(), VmError> {
                    let val = *self.peek()?;
                    let slot_idx = slot as usize;
                    let (env_and_name, args_and_idx) = {
                        let frame = self.frames.last_mut().unwrap();
                        if slot_idx < frame.locals.len() {
                            frame.locals[slot_idx] = val;
                        } else {
                            frame.locals.resize(slot_idx + 1, JsValue::undefined());
                            frame.locals[slot_idx] = val;
                        }
                        let env_and_name = frame.env_obj.and_then(|env| {
                            frame.local_names.get(slot_idx).cloned().map(|name| (env, name))
                        });
                        let args_and_idx = if let Some(mapped) = &frame.mapped_arguments {
                            if mapped.get(slot_idx).copied().unwrap_or(false) {
                                frame.arguments.map(|a| (a, slot_idx))
                            } else {
                                None
                            }
                        } else {
                            None
                        };
                        (env_and_name, args_and_idx)
                    };
                    if let Some((env, name)) = env_and_name {
                        self.set_property(env, &name, val);
                    }
                    if let Some((args_obj, idx)) = args_and_idx {
                        self.set_own_property(args_obj, &idx.to_string(), val);
                    }
        Ok(())
    }
}
