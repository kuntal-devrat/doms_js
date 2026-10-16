//! Allocation for Map/Set/WeakMap/WeakSet, iterators, weak references
//! and finalization registries.

use super::*;

impl Vm {
    pub fn alloc_map(&mut self) -> JsValue {
        let proto = self.globals.get("Map").copied().and_then(|c| {
            let p = self.get_property(c, "prototype");
            if p.is_object() { Some(p) } else { None }
        }).or(self.object_proto);
        let obj = JsObject::new_map(ShapeRegistry::ROOT_SHAPE_ID, proto);
        let ptr = self.allocate_js_object(obj);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }

    pub fn alloc_set(&mut self) -> JsValue {
        let proto = self.globals.get("Set").copied().and_then(|c| {
            let p = self.get_property(c, "prototype");
            if p.is_object() { Some(p) } else { None }
        }).or(self.object_proto);
        let obj = JsObject::new_set(ShapeRegistry::ROOT_SHAPE_ID, proto);
        let ptr = self.allocate_js_object(obj);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }

    pub fn alloc_weak_map(&mut self) -> JsValue {
        let proto = self.globals.get("WeakMap").copied().and_then(|c| {
            let p = self.get_property(c, "prototype");
            if p.is_object() { Some(p) } else { None }
        }).or(self.object_proto);
        let obj = JsObject::new_weak_map(ShapeRegistry::ROOT_SHAPE_ID, proto);
        let ptr = self.allocate_js_object(obj);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }

    pub fn alloc_weak_set(&mut self) -> JsValue {
        let proto = self.globals.get("WeakSet").copied().and_then(|c| {
            let p = self.get_property(c, "prototype");
            if p.is_object() { Some(p) } else { None }
        }).or(self.object_proto);
        let obj = JsObject::new_weak_set(ShapeRegistry::ROOT_SHAPE_ID, proto);
        let ptr = self.allocate_js_object(obj);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }
    pub fn alloc_iterator(&mut self, items: Vec<JsValue>) -> JsValue {
        let iter_obj = self.alloc_object();
        let prev_temp = self.temp_roots.len();
        self.temp_roots.push(iter_obj);

        let idx = Arc::new(std::sync::Mutex::new(0usize));
        let items_arc = Arc::new(items);
        let next_fn = self.alloc_closure_native_function("next", 0, Arc::new({
            let idx = Arc::clone(&idx);
            let items = Arc::clone(&items_arc);
            move |vm: &mut Vm, _this: JsValue, _args: &[JsValue]| -> Result<JsValue, VmError> {
                let mut i = idx.lock().unwrap();
                let res = vm.alloc_object();
                if *i < items.len() {
                    let val = items[*i];
                    *i += 1;
                    vm.set_property(res, "value", val);
                    vm.set_property(res, "done", JsValue::from_boolean(false));
                } else {
                    vm.set_property(res, "value", JsValue::undefined());
                    vm.set_property(res, "done", JsValue::from_boolean(true));
                }
                Ok(res)
            }
        }));
        let cur_iter = self.temp_roots[prev_temp];
        self.set_property(cur_iter, "next", next_fn);
        if let Some(sym_iter) = self.symbol_iterator {
            let self_val = self.temp_roots[prev_temp];
            let self_fn = self.alloc_closure_native_function("[Symbol.iterator]", 0, Arc::new(move |_vm, _this, _args| Ok(self_val)));
            let cur_iter = self.temp_roots[prev_temp];
            self.set_symbol_property(cur_iter, sym_iter, self_fn);
        }
        let res = self.temp_roots[prev_temp];
        self.temp_roots.truncate(prev_temp);
        res
    }
    pub fn alloc_weak_ref(&mut self, target: JsValue) -> JsValue {
        let obj = JsObject::new_weak_ref(ShapeRegistry::ROOT_SHAPE_ID, None, target);
        let ptr = self.allocate_js_object(obj);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }
    pub fn alloc_finalization_registry(&mut self, callback: JsValue) -> JsValue {
        let obj = JsObject::new_finalization_registry(ShapeRegistry::ROOT_SHAPE_ID, None, callback);
        let ptr = self.allocate_js_object(obj);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }
}
