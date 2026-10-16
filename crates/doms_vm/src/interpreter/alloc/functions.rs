//! Allocation for JS functions and native functions.

use super::*;

impl Vm {
    pub fn alloc_native_function(&mut self, name: &str, length: u8, func: NativeFn) -> JsValue {
        let native = NativeFunction::new_fn(name.to_string(), length, func);
        let proto = self.function_proto.or(self.object_proto);
        let obj = JsObject::new_native_function(ShapeRegistry::ROOT_SHAPE_ID, proto, native);
        let ptr = self.allocate_js_object(obj);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }

    pub fn alloc_closure_native_function(&mut self, name: &str, length: u8, func: crate::object::ClosureNativeFn) -> JsValue {
        let native = NativeFunction::new_closure(name.to_string(), length, func);
        let proto = self.function_proto.or(self.object_proto);
        let obj = JsObject::new_native_function(ShapeRegistry::ROOT_SHAPE_ID, proto, native);
        let ptr = self.allocate_js_object(obj);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }
    pub fn alloc_function(&mut self, func: JsFunction) -> JsValue {
        let fn_proto = self.function_proto.or(self.object_proto);
        let len = func.params.iter().take_while(|p| !p.starts_with("...")).count();
        let name_str = func.name.as_deref().unwrap_or("").to_string();
        let obj = JsObject::new_function(ShapeRegistry::ROOT_SHAPE_ID, fn_proto, func);
        let ptr = self.allocate_js_object(obj);
        let fn_val = unsafe { JsValue::from_object_ptr(ptr as *mut u8) };

        let prev_temp = self.temp_roots.len();
        self.temp_roots.push(fn_val);

        let len_val = JsValue::from_int32(len as i32);
        self.set_own_property(self.temp_roots[prev_temp], "length", len_val);
        self.non_enumerable_props.insert((self.temp_roots[prev_temp], "length".to_string()));

        let name_val = self.alloc_string(name_str);
        self.set_own_property(self.temp_roots[prev_temp], "name", name_val);
        self.non_enumerable_props.insert((self.temp_roots[prev_temp], "name".to_string()));

        let proto = self.alloc_object();
        let current_fn_val = self.temp_roots[prev_temp];
        self.set_own_property(proto, "constructor", current_fn_val);
        self.non_enumerable_props.insert((proto, "constructor".to_string()));
        self.set_own_property(current_fn_val, "prototype", proto);
        self.non_enumerable_props.insert((current_fn_val, "prototype".to_string()));

        self.temp_roots.pop();
        self.temp_roots.truncate(prev_temp);
        current_fn_val
    }
}
