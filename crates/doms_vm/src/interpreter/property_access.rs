//! Spec-level get/set/delete property operations.

use super::*;
use crate::builtins::object::descriptor::get_own_property_descriptor_internal;

impl Vm {
    pub fn get_property_result(&mut self, target: JsValue, key: &str) -> Result<JsValue, VmError> {
        self.get_property_with_receiver_result(target, key, target)
    }

    pub fn get_property_with_receiver_result(
        &mut self,
        target: JsValue,
        key: &str,
        receiver: JsValue,
    ) -> Result<JsValue, VmError> {
        if target.is_null() || target.is_undefined() {
            return Err(VmError::TypeError(format!(
                "Cannot read properties of {} (reading '{}')",
                if target.is_null() { "null" } else { "undefined" },
                key
            )));
        }
        if let Some(ptr) = target.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };

            if matches!(obj.kind, ObjectKind::Proxy { .. }) {
                let prop_key = PropertyKeyVal::String(key.to_string());
                return self.proxy_get(target, &prop_key, receiver);
            }

            // 1. Check own accessor
            if let Some(accs) = &obj.accessors {
                if let Some(acc) = accs.get(key) {
                    if let Some(getter) = acc.getter {
                        return self.call(getter, receiver, &[]);
                    } else {
                        return Ok(JsValue::undefined());
                    }
                }
            }

