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
                let prev = vm.temp_roots.len();
                let res = vm.alloc_object();
                vm.temp_roots.push(res);
                if *i < items.len() {
                    let val = items[*i];
                    *i += 1;
                    vm.temp_roots.push(val);
                    let r = vm.temp_roots[prev];
                    let v = vm.temp_roots[prev + 1];
                    vm.set_property(r, "value", v);
                    vm.set_property(r, "done", JsValue::from_boolean(false));
                } else {
                    let r = vm.temp_roots[prev];
                    vm.set_property(r, "value", JsValue::undefined());
                    vm.set_property(r, "done", JsValue::from_boolean(true));
                }
                let out = vm.temp_roots[prev];
                vm.temp_roots.truncate(prev);
                Ok(out)
            }
        }));
        let cur_iter = self.temp_roots[prev_temp];
        self.set_property(cur_iter, "next", next_fn);
        if let Some(sym_iter) = self.symbol_iterator {
            let self_fn = self.alloc_native_function("[Symbol.iterator]", 0, |_vm, this, _args| Ok(this));
            let cur_iter = self.temp_roots[prev_temp];
            self.set_symbol_property(cur_iter, sym_iter, self_fn);
        }
        let res = self.temp_roots[prev_temp];
        self.temp_roots.truncate(prev_temp);
        res
    }

    pub fn alloc_map_iterator(&mut self, map: JsValue, kind: MapIterationKind) -> JsValue {
        let iter_obj = self.alloc_object();
        let prev_temp = self.temp_roots.len();
        self.temp_roots.push(iter_obj);
        if let Some(ptr) = iter_obj.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            obj.kind = ObjectKind::MapIterator {
                map,
                index: 0,
                kind,
            };
        }
        let next_fn = self.alloc_native_function("next", 0, |vm, this, _args| {
            if let Some(ptr) = this.as_object_ptr() {
                let obj = unsafe { &mut *(ptr as *mut JsObject) };
                if let ObjectKind::MapIterator { map, index, kind } = &mut obj.kind {
                    let map_val = *map;
                    let cur_kind = *kind;
                    if let Some(map_ptr) = map_val.as_object_ptr() {
                        let map_obj = unsafe { &*(map_ptr as *mut JsObject) };
                        if let ObjectKind::Map(entries) = &map_obj.kind {
                            while *index < entries.len() {
                                let entry = entries[*index];
                                *index += 1;
                                if let Some((k, v)) = entry {
                                    let result_val = match cur_kind {
                                        MapIterationKind::Key => k,
                                        MapIterationKind::Value => v,
                                        MapIterationKind::KeyValue => vm.alloc_array(vec![k, v]),
                                    };
                                    let res = vm.alloc_object();
                                    vm.set_property(res, "value", result_val);
                                    vm.set_property(res, "done", JsValue::from_boolean(false));
                                    return Ok(res);
                                }
                            }
                        }
                    }
                    let res = vm.alloc_object();
                    vm.set_property(res, "value", JsValue::undefined());
                    vm.set_property(res, "done", JsValue::from_boolean(true));
                    return Ok(res);
                }
            }
            Err(VmError::TypeError("Method %MapIteratorPrototype%.next called on incompatible receiver".into()))
        });
        let cur_iter = self.temp_roots[prev_temp];
        self.set_property(cur_iter, "next", next_fn);
        if let Some(sym_iter) = self.symbol_iterator {
            let self_fn = self.alloc_native_function("[Symbol.iterator]", 0, |_vm, this, _args| Ok(this));
            let cur_iter = self.temp_roots[prev_temp];
            self.set_symbol_property(cur_iter, sym_iter, self_fn);
        }
        if let Some(sym_tag) = self.symbol_to_string_tag {
            let tag_str = self.alloc_string("Map Iterator".into());
            let cur_iter = self.temp_roots[prev_temp];
            self.set_symbol_property(cur_iter, sym_tag, tag_str);
        }
        let res = self.temp_roots[prev_temp];
        self.temp_roots.truncate(prev_temp);
        res
    }

    pub fn alloc_set_iterator(&mut self, set: JsValue, kind: SetIterationKind) -> JsValue {
        let iter_obj = self.alloc_object();
        let prev_temp = self.temp_roots.len();
        self.temp_roots.push(iter_obj);
        if let Some(ptr) = iter_obj.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            obj.kind = ObjectKind::SetIterator {
                set,
                index: 0,
                kind,
            };
        }
        let next_fn = self.alloc_native_function("next", 0, |vm, this, _args| {
            if let Some(ptr) = this.as_object_ptr() {
                let obj = unsafe { &mut *(ptr as *mut JsObject) };
                if let ObjectKind::SetIterator { set, index, kind } = &mut obj.kind {
                    let set_val = *set;
                    let cur_kind = *kind;
                    if let Some(set_ptr) = set_val.as_object_ptr() {
                        let set_obj = unsafe { &*(set_ptr as *mut JsObject) };
                        if let ObjectKind::Set(elements) = &set_obj.kind {
                            while *index < elements.len() {
                                let el = elements[*index];
                                *index += 1;
                                if let Some(v) = el {
                                    let result_val = match cur_kind {
                                        SetIterationKind::Value => v,
                                        SetIterationKind::KeyValue => vm.alloc_array(vec![v, v]),
                                    };
                                    let res = vm.alloc_object();
                                    vm.set_property(res, "value", result_val);
                                    vm.set_property(res, "done", JsValue::from_boolean(false));
                                    return Ok(res);
                                }
                            }
                        }
                    }
                    let res = vm.alloc_object();
                    vm.set_property(res, "value", JsValue::undefined());
                    vm.set_property(res, "done", JsValue::from_boolean(true));
                    return Ok(res);
                }
            }
            Err(VmError::TypeError("Method %SetIteratorPrototype%.next called on incompatible receiver".into()))
        });
        let cur_iter = self.temp_roots[prev_temp];
        self.set_property(cur_iter, "next", next_fn);
        if let Some(sym_iter) = self.symbol_iterator {
            let self_fn = self.alloc_native_function("[Symbol.iterator]", 0, |_vm, this, _args| Ok(this));
            let cur_iter = self.temp_roots[prev_temp];
            self.set_symbol_property(cur_iter, sym_iter, self_fn);
        }
        if let Some(sym_tag) = self.symbol_to_string_tag {
            let tag_str = self.alloc_string("Set Iterator".into());
            let cur_iter = self.temp_roots[prev_temp];
            self.set_symbol_property(cur_iter, sym_tag, tag_str);
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
