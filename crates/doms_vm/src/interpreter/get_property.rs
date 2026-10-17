//! The `[[Get]]` hot path with inline-cache support.

use super::*;

impl Vm {
    pub fn get_property(&mut self, target: JsValue, key: &str) -> JsValue {
        self.get_property_with_receiver(target, key, target)
    }

    pub fn get_property_with_receiver(&mut self, target: JsValue, key: &str, receiver: JsValue) -> JsValue {
        let mut cur_tgt = target;
        let mut cur_key = key.to_string();
        let mut hops = 0;
        while let Some((tgt_mod, tgt_key)) = self.module_reexports.get(&(cur_tgt, cur_key.clone())).cloned() {
            hops += 1;
            if hops > 100 {
                return JsValue::uninitialized();
            }
            cur_tgt = tgt_mod;
            cur_key = tgt_key;
        }
        if cur_key == "*" {
            return cur_tgt;
        }
        if cur_tgt != target || cur_key != key {
            if let Some(ptr) = cur_tgt.as_object_ptr() {
                let obj = unsafe { &*(ptr as *mut JsObject) };
                if !self.shapes.has_property(obj.shape_id, &cur_key) {
                    return JsValue::uninitialized();
                }
            }
            return self.get_property_with_receiver(cur_tgt, &cur_key, receiver);
        }
        if target.is_string() {
            let s = self.to_string_value(target);
            if key == "length" {
                return JsValue::from_int32(s.encode_utf16().count() as i32);
            }
            if let Ok(idx) = key.parse::<usize>() {
                if let Some(ch) = s.chars().nth(idx) {
                    return self.alloc_string(ch.to_string());
                }
            }
            if let Some(proto) = self.string_proto {
                return self.get_property_with_receiver(proto, key, receiver);
            }
            return JsValue::undefined();
        }
        if target.is_number() {
            if let Some(proto) = self.number_proto {
                return self.get_property_with_receiver(proto, key, receiver);
            }
            return JsValue::undefined();
        }
        if target.is_boolean() {
            if let Some(proto) = self.boolean_proto {
                return self.get_property_with_receiver(proto, key, receiver);
            }
            return JsValue::undefined();
        }
        if target.is_symbol() {
            if let Some(proto) = self.symbol_proto {
                return self.get_property_with_receiver(proto, key, receiver);
            }
            return JsValue::undefined();
        }

        if let Some(ptr) = target.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };

            if matches!(obj.kind, ObjectKind::Proxy { .. }) {
                let prop_key = PropertyKeyVal::String(key.to_string());
                return self.proxy_get(target, &prop_key, receiver).unwrap_or(JsValue::undefined());
            }

            // Check own accessor
            if let Some(accs) = &obj.accessors {
                if let Some(acc) = accs.get(key) {
                    if let Some(getter) = acc.getter {
                        return self.call(getter, receiver, &[]).unwrap_or(JsValue::undefined());
                    } else {
                        return JsValue::undefined();
                    }
                }
            }

            // Special properties on Array
            if let ObjectKind::Array(elements) = &obj.kind {
                if key == "length" {
                    let prop_info = self.shapes.get_property_info(obj.shape_id, "length");
                    if let Some(info) = prop_info {
                        if let Some(val) = obj.slots.get(info.offset as usize) {
                            if let Some(n) = val.to_number() {
                                if (n as usize) >= elements.len() {
                                    return *val;
                                }
                            }
                        }
                    }
                    let len = elements.len();
                    if len <= i32::MAX as usize {
                        return JsValue::from_int32(len as i32);
                    } else {
                        return JsValue::from_double(len as f64);
                    }
                }
                if let Ok(idx) = key.parse::<usize>() {
                    if let Some(elem) = elements.get(idx) {
                        if !elem.is_hole() {
                            return *elem;
                        }
                    }
                }
            }

            // Special properties on String or StringObject
            if let ObjectKind::String(s) | ObjectKind::StringObject(s) = &obj.kind {
                if key == "length" {
                    return JsValue::from_int32(s.encode_utf16().count() as i32);
                }
                if let Ok(idx) = key.parse::<usize>() {
                    if let Some(ch) = s.chars().nth(idx) {
                        return self.alloc_string(ch.to_string());
                    }
                }

            }

            // Special properties on Promise (fallback only if no prototype)
            if let ObjectKind::Promise(_) = &obj.kind {
                if obj.prototype.is_none() {
                    match key {
                        "then" => return self.get_promise_then(target).unwrap_or_default(),
                        "catch" => return self.get_promise_catch(target).unwrap_or_default(),
                        "finally" => return self.get_promise_finally(target).unwrap_or_default(),
                        _ => {}
                    }
                }
            }

            // Special properties on Generator
            if let ObjectKind::Generator(_) = &obj.kind {
                match key {
                    "next" => return self.get_generator_next(target).unwrap_or_default(),
                    "return" => return self.get_generator_return(target).unwrap_or_default(),
                    "throw" => return self.get_generator_throw(target).unwrap_or_default(),
                    _ => {}
                }
            }

            // Special properties on SharedArrayBuffer
            if let ObjectKind::SharedArrayBuffer(sab) = &obj.kind {
                if key == "byteLength" {
                    return JsValue::from_int32(sab.byte_length() as i32);
                }
            }

