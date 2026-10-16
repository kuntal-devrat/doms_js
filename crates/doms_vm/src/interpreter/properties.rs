//! Prototype handling and direct property writes.

use super::*;

impl Vm {
    pub fn get_prototype_of(&mut self, target: JsValue) -> Option<JsValue> {
        if let Some(ptr) = target.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            obj.prototype
        } else if target.is_string() {
            self.string_proto
        } else if target.is_number() {
            self.number_proto
        } else if target.is_boolean() {
            self.boolean_proto
        } else if target.is_symbol() {
            self.symbol_proto
        } else {
            None
        }
    }

    pub fn set_prototype(&mut self, target: JsValue, proto: JsValue) {
        if let Some(ptr) = target.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            obj.prototype = if proto.is_null() { None } else { Some(proto) };
        }
    }

    pub fn set_property_direct(&mut self, target: JsValue, key: &str, value: JsValue) {
        self.deleted_props.remove(&(target, key.to_string()));
        if let Some(ptr) = target.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let Some(offset) = self.shapes.lookup_offset(obj.shape_id, key) {
                obj.set_slot(offset, value);
            } else {
                let (new_shape_id, offset) = self.shapes.transition(obj.shape_id, key);
                obj.shape_id = new_shape_id;
                obj.set_slot(offset, value);
            }
        }
    }

    pub fn set_property(&mut self, target: JsValue, key: &str, value: JsValue) {
        self.deleted_props.remove(&(target, key.to_string()));
        if self.non_writable_props.contains(&(target, key.to_string())) {
            return;
        }
        if self.non_extensible_objects.contains(&target) {
            if let Some(ptr) = target.as_object_ptr() {
                let obj = unsafe { &*(ptr as *mut JsObject) };
                if !self.shapes.has_property(obj.shape_id, key) {
                    return;
                }
            }
        }
        if self.non_writable_props.contains(&(target, key.to_string())) {
            return;
        }
        if let Some(ptr) = target.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };

            // 1. Check own accessor
            if let Some(accs) = &obj.accessors {
                if let Some(acc) = accs.get(key) {
                    if let Some(setter) = acc.setter {
                        let _ = self.call(setter, target, &[value]);
                    }
                    return;
                }
            }

            if let ObjectKind::Array(elements) = &mut obj.kind {
                if key == "length" {
                    if let Some(new_len) = value.to_number().map(|n| n as usize) {
                        elements.truncate(new_len);
                        return;
                    }
                }
                if let Ok(idx) = key.parse::<usize>() {
                    if idx < elements.len() {
                        elements[idx] = value;
                    } else if idx < 100_000 {
                        elements.resize(idx + 1, JsValue::undefined());
                        elements[idx] = value;
                    }
                    return;
                }
            }

            if let ObjectKind::TypedArray(ta) = &mut obj.kind {
                if let Ok(idx) = key.parse::<usize>() {
                    Self::write_typed_array_element(ta, idx, value);
                    return;
                }
            }

            // 2. Check prototype chain accessor
            let has_own_data = self.shapes.has_property(obj.shape_id, key)
                || matches!(&obj.kind, ObjectKind::Array(elements) if key == "length" || key.parse::<usize>().map(|idx| idx < elements.len()).unwrap_or(false))
                || matches!(&obj.kind, ObjectKind::TypedArray(ta) if matches!(key, "length" | "byteLength" | "byteOffset" | "buffer") || key.parse::<usize>().map(|idx| idx < ta.length).unwrap_or(false))
                || matches!(&obj.kind, ObjectKind::String(_) | ObjectKind::StringObject(_) if key == "length" || key.parse::<usize>().is_ok());
            if !has_own_data {
                let mut curr_proto = obj.prototype;
                while let Some(proto_val) = curr_proto {
                    if let Some(proto_ptr) = proto_val.as_object_ptr() {
                        let proto_obj = unsafe { &*(proto_ptr as *mut JsObject) };
                        if let Some(accs) = &proto_obj.accessors {
                            if let Some(acc) = accs.get(key) {
                                if let Some(setter) = acc.setter {
                                    let _ = self.call(setter, target, &[value]);
                                }
                                return;
                            }
                        }
                        curr_proto = proto_obj.prototype;
                    } else {
                        break;
                    }
                }
            }

            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let Some(offset) = self.shapes.lookup_offset(obj.shape_id, key) {
                obj.set_slot(offset, value);
            } else {
                let (new_shape_id, offset) = self.shapes.transition(obj.shape_id, key);
                obj.shape_id = new_shape_id;
                obj.set_slot(offset, value);
            }
        }
        if target == self.global_object {
            self.globals.insert(key.to_string(), value);
        }
        self.sync_mapped_arguments_write(target, key, value);
    }

    pub fn set_own_property(&mut self, target: JsValue, key: &str, value: JsValue) {
        self.deleted_props.remove(&(target, key.to_string()));
        if let Some(ptr) = target.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };

            if let ObjectKind::Array(elements) = &mut obj.kind {
                if key == "length" {
                    if let Some(new_len) = value.to_number().map(|n| n as usize) {
                        elements.truncate(new_len);
                        return;
                    }
                }
                if let Ok(idx) = key.parse::<usize>() {
                    if idx < elements.len() {
                        elements[idx] = value;
                    } else if idx < 100_000 {
                        elements.resize(idx + 1, JsValue::undefined());
                        elements[idx] = value;
                    }
                    return;
                }
            }

            if let ObjectKind::TypedArray(ta) = &mut obj.kind {
                if let Ok(idx) = key.parse::<usize>() {
                    Self::write_typed_array_element(ta, idx, value);
                    return;
                }
            }

            if let Some(accs) = &mut obj.accessors {
                accs.remove(key);
            }

            if let Some(offset) = self.shapes.lookup_offset(obj.shape_id, key) {
                obj.set_slot(offset, value);
            } else {
                let (new_shape_id, offset) = self.shapes.transition(obj.shape_id, key);
                obj.shape_id = new_shape_id;
                obj.set_slot(offset, value);
            }
        }
        if target == self.global_object {
            self.globals.insert(key.to_string(), value);
        }
    }

    pub fn sync_mapped_arguments_write(&mut self, target: JsValue, key: &str, val: JsValue) {
        if let Ok(idx) = key.parse::<usize>() {
            let mut env_to_sync = None;
            for frame in self.frames.iter_mut().rev() {
                if frame.arguments == Some(target) {
                    if let Some(mapped) = &frame.mapped_arguments {
                        if mapped.get(idx).copied().unwrap_or(false) {
                            if idx < frame.locals.len() {
                                frame.locals[idx] = val;
                            }
                            if let Some(env) = frame.env_obj {
                                if let Some(name) = frame.local_names.get(idx).cloned() {
                                    env_to_sync = Some((env, name));
                                }
                            }
                        }
                    }
                    break;
                }
            }
            if let Some((env, name)) = env_to_sync {
                self.set_property(env, &name, val);
            }
        }
    }

    pub fn sync_mapped_arguments_delete(&mut self, target: JsValue, key: &str) {
        if let Ok(idx) = key.parse::<usize>() {
            for frame in self.frames.iter_mut().rev() {
                if frame.arguments == Some(target) {
                    if let Some(mapped) = &mut frame.mapped_arguments {
                        if let Some(m) = mapped.get_mut(idx) {
                            *m = false;
                        }
                    }
                    break;
                }
            }
        }
    }

    pub fn has_own_property(&self, target: JsValue, key: &str) -> bool {
        self.has_own_property_key(target, &PropertyKeyVal::String(key.to_string()))
    }

    pub fn create_data_property_or_throw(&mut self, target: JsValue, key: &str, value: JsValue) -> Result<(), VmError> {
        let key_string = key.to_string();
        if let Some(ptr) = target.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::Proxy { target: tgt, handler: hdl } = &obj.kind {
                let tgt = *tgt;
                let hdl = *hdl;
                let trap = self.get_property(hdl, "defineProperty");
                if self.is_callable(trap) {
                    let key_val = self.alloc_string(key_string.clone());
                    let desc = self.alloc_object();
                    self.set_property(desc, "value", value);
                    self.set_property(desc, "writable", JsValue::from_boolean(true));
                    self.set_property(desc, "enumerable", JsValue::from_boolean(true));
                    self.set_property(desc, "configurable", JsValue::from_boolean(true));
                    let res = self.call(trap, hdl, &[tgt, key_val, desc])?;
                    if !res.is_truthy() {
                        return Err(VmError::TypeError(format!("Proxy trap defineProperty returned false for '{}'", key)));
                    }
                    return Ok(());
                } else {
                    return self.create_data_property_or_throw(tgt, key, value);
                }
            }

            // Check non-configurable: CreateDataProperty requires configurable: true.
            // If property is already non-configurable, redefining it as configurable fails.
            if self.non_configurable_props.contains(&(target, key_string.clone())) {
                return Err(VmError::TypeError(format!("Cannot redefine non-configurable property '{}'", key)));
            }

            // Check non-extensible
            if self.non_extensible_objects.contains(&target) && !self.has_own_property(target, key) {
                return Err(VmError::TypeError(format!("Cannot add property '{}' to non-extensible object", key)));
            }
        }

        self.non_writable_props.remove(&(target, key_string.clone()));
        self.non_enumerable_props.remove(&(target, key_string.clone()));
        self.set_own_property(target, key, value);
        Ok(())
    }
}
