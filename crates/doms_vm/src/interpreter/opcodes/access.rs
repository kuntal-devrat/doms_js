//! Property and element reads/writes (`GetProp`, `SetProp`, `GetElem`, `SetElem`).

use super::*;

impl Vm {

    pub(crate) fn op_get_prop(
        &mut self,
        name_idx: u16,
        chunk: &BytecodeChunk,
        target_depth: usize,
    ) -> Result<(), VmError> {
        let name = match &chunk.constants[name_idx as usize] {
            Constant::String(s) => s.clone(),
            _ => return Err(VmError::TypeError("Property name must be string".into())),
        };
        let obj = self.pop()?;

        let chunk_key = chunk.id;
        let call_site_ip = self.frames.last().map(|f| f.ip.saturating_sub(1)).unwrap_or(0);

        if let Some(ptr) = obj.as_object_ptr() {
            let jsobj = unsafe { &*(ptr as *mut JsObject) };
            let has_acc = jsobj.accessors.as_ref().map_or(false, |a| a.contains_key(&name));
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

        let val = match self.get_property_result(obj, &name) {
            Ok(v) => v,
            Err(err) => {
                self.handle_exception_at_depth(err, target_depth)?;
                return Ok(());
            }
        };

        if let Some(ptr) = obj.as_object_ptr() {
            let jsobj = unsafe { &*(ptr as *mut JsObject) };
            let has_acc = jsobj.accessors.as_ref().map_or(false, |a| a.contains_key(&name));
            if !has_acc {
                if let Some(offset) = self.shapes.lookup_offset(jsobj.shape_id, &name) {
                    let ics = self.chunk_ics.entry(chunk_key).or_insert_with(|| {
                        vec![InlineCacheSlot::new(); chunk.instructions.len()]
                    });
                    if call_site_ip < ics.len() {
                        ics[call_site_ip].record_hit(name_idx, jsobj.shape_id, offset);
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
            Constant::String(s) => s.clone(),
            _ => return Err(VmError::TypeError("Property name must be string".into())),
        };
        let val = self.pop()?;
        let obj = self.pop()?;

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

        if self.non_writable_props.contains(&(obj, name.clone())) {
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

        let chunk_key = chunk.id;
        let call_site_ip = self.frames.last().map(|f| f.ip.saturating_sub(1)).unwrap_or(0);

        if let Some(ptr) = obj.as_object_ptr() {
            let jsobj = unsafe { &mut *(ptr as *mut JsObject) };
            let has_acc = jsobj.accessors.as_ref().map_or(false, |a| a.contains_key(&name));
            if !has_acc && jsobj.prototype.is_none() {
                if let Some(offset) = self.chunk_ics.get(&chunk_key).and_then(|ics| {
                    ics.get(call_site_ip).and_then(|slot| slot.lookup(name_idx, jsobj.shape_id))
                }) {
                    jsobj.set_slot(offset, val);
                    self.push(val);
                    return Ok(());
                }
            }
        }

        if let Err(err) = self.set_property_result(obj, &name, val) {
            self.handle_exception_at_depth(err, target_depth)?;
            return Ok(());
        }

        if let Some(ptr) = obj.as_object_ptr() {
            let jsobj = unsafe { &*(ptr as *mut JsObject) };
            let has_acc = jsobj.accessors.as_ref().map_or(false, |a| a.contains_key(&name));
            if !has_acc && jsobj.prototype.is_none() {
                if let Some(offset) = self.shapes.lookup_offset(jsobj.shape_id, &name) {
                    let ics = self.chunk_ics.entry(chunk_key).or_insert_with(|| {
                        vec![InlineCacheSlot::new(); chunk.instructions.len()]
                    });
                    if call_site_ip < ics.len() {
                        ics[call_site_ip].record_hit(name_idx, jsobj.shape_id, offset);
                    }
                }
            }
        }

        self.push(val);
        Ok(())
    }

    pub(crate) fn op_get_elem(
        &mut self,
        target_depth: usize,
    ) -> Result<(), VmError> {
                    let key = self.pop()?;
                    let obj = self.pop()?;
                    if let Some(ptr) = obj.as_object_ptr() {
                        let jsobj = unsafe { &*(ptr as *mut JsObject) };
                        if let ObjectKind::Array(elements) = &jsobj.kind {
                            if let Some(i) = key.to_int32() {
                                if i >= 0 && (i as usize) < elements.len() {
                                    self.push(elements[i as usize]);
                                    return Ok(());
                                }
                            }
                        }
                        if let ObjectKind::String(s) = &jsobj.kind {
                            if let Some(i) = key.to_int32() {
                                if i >= 0 {
                                    let idx = i as usize;
                                    if let Some(ch) = s.chars().nth(idx) {
                                        let ch_str = self.alloc_string(ch.to_string());
                                        self.push(ch_str);
                                        return Ok(());
                                    }
                                }
                            }
                        }
                        if let ObjectKind::TypedArray(ta) = &jsobj.kind {
                            if let Some(i) = key.to_int32() {
                                if i >= 0 {
                                    if let Some(val) = Self::read_typed_array_element(ta, i as usize) {
                                        self.push(val);
                                        return Ok(());
                                    }
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
                    if let Some(ptr) = obj.as_object_ptr() {
                        let jsobj = unsafe { &mut *(ptr as *mut JsObject) };
                        if let ObjectKind::Array(elements) = &mut jsobj.kind {
                            if let Some(i) = key.to_int32() {
                                if i >= 0 {
                                    let idx = i as usize;
                                    if idx < 65536 && idx <= elements.len() + 1024 {
                                        if idx >= elements.len() {
                                            elements.resize(idx + 1, JsValue::undefined());
                                        }
                                        elements[idx] = val;
                                        self.push(val);
                                        return Ok(());
                                    }
                                }
                            }
                        }
                        if let ObjectKind::TypedArray(ta) = &mut jsobj.kind {
                            if let Some(i) = key.to_int32() {
                                if i >= 0 {
                                    if Self::write_typed_array_element(ta, i as usize, val) {
                                        self.push(val);
                                        return Ok(());
                                    }
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
}
