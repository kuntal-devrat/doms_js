//! Allocation for plain objects, arrays, strings, wrapper objects,
//! promises, arguments objects and generators.

use super::*;

impl Vm {
    pub fn alloc_object_old_gen(&mut self) -> JsValue {
        let obj = JsObject::new_ordinary(ShapeRegistry::ROOT_SHAPE_ID, self.object_proto);
        let ptr = self.allocate_js_object_old_gen(obj);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }

    pub fn alloc_object(&mut self) -> JsValue {
        let obj = JsObject::new_ordinary(ShapeRegistry::ROOT_SHAPE_ID, self.object_proto);
        let ptr = self.allocate_js_object(obj);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }

    pub fn alloc_object_with_proto(&mut self, proto: Option<JsValue>) -> JsValue {
        let obj = JsObject::new_ordinary(ShapeRegistry::ROOT_SHAPE_ID, proto.or(self.object_proto));
        let ptr = self.allocate_js_object(obj);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }

    pub fn alloc_object_with_exact_proto(&mut self, proto: Option<JsValue>) -> JsValue {
        let obj = JsObject::new_ordinary(ShapeRegistry::ROOT_SHAPE_ID, proto);
        let ptr = self.allocate_js_object(obj);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }

    pub fn alloc_iter_result(&mut self, value: JsValue, done: bool) -> Result<JsValue, VmError> {
        let prev = self.temp_roots.len();
        self.temp_roots.push(value);
        let res = self.alloc_object();
        self.temp_roots.push(res);
        let val = self.temp_roots[prev];
        let obj = self.temp_roots[prev + 1];
        self.set_property_direct(obj, "value", val);
        self.set_property_direct(obj, "done", JsValue::from_boolean(done));
        self.temp_roots.truncate(prev);
        Ok(obj)
    }

    pub fn alloc_object_with_shape_and_proto(&mut self, shape_id: crate::shape::ShapeId, proto: Option<JsValue>) -> JsValue {
        let mut obj = JsObject::new_ordinary(shape_id, proto.or(self.object_proto));
        let num_props = self.shapes.get(shape_id).map(|s| s.next_offset as usize).unwrap_or(0);
        if num_props > crate::object::IN_OBJECT_SLOTS {
            obj.extra_slots.resize(num_props - crate::object::IN_OBJECT_SLOTS, JsValue::hole());
        }
        let ptr = self.allocate_js_object(obj);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }

    pub fn alloc_array(&mut self, elements: Vec<JsValue>) -> JsValue {
        let proto = self.array_proto.or(self.object_proto);
        let obj = JsObject::new_array(ShapeRegistry::ROOT_SHAPE_ID, proto, elements);
        let ptr = self.allocate_js_object(obj);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }

    pub fn alloc_string(&mut self, s: String) -> JsValue {
        if let Some(sso) = JsValue::try_from_sso_string(&s) {
            return sso;
        }
        let obj = JsObject::new_string(ShapeRegistry::ROOT_SHAPE_ID, self.string_proto, s);
        let ptr = self.allocate_js_object(obj);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }

    pub fn alloc_cons_string(&mut self, left: JsValue, right: JsValue, length: usize) -> JsValue {
        let obj = JsObject::new_cons_string(ShapeRegistry::ROOT_SHAPE_ID, self.string_proto, left, right, length);
        let ptr = self.allocate_js_object(obj);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }

