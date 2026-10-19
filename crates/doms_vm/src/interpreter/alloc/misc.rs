//! Allocation for BigInts, RegExps, Dates and Symbols.

use super::*;

impl Vm {
    pub fn alloc_bigint(&mut self, val: BigInt) -> JsValue {
        let proto = self.globals.get("BigInt").copied().and_then(|c| {
            let p = self.get_property(c, "prototype");
            if p.is_object() { Some(p) } else { None }
        }).or(self.object_proto);
        let obj = JsObject::new_bigint(ShapeRegistry::ROOT_SHAPE_ID, proto, val);
        let ptr = self.allocate_js_object(obj);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }

    pub fn alloc_bigint_object(&mut self, val: BigInt) -> JsValue {
        let proto = self.globals.get("BigInt").copied().and_then(|c| {
            let p = self.get_property(c, "prototype");
            if p.is_object() { Some(p) } else { None }
        }).or(self.object_proto);
        let obj = JsObject::new_with_kind(ShapeRegistry::ROOT_SHAPE_ID, proto, ObjectKind::BigIntObject(val));
        let ptr = self.allocate_js_object(obj);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }
    pub fn alloc_regexp(&mut self, pattern: String, flags: String) -> JsValue {
        let proto = self.regexp_proto.or(self.object_proto);
        let obj = JsObject::new_regexp(ShapeRegistry::ROOT_SHAPE_ID, proto, pattern, flags);
        let ptr = self.allocate_js_object(obj);
        let val = unsafe { JsValue::from_object_ptr(ptr as *mut u8) };
        self.define_builtin_property(
            val,
            "lastIndex",
            JsValue::from_int32(0),
            crate::shape::PropertyAttributes {
                writable: true,
                enumerable: false,
                configurable: false,
            },
        );
        val
    }
    pub fn alloc_date(&mut self, proto: Option<JsValue>, time_ms: f64) -> JsValue {
        let obj = JsObject::new_date(ShapeRegistry::ROOT_SHAPE_ID, proto, time_ms);
        let ptr = self.allocate_js_object(obj);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }
    pub fn alloc_symbol(&mut self, desc: Option<String>) -> JsValue {
        let obj = JsObject::new_symbol(ShapeRegistry::ROOT_SHAPE_ID, self.symbol_proto, desc);
        let ptr = self.allocate_js_object(obj);
        unsafe { JsValue::from_symbol_ptr(ptr as *mut u8) }
    }
}
