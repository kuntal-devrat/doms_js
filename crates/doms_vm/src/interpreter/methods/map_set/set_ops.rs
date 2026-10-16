//! Set-relation methods (`union` … `isDisjointFrom`).

use super::*;

pub(super) fn union(vm: &mut Vm, target: JsValue) -> Option<JsValue> {
    Some(vm.alloc_closure_native_function("union", 1, Arc::new(move |vm, _this, args| {
                let other = args.first().copied().unwrap_or(JsValue::undefined());
                let new_set = vm.alloc_set();
                if let Some(ptr) = target.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    if let ObjectKind::Set(elements) = &obj.kind {
                        for el in elements {
                            let n_obj = unsafe { &mut *(new_set.as_object_ptr().unwrap() as *mut JsObject) };
                            if let ObjectKind::Set(ref mut set_elements) = n_obj.kind {
                                if !set_elements.iter().any(|existing| crate::object::js_values_equal(*existing, *el)) {
                                    set_elements.push(*el);
                                }
                            }
                        }
                    }
                }
                if let Some(ptr) = other.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    if let ObjectKind::Set(elements) = &obj.kind {
                        for el in elements {
                            let n_obj = unsafe { &mut *(new_set.as_object_ptr().unwrap() as *mut JsObject) };
                            if let ObjectKind::Set(ref mut set_elements) = n_obj.kind {
                                if !set_elements.iter().any(|existing| crate::object::js_values_equal(*existing, *el)) {
                                    set_elements.push(*el);
                                }
                            }
                        }
                    }
                }
                Ok(new_set)
            })))
}

pub(super) fn intersection(vm: &mut Vm, target: JsValue) -> Option<JsValue> {
    Some(vm.alloc_closure_native_function("intersection", 1, Arc::new(move |vm, _this, args| {
                let other = args.first().copied().unwrap_or(JsValue::undefined());
                let new_set = vm.alloc_set();
                if let Some(ptr) = target.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    if let ObjectKind::Set(elements) = &obj.kind {
                        for el in elements {
                            let has_fn = vm.get_property(other, "has");
                            let res = vm.call(has_fn, other, &[*el]).unwrap_or_else(|_| JsValue::from_boolean(false));
                            if res.is_truthy() {
                                let n_obj = unsafe { &mut *(new_set.as_object_ptr().unwrap() as *mut JsObject) };
                                if let ObjectKind::Set(ref mut set_elements) = n_obj.kind {
                                    if !set_elements.iter().any(|existing| crate::object::js_values_equal(*existing, *el)) {
                                        set_elements.push(*el);
                                    }
                                }
                            }
                        }
                    }
                }
                Ok(new_set)
            })))
}

pub(super) fn difference(vm: &mut Vm, target: JsValue) -> Option<JsValue> {
    Some(vm.alloc_closure_native_function("difference", 1, Arc::new(move |vm, _this, args| {
                let other = args.first().copied().unwrap_or(JsValue::undefined());
                let new_set = vm.alloc_set();
                if let Some(ptr) = target.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    if let ObjectKind::Set(elements) = &obj.kind {
                        for el in elements {
                            let has_fn = vm.get_property(other, "has");
                            let res = vm.call(has_fn, other, &[*el]).unwrap_or_else(|_| JsValue::from_boolean(false));
                            if !res.is_truthy() {
                                let n_obj = unsafe { &mut *(new_set.as_object_ptr().unwrap() as *mut JsObject) };
                                if let ObjectKind::Set(ref mut set_elements) = n_obj.kind {
                                    if !set_elements.iter().any(|existing| crate::object::js_values_equal(*existing, *el)) {
                                        set_elements.push(*el);
                                    }
                                }
                            }
                        }
                    }
                }
                Ok(new_set)
            })))
}

