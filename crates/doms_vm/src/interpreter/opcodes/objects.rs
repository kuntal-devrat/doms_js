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
            if !jsobj.has_element_attributes() && jsobj.is_extensible() {
                if let ObjectKind::Array(elements) = &mut jsobj.kind {
                    elements.push(val);
                    return Ok(());
                }
            }
            let len = self.get_property(arr_val, "length").to_number().unwrap_or(0.0) as usize;
            self.set_property(arr_val, &len.to_string(), val);
            self.set_property(arr_val, "length", JsValue::from_int32((len + 1) as i32));
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
            if !jsobj.has_element_attributes() && jsobj.is_extensible() {
                if let ObjectKind::Array(elements) = &mut jsobj.kind {
                    elements.extend(items);
                    return Ok(());
                }
            }
            let mut len = self.get_property(arr_val, "length").to_number().unwrap_or(0.0) as usize;
            for item in items {
                self.set_property(arr_val, &len.to_string(), item);
                len += 1;
            }
            self.set_property(arr_val, "length", JsValue::from_int32(len as i32));
        }
        Ok(())
    }

    pub(crate) fn op_require_object_coercible(
        &mut self,
        target_depth: usize,
    ) -> Result<(), VmError> {
        let val = *self.peek()?;
        if val.is_null() || val.is_undefined() {
            self.handle_exception_at_depth(
                VmError::TypeError("Cannot destructure 'undefined' or 'null'".into()),
                target_depth,
            )?;
        }
        Ok(())
    }

    pub(crate) fn op_create_regexp(
        &mut self,
        p_idx: u16,
        f_idx: u16,
        chunk: &BytecodeChunk,
    ) -> Result<(), VmError> {
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
                    let val = self.get_property(obj_val, &name);
                    if val == JsValue::uninitialized() {
                        self.handle_exception_at_depth(
                            VmError::ReferenceError(format!(
                                "Cannot access '{}' before initialization",
                                name
                            )),
                            target_depth,
                        )?;
                        return Ok(Some(JsValue::undefined()));
                    }
                }
            }
        }
        let keys_to_alloc = match self.get_for_in_keys(obj_val) {
            Ok(k) => k,
            Err(err) => {
                self.handle_exception_at_depth(err, target_depth)?;
                return Ok(Some(JsValue::undefined()));
            }
        };
        let keys_vec: Vec<JsValue> =
            keys_to_alloc.into_iter().map(|k| self.alloc_string(k)).collect();
        let arr = self.alloc_array(keys_vec);
        self.push(arr);
        Ok(None)
    }

    pub(crate) fn op_get_iterator(
        &mut self,
        target_depth: usize,
    ) -> Result<Option<JsValue>, VmError> {
        let iterable = self.pop()?;
        let prev_temp = self.temp_roots.len();
        self.temp_roots.push(iterable);

        let cur_iterable = self.temp_roots[prev_temp];
        if cur_iterable.is_nullish() {
            self.temp_roots.truncate(prev_temp);
            let err = VmError::TypeError("Value is not iterable".into());
            self.handle_exception_at_depth(err, target_depth)?;
            return Ok(None);
        }
        let sym_iter = match self.symbol_iterator {
            Some(s) => s,
            None => {
                self.temp_roots.truncate(prev_temp);
                let err = VmError::TypeError("No Symbol.iterator".into());
                self.handle_exception_at_depth(err, target_depth)?;
                return Ok(None);
            }
        };
        let cur_iterable = self.temp_roots[prev_temp];
        let method = match self.get_symbol_property_result(cur_iterable, sym_iter) {
            Ok(m) => m,
            Err(e) => {
                self.temp_roots.truncate(prev_temp);
                self.handle_exception_at_depth(e, target_depth)?;
                return Ok(None);
            }
        };
        if !self.is_callable(method) {
            self.temp_roots.truncate(prev_temp);
            let err = VmError::TypeError("Value is not iterable".into());
            self.handle_exception_at_depth(err, target_depth)?;
            return Ok(None);
        }
        self.temp_roots.push(method);
        let cur_iterable = self.temp_roots[prev_temp];
        let cur_method = self.temp_roots[prev_temp + 1];
        let iterator = match self.call(cur_method, cur_iterable, &[]) {
            Ok(it) => it,
            Err(e) => {
                self.temp_roots.truncate(prev_temp);
                self.handle_exception_at_depth(e, target_depth)?;
                return Ok(None);
            }
        };
        self.temp_roots.push(iterator);
        let cur_iterator = self.temp_roots[prev_temp + 2];
        if !cur_iterator.is_js_object() {
            self.temp_roots.truncate(prev_temp);
            let err = VmError::TypeError("Iterator is not an object".into());
            self.handle_exception_at_depth(err, target_depth)?;
            return Ok(None);
        }
        let cur_iterator = self.temp_roots[prev_temp + 2];
        let next_fn = match self.get_property_result(cur_iterator, "next") {
            Ok(n) => n,
            Err(e) => {
                self.temp_roots.truncate(prev_temp);
                self.handle_exception_at_depth(e, target_depth)?;
                return Ok(None);
            }
        };
        if !self.is_callable(next_fn) {
            self.temp_roots.truncate(prev_temp);
            let err = VmError::TypeError("Iterator next is not callable".into());
            self.handle_exception_at_depth(err, target_depth)?;
            return Ok(None);
        }
        self.temp_roots.push(next_fn);
        let cur_iterator = self.temp_roots[prev_temp + 2];
        let cur_next = self.temp_roots[prev_temp + 3];
        let rec = JsObject::new_with_kind(
            ShapeRegistry::ROOT_SHAPE_ID,
            None,
            ObjectKind::IteratorRecord {
                iterator: cur_iterator,
                next_method: cur_next,
                done: false,
            },
        );
        let ptr = self.allocate_js_object(rec);
        let rec_val = unsafe { JsValue::from_object_ptr(ptr as *mut u8) };
        self.temp_roots.truncate(prev_temp);
        self.push(rec_val);
        Ok(None)
    }

    pub(crate) fn op_iterator_next(
        &mut self,
        target: i32,
        target_depth: usize,
    ) -> Result<Option<JsValue>, VmError> {
        let iter_rec_val = self.pop()?;
        let (iterator, next_fn) = if let Some(ptr) = iter_rec_val.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::IteratorRecord { iterator, next_method, .. } = obj.kind {
                (iterator, next_method)
            } else {
                let next_fn = match self.get_property_result(iter_rec_val, "next") {
                    Ok(n) => n,
                    Err(e) => {
                        self.handle_exception_at_depth(e, target_depth)?;
                        return Ok(None);
                    }
                };
                (iter_rec_val, next_fn)
            }
        } else {
            let err = VmError::TypeError("Iterator is not an object".into());
            self.handle_exception_at_depth(err, target_depth)?;
            return Ok(None);
        };
        if !self.is_callable(next_fn) {
            let err = VmError::TypeError("Iterator next is not callable".into());
            self.handle_exception_at_depth(err, target_depth)?;
            return Ok(None);
        }
        let prev_temp = self.temp_roots.len();
        self.temp_roots.push(iterator);
        self.temp_roots.push(next_fn);
        let step = match self.call(next_fn, iterator, &[]) {
            Ok(s) => s,
            Err(e) => {
                self.temp_roots.truncate(prev_temp);
                self.handle_exception_at_depth(e, target_depth)?;
                return Ok(None);
            }
        };
        self.temp_roots.push(step);
        if !step.is_js_object() {
            self.temp_roots.truncate(prev_temp);
            let err = VmError::TypeError("Iterator result is not an object".into());
            self.handle_exception_at_depth(err, target_depth)?;
            return Ok(None);
        }
        let done = match self.get_property_result(step, "done") {
            Ok(d) => d,
            Err(e) => {
                self.temp_roots.truncate(prev_temp);
                self.handle_exception_at_depth(e, target_depth)?;
                return Ok(None);
            }
        };
        if done.is_truthy() {
            self.temp_roots.truncate(prev_temp);
            if let Some(frame) = self.frames.last_mut() {
                frame.ip = target as usize;
            }
        } else {
            let cur_step = self.temp_roots[prev_temp + 2];
            let val = match self.get_property_result(cur_step, "value") {
                Ok(v) => v,
                Err(e) => {
                    self.temp_roots.truncate(prev_temp);
                    self.handle_exception_at_depth(e, target_depth)?;
                    return Ok(None);
                }
            };
            self.temp_roots.truncate(prev_temp);
            self.push(val);
        }
        Ok(None)
    }

    pub(crate) fn op_iterator_close(
        &mut self,
        is_abrupt: bool,
        target_depth: usize,
    ) -> Result<Option<JsValue>, VmError> {
        let iter_rec_val = self.pop()?;
        let iterator = if let Some(ptr) = iter_rec_val.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::IteratorRecord { iterator, .. } = obj.kind {
                iterator
            } else {
                iter_rec_val
            }
        } else {
            iter_rec_val
        };
        if !iterator.is_js_object() {
            return Ok(None);
        }
        let ret_method = match self.get_property_result(iterator, "return") {
            Ok(m) => m,
            Err(e) => {
                if is_abrupt {
                    return Ok(None);
                } else {
                    self.handle_exception_at_depth(e, target_depth)?;
                    return Ok(None);
                }
            }
        };
        if ret_method.is_undefined() || ret_method.is_null() {
            return Ok(None);
        }
        if !self.is_callable(ret_method) {
            if is_abrupt {
                return Ok(None);
            } else {
                let err = VmError::TypeError("Iterator return is not callable".into());
                self.handle_exception_at_depth(err, target_depth)?;
                return Ok(None);
            }
        }
        let prev_temp = self.temp_roots.len();
        self.temp_roots.push(iterator);
        self.temp_roots.push(ret_method);
        let inner_result = self.call(ret_method, iterator, &[]);
        self.temp_roots.truncate(prev_temp);
        if is_abrupt {
            return Ok(None);
        }
        match inner_result {
            Ok(inner_val) => {
                if !inner_val.is_js_object() {
                    let err = VmError::TypeError("Iterator return did not return an object".into());
                    self.handle_exception_at_depth(err, target_depth)?;
                    return Ok(None);
                }
            }
            Err(e) => {
                self.handle_exception_at_depth(e, target_depth)?;
                return Ok(None);
            }
        }
        Ok(None)
    }

    pub(crate) fn op_get_async_iterator(
        &mut self,
        target_depth: usize,
    ) -> Result<Option<JsValue>, VmError> {
        let iterable = self.pop()?;
        if iterable.is_nullish() {
            let err = VmError::TypeError("Value is not async iterable".into());
            self.handle_exception_at_depth(err, target_depth)?;
            return Ok(None);
        }
        let mut method = if let Some(sym_async) = self.symbol_async_iterator {
            match self.get_symbol_property_result(iterable, sym_async) {
                Ok(m) if self.is_callable(m) => Some(m),
                _ => None,
            }
        } else {
            None
        };
        if method.is_none() {
            if let Some(sym_iter) = self.symbol_iterator {
                if let Ok(m) = self.get_symbol_property_result(iterable, sym_iter) {
                    if self.is_callable(m) {
                        method = Some(m);
                    }
                }
            }
        }
        let method = match method {
            Some(m) => m,
            None => {
                let err = VmError::TypeError("Value is not async iterable".into());
                self.handle_exception_at_depth(err, target_depth)?;
                return Ok(None);
            }
        };

        let prev_temp = self.temp_roots.len();
        self.temp_roots.push(iterable);
        self.temp_roots.push(method);
        let iterator = match self.call(method, iterable, &[]) {
            Ok(it) => it,
            Err(e) => {
                self.temp_roots.truncate(prev_temp);
                self.handle_exception_at_depth(e, target_depth)?;
                return Ok(None);
            }
        };
        self.temp_roots.truncate(prev_temp);
        if !iterator.is_js_object() {
            let err = VmError::TypeError("Iterator is not an object".into());
            self.handle_exception_at_depth(err, target_depth)?;
            return Ok(None);
        }
        let next_fn = match self.get_property_result(iterator, "next") {
            Ok(n) => n,
            Err(e) => {
                self.handle_exception_at_depth(e, target_depth)?;
                return Ok(None);
            }
        };
        if !self.is_callable(next_fn) {
            let err = VmError::TypeError("Iterator next is not callable".into());
            self.handle_exception_at_depth(err, target_depth)?;
            return Ok(None);
        }
        let rec = JsObject::new_with_kind(
            ShapeRegistry::ROOT_SHAPE_ID,
            None,
            ObjectKind::AsyncIteratorRecord { iterator, next_method: next_fn, done: false },
        );
        let ptr = self.allocate_js_object(rec);
        let rec_val = unsafe { JsValue::from_object_ptr(ptr as *mut u8) };
        self.push(rec_val);
        Ok(None)
    }

    pub(crate) fn op_async_iterator_next(
        &mut self,
        target: i32,
        target_depth: usize,
    ) -> Result<Option<JsValue>, VmError> {
        let iter_rec_val = self.pop()?;
        let (iterator, next_fn) = if let Some(ptr) = iter_rec_val.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::AsyncIteratorRecord { iterator, next_method, .. } = obj.kind {
                (iterator, next_method)
            } else {
                let next_fn = match self.get_property_result(iter_rec_val, "next") {
                    Ok(n) => n,
                    Err(e) => {
                        self.handle_exception_at_depth(e, target_depth)?;
                        return Ok(None);
                    }
                };
                (iter_rec_val, next_fn)
            }
        } else {
            let err = VmError::TypeError("Iterator is not an object".into());
            self.handle_exception_at_depth(err, target_depth)?;
            return Ok(None);
        };
        if !self.is_callable(next_fn) {
            let err = VmError::TypeError("Iterator next is not callable".into());
            self.handle_exception_at_depth(err, target_depth)?;
            return Ok(None);
        }
        let prev_temp = self.temp_roots.len();
        self.temp_roots.push(iterator);
        self.temp_roots.push(next_fn);
        let step_promise = match self.call(next_fn, iterator, &[]) {
            Ok(s) => s,
            Err(e) => {
                self.temp_roots.truncate(prev_temp);
                self.handle_exception_at_depth(e, target_depth)?;
                return Ok(None);
            }
        };
        self.temp_roots.push(step_promise);
        let step = match self.await_value(step_promise) {
            Ok(s) => s,
            Err(e) => {
                self.temp_roots.truncate(prev_temp);
                self.handle_exception_at_depth(e, target_depth)?;
                return Ok(None);
            }
        };
        self.temp_roots.push(step);
        if !step.is_js_object() {
            self.temp_roots.truncate(prev_temp);
            let err = VmError::TypeError("Iterator result is not an object".into());
            self.handle_exception_at_depth(err, target_depth)?;
            return Ok(None);
        }
        let done = match self.get_property_result(step, "done") {
            Ok(d) => d,
            Err(e) => {
                self.temp_roots.truncate(prev_temp);
                self.handle_exception_at_depth(e, target_depth)?;
                return Ok(None);
            }
        };
        if done.is_truthy() {
            self.temp_roots.truncate(prev_temp);
            if let Some(frame) = self.frames.last_mut() {
                frame.ip = target as usize;
            }
        } else {
            let val = match self.get_property_result(step, "value") {
                Ok(v) => v,
                Err(e) => {
                    self.temp_roots.truncate(prev_temp);
                    self.handle_exception_at_depth(e, target_depth)?;
                    return Ok(None);
                }
            };
            self.temp_roots.truncate(prev_temp);
            self.push(val);
        }
        Ok(None)
    }

    pub(crate) fn op_async_iterator_close(
        &mut self,
        is_abrupt: bool,
        target_depth: usize,
    ) -> Result<Option<JsValue>, VmError> {
        let iter_rec_val = self.pop()?;
        let iterator = if let Some(ptr) = iter_rec_val.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::AsyncIteratorRecord { iterator, .. } = obj.kind {
                iterator
            } else {
                iter_rec_val
            }
        } else {
            iter_rec_val
        };
        if !iterator.is_js_object() {
            return Ok(None);
        }
        let ret_method = match self.get_property_result(iterator, "return") {
            Ok(m) => m,
            Err(e) => {
                if is_abrupt {
                    return Ok(None);
                } else {
                    self.handle_exception_at_depth(e, target_depth)?;
                    return Ok(None);
                }
            }
        };
        if ret_method.is_undefined() || ret_method.is_null() {
            return Ok(None);
        }
        if !self.is_callable(ret_method) {
            if is_abrupt {
                return Ok(None);
            } else {
                let err = VmError::TypeError("Iterator return is not callable".into());
                self.handle_exception_at_depth(err, target_depth)?;
                return Ok(None);
            }
        }
        let prev_temp = self.temp_roots.len();
        self.temp_roots.push(iterator);
        self.temp_roots.push(ret_method);
        let ret_promise = match self.call(ret_method, iterator, &[]) {
            Ok(r) => r,
            Err(e) => {
                self.temp_roots.truncate(prev_temp);
                if is_abrupt {
                    return Ok(None);
                } else {
                    self.handle_exception_at_depth(e, target_depth)?;
                    return Ok(None);
                }
            }
        };
        self.temp_roots.push(ret_promise);
        let ret_res = match self.await_value(ret_promise) {
            Ok(r) => r,
            Err(e) => {
                self.temp_roots.truncate(prev_temp);
                if is_abrupt {
                    return Ok(None);
                } else {
                    self.handle_exception_at_depth(e, target_depth)?;
                    return Ok(None);
                }
            }
        };
        self.temp_roots.truncate(prev_temp);
        if !ret_res.is_js_object() && !is_abrupt {
            let err = VmError::TypeError("Iterator return did not return an object".into());
            self.handle_exception_at_depth(err, target_depth)?;
            return Ok(None);
        }
        Ok(None)
    }

    pub(crate) fn op_define_getter(
        &mut self,
        prop_idx: u16,
        chunk: &BytecodeChunk,
    ) -> Result<(), VmError> {
        let name = match &chunk.constants[prop_idx as usize] {
            Constant::String(s) => s.clone(),
            _ => return Err(VmError::TypeError("Property name must be string".into())),
        };
        let getter = self.pop()?;
        let obj = self.pop()?;
        if let Some(val_ptr) = getter.as_object_ptr() {
            let val_obj = unsafe { &mut *(val_ptr as *mut JsObject) };
            if let ObjectKind::Function(ref mut f) = val_obj.kind {
                f.home_object = Some(obj);
            }
        }
        if let Some(ptr) = obj.as_object_ptr() {
            let jsobj = unsafe { &mut *(ptr as *mut JsObject) };
            jsobj.define_getter(name, getter);
        }
        Ok(())
    }

    pub(crate) fn op_define_setter(
        &mut self,
        prop_idx: u16,
        chunk: &BytecodeChunk,
    ) -> Result<(), VmError> {
        let name = match &chunk.constants[prop_idx as usize] {
            Constant::String(s) => s.clone(),
            _ => return Err(VmError::TypeError("Property name must be string".into())),
        };
        let setter = self.pop()?;
        let obj = self.pop()?;
        if let Some(val_ptr) = setter.as_object_ptr() {
            let val_obj = unsafe { &mut *(val_ptr as *mut JsObject) };
            if let ObjectKind::Function(ref mut f) = val_obj.kind {
                f.home_object = Some(obj);
            }
        }
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
        if let Some(val_ptr) = getter_val.as_object_ptr() {
            let val_obj = unsafe { &mut *(val_ptr as *mut JsObject) };
            if let ObjectKind::Function(ref mut f) = val_obj.kind {
                f.home_object = Some(obj_val);
            }
        }
        if let Some(ptr) = obj_val.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            match prop_key {
                PropertyKeyVal::String(s) => {
                    let accs = obj
                        .accessors
                        .get_or_insert_with(|| Box::new(std::collections::HashMap::new()));
                    accs.entry(s).or_default().getter = Some(getter_val);
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
        if let Some(val_ptr) = setter_val.as_object_ptr() {
            let val_obj = unsafe { &mut *(val_ptr as *mut JsObject) };
            if let ObjectKind::Function(ref mut f) = val_obj.kind {
                f.home_object = Some(obj_val);
            }
        }
        if let Some(ptr) = obj_val.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            match prop_key {
                PropertyKeyVal::String(s) => {
                    let accs = obj
                        .accessors
                        .get_or_insert_with(|| Box::new(std::collections::HashMap::new()));
                    accs.entry(s).or_default().setter = Some(setter_val);
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
