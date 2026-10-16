//! Allocation for JS functions and native functions.

use super::*;

impl Vm {
    pub fn alloc_native_function(&mut self, name: &str, length: u8, func: NativeFn) -> JsValue {
        let native = NativeFunction::new_fn(name.to_string(), length, func);
        let proto = self.function_proto.or(self.object_proto);
        let obj = Box::new(JsObject::new_native_function(ShapeRegistry::ROOT_SHAPE_ID, proto, native));
        let ptr = Box::into_raw(obj);
        self.track_ptr(ptr);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }

    pub fn alloc_closure_native_function(&mut self, name: &str, length: u8, func: crate::object::ClosureNativeFn) -> JsValue {
        let native = NativeFunction::new_closure(name.to_string(), length, func);
        let proto = self.function_proto.or(self.object_proto);
        let obj = Box::new(JsObject::new_native_function(ShapeRegistry::ROOT_SHAPE_ID, proto, native));
        let ptr = Box::into_raw(obj);
        self.track_ptr(ptr);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }
    pub fn alloc_function(&mut self, func: JsFunction) -> JsValue {
        let fn_proto = self.function_proto.or(self.object_proto);
        let len = func.params.iter().take_while(|p| !p.starts_with("...")).count();
        let name_str = func.name.as_deref().unwrap_or("").to_string();
        let obj = Box::new(JsObject::new_function(ShapeRegistry::ROOT_SHAPE_ID, fn_proto, func));
        let ptr = Box::into_raw(obj);
        self.track_ptr(ptr);
        let fn_val = unsafe { JsValue::from_object_ptr(ptr as *mut u8) };

        let len_val = JsValue::from_int32(len as i32);
        self.set_own_property(fn_val, "length", len_val);
        self.non_enumerable_props.insert((fn_val, "length".to_string()));

        let name_val = self.alloc_string(name_str);
        self.set_own_property(fn_val, "name", name_val);
        self.non_enumerable_props.insert((fn_val, "name".to_string()));

        let proto = self.alloc_object();
        self.set_own_property(proto, "constructor", fn_val);
        self.non_enumerable_props.insert((proto, "constructor".to_string()));
        self.set_own_property(fn_val, "prototype", proto);
        self.non_enumerable_props.insert((fn_val, "prototype".to_string()));

        fn_val
    }
}
