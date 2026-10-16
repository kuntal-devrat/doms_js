//! Allocation for plain objects, arrays, strings, wrapper objects,
//! promises, arguments objects and generators.

use super::*;

impl Vm {
    pub fn alloc_object(&mut self) -> JsValue {
        let obj = Box::new(JsObject::new_ordinary(ShapeRegistry::ROOT_SHAPE_ID, self.object_proto));
        let ptr = Box::into_raw(obj);
        self.track_ptr(ptr);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }

    pub fn alloc_array(&mut self, elements: Vec<JsValue>) -> JsValue {
        let obj = Box::new(JsObject::new_array(ShapeRegistry::ROOT_SHAPE_ID, self.array_proto.or(self.object_proto), elements));
        let ptr = Box::into_raw(obj);
        self.track_ptr(ptr);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }

    pub fn alloc_string(&mut self, s: String) -> JsValue {
        let obj = Box::new(JsObject::new_string(ShapeRegistry::ROOT_SHAPE_ID, self.string_proto, s));
        let ptr = Box::into_raw(obj);
        self.track_ptr(ptr);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }

    pub fn alloc_proxy(&mut self, target: JsValue, handler: JsValue) -> JsValue {
        let proto = if let Some(ptr) = target.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            obj.prototype
        } else {
            None
        };
        let obj = Box::new(JsObject::new_proxy(ShapeRegistry::ROOT_SHAPE_ID, proto, target, handler));
        let ptr = Box::into_raw(obj);
        self.track_ptr(ptr);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }

    pub fn alloc_string_object(&mut self, s: String, proto: Option<JsValue>) -> JsValue {
        let obj = Box::new(JsObject::new_string_object(ShapeRegistry::ROOT_SHAPE_ID, proto, s));
        let ptr = Box::into_raw(obj);
        self.track_ptr(ptr);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }

    pub fn alloc_number_object(&mut self, n: f64, proto: Option<JsValue>) -> JsValue {
        let obj = Box::new(JsObject::new_number_object(ShapeRegistry::ROOT_SHAPE_ID, proto, n));
        let ptr = Box::into_raw(obj);
        self.track_ptr(ptr);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }

    pub fn alloc_boolean_object(&mut self, b: bool, proto: Option<JsValue>) -> JsValue {
        let obj = Box::new(JsObject::new_boolean_object(ShapeRegistry::ROOT_SHAPE_ID, proto, b));
        let ptr = Box::into_raw(obj);
        self.track_ptr(ptr);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }
    pub fn alloc_promise(&mut self, state: PromiseState, result: JsValue) -> JsValue {
        let proto = self.promise_proto.or(self.object_proto);
        let obj = Box::new(JsObject::new_promise(ShapeRegistry::ROOT_SHAPE_ID, proto, state, result));
        let ptr = Box::into_raw(obj);
        self.track_ptr(ptr);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }
    pub fn alloc_arguments(&mut self, callee: JsValue, args: Vec<JsValue>, is_strict: bool) -> JsValue {
        let args_obj = self.alloc_object();
        let len = args.len();
        for (i, val) in args.into_iter().enumerate() {
            self.set_own_property(args_obj, &i.to_string(), val);
        }
        self.set_own_property(args_obj, "length", JsValue::from_int32(len as i32));
        self.non_enumerable_props.insert((args_obj, "length".to_string()));

        if let Some(sym_tag) = self.symbol_to_string_tag {
            let tag_val = self.alloc_string("Arguments".to_string());
            self.set_symbol_property(args_obj, sym_tag, tag_val);
            self.non_enumerable_props.insert((args_obj, "[Symbol.toStringTag]".to_string()));
        }
        if let Some(sym_iter) = self.symbol_iterator {
            if let Some(arr_proto) = self.array_proto {
                let iter_fn = self.get_property(arr_proto, "values");
                if iter_fn.is_object() {
                    self.set_symbol_property(args_obj, sym_iter, iter_fn);
                }
            }
        }

        if !is_strict {
            self.set_own_property(args_obj, "callee", callee);
            self.non_enumerable_props.insert((args_obj, "callee".to_string()));
        } else {
            let thrower = self.alloc_native_function("thrower", 0, |_vm, _this, _args| {
                Err(VmError::TypeError("'arguments.callee' cannot be accessed in strict mode".into()))
            });
            if let Some(ptr) = args_obj.as_object_ptr() {
                let obj = unsafe { &mut *(ptr as *mut JsObject) };
                obj.define_getter("callee".into(), thrower);
                obj.define_setter("callee".into(), thrower);
            }
            self.non_configurable_props.insert((args_obj, "callee".to_string()));
            self.non_enumerable_props.insert((args_obj, "callee".to_string()));
        }
        args_obj
    }
    pub fn alloc_generator(&mut self, generator: JsGenerator) -> JsValue {
        let obj = Box::new(JsObject::new_generator(ShapeRegistry::ROOT_SHAPE_ID, None, generator));
        let ptr = Box::into_raw(obj);
        self.track_ptr(ptr);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }
}
