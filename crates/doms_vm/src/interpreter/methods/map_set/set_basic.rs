//! Basic `Set.prototype` methods.

use super::*;

pub(super) fn values_or_keys(vm: &mut Vm, target: JsValue) -> Option<JsValue> {
    Some(vm.alloc_closure_native_function("values", 0, Arc::new(move |vm, _this, _args| {
                let elements = if let Some(ptr) = target.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    if let ObjectKind::Set(elements) = &obj.kind {
                        elements.clone()
                    } else {
                        Vec::new()
                    }
                } else {
                    Vec::new()
                };
                Ok(vm.alloc_iterator(elements))
            })))
}

pub(super) fn entries(vm: &mut Vm, target: JsValue) -> Option<JsValue> {
    Some(vm.alloc_closure_native_function("entries", 0, Arc::new(move |vm, _this, _args| {
                let entries = if let Some(ptr) = target.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    if let ObjectKind::Set(elements) = &obj.kind {
                        elements.iter().map(|el| vm.alloc_array(vec![*el, *el])).collect()
                    } else {
                        Vec::new()
                    }
                } else {
                    Vec::new()
                };
                Ok(vm.alloc_iterator(entries))
            })))
}

pub(super) fn add(vm: &mut Vm, target: JsValue) -> Option<JsValue> {
    Some(vm.alloc_closure_native_function("add", 1, Arc::new(move |_vm, _this, args| {
                let val = args.first().copied().unwrap_or(JsValue::undefined());
                if let Some(ptr) = target.as_object_ptr() {
                    let obj = unsafe { &mut *(ptr as *mut JsObject) };
                    if let ObjectKind::Set(elements) = &mut obj.kind {
                        if !elements.iter().any(|el| crate::object::js_values_equal(*el, val)) {
                            elements.push(val);
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
                        return Ok(JsValue::from_boolean(elements.iter().any(|el| crate::object::js_values_equal(*el, val))));
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
                        if let Some(idx) = elements.iter().position(|el| crate::object::js_values_equal(*el, val)) {
                            elements.remove(idx);
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
                    if let ObjectKind::Set(elements) = &mut obj.kind {
                        elements.clear();
                    }
                }
                Ok(JsValue::undefined())
            })))
}

pub(super) fn for_each(vm: &mut Vm, target: JsValue) -> Option<JsValue> {
    Some(vm.alloc_closure_native_function("forEach", 1, Arc::new(move |vm, _this, args| {
                let cb = args.first().copied().unwrap_or(JsValue::undefined());
                let elements = if let Some(ptr) = target.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    if let ObjectKind::Set(elements) = &obj.kind {
                        elements.clone()
                    } else {
                        Vec::new()
                    }
                } else {
                    Vec::new()
                };
                for el in elements {
                    let _ = vm.call(cb, target, &[el, el, target]);
                }
                Ok(JsValue::undefined())
            })))
}
