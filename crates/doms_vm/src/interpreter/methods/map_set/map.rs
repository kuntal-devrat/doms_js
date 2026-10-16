//! `Map.prototype` method implementations.

use super::*;

pub(super) fn get(vm: &mut Vm, target: JsValue) -> Option<JsValue> {
    Some(vm.alloc_closure_native_function("get", 1, Arc::new(move |_vm, _this, args| {
                let key = args.first().copied().unwrap_or(JsValue::undefined());
                if let Some(ptr) = target.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    if let ObjectKind::Map(entries) = &obj.kind {
                        for (k, v) in entries {
                            if crate::object::js_values_equal(*k, key) {
                                return Ok(*v);
                            }
                        }
                    }
                }
                Ok(JsValue::undefined())
            })))
}

pub(super) fn set(vm: &mut Vm, target: JsValue) -> Option<JsValue> {
    Some(vm.alloc_closure_native_function("set", 2, Arc::new(move |_vm, _this, args| {
                let key = args.first().copied().unwrap_or(JsValue::undefined());
                let val = args.get(1).copied().unwrap_or(JsValue::undefined());
                if let Some(ptr) = target.as_object_ptr() {
                    let obj = unsafe { &mut *(ptr as *mut JsObject) };
                    if let ObjectKind::Map(entries) = &mut obj.kind {
                        for entry in entries.iter_mut() {
                            if crate::object::js_values_equal(entry.0, key) {
                                entry.1 = val;
                                return Ok(target);
                            }
                        }
                        entries.push((key, val));
                    }
                }
                Ok(target)
            })))
}

pub(super) fn has(vm: &mut Vm, target: JsValue) -> Option<JsValue> {
    Some(vm.alloc_closure_native_function("has", 1, Arc::new(move |_vm, _this, args| {
                let key = args.first().copied().unwrap_or(JsValue::undefined());
                if let Some(ptr) = target.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    if let ObjectKind::Map(entries) = &obj.kind {
                        for (k, _) in entries {
                            if crate::object::js_values_equal(*k, key) {
                                return Ok(JsValue::from_boolean(true));
                            }
                        }
                    }
                }
                Ok(JsValue::from_boolean(false))
            })))
}

pub(super) fn delete(vm: &mut Vm, target: JsValue) -> Option<JsValue> {
    Some(vm.alloc_closure_native_function("delete", 1, Arc::new(move |_vm, _this, args| {
                let key = args.first().copied().unwrap_or(JsValue::undefined());
                if let Some(ptr) = target.as_object_ptr() {
                    let obj = unsafe { &mut *(ptr as *mut JsObject) };
                    if let ObjectKind::Map(entries) = &mut obj.kind {
                        if let Some(idx) = entries.iter().position(|(k, _)| crate::object::js_values_equal(*k, key)) {
                            entries.remove(idx);
                            return Ok(JsValue::from_boolean(true));
                        }
                    }
                }
                Ok(JsValue::from_boolean(false))
            })))
}

pub(super) fn clear(vm: &mut Vm, target: JsValue) -> Option<JsValue> {
    Some(vm.alloc_closure_native_function("clear", 0, Arc::new(move |_vm, _this, _args| {
                if let Some(ptr) = target.as_object_ptr() {
                    let obj = unsafe { &mut *(ptr as *mut JsObject) };
                    if let ObjectKind::Map(entries) = &mut obj.kind {
                        entries.clear();
                    }
                }
                Ok(JsValue::undefined())
            })))
}

pub(super) fn for_each(vm: &mut Vm, target: JsValue) -> Option<JsValue> {
    Some(vm.alloc_closure_native_function("forEach", 1, Arc::new(move |vm, _this, args| {
                let cb = args.first().copied().unwrap_or(JsValue::undefined());
                let entries = if let Some(ptr) = target.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    if let ObjectKind::Map(entries) = &obj.kind {
                        entries.clone()
                    } else {
                        Vec::new()
                    }
                } else {
                    Vec::new()
                };
                for (k, v) in entries {
                    let _ = vm.call(cb, target, &[v, k, target]);
                }
                Ok(JsValue::undefined())
            })))
}

pub(super) fn entries(vm: &mut Vm, target: JsValue) -> Option<JsValue> {
    Some(vm.alloc_closure_native_function("entries", 0, Arc::new(move |vm, _this, _args| {
                let entries = if let Some(ptr) = target.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    if let ObjectKind::Map(entries) = &obj.kind {
                        entries.iter().map(|(k, v)| vm.alloc_array(vec![*k, *v])).collect()
                    } else {
                        Vec::new()
                    }
                } else {
                    Vec::new()
                };
                Ok(vm.alloc_iterator(entries))
            })))
}

pub(super) fn keys(vm: &mut Vm, target: JsValue) -> Option<JsValue> {
    Some(vm.alloc_closure_native_function("keys", 0, Arc::new(move |vm, _this, _args| {
                let keys = if let Some(ptr) = target.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    if let ObjectKind::Map(entries) = &obj.kind {
                        entries.iter().map(|(k, _)| *k).collect()
                    } else {
                        Vec::new()
                    }
                } else {
                    Vec::new()
                };
                Ok(vm.alloc_iterator(keys))
            })))
}

pub(super) fn values(vm: &mut Vm, target: JsValue) -> Option<JsValue> {
    Some(vm.alloc_closure_native_function("values", 0, Arc::new(move |vm, _this, _args| {
                let values = if let Some(ptr) = target.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    if let ObjectKind::Map(entries) = &obj.kind {
                        entries.iter().map(|(_, v)| *v).collect()
                    } else {
                        Vec::new()
                    }
                } else {
                    Vec::new()
                };
                Ok(vm.alloc_iterator(values))
            })))
}
