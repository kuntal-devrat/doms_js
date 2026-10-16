//! The `delete` operator opcodes (`DeleteProp`, `DeleteElem`, `DeleteIdent`).

use super::*;

impl Vm {

    pub(crate) fn op_delete_prop(
        &mut self,
        name_idx: u16,
        chunk: &BytecodeChunk,
        target_depth: usize,
    ) -> Result<Option<JsValue>, VmError> {
                    let prop = match &chunk.constants[name_idx as usize] {
                        Constant::String(s) => s.clone(),
                        _ => return Err(VmError::TypeError("Property name must be string".into())),
                    };
                    let target = self.pop()?;
                    let is_strict = self.frames.last().map(|f| f.is_strict).unwrap_or(self.is_strict);
                    if let Some(ptr) = target.as_object_ptr() {
                        let obj = unsafe { &mut *(ptr as *mut JsObject) };
                        if self.non_configurable_props.contains(&(target, prop.clone())) {
                            if is_strict {
                                self.handle_exception_at_depth(
                                    VmError::TypeError(format!("Cannot delete property '{}'", prop)),
                                    target_depth,
                                )?;
                                return Ok(Some(JsValue::undefined()));
                            } else {
                                self.push(JsValue::from_boolean(false));
                            }
                        } else {
                            self.deleted_props.insert((target, prop.clone()));
                            if let Some(accs) = &mut obj.accessors {
                                accs.remove(&prop);
                            }
                            let new_shape_id = self.shapes.remove_property(obj.shape_id, &prop);
                            obj.shape_id = new_shape_id;
                            if target == self.global_object {
                                self.globals.remove(&prop);
                            }
                            self.push(JsValue::from_boolean(true));
                        }
                    } else {
                        self.push(JsValue::from_boolean(true));
                    }
        Ok(None)
    }

    pub(crate) fn op_delete_elem(
        &mut self,
        target_depth: usize,
    ) -> Result<Option<JsValue>, VmError> {
                    let key_val = self.pop()?;
                    let target = self.pop()?;
                    let is_strict = self.frames.last().map(|f| f.is_strict).unwrap_or(self.is_strict);
                    if let Some(ptr) = target.as_object_ptr() {
                        let obj = unsafe { &mut *(ptr as *mut JsObject) };
                        if key_val.is_symbol() || key_val.as_object_ptr().is_some_and(|p| matches!(unsafe { &(*(p as *mut JsObject)).kind }, ObjectKind::Symbol(_))) {
                            if self.module_namespace_objects.contains(&target)
                                || self.non_configurable_props.contains(&(target, "[Symbol.toStringTag]".to_string()))
                            {
                                if is_strict {
                                    self.handle_exception_at_depth(
                                        VmError::TypeError("Cannot delete property".into()),
                                        target_depth,
                                    )?;
                                    return Ok(Some(JsValue::undefined()));
                                } else {
                                    self.push(JsValue::from_boolean(false));
                                    return Ok(None);
                                }
                            }
                            if let Some(syms) = &mut obj.symbol_properties {
                                syms.remove(&key_val);
                            }
                            if let Some(accs) = &mut obj.symbol_accessors {
                                accs.remove(&key_val);
                            }
                            if let Some(order) = &mut obj.symbol_order {
                                order.retain(|s| s != &key_val);
                            }
                            self.push(JsValue::from_boolean(true));
                        } else {
                            let prop = self.to_string_value(key_val);
                            if self.non_configurable_props.contains(&(target, prop.clone())) {
                                if is_strict {
                                    self.handle_exception_at_depth(
                                        VmError::TypeError(format!("Cannot delete property '{}'", prop)),
                                        target_depth,
                                    )?;
                                    return Ok(Some(JsValue::undefined()));
                                } else {
                                    self.push(JsValue::from_boolean(false));
                                }
                            } else {
                                self.deleted_props.insert((target, prop.clone()));
                                if let Some(accs) = &mut obj.accessors {
                                    accs.remove(&prop);
                                }
                                let new_shape_id = self.shapes.remove_property(obj.shape_id, &prop);
                                obj.shape_id = new_shape_id;
                                if target == self.global_object {
                                    self.globals.remove(&prop);
                                }
                                self.sync_mapped_arguments_delete(target, &prop);
                                self.push(JsValue::from_boolean(true));
                            }
                        }
                    } else {
                        self.push(JsValue::from_boolean(true));
                    }
        Ok(None)
    }

    pub(crate) fn op_delete_ident(
        &mut self,
        name_idx: u16,
        chunk: &BytecodeChunk,
        target_depth: usize,
    ) -> Result<Option<JsValue>, VmError> {
                    let name = match &chunk.constants[name_idx as usize] {
                        Constant::String(s) => s.clone(),
                        _ => return Err(VmError::TypeError("Identifier name must be string".into())),
                    };
                    if name == "arguments" {
                        self.push(JsValue::from_boolean(false));
                        return Ok(None);
                    }
                    let is_strict = self.frames.last().map(|f| f.is_strict).unwrap_or(self.is_strict);
                    let mut scope_found = false;
                    let num_frames = self.frames.len();
                    for f_idx in (0..num_frames).rev() {
                        let scopes = self.frames[f_idx].scopes.clone();
                        for scope_obj in scopes.into_iter().rev() {
                            if scope_obj.is_object() && self.has_property(scope_obj, &name).unwrap_or(false) {
                                if self.non_configurable_props.contains(&(scope_obj, name.clone())) {
                                    if is_strict {
                                        self.handle_exception_at_depth(
                                            VmError::TypeError(format!("Cannot delete property '{}'", name)),
                                            target_depth,
                                        )?;
                                        return Ok(Some(JsValue::undefined()));
                                    } else {
                                        self.push(JsValue::from_boolean(false));
                                    }
                                } else {
                                    let _ = self.delete_property(scope_obj, &name);
                                    self.deleted_props.insert((scope_obj, name.clone()));
                                    self.push(JsValue::from_boolean(true));
                                }
                                scope_found = true;
                                break;
                            }
                        }
                        if scope_found { break; }
                    }
                    if scope_found {
                        return Ok(None);
                    }

                    if self.global_lexicals.contains_key(&name) || self.var_names.contains(&name) {
                        if is_strict {
                            self.handle_exception_at_depth(
                                VmError::SyntaxError(format!("Delete of unqualified identifier in strict mode: {}", name)),
                                target_depth,
                            )?;
                            return Ok(Some(JsValue::undefined()));
                        } else {
                            self.push(JsValue::from_boolean(false));
                        }
                    } else if let Some(ptr) = self.global_object.as_object_ptr() {
                        let obj = unsafe { &mut *(ptr as *mut JsObject) };
                        if self.shapes.has_property(obj.shape_id, &name) {
                            if self.non_configurable_props.contains(&(self.global_object, name.clone())) {
                                if is_strict {
                                    self.handle_exception_at_depth(
                                        VmError::TypeError(format!("Cannot delete property '{}'", name)),
                                        target_depth,
                                    )?;
                                    return Ok(Some(JsValue::undefined()));
                                } else {
                                    self.push(JsValue::from_boolean(false));
                                }
                            } else {
                                let new_shape_id = self.shapes.remove_property(obj.shape_id, &name);
                                obj.shape_id = new_shape_id;
                                self.globals.remove(&name);
                                self.push(JsValue::from_boolean(true));
                            }
                        } else {
                            self.push(JsValue::from_boolean(true));
                        }
                    } else {
                        self.push(JsValue::from_boolean(true));
                    }
        Ok(None)
    }
}
