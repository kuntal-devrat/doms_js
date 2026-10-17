//! Basic `Set.prototype` methods.

use super::*;

pub(super) fn values_or_keys(vm: &mut Vm, target: JsValue) -> Option<JsValue> {
    Some(vm.alloc_closure_native_function("values", 0, Arc::new(move |vm, this, _args| {
        let receiver = if this.is_object() { this } else { target };
        Ok(vm.alloc_set_iterator(receiver, SetIterationKind::Value))
    })))
}

pub(super) fn entries(vm: &mut Vm, target: JsValue) -> Option<JsValue> {
    Some(vm.alloc_closure_native_function("entries", 0, Arc::new(move |vm, this, _args| {
        let receiver = if this.is_object() { this } else { target };
        Ok(vm.alloc_set_iterator(receiver, SetIterationKind::KeyValue))
    })))
}

pub(super) fn add(vm: &mut Vm, target: JsValue) -> Option<JsValue> {
    Some(vm.alloc_closure_native_function("add", 1, Arc::new(move |_vm, _this, args| {
                let val = args.first().copied().unwrap_or(JsValue::undefined());
                if let Some(ptr) = target.as_object_ptr() {
                    let obj = unsafe { &mut *(ptr as *mut JsObject) };
                    if let ObjectKind::Set(elements) = &mut obj.kind {
                        if !elements.iter().any(|el| el.as_ref().map_or(false, |e| crate::object::js_values_equal(*e, val))) {
                            elements.push(Some(val));
                        }
                    }
                }
                Ok(target)
            })))
}

pub(super) fn has(vm: &mut Vm, target: JsValue) -> Option<JsValue> {
    Some(vm.alloc_closure_native_function("has", 1, Arc::new(move |_vm, _this, args| {
                let val = args.first().copied().unwrap_or(JsValue::undefined());
                if let Some(ptr) = target.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    if let ObjectKind::Set(elements) = &obj.kind {
                        return Ok(JsValue::from_boolean(elements.iter().any(|el| el.as_ref().map_or(false, |e| crate::object::js_values_equal(*e, val)))));
                    }
                }
                Ok(JsValue::from_boolean(false))
            })))
}

pub(super) fn delete(vm: &mut Vm, target: JsValue) -> Option<JsValue> {
    Some(vm.alloc_closure_native_function("delete", 1, Arc::new(move |_vm, _this, args| {
                let val = args.first().copied().unwrap_or(JsValue::undefined());
                if let Some(ptr) = target.as_object_ptr() {
                    let obj = unsafe { &mut *(ptr as *mut JsObject) };
                    if let ObjectKind::Set(elements) = &mut obj.kind {
                        for el in elements.iter_mut() {
                            if let Some(e) = el {
                                if crate::object::js_values_equal(*e, val) {
                                    *el = None;
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
                    if let ObjectKind::Set(elements) = &mut obj.kind {
                        for el in elements.iter_mut() {
                            *el = None;
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
                    let el = if let Some(ptr) = target.as_object_ptr() {
                        let obj = unsafe { &*(ptr as *mut JsObject) };
                        if let ObjectKind::Set(elements) = &obj.kind {
                            if idx < elements.len() {
                                elements[idx]
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
                    if let Some(el) = el {
                        let _ = vm.call(cb, this_arg, &[el, el, target])?;
                    }
                }
                Ok(JsValue::undefined())
            })))
}