pub(super) fn symmetric_difference(vm: &mut Vm, target: JsValue) -> Option<JsValue> {
    Some(vm.alloc_closure_native_function("symmetricDifference", 1, Arc::new(move |vm, _this, args| {
                let other = args.first().copied().unwrap_or(JsValue::undefined());
                let new_set = vm.alloc_set();
                if let Some(ptr) = target.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    if let ObjectKind::Set(elements) = &obj.kind {
                        for el in elements {
                            let has_fn = vm.get_property(other, "has");
                            let res = vm.call(has_fn, other, &[*el]).unwrap_or_else(|_| JsValue::from_boolean(false));
                            if !res.is_truthy() {
                                let n_obj = unsafe { &mut *(new_set.as_object_ptr().unwrap() as *mut JsObject) };
                                if let ObjectKind::Set(ref mut set_elements) = n_obj.kind {
                                    if !set_elements.iter().any(|existing| crate::object::js_values_equal(*existing, *el)) {
                                        set_elements.push(*el);
                                    }
                                }
                            }
                        }
                    }
                }
                if let Some(ptr) = other.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    if let ObjectKind::Set(elements) = &obj.kind {
                        for el in elements {
                            let has_fn = vm.get_property(target, "has");
                            let res = vm.call(has_fn, target, &[*el]).unwrap_or_else(|_| JsValue::from_boolean(false));
                            if !res.is_truthy() {
                                let n_obj = unsafe { &mut *(new_set.as_object_ptr().unwrap() as *mut JsObject) };
                                if let ObjectKind::Set(ref mut set_elements) = n_obj.kind {
                                    if !set_elements.iter().any(|existing| crate::object::js_values_equal(*existing, *el)) {
                                        set_elements.push(*el);
                                    }
                                }
                            }
                        }
                    }
                }
                Ok(new_set)
            })))
}

pub(super) fn is_subset_of(vm: &mut Vm, target: JsValue) -> Option<JsValue> {
    Some(vm.alloc_closure_native_function("isSubsetOf", 1, Arc::new(move |vm, _this, args| {
                let other = args.first().copied().unwrap_or(JsValue::undefined());
                if let Some(ptr) = target.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    if let ObjectKind::Set(elements) = &obj.kind {
                        for el in elements {
                            let has_fn = vm.get_property(other, "has");
                            let res = vm.call(has_fn, other, &[*el]).unwrap_or_else(|_| JsValue::from_boolean(false));
                            if !res.is_truthy() {
                                return Ok(JsValue::from_boolean(false));
                            }
                        }
                    }
                }
                Ok(JsValue::from_boolean(true))
            })))
}

pub(super) fn is_superset_of(vm: &mut Vm, target: JsValue) -> Option<JsValue> {
    Some(vm.alloc_closure_native_function("isSupersetOf", 1, Arc::new(move |vm, _this, args| {
                let other = args.first().copied().unwrap_or(JsValue::undefined());
                if let Some(ptr) = other.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    if let ObjectKind::Set(elements) = &obj.kind {
                        for el in elements {
                            let has_fn = vm.get_property(target, "has");
                            let res = vm.call(has_fn, target, &[*el]).unwrap_or_else(|_| JsValue::from_boolean(false));
                            if !res.is_truthy() {
                                return Ok(JsValue::from_boolean(false));
                            }
                        }
                    }
                }
                Ok(JsValue::from_boolean(true))
            })))
}

pub(super) fn is_disjoint_from(vm: &mut Vm, target: JsValue) -> Option<JsValue> {
    Some(vm.alloc_closure_native_function("isDisjointFrom", 1, Arc::new(move |vm, _this, args| {
                let other = args.first().copied().unwrap_or(JsValue::undefined());
                if let Some(ptr) = target.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    if let ObjectKind::Set(elements) = &obj.kind {
                        for el in elements {
                            let has_fn = vm.get_property(other, "has");
                            let res = vm.call(has_fn, other, &[*el]).unwrap_or_else(|_| JsValue::from_boolean(false));
                            if res.is_truthy() {
                                return Ok(JsValue::from_boolean(false));
                            }
                        }
                    }
                }
                Ok(JsValue::from_boolean(true))
            })))
}
