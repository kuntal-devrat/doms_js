//! Property and element reads/writes (`GetProp`, `SetProp`, `GetElem`, `SetElem`).

use super::*;
use crate::object::PrivateElement;

impl Vm {

    pub(crate) fn op_get_prop(
        &mut self,
        name_idx: u16,
        chunk: &BytecodeChunk,
        target_depth: usize,
    ) -> Result<(), VmError> {
        let name = match &chunk.constants[name_idx as usize] {
            Constant::String(s) => s.as_str(),
            _ => return Err(VmError::TypeError("Property name must be string".into())),
        };
        let obj = self.pop()?;

        if name.starts_with('#') {
            if !obj.is_js_object() {
                let err = VmError::TypeError(format!(
                    "Cannot read private member {} from non-object",
                    name
                ));
                self.handle_exception_at_depth(err, target_depth)?;
                return Ok(());
            }
            if let Some(ptr) = obj.as_object_ptr() {
                let jsobj = unsafe { &*(ptr as *mut JsObject) };
                if let Some(elem) = jsobj.get_private_element(&name).cloned() {
                    match elem {
                        PrivateElement::Value(v) => {
                            self.push(v);
                            return Ok(());
                        }
                        PrivateElement::Accessor(acc) => {
                            if let Some(getter) = acc.getter {
                                let val = self.call(getter, obj, &[]).unwrap_or(JsValue::undefined());
                                self.push(val);
                                return Ok(());
                            } else {
                                let err = VmError::TypeError(format!(
                                    "Cannot read private member {} which has no getter",
                                    name
                                ));
                                self.handle_exception_at_depth(err, target_depth)?;
                                return Ok(());
                            }
                        }
                    }
                }
                let err = VmError::TypeError(format!(
                    "Cannot read private member {} from an object whose class did not declare it",
                    name
                ));
                self.handle_exception_at_depth(err, target_depth)?;
                return Ok(());
            } else {
                let err = VmError::TypeError(format!(
                    "Cannot read private member {} from non-object",
                    name
                ));
                self.handle_exception_at_depth(err, target_depth)?;
                return Ok(());
            }
        }

        let chunk_key = chunk.id;
        let call_site_ip = self.frames.last().map(|f| f.ip.saturating_sub(1)).unwrap_or(0);

        if let Some(ptr) = obj.as_object_ptr() {
            let jsobj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::Array(elements) = &jsobj.kind {
                if name == "length" {
                    chunk.record_feedback_array_length(call_site_ip);
                    let val = if let Some(info) = self.shapes.get_property_info(jsobj.shape_id, "length") {
                        if let Some(v) = jsobj.get_slot(info.offset) {
                            if let Some(n) = v.to_number() {
                                if (n as usize) >= elements.len() {
                                    Some(v)
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    };
                    let val = val.unwrap_or_else(|| {
                        let len = elements.len();
                        if len <= i32::MAX as usize {
                            JsValue::from_int32(len as i32)
                        } else {
                            JsValue::from_double(len as f64)
                        }
                    });
                    self.push(val);
                    return Ok(());
                }
            }
            let has_acc = jsobj.accessors.as_ref().map_or(false, |a| a.contains_key(name));
            if !has_acc && !self.module_namespace_objects.contains(&obj) {
                if let Some(offset) = self.chunk_ics.get(&chunk_key).and_then(|ics| {
                    ics.get(call_site_ip).and_then(|slot| slot.lookup(name_idx, jsobj.shape_id))
                }) {
                    if let Some(val) = jsobj.get_slot(offset) {
                        self.push(val);
                        return Ok(());
                    }
                }
            }
        }

        self.temp_roots.push(obj);
        let val_res = self.get_property_result(self.temp_roots.last().copied().unwrap(), name);
        let rooted_obj = self.temp_roots.pop().unwrap();
        let val = match val_res {
            Ok(v) => v,
            Err(err) => {
                self.handle_exception_at_depth(err, target_depth)?;
                return Ok(());
            }
        };

        if let Some(ptr) = rooted_obj.as_object_ptr() {
            let jsobj = unsafe { &*(ptr as *mut JsObject) };
            if matches!(jsobj.kind, ObjectKind::Array(_)) {
                if name == "length" {
                    chunk.record_feedback_array_length(call_site_ip);
                } else if name == "push" {
                    chunk.record_feedback_array_push(call_site_ip);
                }
            } else {
                let has_acc = jsobj.accessors.as_ref().map_or(false, |a| a.contains_key(name));
                if !has_acc {
                    if let Some(offset) = self.shapes.lookup_offset(jsobj.shape_id, name) {
                        let ics = self.chunk_ics.entry(chunk_key).or_insert_with(|| {
                            vec![InlineCacheSlot::new(); chunk.instructions.len()]
                        });
                        if call_site_ip < ics.len() {
                            ics[call_site_ip].record_hit(name_idx, jsobj.shape_id, offset);
                        }
                        chunk.record_feedback_shape(call_site_ip, jsobj.shape_id, offset);
                    }
                }
            }
        }

        self.push(val);
        Ok(())
    }

    pub(crate) fn op_set_prop(
        &mut self,
        name_idx: u16,
        chunk: &BytecodeChunk,
        target_depth: usize,
    ) -> Result<(), VmError> {
        let name = match &chunk.constants[name_idx as usize] {
            Constant::String(s) => s.as_str(),
            _ => return Err(VmError::TypeError("Property name must be string".into())),
        };
        let val = self.pop()?;
        let obj = self.pop()?;

        if name.starts_with('#') {
            if !obj.is_js_object() {
                let err = VmError::TypeError(format!(
                    "Cannot write private member {} to non-object",
                    name
                ));
                self.handle_exception_at_depth(err, target_depth)?;
                return Ok(());
            }
            if let Some(ptr) = obj.as_object_ptr() {
                let jsobj = unsafe { &mut *(ptr as *mut JsObject) };
                if let Some(elem) = jsobj.get_private_element(name).cloned() {
                    match elem {
                        PrivateElement::Value(_) => {
                            jsobj.insert_private_element(name.to_string(), val);
                            self.push(val);
                            return Ok(());
                        }
                        PrivateElement::Accessor(acc) => {
                            if let Some(setter) = acc.setter {
                                if let Err(err) = self.call(setter, obj, &[val]) {
                                    self.handle_exception_at_depth(err, target_depth)?;
                                    return Ok(());
                                }
                                self.push(val);
                                return Ok(());
                            } else {
                                let err = VmError::TypeError(format!(
                                    "Cannot write private member {} which has no setter",
                                    name
                                ));
                                self.handle_exception_at_depth(err, target_depth)?;
                                return Ok(());
                            }
                        }
                    }
                }
                let err = VmError::TypeError(format!(
                    "Cannot write private member {} to an object whose class did not declare it",
                    name
                ));
                self.handle_exception_at_depth(err, target_depth)?;
                return Ok(());
            } else {
                let err = VmError::TypeError(format!(
                    "Cannot write private member {} to non-object",
                    name
                ));
                self.handle_exception_at_depth(err, target_depth)?;
                return Ok(());
            }
        }

        let this_val = self.frames.last().map(|f| f.this_val).unwrap_or(JsValue::undefined());
        if self.module_namespace_objects.contains(&this_val) {
            let prop_val = self.get_property(this_val, &name);
            if prop_val == JsValue::uninitialized() {
                self.handle_exception_at_depth(
                    VmError::ReferenceError(format!("Cannot access '{}' before initialization", name)),
                    target_depth,
                )?;
                return Ok(());
            }
        }

        if !self.is_own_property_writable(obj, &name) {
            let is_strict = self.frames.last().map(|f| f.is_strict).unwrap_or(self.is_strict);
            if is_strict {
                self.handle_exception_at_depth(
                    VmError::TypeError(format!("Cannot assign to read only property '{}'", name)),
                    target_depth,
                )?;
                return Ok(());
            }
            self.push(val);
            return Ok(());
        }

        if let Some(val_ptr) = val.as_object_ptr() {
            let val_obj = unsafe { &mut *(val_ptr as *mut JsObject) };
            if let ObjectKind::Function(ref mut f) = val_obj.kind {
                if f.is_method && f.home_object.is_none() {
                    f.home_object = Some(obj);
                }
            }
        }

        let chunk_key = chunk.id;
        let call_site_ip = self.frames.last().map(|f| f.ip.saturating_sub(1)).unwrap_or(0);

        if let Some(ptr) = obj.as_object_ptr() {
            let jsobj = unsafe { &mut *(ptr as *mut JsObject) };
            let has_acc = jsobj.accessors.as_ref().map_or(false, |a| a.contains_key(name));
            if !has_acc {
                if let Some(offset) = self.chunk_ics.get(&chunk_key).and_then(|ics| {
                    ics.get(call_site_ip).and_then(|slot| slot.lookup(name_idx, jsobj.shape_id))
                }) {
                    jsobj.set_slot(offset, val);
                    self.push(val);
                    return Ok(());
                }
            }
        }

        self.temp_roots.push(obj);
        self.temp_roots.push(val);
        let res = self.set_property_result(self.temp_roots[self.temp_roots.len() - 2], name, self.temp_roots[self.temp_roots.len() - 1]);
        let _rooted_val = self.temp_roots.pop().unwrap();
        let rooted_obj = self.temp_roots.pop().unwrap();
        if let Err(err) = res {
            self.handle_exception_at_depth(err, target_depth)?;
            return Ok(());
        }

        if let Some(ptr) = rooted_obj.as_object_ptr() {
            let jsobj = unsafe { &*(ptr as *mut JsObject) };
            let has_acc = jsobj.accessors.as_ref().map_or(false, |a| a.contains_key(name));
            if !has_acc {
                if let Some(offset) = self.shapes.lookup_offset(jsobj.shape_id, name) {
                    let ics = self.chunk_ics.entry(chunk_key).or_insert_with(|| {
                        vec![InlineCacheSlot::new(); chunk.instructions.len()]
                    });
                    if call_site_ip < ics.len() {
                        ics[call_site_ip].record_hit(name_idx, jsobj.shape_id, offset);
                    }
                    chunk.record_feedback_shape(call_site_ip, jsobj.shape_id, offset);
                }
            }
        }

        self.push(val);
        Ok(())
    }

#[inline(always)]
fn to_canonical_array_index(key: JsValue) -> Option<usize> {
    if key.is_int32() {
        let i = key.as_int32().unwrap();
        if i >= 0 {
            return Some(i as usize);
        }
    } else if let Some(d) = key.as_double() {
        if !d.is_nan() && !d.is_infinite() && d >= 0.0 && d.fract() == 0.0 && d < 4294967295.0 {
            return Some(d as usize);
        }
    }
    None
}

    pub(crate) fn op_get_elem(
        &mut self,
        target_depth: usize,
    ) -> Result<(), VmError> {
                    let key = self.pop()?;
                    let obj = self.pop()?;
                    if let Some(idx) = Self::to_canonical_array_index(key) {
                        if obj.is_sso_string() {
                            let len = ((obj.0 >> 40) & 0x7) as usize;
                            if idx < len {
                                let payload = obj.0 & 0x0000_00FF_FFFF_FFFF;
                                let byte = ((payload >> (idx * 8)) & 0xFF) as u8;
                                let ch_str = self.alloc_string((byte as char).to_string());
                                self.push(ch_str);
                                return Ok(());
                            }
                        }
                        if let Some(ptr) = obj.as_object_ptr() {
                            let jsobj = unsafe { &*(ptr as *mut JsObject) };
                            if let ObjectKind::Array(elements) = &jsobj.kind {
                                if !jsobj.has_element_attributes() && idx < elements.len() {
                                    let el = elements[idx];
                                    if !el.is_hole() {
                                        self.push(el);
                                        return Ok(());
                                    }
                                }
                            }
                            if let ObjectKind::String(s) = &jsobj.kind {
                                if let Some(ch) = s.chars().nth(idx) {
                                    let ch_str = self.alloc_string(ch.to_string());
                                    self.push(ch_str);
                                    return Ok(());
                                }
                            }
                            if let ObjectKind::ConsString { length, .. } = &jsobj.kind {
                                if idx < *length {
                                    let s = self.to_string_value(obj);
                                    if let Some(ch) = s.chars().nth(idx) {
                                        let ch_str = self.alloc_string(ch.to_string());
                                        self.push(ch_str);
                                        return Ok(());
                                    }
                                }
                            }
                            if let ObjectKind::TypedArray(ta) = &jsobj.kind {
                                let ta = ta.clone();
                                if let Some(val) = self.read_typed_array_element_vm(&ta, idx) {
                                    self.push(val);
                                    return Ok(());
                                }
                            }
                        }
                    }
                    let prop_key = match self.to_property_key(key) {
                        Ok(k) => k,
                        Err(err) => {
                            self.handle_exception_at_depth(err, target_depth)?;
                            return Ok(());
                        }
                    };
                    let val = match prop_key {
                        PropertyKeyVal::Symbol(sym) => {
                            match self.get_symbol_property_result(obj, sym) {
                                Ok(v) => v,
                                Err(err) => {
                                    self.handle_exception_at_depth(err, target_depth)?;
                                    return Ok(());
                                }
                            }
                        }
                        PropertyKeyVal::String(key_str) => {
                            match self.get_property_result(obj, &key_str) {
                                Ok(v) => v,
                                Err(err) => {
                                    self.handle_exception_at_depth(err, target_depth)?;
                                    return Ok(());
                                }
                            }
                        }
                    };
                    self.push(val);
        Ok(())
    }

    pub(crate) fn op_set_elem(
        &mut self,
        target_depth: usize,
    ) -> Result<(), VmError> {
                    let val = self.pop()?;
                    let key = self.pop()?;
                    let obj = self.pop()?;
                    if let Some(idx) = Self::to_canonical_array_index(key) {
                        if let Some(ptr) = obj.as_object_ptr() {
                            let jsobj = unsafe { &mut *(ptr as *mut JsObject) };
                            let is_extensible = jsobj.is_extensible();
                            let has_elem_attrs = jsobj.has_element_attributes();
                            if let ObjectKind::Array(elements) = &mut jsobj.kind {
                                let is_existing = idx < elements.len() && !elements[idx].is_hole();
                                let is_standard_proto = jsobj.prototype.is_none() || jsobj.prototype == self.array_proto;
                                if !has_elem_attrs && (is_existing || is_standard_proto) && idx < 65536 && idx <= elements.len() + 1024 && is_extensible {
                                    if idx < elements.len() {
                                        elements[idx] = val;
                                    } else if idx == elements.len() {
                                        elements.push(val);
                                    } else {
                                        elements.resize(idx + 1, JsValue::hole());
                                        elements[idx] = val;
                                    }
                                    self.push(val);
                                    return Ok(());
                                }
                            }
                            if let ObjectKind::TypedArray(ta) = &mut jsobj.kind {
                                match self.write_typed_array_element_vm(ta, idx, val) {
                                    Ok(true) => {
                                        self.push(val);
                                        return Ok(());
                                    }
                                    Err(err) => {
                                        self.handle_exception_at_depth(err, target_depth)?;
                                        return Ok(());
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                    let prop_key = match self.to_property_key(key) {
                        Ok(k) => k,
                        Err(err) => {
                            self.handle_exception_at_depth(err, target_depth)?;
                            return Ok(());
                        }
                    };
                    match prop_key {
                        PropertyKeyVal::Symbol(sym) => {
                            if let Err(err) = self.set_symbol_property_result(obj, sym, val) {
                                self.handle_exception_at_depth(err, target_depth)?;
                                return Ok(());
                            }
                        }
                        PropertyKeyVal::String(key_str) => {
                            if let Err(err) = self.set_property_result(obj, &key_str, val) {
                                self.handle_exception_at_depth(err, target_depth)?;
                                return Ok(());
                            }
                        }
                    }
                    self.push(val);
        Ok(())
    }

    pub(crate) fn op_set_super_prop(
        &mut self,
        name_idx: u16,
        chunk: &BytecodeChunk,
        target_depth: usize,
    ) -> Result<(), VmError> {
        let name = match &chunk.constants[name_idx as usize] {
            Constant::String(s) => s.clone(),
            _ => return Err(VmError::TypeError("Property name must be string".into())),
        };
        let val = self.pop()?;

        let (receiver, base) = {
            let frame = match self.frames.last() {
                Some(f) => f,
                None => {
                    self.push(val);
                    return Ok(());
                }
            };
            if frame.this_val == JsValue::uninitialized() {
                self.handle_exception_at_depth(
                    VmError::ReferenceError("Must call super constructor in derived class before accessing 'this'".into()),
                    target_depth,
                )?;
                return Ok(());
            }
            let receiver = frame.this_val;
            let base = if let Some(home_obj) = frame.home_object {
                if let Some(ptr) = home_obj.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    obj.prototype.unwrap_or(JsValue::null())
                } else {
                    JsValue::null()
                }
            } else if let Some(ptr) = frame.this_val.as_object_ptr() {
                let obj = unsafe { &*(ptr as *mut JsObject) };
                obj.prototype.unwrap_or(JsValue::null())
            } else {
                JsValue::null()
            };
            (receiver, base)
        };

        if base.is_null() || base.is_undefined() {
            let err = VmError::TypeError(format!(
                "Cannot set property '{}' of {}",
                name,
                if base.is_null() { "null" } else { "undefined" }
            ));
            self.handle_exception_at_depth(err, target_depth)?;
            return Ok(());
        }

        self.temp_roots.push(base);
        self.temp_roots.push(receiver);
        self.temp_roots.push(val);
        let res = self.set_property_with_receiver_result(base, &name, val, receiver);
        let rooted_val = self.temp_roots.pop().unwrap();
        let _rooted_receiver = self.temp_roots.pop().unwrap();
        let _rooted_base = self.temp_roots.pop().unwrap();
        if let Err(err) = res {
            self.handle_exception_at_depth(err, target_depth)?;
            return Ok(());
        }

        self.push(rooted_val);
        Ok(())
    }

    pub(crate) fn op_set_super_elem(
        &mut self,
        target_depth: usize,
    ) -> Result<(), VmError> {
        let val = self.pop()?;
        let key = self.pop()?;

        self.temp_roots.push(val);
        let prop_key = match self.to_property_key(key) {
            Ok(k) => k,
            Err(err) => {
                self.temp_roots.pop();
                self.handle_exception_at_depth(err, target_depth)?;
                return Ok(());
            }
        };
        let val = self.temp_roots.pop().unwrap();

        let (receiver, base) = {
            let frame = match self.frames.last() {
                Some(f) => f,
                None => {
                    self.push(val);
                    return Ok(());
                }
            };
            if frame.this_val == JsValue::uninitialized() {
                self.handle_exception_at_depth(
                    VmError::ReferenceError("Must call super constructor in derived class before accessing 'this'".into()),
                    target_depth,
                )?;
                return Ok(());
            }
            let receiver = frame.this_val;
            let base = if let Some(home_obj) = frame.home_object {
                if let Some(ptr) = home_obj.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    obj.prototype.unwrap_or(JsValue::null())
                } else {
                    JsValue::null()
                }
            } else if let Some(ptr) = frame.this_val.as_object_ptr() {
                let obj = unsafe { &*(ptr as *mut JsObject) };
                obj.prototype.unwrap_or(JsValue::null())
            } else {
                JsValue::null()
            };
            (receiver, base)
        };

        if base.is_null() || base.is_undefined() {
            let err = VmError::TypeError(format!(
                "Cannot set property of {}",
                if base.is_null() { "null" } else { "undefined" }
            ));
            self.handle_exception_at_depth(err, target_depth)?;
            return Ok(());
        }

        self.temp_roots.push(base);
        self.temp_roots.push(receiver);
        self.temp_roots.push(val);
        let res = match prop_key {
            PropertyKeyVal::Symbol(sym) => {
                self.set_symbol_property_with_receiver_result(base, sym, val, receiver)
            }
            PropertyKeyVal::String(key_str) => {
                self.set_property_with_receiver_result(base, &key_str, val, receiver)
            }
        };
        let rooted_val = self.temp_roots.pop().unwrap();
        let _rooted_receiver = self.temp_roots.pop().unwrap();
        let _rooted_base = self.temp_roots.pop().unwrap();

        if let Err(err) = res {
            self.handle_exception_at_depth(err, target_depth)?;
            return Ok(());
        }

        self.push(rooted_val);
        Ok(())
    }
}