            // 2. Check prototype chain accessor only if object does NOT have own data property
            let has_own_data = self.shapes.has_property(obj.shape_id, key)
                || matches!(&obj.kind, ObjectKind::Array(elements) if key == "length" || key.parse::<usize>().ok().and_then(|idx| elements.get(idx)).is_some_and(|elem| !elem.is_hole()))
                || matches!(&obj.kind, ObjectKind::TypedArray(ta) if key.parse::<usize>().map(|idx| idx < ta.length).unwrap_or(false))
                || matches!(&obj.kind, ObjectKind::String(_) | ObjectKind::StringObject(_) if key == "length" || key.parse::<usize>().is_ok());
            if !has_own_data {
                let mut curr_proto = obj.prototype;
                while let Some(proto_val) = curr_proto {
                    if let Some(proto_ptr) = proto_val.as_object_ptr() {
                        let proto_obj = unsafe { &*(proto_ptr as *mut JsObject) };
                        if matches!(proto_obj.kind, ObjectKind::Proxy { .. }) {
                            let prop_key = PropertyKeyVal::String(key.to_string());
                            return self.proxy_get(proto_val, &prop_key, receiver);
                        }
                        if let Some(accs) = &proto_obj.accessors {
                            if let Some(acc) = accs.get(key) {
                                if let Some(getter) = acc.getter {
                                    return self.call(getter, receiver, &[]);
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
                return Err(VmError::ReferenceError(format!(
                    "Cannot access '{}' before initialization",
                    key
                )));
            }
            return Ok(val);
        }
        Ok(self.get_property_with_receiver(target, key, receiver))
    }

    pub fn set_property_result(
        &mut self,
        target: JsValue,
        key: &str,
        value: JsValue,
    ) -> Result<(), VmError> {
        self.set_property_with_receiver_result(target, key, value, target)
    }

    pub fn set_property_throw(
        &mut self,
        target: JsValue,
        key: &str,
        value: JsValue,
    ) -> Result<(), VmError> {
        let ok = self.set_property_with_receiver(target, key, value, target)?;
        if !ok {
            return Err(VmError::TypeError(format!(
                "Cannot assign to property '{}' on {}",
                key,
                if target.is_null() {
                    "null"
                } else if target.is_undefined() {
                    "undefined"
                } else {
                    "object"
                }
            )));
        }
        Ok(())
    }

    pub fn set_property_with_receiver_result(
        &mut self,
        target: JsValue,
        key: &str,
        value: JsValue,
        receiver: JsValue,
    ) -> Result<(), VmError> {
        let is_strict = self.is_strict || self.frames.last().map(|f| f.is_strict).unwrap_or(false);
        let ok = self.set_property_with_receiver(target, key, value, receiver)?;
        if !ok && is_strict {
            return Err(VmError::TypeError(format!(
                "Cannot assign to property '{}' on {}",
                key,
                if receiver.is_null() {
                    "null"
                } else if receiver.is_undefined() {
                    "undefined"
                } else {
                    "object"
                }
            )));
        }
        Ok(())
    }

    pub fn set_property_with_receiver(
        &mut self,
        target: JsValue,
        key: &str,
        value: JsValue,
        receiver: JsValue,
    ) -> Result<bool, VmError> {
        if target.is_null() || target.is_undefined() {
            return Err(VmError::TypeError(format!(
                "Cannot set properties of {} (setting '{}')",
                if target.is_null() { "null" } else { "undefined" },
                key
            )));
        }

        // If target is Proxy, invoke proxy_set trap
        if let Some(ptr) = target.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if matches!(obj.kind, ObjectKind::Proxy { .. }) {
                let prop_key = PropertyKeyVal::String(key.to_string());
                return self.proxy_set(target, &prop_key, value, receiver);
            }
        }

        if self.module_namespace_objects.contains(&target) {
            return Ok(false);
        }

        let is_func = target.as_object_ptr().is_some_and(|p| {
            matches!(
                unsafe { &(*(p as *mut JsObject)).kind },
                ObjectKind::Function(_) | ObjectKind::NativeFunction(_)
            )
        });
        if is_func && (key == "length" || key == "name") {
            return Ok(false);
        }

        let is_str_obj = target.as_object_ptr().is_some_and(|p| {
            if let ObjectKind::StringObject(s) = unsafe { &(*(p as *mut JsObject)).kind } {
                key == "length" || key.parse::<usize>().is_ok_and(|idx| idx < s.chars().count())
            } else {
                false
            }
        });
        if is_str_obj {
            return Ok(false);
        }

        if !self.is_own_property_writable(target, key) {
            return Ok(false);
        }

        let is_prim_str = target.as_object_ptr().is_some_and(|p| {
            matches!(unsafe { &(*(p as *mut JsObject)).kind }, ObjectKind::String(_))
        });
        if !target.is_object() || is_prim_str {
            let proto = if target.is_number() {
                self.number_proto
            } else if target.is_string() || is_prim_str {
                self.string_proto
            } else if target.is_boolean() {
                self.boolean_proto
            } else if target.is_symbol() {
                self.symbol_proto
            } else {
                None
            };
            if let Some(proto_val) = proto {
                let mut curr_proto = Some(proto_val);
                while let Some(pv) = curr_proto {
                    if let Some(ptr) = pv.as_object_ptr() {
                        let pobj = unsafe { &*(ptr as *mut JsObject) };
                        if matches!(pobj.kind, ObjectKind::Proxy { .. }) {
                            let prop_key = PropertyKeyVal::String(key.to_string());
                            return self.proxy_set(pv, &prop_key, value, receiver);
                        }
                        if let Some(accs) = &pobj.accessors {
                            if let Some(acc) = accs.get(key) {
                                if let Some(setter) = acc.setter {
                                    self.call(setter, receiver, &[value])?;
                                    return Ok(true);
                                } else {
                                    return Ok(false);
                                }
                            }
                        }
                        curr_proto = pobj.prototype;
                    } else {
                        break;
                    }
                }
            }
            return Ok(false);
        }

        if let Some(ptr) = target.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };

            // 1. Check own accessor
            if let Some(accs) = &obj.accessors {
                if let Some(acc) = accs.get(key) {
                    if let Some(setter) = acc.setter {
                        self.call(setter, receiver, &[value])?;
                        return Ok(true);
                    } else {
                        return Ok(false);
                    }
                }
            }

            // 2. Check prototype chain accessor only if object does NOT have own data property
            let has_own_data = self.shapes.has_property(obj.shape_id, key)
                || matches!(&obj.kind, ObjectKind::Array(elements) if key == "length" || key.parse::<usize>().ok().and_then(|idx| elements.get(idx)).is_some_and(|elem| !elem.is_hole()))
                || matches!(&obj.kind, ObjectKind::TypedArray(ta) if key.parse::<usize>().map(|idx| idx < ta.length).unwrap_or(false))
                || matches!(&obj.kind, ObjectKind::String(_) | ObjectKind::StringObject(_) if key == "length" || key.parse::<usize>().is_ok());

            if !has_own_data {
                let mut curr_proto = obj.prototype;
                while let Some(proto_val) = curr_proto {
                    if let Some(proto_ptr) = proto_val.as_object_ptr() {
                        let proto_obj = unsafe { &*(proto_ptr as *mut JsObject) };
                        if matches!(proto_obj.kind, ObjectKind::Proxy { .. }) {
                            let prop_key = PropertyKeyVal::String(key.to_string());
                            return self.proxy_set(proto_val, &prop_key, value, receiver);
                        }
                        if let Some(accs) = &proto_obj.accessors {
                            if let Some(acc) = accs.get(key) {
                                if let Some(setter) = acc.setter {
                                    self.call(setter, receiver, &[value])?;
                                    return Ok(true);
                                } else {
                                    return Ok(false);
                                }
                            }
                        }
                        if let Some(proto_info) =
                            self.shapes.get_property_info(proto_obj.shape_id, key)
                        {
                            if !proto_info.attributes.writable {
                                return Ok(false);
                            }
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
            let r_obj = unsafe { &*(r_ptr as *mut JsObject) };
            if matches!(r_obj.kind, ObjectKind::Proxy { .. }) {
                let prop_key = PropertyKeyVal::String(key.to_string());
                let existing_desc =
                    get_own_property_descriptor_internal(self, receiver, &prop_key)?;
                if !existing_desc.is_undefined() {
                    let desc_writ = self.get_property(existing_desc, "writable");
                    let desc_set = self.get_property(existing_desc, "set");
                    let desc_get = self.get_property(existing_desc, "get");
                    if !desc_set.is_undefined() || !desc_get.is_undefined() {
                        return Ok(false);
                    }
                    if !desc_writ.is_undefined() && !desc_writ.is_truthy() {
                        return Ok(false);
                    }
                    let val_desc = self.alloc_object();
                    self.set_property(val_desc, "value", value);
                    return self.proxy_define_own_property(receiver, &prop_key, val_desc);
                } else {
                    let val_desc = self.alloc_object();
                    self.set_property(val_desc, "value", value);
                    self.set_property(val_desc, "writable", JsValue::from_boolean(true));
                    self.set_property(val_desc, "enumerable", JsValue::from_boolean(true));
                    self.set_property(val_desc, "configurable", JsValue::from_boolean(true));
                    return self.proxy_define_own_property(receiver, &prop_key, val_desc);
                }
            }
            if let Some(accs) = &r_obj.accessors {
                if accs.contains_key(key) {
                    return Ok(false);
                }
            }
            if let Some(r_info) = self.shapes.get_property_info(r_obj.shape_id, key) {
                if !r_info.attributes.writable {
                    return Ok(false);
                }
            }
            if key == "length" && !r_obj.is_array_length_writable() {
                return Ok(false);
            }
            if key == "length" && matches!(r_obj.kind, ObjectKind::Array(_)) {
                let num = self.to_number_value(value)?;
                let u32_len = num as u32;
                if (u32_len as f64) != num || num.is_nan() || num.is_sign_negative() {
                    return Err(VmError::RangeError("Invalid array length".into()));
                }
            }
            if !r_obj.is_extensible() && !self.shapes.has_property(r_obj.shape_id, key) {
                return Ok(false);
            }
        }

        self.set_property(receiver, key, value);
        Ok(true)
    }

    pub fn delete_property(&mut self, target: JsValue, key: &str) -> Result<bool, VmError> {
        if let Some(ptr) = target.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if matches!(obj.kind, ObjectKind::Proxy { .. }) {
                return self
                    .proxy_delete_property(target, &PropertyKeyVal::String(key.to_string()));
            }
            if let Some(info) = self.shapes.get_property_info(obj.shape_id, key) {
                if !info.attributes.configurable {
                    return Ok(false);
                }
            }
            if let Some(accs) = &obj.accessors {
                if let Some(acc) = accs.get(key) {
                    if !acc.configurable {
                        return Ok(false);
                    }
                }
            }
            if let ObjectKind::StringObject(s) = &obj.kind {
                if key == "length" || key.parse::<usize>().is_ok_and(|idx| idx < s.chars().count())
                {
                    return Ok(false);
                }
            }
            if let ObjectKind::Array(elements) = &mut obj.kind {
                if key == "length" {
                    return Ok(false);
                }
                if let Ok(idx) = key.parse::<usize>() {
                    if idx < elements.len() {
                        elements[idx] = JsValue::hole();
                        return Ok(true);
                    }
                }
            }
            if let Some(accs) = &mut obj.accessors {
                accs.remove(key);
            }
            if let Some(order) = &mut obj.accessor_order {
                order.retain(|k| k != key);
            }
            obj.remove_prop_order(key);
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

    pub fn delete_property_or_throw(&mut self, target: JsValue, key: &str) -> Result<(), VmError> {
        let success = self.delete_property(target, key)?;
        if !success {
            return Err(VmError::TypeError(format!("Cannot delete property '{}'", key)));
        }
        Ok(())
    }

    pub fn delete_property_key(
        &mut self,
        target: JsValue,
        key: &PropertyKeyVal,
    ) -> Result<bool, VmError> {
        if let Some(ptr) = target.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if matches!(obj.kind, ObjectKind::Proxy { .. }) {
                return self.proxy_delete_property(target, key);
            }
        }
        match key {
            PropertyKeyVal::String(s) => self.delete_property(target, s),
            PropertyKeyVal::Symbol(sym) => {
                if let Some(ptr) = target.as_object_ptr() {
                    let obj = unsafe { &mut *(ptr as *mut JsObject) };
                    if let Some(attrs) = obj.symbol_attributes.as_ref().and_then(|m| m.get(sym)) {
                        if !attrs.configurable {
                            return Ok(false);
                        }
                    }
                    if let Some(accs) = obj.symbol_accessors.as_ref().and_then(|m| m.get(sym)) {
                        if !accs.configurable {
                            return Ok(false);
                        }
                    }
                    if let Some(attrs_map) = &mut obj.symbol_attributes {
                        attrs_map.remove(sym);
                    }
                    if let Some(syms) = &mut obj.symbol_properties {
                        syms.remove(sym);
                    }
                    if let Some(accs) = &mut obj.symbol_accessors {
                        accs.remove(sym);
                    }
                    if let Some(order) = &mut obj.symbol_order {
                        order.retain(|s| s != sym);
                    }
                }
                Ok(true)
            }
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
        } else if prim.as_object_ptr().is_some_and(|p| {
            matches!(unsafe { &(*(p as *mut JsObject)).kind }, ObjectKind::Symbol(_))
        }) {
            Ok(PropertyKeyVal::Symbol(prim))
        } else {
            Ok(PropertyKeyVal::String(self.to_string_value(prim)))
        }
    }
}