    pub fn alloc_proxy(&mut self, target: JsValue, handler: JsValue) -> JsValue {
        let proto = if let Some(ptr) = target.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            obj.prototype
        } else {
            None
        };
        let obj = JsObject::new_proxy(ShapeRegistry::ROOT_SHAPE_ID, proto, target, handler);
        let ptr = self.allocate_js_object(obj);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }

    pub fn alloc_string_object(&mut self, s: String, proto: Option<JsValue>) -> JsValue {
        let obj = JsObject::new_string_object(ShapeRegistry::ROOT_SHAPE_ID, proto, s);
        let ptr = self.allocate_js_object(obj);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }

    pub fn alloc_number_object(&mut self, n: f64, proto: Option<JsValue>) -> JsValue {
        let obj = JsObject::new_number_object(ShapeRegistry::ROOT_SHAPE_ID, proto, n);
        let ptr = self.allocate_js_object(obj);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }

    pub fn alloc_boolean_object(&mut self, b: bool, proto: Option<JsValue>) -> JsValue {
        let obj = JsObject::new_boolean_object(ShapeRegistry::ROOT_SHAPE_ID, proto, b);
        let ptr = self.allocate_js_object(obj);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }
    pub fn alloc_promise(&mut self, state: PromiseState, result: JsValue) -> JsValue {
        let proto = self.promise_proto.or(self.object_proto);
        let obj = JsObject::new_promise(ShapeRegistry::ROOT_SHAPE_ID, proto, state, result);
        let ptr = self.allocate_js_object(obj);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }
    pub fn alloc_arguments(&mut self, callee: JsValue, args: Vec<JsValue>, is_strict: bool) -> JsValue {
        let prev_temp = self.temp_roots.len();
        self.temp_roots.push(callee);
        let args_start = self.temp_roots.len();
        self.temp_roots.extend_from_slice(&args);
        let args_len = args.len();

        let args_obj = self.alloc_object();
        self.temp_roots.push(args_obj);
        let args_obj_idx = args_start + args_len;

        for i in 0..args_len {
            let val = self.temp_roots[args_start + i];
            self.set_own_property(self.temp_roots[args_obj_idx], &i.to_string(), val);
        }
        self.define_builtin_property(
            self.temp_roots[args_obj_idx],
            "length",
            JsValue::from_int32(args_len as i32),
            crate::shape::PropertyAttributes::BUILTIN,
        );

        if self.symbol_to_string_tag.is_some() {
            let tag_val = self.alloc_string("Arguments".to_string());
            if let Some(sym_tag) = self.symbol_to_string_tag {
                self.set_symbol_property(self.temp_roots[args_obj_idx], sym_tag, tag_val);
            }
        }
        if let Some(arr_proto) = self.array_proto {
            let iter_fn = self.get_property(arr_proto, "values");
            if iter_fn.is_object() {
                if let Some(sym_iter) = self.symbol_iterator {
                    self.set_symbol_property(self.temp_roots[args_obj_idx], sym_iter, iter_fn);
                }
            }
        }

        if !is_strict {
            let cur_callee = self.temp_roots[prev_temp];
            self.define_builtin_property(
                self.temp_roots[args_obj_idx],
                "callee",
                cur_callee,
                crate::shape::PropertyAttributes::BUILTIN,
            );
        } else {
            let thrower = self.alloc_native_function("thrower", 0, |_vm, _this, _args| {
                Err(VmError::TypeError("'arguments.callee' cannot be accessed in strict mode".into()))
            });
            let cur_args = self.temp_roots[args_obj_idx];
            if let Some(ptr) = cur_args.as_object_ptr() {
                let obj = unsafe { &mut *(ptr as *mut JsObject) };
                obj.define_getter("callee".into(), thrower);
                obj.define_setter("callee".into(), thrower);
                if let Some(accs) = &mut obj.accessors {
                    if let Some(acc) = accs.get_mut("callee") {
                        acc.enumerable = false;
                        acc.configurable = false;
                    }
                }
            }
        }
        let res = self.temp_roots[args_obj_idx];
        self.temp_roots.truncate(prev_temp);
        res
    }
    pub fn alloc_generator(&mut self, generator: JsGenerator) -> JsValue {
        let proto = self.generator_proto.or(self.iterator_proto);
        let obj = JsObject::new_generator(ShapeRegistry::ROOT_SHAPE_ID, proto, generator);
        let ptr = self.allocate_js_object(obj);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }
}
