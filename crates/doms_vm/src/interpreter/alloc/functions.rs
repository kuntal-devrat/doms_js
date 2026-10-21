//! Allocation for JS functions and native functions.

use super::*;

impl Vm {
    pub fn alloc_native_function(&mut self, name: &str, length: u8, func: NativeFn) -> JsValue {
        let native = NativeFunction::new_fn(name.to_string(), length, func);
        let proto = self.function_proto.or(self.object_proto);
        let obj = JsObject::new_native_function(ShapeRegistry::ROOT_SHAPE_ID, proto, native);
        let ptr = self.allocate_js_object_old_gen(obj);
        let fn_val = unsafe { JsValue::from_object_ptr(ptr as *mut u8) };

        let prev_temp = self.temp_roots.len();
        self.temp_roots.push(fn_val);

        let fn_attrs = crate::shape::PropertyAttributes {
            writable: false,
            enumerable: false,
            configurable: true,
        };

        let len_val = JsValue::from_int32(length as i32);
        self.define_builtin_property(self.temp_roots[prev_temp], "length", len_val, fn_attrs);

        let name_val = self.alloc_string(name.to_string());
        self.define_builtin_property(self.temp_roots[prev_temp], "name", name_val, fn_attrs);

        let res = self.temp_roots[prev_temp];
        self.temp_roots.truncate(prev_temp);
        res
    }

    pub fn alloc_closure_native_function(
        &mut self,
        name: &str,
        length: u8,
        func: crate::object::ClosureNativeFn,
    ) -> JsValue {
        let native = NativeFunction::new_closure(name.to_string(), length, func);
        let proto = self.function_proto.or(self.object_proto);
        let obj = JsObject::new_native_function(ShapeRegistry::ROOT_SHAPE_ID, proto, native);
        let ptr = self.allocate_js_object_old_gen(obj);
        let fn_val = unsafe { JsValue::from_object_ptr(ptr as *mut u8) };

        let prev_temp = self.temp_roots.len();
        self.temp_roots.push(fn_val);

        let fn_attrs = crate::shape::PropertyAttributes {
            writable: false,
            enumerable: false,
            configurable: true,
        };

        let len_val = JsValue::from_int32(length as i32);
        self.define_builtin_property(self.temp_roots[prev_temp], "length", len_val, fn_attrs);

        let name_val = self.alloc_string(name.to_string());
        self.define_builtin_property(self.temp_roots[prev_temp], "name", name_val, fn_attrs);

        let res = self.temp_roots[prev_temp];
        self.temp_roots.truncate(prev_temp);
        res
    }
    pub fn alloc_function(&mut self, func: JsFunction) -> JsValue {
        let fn_proto = self.function_proto.or(self.object_proto);
        let fn_len = func.length;
        let has_prototype = !func.is_arrow && !func.is_method && !func.is_generator && !func.is_async;
        let name_str = func.name.as_deref().unwrap_or("").to_string();
        let obj = JsObject::new_function(ShapeRegistry::ROOT_SHAPE_ID, fn_proto, func);
        let ptr = self.allocate_js_object_old_gen(obj);
        let fn_val = unsafe { JsValue::from_object_ptr(ptr as *mut u8) };

        let prev_temp = self.temp_roots.len();
        self.temp_roots.push(fn_val);

        let fn_attrs = crate::shape::PropertyAttributes {
            writable: false,
            enumerable: false,
            configurable: true,
        };

        let len_val = JsValue::from_int32(fn_len as i32);
        self.define_builtin_property(self.temp_roots[prev_temp], "length", len_val, fn_attrs);

        let name_val = self.alloc_string(name_str);
        let current_fn_val = self.temp_roots[prev_temp];
        self.define_builtin_property(current_fn_val, "name", name_val, fn_attrs);
        if has_prototype {
            let proto = self.alloc_object_old_gen();
            self.define_builtin_property(
                proto,
                "constructor",
                current_fn_val,
                crate::shape::PropertyAttributes {
                    writable: true,
                    enumerable: false,
                    configurable: true,
                },
            );

            self.define_builtin_property(
                current_fn_val,
                "prototype",
                proto,
                crate::shape::PropertyAttributes {
                    writable: true,
                    enumerable: false,
                    configurable: false,
                },
            );
            if let Some(p) = current_fn_val.as_object_ptr() {
                let jsobj = unsafe { &mut *(p as *mut JsObject) };
                if let ObjectKind::Function(ref mut f) = jsobj.kind {
                    f.prototype = Some(proto);
                }
            }
        }

        self.temp_roots.pop();
        self.temp_roots.truncate(prev_temp);
        current_fn_val
    }
}
