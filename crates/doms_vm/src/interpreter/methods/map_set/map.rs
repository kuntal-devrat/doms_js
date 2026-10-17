//! `Map.prototype` method implementations.

use super::*;

pub(super) fn get(vm: &mut Vm, target: JsValue) -> Option<JsValue> {
    Some(vm.alloc_closure_native_function("get", 1, Arc::new(move |_vm, _this, args| {
                let key = args.first().copied().unwrap_or(JsValue::undefined());
                if let Some(ptr) = target.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    if let ObjectKind::Map(entries) = &obj.kind {
                        for entry in entries {
                            if let Some((k, v)) = entry {
                                if crate::object::js_values_equal(*k, key) {
                                    return Ok(*v);
                                }
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
                            if let Some((k, v)) = entry {
                                if crate::object::js_values_equal(*k, key) {
                                    *v = val;
                                    return Ok(target);
                                }
                            }
                        }
                        entries.push(Some((key, val)));
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
                        for entry in entries {
                            if let Some((k, _)) = entry {
                                if crate::object::js_values_equal(*k, key) {
                                    return Ok(JsValue::from_boolean(true));
                                }
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
                        for entry in entries.iter_mut() {
                            if let Some((k, _)) = entry {
                                if crate::object::js_values_equal(*k, key) {
                                    *entry = None;
                                    return Ok(JsValue::from_boolean(true));
                                }
                            }
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
                        for entry in entries.iter_mut() {
                            *entry = None;
                        }
                    }
                }
                Ok(JsValue::undefined())
            })))
}

pub(super) fn for_each(vm: &mut Vm, target: JsValue) -> Option<JsValue> {
    Some(vm.alloc_closure_native_function("forEach", 1, Arc::new(move |vm, _this, args| {
                let cb = args.first().copied().unwrap_or(JsValue::undefined());
                let this_arg = args.get(1).copied().unwrap_or(JsValue::undefined());
                let mut idx = 0;
                loop {
                    let entry = if let Some(ptr) = target.as_object_ptr() {
                        let obj = unsafe { &*(ptr as *mut JsObject) };
                        if let ObjectKind::Map(entries) = &obj.kind {
                            if idx < entries.len() {
                                entries[idx]
                            } else {
                                break;
                            }
                        } else {
                            break;
                        }
                    } else {
                        break;
                    };
                    idx += 1;
                    if let Some((k, v)) = entry {
                        let _ = vm.call(cb, this_arg, &[v, k, target])?;
                    }
                }
                Ok(JsValue::undefined())
            })))
}

pub(super) fn entries(vm: &mut Vm, target: JsValue) -> Option<JsValue> {
    Some(vm.alloc_closure_native_function("entries", 0, Arc::new(move |vm, this, _args| {
        let receiver = if this.is_object() { this } else { target };
        Ok(vm.alloc_map_iterator(receiver, MapIterationKind::KeyValue))
    })))
}

pub(super) fn keys(vm: &mut Vm, target: JsValue) -> Option<JsValue> {
    Some(vm.alloc_closure_native_function("keys", 0, Arc::new(move |vm, this, _args| {
        let receiver = if this.is_object() { this } else { target };
        Ok(vm.alloc_map_iterator(receiver, MapIterationKind::Key))
    })))
}

pub(super) fn values(vm: &mut Vm, target: JsValue) -> Option<JsValue> {
    Some(vm.alloc_closure_native_function("values", 0, Arc::new(move |vm, this, _args| {
        let receiver = if this.is_object() { this } else { target };
        Ok(vm.alloc_map_iterator(receiver, MapIterationKind::Value))
    })))
}
