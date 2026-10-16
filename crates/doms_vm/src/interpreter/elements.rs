//! Fast paths for indexed element get/set.

use super::*;

impl Vm {
    pub fn get_element(&mut self, obj: JsValue, index: usize) -> JsValue {
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
                if let Some(v) = Self::read_typed_array_element(ta, index) {
                    return v;
                }
            }
        }
        let key = index.to_string();
        self.get_property(obj, &key)
    }

    pub fn get_element_result(&mut self, obj: JsValue, index: usize) -> Result<JsValue, VmError> {
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
                if let Some(v) = Self::read_typed_array_element(ta, index) {
                    return Ok(v);
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
            if let ObjectKind::Array(elements) = &mut jsobj.kind {
                if index < elements.len() {
                    elements[index] = val;
                    return;
                }
            }
            if let ObjectKind::TypedArray(ta) = &mut jsobj.kind {
                Self::write_typed_array_element(ta, index, val);
                return;
            }
        }
        let key = index.to_string();
        self.set_property(obj, &key, val);
    }
}