            // Special properties on ArrayBuffer
            if let ObjectKind::ArrayBuffer(ab) = &obj.kind {
                if key == "byteLength" {
                    return JsValue::from_int32(ab.byte_length() as i32);
                }
            }

            // Special properties on DataView
            if let ObjectKind::DataView { buffer, byte_offset, byte_length } = &obj.kind {
                match key {
                    "byteLength" => return JsValue::from_int32(*byte_length as i32),
                    "byteOffset" => return JsValue::from_int32(*byte_offset as i32),
                    "buffer" => return self.alloc_array_buffer(Arc::clone(buffer)),
                    _ => {}
                }
            }

            // Special properties on TypedArray
            if let ObjectKind::TypedArray(ta) = &obj.kind {
                match key {
                    "length" => return JsValue::from_int32(ta.length as i32),
                    "byteLength" => return JsValue::from_int32((ta.length * ta.kind.bytes_per_element()) as i32),
                    "byteOffset" => return JsValue::from_int32(ta.byte_offset as i32),
                    "buffer" => return self.alloc_array_buffer(Arc::clone(&ta.buffer)),
                    "BYTES_PER_ELEMENT" => return JsValue::from_int32(ta.kind.bytes_per_element() as i32),
                    _ => {
                        if let Ok(idx) = key.parse::<usize>() {
                            if let Some(val) = Self::read_typed_array_element(ta, idx) {
                                return val;
                            }
                        }
                    }
                }
            }

            // Special properties on Worker
            if let ObjectKind::Worker(port) = &obj.kind {
                let port_clone = Arc::clone(port);
                match key {
                    "postMessage" => return self.get_worker_post_message(port_clone).unwrap_or_default(),
                    "receiveMessage" => return self.get_worker_receive_message(port_clone).unwrap_or_default(),
                    "terminate" => return self.alloc_native_function("terminate", 0, |_vm, _this, _args| Ok(JsValue::undefined())),
                    _ => {}
                }
            }

            // Special properties on WeakRef
            if let ObjectKind::WeakRef(target_ref) = &obj.kind {
                let tr = *target_ref;
                if key == "deref" {
                    return self.alloc_closure_native_function("deref", 0, Arc::new(move |_vm, _this, _args| {
                        Ok(tr)
                    }));
                }
            }

            // Special properties on FinalizationRegistry
            if let ObjectKind::FinalizationRegistry { .. } = &obj.kind {
                if let Some(m) = self.get_finalization_registry_method(target, key) {
                    return m;
                }
            }


            // Shape offset lookup (own properties take precedence)
            if let Some(offset) = self.shapes.lookup_offset(obj.shape_id, key) {
                if let Some(val) = obj.get_slot(offset) {
                    return val;
                }
            }

            // Special properties on Error
            if let ObjectKind::Error { name, message } = &obj.kind {
                match key {
                    "name" => return self.alloc_string(name.clone()),
                    "message" => return self.alloc_string(message.clone()),
                    _ => {}
                }
            }

            // Special properties on Function / NativeFunction
            if matches!(obj.kind, ObjectKind::Function(_) | ObjectKind::NativeFunction(_)) && !self.deleted_props.contains(&(target, key.to_string())) {
                match key {
                    "name" => {
                        let name_str = match &obj.kind {
                            ObjectKind::Function(f) => f.name.as_deref().unwrap_or(""),
                            ObjectKind::NativeFunction(n) => &n.name,
                            _ => "",
                        };
                        return self.alloc_string(name_str.to_string());
                    }
                    "length" => {
                        let len = match &obj.kind {
                            ObjectKind::Function(f) => f.params.iter().take_while(|p| !p.starts_with("...")).count(),
                            ObjectKind::NativeFunction(n) => n.length as usize,
                            _ => 0,
                        };
                        return JsValue::from_int32(len as i32);
                    }
                    _ => {}
                }
            }

            // Prototype chain lookup
            let mut curr_proto = obj.prototype;
            while let Some(proto_val) = curr_proto {
                if let Some(proto_ptr) = proto_val.as_object_ptr() {
                    let proto_obj = unsafe { &*(proto_ptr as *mut JsObject) };
                    if matches!(proto_obj.kind, ObjectKind::Proxy { .. }) {
                        let prop_key = PropertyKeyVal::String(key.to_string());
                        return self.proxy_get(proto_val, &prop_key, receiver).unwrap_or(JsValue::undefined());
                    }
                    if let Some(accs) = &proto_obj.accessors {
                        if let Some(acc) = accs.get(key) {
                            if let Some(getter) = acc.getter {
                                return self.call(getter, receiver, &[]).unwrap_or(JsValue::undefined());
                            } else {
                                return JsValue::undefined();
                            }
                        }
                    }
                    if let Some(offset) = self.shapes.lookup_offset(proto_obj.shape_id, key) {
                        if let Some(val) = proto_obj.get_slot(offset) {
                            return val;
                        }
                    }
                    curr_proto = proto_obj.prototype;
                } else {
                    break;
                }
            }
        }

        if let Some(ptr) = target.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if matches!(&obj.kind, ObjectKind::String(_)) {
                if let Some(proto) = self.string_proto {
                    return self.get_property(proto, key);
                }
            }
            if matches!(&obj.kind, ObjectKind::Symbol(_)) {
                if let Some(proto) = self.symbol_proto {
                    return self.get_property(proto, key);
                }
            }
        }

        JsValue::undefined()
    }
}
