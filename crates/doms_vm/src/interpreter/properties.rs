//! Prototype handling and direct property writes.

use super::*;

impl Vm {
    /// Whether `key` is a canonical array index string (`"0"`, `"42"`, …),
    /// excluding `2^32 - 1` which is an ordinary property per spec.
    pub(crate) fn canonical_array_index(key: &str) -> Option<usize> {
        if key.is_empty() || key.len() > 10 {
            return None;
        }
        let idx: u32 = key.parse().ok()?;
        if idx == u32::MAX || idx.to_string() != key {
            return None;
        }
        Some(idx as usize)
    }

    /// Store a sparse array element (an index beyond the dense `elements`
    /// representation) as a shape property and grow `length` to `idx + 1`.
    ///
    /// Marking the array as having element attributes forces the dense fast
    /// paths to fall back to the spec-level property operations.
    fn set_sparse_array_element(&mut self, obj: &mut JsObject, idx: usize, value: JsValue) {
        let key = idx.to_string();
        obj.mark_element_attributes();
        if let Some(offset) = self.shapes.lookup_offset(obj.shape_id, &key) {
            obj.set_slot(offset, value);
        } else {
            let (new_shape_id, offset) = self.shapes.transition(obj.shape_id, &key);
            obj.shape_id = new_shape_id;
            obj.set_slot(offset, value);
            obj.add_prop_order(key);
        }
        self.grow_array_length_slot(obj, idx + 1);
    }

    /// Ensure the `length` slot of an array is at least `len` (never shrinks).
    fn grow_array_length_slot(&mut self, obj: &mut JsObject, len: usize) {
        let dense_len = match &obj.kind {
            ObjectKind::Array(elements) => elements.len(),
            _ => 0,
        };
        let current = self
            .shapes
            .get_property_info(obj.shape_id, "length")
            .and_then(|info| obj.get_slot(info.offset))
            .and_then(|v| v.to_number())
            .map(|n| n as usize)
            .unwrap_or(dense_len);
        if current >= len {
            return;
        }
        let value = JsValue::from_double(len as f64);
        let attrs = crate::shape::PropertyAttributes {
            writable: true,
            enumerable: false,
            configurable: false,
        };
        if let Some(info) = self.shapes.get_property_info(obj.shape_id, "length") {
            let new_shape_id = self.shapes.transition_redefine(obj.shape_id, "length", attrs);
            obj.shape_id = new_shape_id;
            obj.set_slot(info.offset, value);
        } else {
            let (new_shape_id, offset) = self.shapes.transition_with_attributes(obj.shape_id, "length", attrs);
            obj.shape_id = new_shape_id;
            obj.set_slot(offset, value);
            obj.add_prop_order("length".to_string());
        }
    }

    /// Write the (already validated) numeric `length` value of an array.
    fn set_array_length_slot(&mut self, obj: &mut JsObject, num: f64) {
        let attrs = crate::shape::PropertyAttributes {
            writable: obj.is_array_length_writable(),
            enumerable: false,
            configurable: false,
        };
        let value = JsValue::from_double(num);
        if let Some(info) = self.shapes.get_property_info(obj.shape_id, "length") {
            let new_shape_id = self.shapes.transition_redefine(obj.shape_id, "length", attrs);
            obj.shape_id = new_shape_id;
            obj.set_slot(info.offset, value);
        } else {
            let (new_shape_id, offset) = self.shapes.transition_with_attributes(obj.shape_id, "length", attrs);
            obj.shape_id = new_shape_id;
            obj.set_slot(offset, value);
            obj.add_prop_order("length".to_string());
        }
    }

    /// Apply an array `length` assignment: coerce the value, validate it and
    /// truncate or grow the dense storage.
    ///
    /// Returns `false` when the value is not a valid array length (the caller
    /// decides whether that is a `RangeError`).
    fn assign_array_length(&mut self, obj: &mut JsObject, value: JsValue) -> bool {
        if !obj.is_array_length_writable() {
            return false;
        }
        let num = match self.to_number_value(value) {
            Ok(n) => n,
            Err(_) => return false,
        };
        let u32_len = num as u32;
        if (u32_len as f64) != num || num.is_nan() || num.is_sign_negative() {
            return false;
        }
        let new_len = u32_len as usize;
        if let ObjectKind::Array(elements) = &mut obj.kind {
            if new_len < elements.len() {
                elements.truncate(new_len);
            } else if new_len < 100_000 {
                elements.resize(new_len, JsValue::hole());
            }
        }
        self.set_array_length_slot(obj, num);
        true
    }

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

            if matches!(obj.kind, ObjectKind::Array(_)) {
                if key == "length" {
                    self.assign_array_length(obj, value);
                    return;
                }
                if let Some(idx) = Self::canonical_array_index(key) {
                    let dense_len = match &obj.kind {
                        ObjectKind::Array(elements) => elements.len(),
                        _ => 0,
                    };
                    if idx < dense_len {
                        if let ObjectKind::Array(elements) = &mut obj.kind {
                            elements[idx] = value;
                        }
                        return;
                    } else if idx < 100_000 {
                        if let ObjectKind::Array(elements) = &mut obj.kind {
                            elements.resize(idx + 1, JsValue::hole());
                            elements[idx] = value;
                        }
                        return;
                    } else {
                        self.set_sparse_array_element(obj, idx, value);
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

            if matches!(obj.kind, ObjectKind::Array(_)) {
                if key == "length" {
                    self.assign_array_length(obj, value);
                    return;
                }
                if let Some(idx) = Self::canonical_array_index(key) {
                    let dense_len = match &obj.kind {
                        ObjectKind::Array(elements) => elements.len(),
                        _ => 0,
                    };
                    if idx < dense_len {
                        if let ObjectKind::Array(elements) = &mut obj.kind {
                            elements[idx] = value;
                        }
                        return;
                    } else if idx < 100_000 {
                        if let ObjectKind::Array(elements) = &mut obj.kind {
                            elements.resize(idx + 1, JsValue::hole());
                            elements[idx] = value;
                        }
                        return;
                    } else {
                        self.set_sparse_array_element(obj, idx, value);
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

    pub fn create_data_property(&mut self, target: JsValue, key: &str, value: JsValue) -> Result<bool, VmError> {
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
                    return Ok(res.is_truthy());
                } else {
                    return self.create_data_property(tgt, key, value);
                }
            }

            // Check non-configurable: CreateDataProperty requires configurable: true.
            // If property is already non-configurable, redefining it as configurable fails.
            if !self.is_own_property_configurable(target, key) {
                return Ok(false);
            }

            // CreateDataProperty must fail on a non-writable data property or
            // an accessor property rather than silently overwriting it.
            if self.has_own_property(target, key) {
                if !self.is_own_property_writable(target, key) {
                    return Ok(false);
                }
                let is_accessor = matches!(
                    unsafe { &*(ptr as *const JsObject) }.accessors.as_deref(),
                    Some(accs) if accs.contains_key(key)
                );
                if is_accessor {
                    return Ok(false);
                }
            }

            // Check non-extensible
            if !self.is_extensible_object(target) && !self.has_own_property(target, key) {
                return Ok(false);
            }
        }

        self.set_own_property(target, key, value);
        Ok(true)
    }

    pub fn create_data_property_or_throw(&mut self, target: JsValue, key: &str, value: JsValue) -> Result<(), VmError> {
        if !self.create_data_property(target, key, value)? {
            return Err(VmError::TypeError(format!("Cannot create data property '{}'", key)));
        }
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
