//! Fast paths for indexed element get/set.

use super::*;

impl Vm {
    pub fn get_element(&mut self, obj: JsValue, index: usize) -> JsValue {
        if obj.is_sso_string() {
            let len = ((obj.0 >> 40) & 0x7) as usize;
            if index < len {
                let payload = obj.0 & 0x0000_00FF_FFFF_FFFF;
                let byte = ((payload >> (index * 8)) & 0xFF) as u8;
                return self.alloc_string((byte as char).to_string());
            }
            if let Some(proto) = self.string_proto {
                let key = index.to_string();
                return self.get_property_with_receiver(proto, &key, obj);
            }
            return JsValue::undefined();
        }
        if let Some(ptr) = obj.as_object_ptr() {
            let jsobj = unsafe { &*(ptr as *mut JsObject) };
            if jsobj.accessors.is_some() || matches!(jsobj.kind, ObjectKind::Proxy { .. }) {
                let key = index.to_string();
                return self.get_property(obj, &key);
            }
            if let ObjectKind::Array(elements) = &jsobj.kind {
                if index < elements.len() {
                    let v = elements[index];
                    if !v.is_hole() {
                        return v;
                    }
                }
                let key = index.to_string();
                return self.get_property(obj, &key);
            }
            if let ObjectKind::TypedArray(ta) = &jsobj.kind {
                let ta = ta.clone();
                if let Some(v) = self.read_typed_array_element_vm(&ta, index) {
                    return v;
                }
            }
            if let ObjectKind::String(s) = &jsobj.kind {
                if let Some(ch) = s.chars().nth(index) {
                    return self.alloc_string(ch.to_string());
                }
            }
            if let ObjectKind::ConsString { length, .. } = &jsobj.kind {
                if index < *length {
                    let s = self.to_string_value(obj);
                    if let Some(ch) = s.chars().nth(index) {
                        return self.alloc_string(ch.to_string());
                    }
                }
            }
        }
        let key = index.to_string();
        self.get_property(obj, &key)
    }

    pub fn get_element_result(&mut self, obj: JsValue, index: usize) -> Result<JsValue, VmError> {
        if obj.is_sso_string() {
            let len = ((obj.0 >> 40) & 0x7) as usize;
            if index < len {
                let payload = obj.0 & 0x0000_00FF_FFFF_FFFF;
                let byte = ((payload >> (index * 8)) & 0xFF) as u8;
                return Ok(self.alloc_string((byte as char).to_string()));
            }
            if let Some(proto) = self.string_proto {
                let key = index.to_string();
                return Ok(self.get_property_with_receiver(proto, &key, obj));
            }
            return Ok(JsValue::undefined());
        }
        if let Some(ptr) = obj.as_object_ptr() {
            let jsobj = unsafe { &*(ptr as *mut JsObject) };
            if jsobj.accessors.is_some() || matches!(jsobj.kind, ObjectKind::Proxy { .. }) {
                let key = index.to_string();
                return self.get_property_result(obj, &key);
            }
            if let ObjectKind::Array(elements) = &jsobj.kind {
                if index < elements.len() {
                    let v = elements[index];
                    if !v.is_hole() {
                        return Ok(v);
                    }
                }
                let key = index.to_string();
                return self.get_property_result(obj, &key);
            }
            if let ObjectKind::TypedArray(ta) = &jsobj.kind {
                let ta = ta.clone();
                if let Some(v) = self.read_typed_array_element_vm(&ta, index) {
                    return Ok(v);
                }
            }
            if let ObjectKind::String(s) = &jsobj.kind {
                if let Some(ch) = s.chars().nth(index) {
                    return Ok(self.alloc_string(ch.to_string()));
                }
            }
            if let ObjectKind::ConsString { length, .. } = &jsobj.kind {
                if index < *length {
                    let s = self.to_string_value(obj);
                    if let Some(ch) = s.chars().nth(index) {
                        return Ok(self.alloc_string(ch.to_string()));
                    }
                }
            }
        }
        let key = index.to_string();
        self.get_property_result(obj, &key)
    }

    pub fn set_element(&mut self, obj: JsValue, index: usize, val: JsValue) {
        if let Some(ptr) = obj.as_object_ptr() {
            let jsobj = unsafe { &mut *(ptr as *mut JsObject) };
            if jsobj.accessors.is_some() || matches!(jsobj.kind, ObjectKind::Proxy { .. }) {
                let key = index.to_string();
                self.set_property(obj, &key, val);
                return;
            }
            let is_extensible = jsobj.is_extensible();
            if let ObjectKind::Array(elements) = &mut jsobj.kind {
                if index < elements.len() {
                    elements[index] = val;
                    return;
                } else if is_extensible {
                    if index == elements.len() {
                        elements.push(val);
                        return;
                    } else if index < elements.len() + 1024 {
                        elements.resize(index, JsValue::hole());
                        elements.push(val);
                        return;
                    }
                }
            }
            if let ObjectKind::TypedArray(ta) = &mut jsobj.kind {
                let _ = self.write_typed_array_element_vm(ta, index, val);
                return;
            }
        }
        let key = index.to_string();
        self.set_property(obj, &key, val);
    }

    pub fn has_element_result(&mut self, obj: JsValue, index: usize) -> Result<bool, VmError> {
        if obj.is_sso_string() {
            let len = ((obj.0 >> 40) & 0x7) as usize;
            return Ok(index < len);
        }
        if let Some(ptr) = obj.as_object_ptr() {
            let jsobj = unsafe { &*(ptr as *mut JsObject) };
            if jsobj.accessors.is_some() || matches!(jsobj.kind, ObjectKind::Proxy { .. }) {
                let key = index.to_string();
                return self.has_property(obj, &key);
            }
            if let ObjectKind::Array(elements) = &jsobj.kind {
                if index < elements.len() {
                    if !elements[index].is_hole() {
                        return Ok(true);
                    }
                }
                let key = index.to_string();
                return self.has_property(obj, &key);
            }
            if let ObjectKind::TypedArray(ta) = &jsobj.kind {
                return Ok(index < ta.length);
            }
            if let ObjectKind::String(s) = &jsobj.kind {
                return Ok(index < s.chars().count());
            }
            if let ObjectKind::ConsString { length, .. } = &jsobj.kind {
                return Ok(index < *length);
            }
        }
        let key = index.to_string();
        self.has_property(obj, &key)
    }

    pub fn has_element(&mut self, obj: JsValue, index: usize) -> bool {
        self.has_element_result(obj, index).unwrap_or(false)
    }
}
