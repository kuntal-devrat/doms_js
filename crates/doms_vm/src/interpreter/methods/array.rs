//! Fast dispatch for `Array.prototype` methods.

use super::*;

impl Vm {
    pub(crate) fn get_array_method(&mut self, name: &str) -> Option<JsValue> {
        match name {
            "push" => Some(self.alloc_native_function("push", 1, |_vm, this, args| {
                if let Some(ptr) = this.as_object_ptr() {
                    let obj = unsafe { &mut *(ptr as *mut JsObject) };
                    if let ObjectKind::Array(elements) = &mut obj.kind {
                        for arg in args {
                            elements.push(*arg);
                        }
                        return Ok(JsValue::from_int32(elements.len() as i32));
                    }
                }
                Ok(JsValue::undefined())
            })),
            "pop" => Some(self.alloc_native_function("pop", 0, |_vm, this, _args| {
                if let Some(ptr) = this.as_object_ptr() {
                    let obj = unsafe { &mut *(ptr as *mut JsObject) };
                    if let ObjectKind::Array(elements) = &mut obj.kind {
                        return Ok(elements.pop().unwrap_or(JsValue::undefined()));
                    }
                }
                Ok(JsValue::undefined())
            })),
            "shift" => Some(self.alloc_native_function("shift", 0, |_vm, this, _args| {
                if let Some(ptr) = this.as_object_ptr() {
                    let obj = unsafe { &mut *(ptr as *mut JsObject) };
                    if let ObjectKind::Array(elements) = &mut obj.kind {
                        if !elements.is_empty() {
                            return Ok(elements.remove(0));
                        }
                    }
                }
                Ok(JsValue::undefined())
            })),
            "unshift" => Some(self.alloc_native_function("unshift", 1, |_vm, this, args| {
                if let Some(ptr) = this.as_object_ptr() {
                    let obj = unsafe { &mut *(ptr as *mut JsObject) };
                    if let ObjectKind::Array(elements) = &mut obj.kind {
                        for (i, arg) in args.iter().enumerate() {
                            elements.insert(i, *arg);
                        }
                        return Ok(JsValue::from_int32(elements.len() as i32));
                    }
                }
                Ok(JsValue::undefined())
            })),
            "join" => Some(self.alloc_native_function("join", 1, |vm, this, args| {
                let sep = args.first().map(|v| vm.to_string_value(*v)).unwrap_or_else(|| ",".to_string());
                if let Some(ptr) = this.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    if let ObjectKind::Array(elements) = &obj.kind {
                        let strs: Vec<String> = elements.iter().map(|e| vm.to_string_value(*e)).collect();
                        let joined = strs.join(&sep);
                        return Ok(vm.alloc_string(joined));
                    }
                }
                Ok(vm.alloc_string("".to_string()))
            })),
            "slice" => Some(self.alloc_native_function("slice", 2, |vm, this, args| {
                if let Some(ptr) = this.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    if let ObjectKind::Array(elements) = &obj.kind {
                        let len = elements.len() as i32;
                        let start = args.first().and_then(|v| v.to_int32()).unwrap_or(0);
                        let end = args.get(1).and_then(|v| v.to_int32()).unwrap_or(len);
                        let s = start.clamp(0, len) as usize;
                        let e = end.clamp(0, len) as usize;
                        let sliced = if s <= e { elements[s..e].to_vec() } else { Vec::new() };
                        return Ok(vm.alloc_array(sliced));
                    }
                }
                Ok(vm.alloc_array(Vec::new()))
            })),
            "indexOf" => Some(self.alloc_native_function("indexOf", 1, |_vm, this, args| {
                let target = args.first().copied().unwrap_or(JsValue::undefined());
                if let Some(ptr) = this.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    if let ObjectKind::Array(elements) = &obj.kind {
                        for (i, el) in elements.iter().enumerate() {
                            if crate::object::js_strict_equal(*el, target) {
                                return Ok(JsValue::from_int32(i as i32));
                            }
                        }
                    }
                }
                Ok(JsValue::from_int32(-1))
            })),
            "includes" => Some(self.alloc_native_function("includes", 1, |_vm, this, args| {
                let target = args.first().copied().unwrap_or(JsValue::undefined());
                if let Some(ptr) = this.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    if let ObjectKind::Array(elements) = &obj.kind {
                        for el in elements {
                            if crate::object::js_values_equal(*el, target) {
                                return Ok(JsValue::from_boolean(true));
                            }
                        }
                    }
                }
                Ok(JsValue::from_boolean(false))
            })),
            "map" => Some(self.alloc_native_function("map", 1, |vm, this, args| {
                let cb = args.first().copied().unwrap_or(JsValue::undefined());
                let len_val = vm.get_property(this, "length");
                let len = len_val.to_int32().unwrap_or(0).max(0) as usize;
                let mut mapped = Vec::with_capacity(len.min(65536));
                for i in 0..len {
                    let el = vm.get_element(this, i);
                    let res = vm.call(cb, JsValue::undefined(), &[el, JsValue::from_int32(i as i32), this])?;
                    mapped.push(res);
                }
                Ok(vm.alloc_array(mapped))
            })),
            "forEach" => Some(self.alloc_native_function("forEach", 1, |vm, this, args| {
                let cb = args.first().copied().unwrap_or(JsValue::undefined());
                let len_val = vm.get_property(this, "length");
                let len = len_val.to_int32().unwrap_or(0).max(0) as usize;
                for i in 0..len {
                    let el = vm.get_element(this, i);
                    let _ = vm.call(cb, JsValue::undefined(), &[el, JsValue::from_int32(i as i32), this])?;
                }
                Ok(JsValue::undefined())
            })),
            "filter" => Some(self.alloc_native_function("filter", 1, |vm, this, args| {
                let cb = args.first().copied().unwrap_or(JsValue::undefined());
                let len_val = vm.get_property(this, "length");
                let len = len_val.to_int32().unwrap_or(0).max(0) as usize;
                let mut filtered = Vec::new();
                for i in 0..len {
                    let el = vm.get_element(this, i);
                    let res = vm.call(cb, JsValue::undefined(), &[el, JsValue::from_int32(i as i32), this])?;
                    if res.is_truthy() {
                        filtered.push(el);
                    }
                }
                Ok(vm.alloc_array(filtered))
            })),
            _ => None,
        }
    }
}
