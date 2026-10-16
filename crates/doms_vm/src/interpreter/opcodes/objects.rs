//! Array and object creation/manipulation dispatch arms.

use super::*;

impl Vm {
    pub(crate) fn op_create_array(&mut self, count: u16) -> Result<(), VmError> {
        let mut elements = Vec::with_capacity(count as usize);
        for _ in 0..count {
            elements.push(self.pop()?);
        }
        elements.reverse();
        let arr = self.alloc_array(elements);
        self.push(arr);
        Ok(())
    }

    pub(crate) fn op_array_push(&mut self) -> Result<(), VmError> {
        let val = self.pop()?;
        let arr_val = *self.peek()?;
        if let Some(ptr) = arr_val.as_object_ptr() {
            let jsobj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::Array(elements) = &mut jsobj.kind {
                elements.push(val);
            } else {
                let len = self.get_property(arr_val, "length").to_number().unwrap_or(0.0) as usize;
                self.set_property(arr_val, &len.to_string(), val);
                self.set_property(arr_val, "length", JsValue::from_int32((len + 1) as i32));
            }
        }
        Ok(())
    }

    pub(crate) fn op_array_spread_push(&mut self, target_depth: usize) -> Result<(), VmError> {
        let spread_target = self.pop()?;
        let items = match self.to_spread_args(spread_target) {
            Ok(items) => items,
            Err(err) => {
                self.handle_exception_at_depth(err, target_depth)?;
                return Ok(());
            }
        };
        let arr_val = *self.peek()?;
        if let Some(ptr) = arr_val.as_object_ptr() {
            let jsobj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::Array(elements) = &mut jsobj.kind {
                elements.extend(items);
            } else {
                let mut len = self.get_property(arr_val, "length").to_number().unwrap_or(0.0) as usize;
                for item in items {
                    self.set_property(arr_val, &len.to_string(), item);
                    len += 1;
                }
                self.set_property(arr_val, "length", JsValue::from_int32(len as i32));
            }
        }
        Ok(())
    }

    pub(crate) fn op_require_object_coercible(&mut self, target_depth: usize) -> Result<(), VmError> {
        let val = *self.peek()?;
        if val.is_null() || val.is_undefined() {
            self.handle_exception_at_depth(
                VmError::TypeError("Cannot destructure 'undefined' or 'null'".into()),
                target_depth,
            )?;
        }
        Ok(())
    }

    pub(crate) fn op_create_regexp(&mut self, p_idx: u16, f_idx: u16, chunk: &BytecodeChunk) -> Result<(), VmError> {
        let pattern = match &chunk.constants[p_idx as usize] {
            Constant::String(s) => s.clone(),
            _ => String::new(),
        };
        let flags = match &chunk.constants[f_idx as usize] {
            Constant::String(s) => s.clone(),
            _ => String::new(),
        };
        let re = self.alloc_regexp(pattern, flags);
        self.push(re);
        Ok(())
    }

    pub(crate) fn op_get_keys(&mut self, target_depth: usize) -> Result<Option<JsValue>, VmError> {
        let obj_val = self.pop()?;
        if self.module_namespace_objects.contains(&obj_val) {
            if let Some(ptr) = obj_val.as_object_ptr() {
                let obj = unsafe { &*(ptr as *mut JsObject) };
                let names = self.shapes.all_properties(obj.shape_id);
                for name in names {
                    if !self.deleted_props.contains(&(obj_val, name.clone())) {
                        let val = self.get_property(obj_val, &name);
                        if val == JsValue::uninitialized() {
                            self.handle_exception_at_depth(
                                VmError::ReferenceError(format!("Cannot access '{}' before initialization", name)),
                                target_depth,
                            )?;
                            return Ok(Some(JsValue::undefined()));
                        }
                    }
                }
            }
        }
        let keys_to_alloc = self.get_for_in_keys(obj_val);
        let keys_vec: Vec<JsValue> = keys_to_alloc.into_iter().map(|k| self.alloc_string(k)).collect();
        let arr = self.alloc_array(keys_vec);
        self.push(arr);
        Ok(None)
    }

