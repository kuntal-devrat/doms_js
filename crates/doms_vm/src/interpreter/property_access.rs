//! Spec-level get/set/delete property operations.

use super::*;

impl Vm {
    pub fn get_property_result(&mut self, target: JsValue, key: &str) -> Result<JsValue, VmError> {
        if target.is_null() || target.is_undefined() {
            return Err(VmError::TypeError(format!(
                "Cannot read properties of {} (reading '{}')",
                if target.is_null() { "null" } else { "undefined" },
                key
            )));
        }
        if let Some(ptr) = target.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };

            // 1. Check own accessor
            if let Some(accs) = &obj.accessors {
                if let Some(acc) = accs.get(key) {
                    if let Some(getter) = acc.getter {
                        return self.call(getter, target, &[]);
                    } else {
                        return Ok(JsValue::undefined());
                    }
                }
            }

            // 2. Check prototype chain accessor only if object does NOT have own data property
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
                                if let Some(getter) = acc.getter {
                                    return self.call(getter, target, &[]);
                                } else {
                                    return Ok(JsValue::undefined());
                                }
                            }
                        }
                        curr_proto = proto_obj.prototype;
                    } else {
                        break;
                    }
                }
            }
        }
        if self.module_namespace_objects.contains(&target) {
            let val = self.get_property(target, key);
            if val == JsValue::uninitialized() {
                return Err(VmError::ReferenceError(format!("Cannot access '{}' before initialization", key)));
            }
            return Ok(val);
        }
        Ok(self.get_property(target, key))
    }

    pub fn set_property_result(&mut self, target: JsValue, key: &str, value: JsValue) -> Result<(), VmError> {
        if target.is_null() || target.is_undefined() {
            return Err(VmError::TypeError(format!(
                "Cannot set properties of {} (setting '{}')",
                if target.is_null() { "null" } else { "undefined" },
                key
            )));
        }
        if self.module_namespace_objects.contains(&target) {
            let is_strict = self.frames.last().map(|f| f.is_strict).unwrap_or(self.is_strict);
            if is_strict {
                return Err(VmError::TypeError(format!("Cannot assign to read only property '{}'", key)));
            }
            return Ok(());
        }
        let is_func = target.as_object_ptr().is_some_and(|p| matches!(unsafe { &(*(p as *mut JsObject)).kind }, ObjectKind::Function(_) | ObjectKind::NativeFunction(_)));
        if is_func && (key == "length" || key == "name") {
            let is_strict = self.frames.last().map(|f| f.is_strict).unwrap_or(self.is_strict);
            if is_strict {
                return Err(VmError::TypeError(format!("Cannot assign to read only property '{}'", key)));
            }
            return Ok(());
        }
        if self.non_writable_props.contains(&(target, key.to_string())) {
            let is_strict = self.frames.last().map(|f| f.is_strict).unwrap_or(self.is_strict);
            if is_strict {
                return Err(VmError::TypeError(format!("Cannot assign to read only property '{}'", key)));
            }
            return Ok(());
        }
        let is_prim_str = target.as_object_ptr().is_some_and(|p| matches!(unsafe { &(*(p as *mut JsObject)).kind }, ObjectKind::String(_)));
        let is_prim_sym = target.as_object_ptr().is_some_and(|p| matches!(unsafe { &(*(p as *mut JsObject)).kind }, ObjectKind::Symbol(_)));
        if !target.is_object() || is_prim_str || is_prim_sym {
            let proto = if target.is_number() {
                self.number_proto
            } else if target.is_string() || is_prim_str {
                self.string_proto
            } else if target.is_boolean() {
                self.boolean_proto
            } else if target.is_symbol() || is_prim_sym {
                self.symbol_proto
            } else {
                None
            };
            if let Some(proto_val) = proto {
                let mut curr_proto = Some(proto_val);
                while let Some(pv) = curr_proto {
                    if let Some(ptr) = pv.as_object_ptr() {
                        let pobj = unsafe { &*(ptr as *mut JsObject) };
                        if let ObjectKind::Proxy { target: tgt, handler: hdl } = &pobj.kind {
                            let tgt = *tgt;
                            let hdl = *hdl;
                            let trap = self.get_property(hdl, "set");
                            if self.is_callable(trap) {
                                let key_val = self.alloc_string(key.to_string());
                                self.call(trap, hdl, &[tgt, key_val, value, target])?;
                                return Ok(());
                            }
                        }
                        if let Some(accs) = &pobj.accessors {
                            if let Some(acc) = accs.get(key) {
                                if let Some(setter) = acc.setter {
                                    self.call(setter, target, &[value])?;
                                } else {
                                    let is_strict = self.frames.last().map(|f| f.is_strict).unwrap_or(self.is_strict);
                                    if is_strict {
                                        return Err(VmError::TypeError(format!("Cannot set property '{}' of object which has only a getter", key)));
                                    }
                                }
                                return Ok(());
                            }
                        }
                        curr_proto = pobj.prototype;
                    } else {
                        break;
                    }
                }
            }
            let is_strict = self.frames.last().map(|f| f.is_strict).unwrap_or(self.is_strict);
            if is_strict {
                return Err(VmError::TypeError(format!("Cannot create property '{}' on primitive", key)));
            }
            return Ok(());
        }
        if let Some(ptr) = target.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };

            if let ObjectKind::Proxy { target: tgt, handler: hdl } = &obj.kind {
                let tgt = *tgt;
                let hdl = *hdl;
                let trap = self.get_property(hdl, "set");
                if self.is_callable(trap) {
                    let key_val = self.alloc_string(key.to_string());
                    self.call(trap, hdl, &[tgt, key_val, value, target])?;
                    return Ok(());
                } else {
                    return self.set_property_result(tgt, key, value);
                }
            }

            // 1. Check own accessor
            if let Some(accs) = &obj.accessors {
                if let Some(acc) = accs.get(key) {
                    if let Some(setter) = acc.setter {
                        self.call(setter, target, &[value])?;
                    } else {
                        let is_strict = self.frames.last().map(|f| f.is_strict).unwrap_or(self.is_strict);
                        if is_strict {
                            return Err(VmError::TypeError(format!("Cannot set property '{}' of object which has only a getter", key)));
                        }
                    }
                    return Ok(());
                }
            }

            if self.non_writable_props.contains(&(target, key.to_string())) {
                let is_strict = self.frames.last().map(|f| f.is_strict).unwrap_or(self.is_strict);
                if is_strict {
                    return Err(VmError::TypeError(format!("Cannot assign to read only property '{}'", key)));
                }
                return Ok(());
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
                        if let ObjectKind::Proxy { target: tgt, handler: hdl } = &proto_obj.kind {
                            let tgt = *tgt;
                            let hdl = *hdl;
                            let trap = self.get_property(hdl, "set");
                            if self.is_callable(trap) {
                                let key_val = self.alloc_string(key.to_string());
                                self.call(trap, hdl, &[tgt, key_val, value, target])?;
                                return Ok(());
                            }
                        }
                        if let Some(accs) = &proto_obj.accessors {
                            if let Some(acc) = accs.get(key) {
                                if let Some(setter) = acc.setter {
                                    self.call(setter, target, &[value])?;
                                } else {
                                    let is_strict = self.frames.last().map(|f| f.is_strict).unwrap_or(self.is_strict);
                                    if is_strict {
                                        return Err(VmError::TypeError(format!("Cannot set property '{}' of object which has only a getter", key)));
                                    }
                                }
                                return Ok(());
                            }
                        }
                        curr_proto = proto_obj.prototype;
                    } else {
                        break;
                    }
                }
            }

            if self.non_extensible_objects.contains(&target) && !self.shapes.has_property(obj.shape_id, key) {
                let is_strict = self.frames.last().map(|f| f.is_strict).unwrap_or(self.is_strict);
                if is_strict {
                    return Err(VmError::TypeError(format!("Cannot add property '{}', object is not extensible", key)));
                }
                return Ok(());
            }
        }
        self.set_property(target, key, value);
        Ok(())
    }

    pub fn delete_property(&mut self, target: JsValue, key: &str) -> Result<bool, VmError> {
        if let Some(ptr) = target.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if self.non_configurable_props.contains(&(target, key.to_string())) {
                return Ok(false);
            }
            if let Some(accs) = &mut obj.accessors {
                accs.remove(key);
            }
            let new_shape_id = self.shapes.remove_property(obj.shape_id, key);
            obj.shape_id = new_shape_id;
            if target == self.global_object {
                self.globals.remove(key);
            }
            self.sync_mapped_arguments_delete(target, key);
            Ok(true)
        } else {
            Ok(true)
        }
    }

    pub fn to_property_key(&mut self, val: JsValue) -> Result<PropertyKeyVal, VmError> {
        if val.is_symbol() {
            return Ok(PropertyKeyVal::Symbol(val));
        }
        if let Some(ptr) = val.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if matches!(obj.kind, ObjectKind::Symbol(_)) {
                return Ok(PropertyKeyVal::Symbol(val));
            }
        }
        let prim = self.to_primitive(val, PreferredType::String)?;
        if prim.is_symbol() {
            Ok(PropertyKeyVal::Symbol(prim))
        } else if prim.as_object_ptr().is_some_and(|p| matches!(unsafe { &(*(p as *mut JsObject)).kind }, ObjectKind::Symbol(_))) {
            Ok(PropertyKeyVal::Symbol(prim))
        } else {
            Ok(PropertyKeyVal::String(self.to_string_value(prim)))
        }
    }
}
