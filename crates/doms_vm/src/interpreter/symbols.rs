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
                        ObjectKind::Generator(_) => {
                            let iter_self = self.alloc_closure_native_function(
                                "[Symbol.iterator]",
                                0,
                                Arc::new(move |_vm, this, _args| Ok(this)),
                            );
                            return Ok(iter_self);
                        }
                        _ => {}
                    }
                }
            }
        }

        if let Some(sym_async) = self.symbol_async_iterator {
            if key == sym_async {
                if let Some(ptr) = target.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    match &obj.kind {
                        ObjectKind::Generator(g) if g.function.is_async => {
                            let iter_self = self.alloc_closure_native_function(
                                "[Symbol.asyncIterator]",
                                0,
                                Arc::new(move |_vm, this, _args| Ok(this)),
                            );
                            return Ok(iter_self);
                        }
                        _ => {}
                    }
                }
            }
        }

        if let Some(ptr) = target.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if matches!(obj.kind, ObjectKind::Proxy { .. }) {
                let prop_key = PropertyKeyVal::Symbol(key);
                return self.proxy_get(target, &prop_key, target);
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
                if matches!(proto_obj.kind, ObjectKind::Proxy { .. }) {
                    let prop_key = PropertyKeyVal::Symbol(key);
                    return self.proxy_get(proto_val, &prop_key, target);
                }
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

    pub fn set_symbol_property_with_receiver(
        &mut self,
        target: JsValue,
        key: JsValue,
        value: JsValue,
        receiver: JsValue,
    ) -> Result<bool, VmError> {
        if self.module_namespace_objects.contains(&target) {
            return Ok(false);
        }
        let key_name = format!("[Symbol:{:x}]", key.raw_bits());
        if self.non_writable_props.contains(&(target, key_name.clone())) {
            return Ok(false);
        }
        if !target.is_js_object() {
            return Ok(false);
        }

        if let Some(ptr) = target.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };

            if matches!(obj.kind, ObjectKind::Proxy { .. }) {
                let prop_key = PropertyKeyVal::Symbol(key);
                return self.proxy_set(target, &prop_key, value, receiver);
            }

            if let Some(accs) = &obj.symbol_accessors {
                if let Some(acc) = accs.get(&key) {
                    if let Some(setter) = acc.setter {
                        self.call(setter, receiver, &[value])?;
                        return Ok(true);
                    } else {
                        return Ok(false);
                    }
                }
            }

            let has_own_data = obj.symbol_properties.as_ref().map(|m| m.contains_key(&key)).unwrap_or(false);
            if !has_own_data {
                let mut curr_proto = obj.prototype;
                while let Some(proto_val) = curr_proto {
                    if let Some(proto_ptr) = proto_val.as_object_ptr() {
                        let proto_obj = unsafe { &*(proto_ptr as *mut JsObject) };
                        if matches!(proto_obj.kind, ObjectKind::Proxy { .. }) {
                            let prop_key = PropertyKeyVal::Symbol(key);
                            return self.proxy_set(proto_val, &prop_key, value, receiver);
                        }
                        if let Some(accs) = &proto_obj.symbol_accessors {
                            if let Some(acc) = accs.get(&key) {
                                if let Some(setter) = acc.setter {
                                    self.call(setter, receiver, &[value])?;
                                    return Ok(true);
                                } else {
                                    return Ok(false);
                                }
                            }
                        }
                        if self.non_writable_props.contains(&(proto_val, key_name.clone())) {
                            return Ok(false);
                        }
                        curr_proto = proto_obj.prototype;
                    } else {
                        break;
                    }
                }
            }
        }

        if !receiver.is_js_object() {
            return Ok(false);
        }

        if let Some(r_ptr) = receiver.as_object_ptr() {
            let r_obj = unsafe { &mut *(r_ptr as *mut JsObject) };
            if matches!(r_obj.kind, ObjectKind::Proxy { .. }) {
                let desc = self.alloc_object();
                self.set_property(desc, "value", value);
                self.set_property(desc, "writable", JsValue::from_boolean(true));
                self.set_property(desc, "enumerable", JsValue::from_boolean(true));
                self.set_property(desc, "configurable", JsValue::from_boolean(true));
                let prop_key = PropertyKeyVal::Symbol(key);
                return self.proxy_define_own_property(receiver, &prop_key, desc);
            }
            if let Some(accs) = &r_obj.symbol_accessors {
                if accs.contains_key(&key) {
                    return Ok(false);
                }
            }
            if self.non_writable_props.contains(&(receiver, key_name.clone())) {
                return Ok(false);
            }
            let has_sym = r_obj.symbol_properties.as_ref().map(|m| m.contains_key(&key)).unwrap_or(false);
            if self.non_extensible_objects.contains(&receiver) && !has_sym {
                return Ok(false);
            }
            r_obj.insert_symbol_property(key, value);
            return Ok(true);
        }

        Ok(false)
    }

    pub fn set_symbol_property_result(&mut self, target: JsValue, key: JsValue, value: JsValue) -> Result<(), VmError> {
        self.set_symbol_property_with_receiver_result(target, key, value, target)
    }

    pub fn set_symbol_property_with_receiver_result(
        &mut self,
        target: JsValue,
        key: JsValue,
        value: JsValue,
        receiver: JsValue,
    ) -> Result<(), VmError> {
        let is_strict = self.is_strict || self.frames.last().map(|f| f.is_strict).unwrap_or(false);
        let ok = self.set_symbol_property_with_receiver(target, key, value, receiver)?;
        if !ok && is_strict {
            return Err(VmError::TypeError("Cannot assign to property".into()));
        }
        Ok(())
    }

    pub fn set_symbol_property(&mut self, target: JsValue, key: JsValue, value: JsValue) {
        let _ = self.set_symbol_property_result(target, key, value);
    }

    pub fn has_property_key(&mut self, target: JsValue, key: &PropertyKeyVal) -> Result<bool, VmError> {
        if !target.is_object() {
            return Ok(false);
        }
        if let PropertyKeyVal::String(s) = key {
            if self.deleted_props.contains(&(target, s.clone())) {
                return Ok(false);
            }
            if self.module_reexports.contains_key(&(target, s.clone())) {
                return Ok(true);
            }
        }
        let mut curr = Some(target);
        while let Some(c_val) = curr {
            if let Some(c_ptr) = c_val.as_object_ptr() {
                let c_obj = unsafe { &*(c_ptr as *mut JsObject) };
                if matches!(c_obj.kind, ObjectKind::Proxy { .. }) {
                    return self.proxy_has(c_val, key);
                }
                if self.has_own_property_key(c_val, key) {
                    return Ok(true);
                }
                curr = c_obj.prototype;
            } else {
                break;
            }
        }
        Ok(false)
    }

    pub fn has_property(&mut self, target: JsValue, key: &str) -> Result<bool, VmError> {
        self.has_property_key(target, &PropertyKeyVal::String(key.to_string()))
    }

    pub fn has_own_property_key(&self, target: JsValue, prop_key: &PropertyKeyVal) -> bool {
        if !target.is_object() {
            return false;
        }
        let ptr = target.as_object_ptr().unwrap();
        let obj = unsafe { &*(ptr as *mut JsObject) };

        match prop_key {
            PropertyKeyVal::Symbol(sym) => {
                if let Some(sym_iter) = self.symbol_iterator {
                    if *sym == sym_iter && matches!(&obj.kind, ObjectKind::Map(_) | ObjectKind::Set(_) | ObjectKind::Generator(_)) {
                        return true;
                    }
                }
                if let Some(sym_async) = self.symbol_async_iterator {
                    if *sym == sym_async && matches!(&obj.kind, ObjectKind::Generator(g) if g.function.is_async) {
                        return true;
                    }
                }
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
                    ObjectKind::Generator(_) => {
                        if matches!(key.as_str(), "next" | "return" | "throw") {
                            return true;
                        }
                    }
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