    pub(crate) fn op_define_getter(&mut self, prop_idx: u16, chunk: &BytecodeChunk) -> Result<(), VmError> {
        let name = match &chunk.constants[prop_idx as usize] {
            Constant::String(s) => s.clone(),
            _ => return Err(VmError::TypeError("Property name must be string".into())),
        };
        let getter = self.pop()?;
        let obj = self.pop()?;
        if let Some(ptr) = obj.as_object_ptr() {
            let jsobj = unsafe { &mut *(ptr as *mut JsObject) };
            jsobj.define_getter(name, getter);
        }
        Ok(())
    }

    pub(crate) fn op_define_setter(&mut self, prop_idx: u16, chunk: &BytecodeChunk) -> Result<(), VmError> {
        let name = match &chunk.constants[prop_idx as usize] {
            Constant::String(s) => s.clone(),
            _ => return Err(VmError::TypeError("Property name must be string".into())),
        };
        let setter = self.pop()?;
        let obj = self.pop()?;
        if let Some(ptr) = obj.as_object_ptr() {
            let jsobj = unsafe { &mut *(ptr as *mut JsObject) };
            jsobj.define_setter(name, setter);
        }
        Ok(())
    }

    pub(crate) fn op_define_computed_getter(&mut self, target_depth: usize) -> Result<(), VmError> {
        let getter_val = self.pop()?;
        let key_val = self.pop()?;
        let obj_val = self.pop()?;
        let prop_key = match self.to_property_key(key_val) {
            Ok(k) => k,
            Err(err) => {
                self.handle_exception_at_depth(err, target_depth)?;
                return Ok(());
            }
        };
        if let Some(ptr) = obj_val.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            match prop_key {
                PropertyKeyVal::String(s) => {
                    let accs = obj.accessors.get_or_insert_with(|| Box::new(std::collections::HashMap::new()));
                    accs.entry(s).or_insert(Accessor { getter: None, setter: None }).getter = Some(getter_val);
                }
                PropertyKeyVal::Symbol(sym) => {
                    obj.insert_symbol_accessor_getter(sym, getter_val);
                }
            }
        }
        Ok(())
    }

    pub(crate) fn op_define_computed_setter(&mut self, target_depth: usize) -> Result<(), VmError> {
        let setter_val = self.pop()?;
        let key_val = self.pop()?;
        let obj_val = self.pop()?;
        let prop_key = match self.to_property_key(key_val) {
            Ok(k) => k,
            Err(err) => {
                self.handle_exception_at_depth(err, target_depth)?;
                return Ok(());
            }
        };
        if let Some(ptr) = obj_val.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            match prop_key {
                PropertyKeyVal::String(s) => {
                    let accs = obj.accessors.get_or_insert_with(|| Box::new(std::collections::HashMap::new()));
                    accs.entry(s).or_insert(Accessor { getter: None, setter: None }).setter = Some(setter_val);
                }
                PropertyKeyVal::Symbol(sym) => {
                    obj.insert_symbol_accessor_setter(sym, setter_val);
                }
            }
        }
        Ok(())
    }

    pub(crate) fn op_inherit_class(&mut self) -> Result<(), VmError> {
        let super_ctor = self.pop()?;
        let ctor = *self.stack.last().ok_or(VmError::StackUnderflow)?;

        if let Some(_super_ptr) = super_ctor.as_object_ptr() {
            if let Some(ctor_ptr) = ctor.as_object_ptr() {
                let ctor_obj = unsafe { &mut *(ctor_ptr as *mut JsObject) };
                ctor_obj.prototype = Some(super_ctor);

                let ctor_proto = self.get_property(ctor, "prototype");
                let super_proto = self.get_property(super_ctor, "prototype");
                if let Some(ctor_proto_ptr) = ctor_proto.as_object_ptr() {
                    let ctor_proto_obj = unsafe { &mut *(ctor_proto_ptr as *mut JsObject) };
                    ctor_proto_obj.prototype = Some(super_proto);
                }
            }
        }
        Ok(())
    }
}
