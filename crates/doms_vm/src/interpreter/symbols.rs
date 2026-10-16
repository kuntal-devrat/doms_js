//! Symbol-keyed properties and property *has* checks.

use super::*;

impl Vm {
    pub fn get_symbol_property_result(&mut self, target: JsValue, key: JsValue) -> Result<JsValue, VmError> {
        if target.is_null() || target.is_undefined() {
            return Err(VmError::TypeError("Cannot read property of null or undefined".into()));
        }
        if let Some(sym_iter) = self.symbol_iterator {
            if key == sym_iter {
                if let Some(ptr) = target.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    match &obj.kind {
                        ObjectKind::Map(_) => {
                            if let Some(m) = self.get_map_method(target, "entries") {
                                return Ok(m);
                            }
                        }
                        ObjectKind::Set(_) => {
                            if let Some(m) = self.get_set_method(target, "values") {
                                return Ok(m);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        let mut curr_proto = if let Some(ptr) = target.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };

            if let Some(accs) = &obj.symbol_accessors {
                if let Some(acc) = accs.get(&key) {
                    if let Some(getter) = acc.getter {
                        return self.call(getter, target, &[]);
                    } else {
                        return Ok(JsValue::undefined());
                    }
                }
            }

            if let Some(syms) = &obj.symbol_properties {
                if let Some(val) = syms.get(&key) {
                    return Ok(*val);
                }
            }

            if obj.prototype.is_some() {
                obj.prototype
            } else if matches!(obj.kind, ObjectKind::String(_)) {
                self.string_proto
            } else {
                None
            }
        } else if target.is_string() {
            self.string_proto
        } else if target.is_symbol() {
            self.symbol_proto
        } else if target.is_number() {
            self.number_proto
        } else if target.is_boolean() {
            self.boolean_proto
        } else {
            None
        };

        while let Some(proto_val) = curr_proto {
            if let Some(proto_ptr) = proto_val.as_object_ptr() {
                let proto_obj = unsafe { &*(proto_ptr as *mut JsObject) };
                if let Some(accs) = &proto_obj.symbol_accessors {
                    if let Some(acc) = accs.get(&key) {
                        if let Some(getter) = acc.getter {
                            return self.call(getter, target, &[]);
                        } else {
                            return Ok(JsValue::undefined());
                        }
                    }
                }
                if let Some(syms) = &proto_obj.symbol_properties {
                    if let Some(val) = syms.get(&key) {
                        return Ok(*val);
                    }
                }
                curr_proto = proto_obj.prototype;
            } else {
                break;
            }
        }
        Ok(JsValue::undefined())
    }

    pub fn get_symbol_property(&mut self, target: JsValue, key: JsValue) -> JsValue {
        self.get_symbol_property_result(target, key).unwrap_or(JsValue::undefined())
    }

    pub fn set_symbol_property_result(&mut self, target: JsValue, key: JsValue, value: JsValue) -> Result<(), VmError> {
        if self.module_namespace_objects.contains(&target) {
            let is_strict = self.frames.last().map(|f| f.is_strict).unwrap_or(self.is_strict);
            if is_strict {
                return Err(VmError::TypeError("Cannot assign to read only property on module namespace object".into()));
            }
            return Ok(());
        }
        if let Some(ptr) = target.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };

            if let Some(accs) = &obj.symbol_accessors {
                if let Some(acc) = accs.get(&key) {
                    if let Some(setter) = acc.setter {
                        let _ = self.call(setter, target, &[value])?;
                    }
                    return Ok(());
                }
            }

            let mut curr_proto = obj.prototype;
            while let Some(proto_val) = curr_proto {
                if let Some(proto_ptr) = proto_val.as_object_ptr() {
                    let proto_obj = unsafe { &*(proto_ptr as *mut JsObject) };
                    if let Some(accs) = &proto_obj.symbol_accessors {
                        if let Some(acc) = accs.get(&key) {
                            if let Some(setter) = acc.setter {
                                let _ = self.call(setter, target, &[value])?;
                            }
                            return Ok(());
                        }
                    }
                    curr_proto = proto_obj.prototype;
                } else {
                    break;
                }
            }

            obj.insert_symbol_property(key, value);
        }
        Ok(())
    }

    pub fn set_symbol_property(&mut self, target: JsValue, key: JsValue, value: JsValue) {
        let _ = self.set_symbol_property_result(target, key, value);
    }

    pub fn has_property(&mut self, target: JsValue, key: &str) -> Result<bool, VmError> {
        if !target.is_object() {
            return Ok(false);
        }
        if self.deleted_props.contains(&(target, key.to_string())) {
            return Ok(false);
        }
        if self.module_reexports.contains_key(&(target, key.to_string())) {
            return Ok(true);
        }
        let mut curr = Some(target);
        while let Some(c_val) = curr {
            if let Some(c_ptr) = c_val.as_object_ptr() {
                let c_obj = unsafe { &*(c_ptr as *mut JsObject) };
                if let ObjectKind::Proxy { target: tgt, handler: hdl } = &c_obj.kind {
                    let tgt = *tgt;
                    let hdl = *hdl;
                    let trap = self.get_property(hdl, "has");
                    if self.is_callable(trap) {
                        let key_val = self.alloc_string(key.to_string());
                        let res = self.call(trap, hdl, &[tgt, key_val])?;
                        return Ok(res.is_truthy());
                    }
                    return self.has_property(tgt, key);
                }
                if self.has_own_property_key(c_val, &PropertyKeyVal::String(key.to_string())) {
                    return Ok(true);
                }
                curr = c_obj.prototype;
            } else {
                break;
            }
        }
        Ok(false)
    }

    pub fn has_own_property_key(&self, target: JsValue, prop_key: &PropertyKeyVal) -> bool {
        if !target.is_object() {
            return false;
        }
        let ptr = target.as_object_ptr().unwrap();
        let obj = unsafe { &*(ptr as *mut JsObject) };

        match prop_key {
            PropertyKeyVal::Symbol(sym) => {
                if let Some(syms) = &obj.symbol_properties {
                    if syms.contains_key(sym) {
                        return true;
                    }
                }
                if let Some(accs) = &obj.symbol_accessors {
                    if accs.contains_key(sym) {
                        return true;
                    }
                }
                false
            }
            PropertyKeyVal::String(key) => {
                if self.deleted_props.contains(&(target, key.clone())) {
                    return false;
                }
                match &obj.kind {
                    ObjectKind::Array(elements) => {
                        if key == "length" {
                            return true;
                        }
                        if let Ok(idx) = key.parse::<usize>() {
                            if let Some(elem) = elements.get(idx) {
                                return !elem.is_hole();
                            }
                        }
                    }
                    ObjectKind::String(s) | ObjectKind::StringObject(s) => {
                        if key == "length" || key.parse::<usize>().map(|idx| idx < s.chars().count()).unwrap_or(false) {
                            return true;
                        }
                    }
                    ObjectKind::TypedArray(ta) => {
                        if matches!(key.as_str(), "length" | "byteLength" | "byteOffset" | "buffer")
                            || key.parse::<usize>().map(|idx| idx < ta.length).unwrap_or(false) {
                            return true;
                        }
                    }
                    ObjectKind::Function(_) | ObjectKind::NativeFunction(_) => {
                        if key == "length" || key == "name" || (matches!(&obj.kind, ObjectKind::Function(_)) && key == "prototype") {
                            return true;
                        }
                    }
                    _ => {}
                }
                self.shapes.has_property(obj.shape_id, key)
                    || obj.accessors.as_ref().map(|a| a.contains_key(key)).unwrap_or(false)
                    || self.module_reexports.contains_key(&(target, key.clone()))
            }
        }
    }
}
