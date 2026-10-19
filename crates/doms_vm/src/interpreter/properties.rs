//! Prototype handling and direct property writes.

use super::*;

impl Vm {
    pub fn get_prototype_of(&mut self, target: JsValue) -> Option<JsValue> {
        if let Some(ptr) = target.as_object_ptr() {
            if !self.is_valid_heap_object(ptr) {
                return None;
            }
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

    pub fn is_instance_of_iterator_proto(&mut self, val: JsValue) -> bool {
        let Some(iter_proto) = self.iterator_proto else { return false; };
        let mut curr = self.get_prototype_of(val);
        while let Some(proto) = curr {
            if proto == iter_proto {
                return true;
            }
            curr = self.get_prototype_of(proto);
        }
        false
    }

    pub fn set_prototype(&mut self, target: JsValue, proto: JsValue) {
        if let Some(ptr) = target.as_object_ptr() {
            if !self.is_valid_heap_object(ptr) {
                return;
            }
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            obj.prototype = if proto.is_null() { None } else { Some(proto) };
        }
    }

    pub fn set_property_direct(&mut self, target: JsValue, key: &str, value: JsValue) {
        if let Some(ptr) = target.as_object_ptr() {
            if !self.is_valid_heap_object(ptr) {
                return;
            }
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let Some(offset) = self.shapes.lookup_offset(obj.shape_id, key) {
                obj.set_slot(offset, value);
            } else {
                let (new_shape_id, offset) = self.shapes.transition(obj.shape_id, key);
                obj.shape_id = new_shape_id;
                obj.set_slot(offset, value);
                obj.add_prop_order(key.to_string());
            }
        }
    }

    pub fn set_property(&mut self, target: JsValue, key: &str, value: JsValue) {
        if let Some(info) = self.get_own_property_info(target, key) {
            if !info.attributes.writable {
                return;
            }
        }
        if key == "length" && !self.is_array_length_writable(target) {
            return;
        }
        if self.non_writable_props.contains(&(target, key.to_string())) {
            return;
        }
        if !self.is_extensible_object(target) {
            if let Some(ptr) = target.as_object_ptr() {
                if !self.is_valid_heap_object(ptr) {
                    return;
                }
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
            if !self.is_valid_heap_object(ptr) {
                return;
            }
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
                        if new_len < elements.len() {
                            elements.truncate(new_len);
                        } else if new_len < 100_000 {
                            elements.resize(new_len, JsValue::hole());
                        }
                    }
                    if let Some(offset) = self.shapes.lookup_offset(obj.shape_id, key) {
                        obj.set_slot(offset, value);
                    } else {
                        let (new_shape_id, offset) = self.shapes.transition(obj.shape_id, key);
                        obj.shape_id = new_shape_id;
                        obj.set_slot(offset, value);
                        obj.add_prop_order(key.to_string());
                    }
                    return;
                }
                if let Ok(idx) = key.parse::<usize>() {
                    if idx < elements.len() {
                        elements[idx] = value;
                        return;
                    } else if idx < 100_000 {
                        elements.resize(idx + 1, JsValue::hole());
                        elements[idx] = value;
                        return;
                    }
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
                obj.add_prop_order(key.to_string());
            }
        }
        if target == self.global_object {
            self.globals.insert(key.to_string(), value);
        }
        self.sync_mapped_arguments_write(target, key, value);
    }

    pub fn set_own_property(&mut self, target: JsValue, key: &str, value: JsValue) {
        if let Some(ptr) = target.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };

            if let ObjectKind::Array(elements) = &mut obj.kind {
                if key == "length" {
                    if let Some(new_len) = value.to_number().map(|n| n as usize) {
                        if new_len < elements.len() {
                            elements.truncate(new_len);
                        } else if new_len < 100_000 {
                            elements.resize(new_len, JsValue::hole());
                        }
                    }
                    if let Some(offset) = self.shapes.lookup_offset(obj.shape_id, key) {
                        obj.set_slot(offset, value);
                    } else {
                        let (new_shape_id, offset) = self.shapes.transition(obj.shape_id, key);
                        obj.shape_id = new_shape_id;
                        obj.set_slot(offset, value);
                        obj.add_prop_order(key.to_string());
                    }
                    return;
                }
                if let Ok(idx) = key.parse::<usize>() {
                    if idx < elements.len() {
                        elements[idx] = value;
                        return;
                    } else if idx < 100_000 {
                        elements.resize(idx + 1, JsValue::hole());
                        elements[idx] = value;
                        return;
                    }
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
                obj.add_prop_order(key.to_string());
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
            if !self.is_extensible_object(target) && !self.has_own_property(target, key) {
                return Err(VmError::TypeError(format!("Cannot add property '{}' to non-extensible object", key)));
            }
        }

        self.non_writable_props.remove(&(target, key_string.clone()));
        self.non_enumerable_props.remove(&(target, key_string.clone()));
        self.set_own_property(target, key, value);
        Ok(())
    }

    pub fn copy_data_properties(
        &mut self,
        target: JsValue,
        source: JsValue,
        excluded_keys: &[JsValue],
    ) -> Result<(), VmError> {
        if source.is_nullish() {
            return Ok(());
        }
        let from = self.to_object_value(source)?;
        let keys = crate::builtins::object::names::get_own_property_keys(self, from)?;
        for key in keys {
            let is_excluded = excluded_keys.iter().any(|excluded| {
                match &key {
                    PropertyKeyVal::String(s) => {
                        let exc_s = self.to_string_value(*excluded);
                        s == &exc_s
                    }
                    PropertyKeyVal::Symbol(sym) => {
                        excluded.is_symbol() && *excluded == *sym
                    }
                }
            });
            if is_excluded {
                continue;
            }

            let desc = crate::builtins::object::descriptor::get_own_property_descriptor_internal(self, from, &key)?;
            if desc.is_object() {
                let enum_val = self.get_property(desc, "enumerable");
                if enum_val.is_truthy() {
                    let val = match &key {
                        PropertyKeyVal::String(s) => self.get_property_result(from, s)?,
                        PropertyKeyVal::Symbol(sym) => self.get_symbol_property_result(from, *sym)?,
                    };
                    match &key {
                        PropertyKeyVal::String(s) => {
                            self.create_data_property_or_throw(target, s, val)?;
                        }
                        PropertyKeyVal::Symbol(sym) => {
                            if let Some(ptr) = target.as_object_ptr() {
                                let obj = unsafe { &mut *(ptr as *mut JsObject) };
                                obj.insert_symbol_property(*sym, val);
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }
}
