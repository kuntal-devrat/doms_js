//! JavaScript value coercion and debug/JSON string conversion.

use super::*;

impl Vm {
    pub fn to_object_value(&mut self, val: JsValue) -> Result<JsValue, VmError> {
        if val.is_nullish() {
            return Err(VmError::TypeError("Cannot convert undefined or null to object".into()));
        }
        if let Some(bi) = self.as_bigint_val(val) {
            return Ok(self.alloc_bigint_object(bi));
        }
        if let Some(ptr) = val.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            match &obj.kind {
                ObjectKind::String(s) => {
                    let s_clone = s.clone();
                    return Ok(self.alloc_string_object(s_clone, self.string_proto));
                }
                _ => return Ok(val),
            }
        }
        if val.is_string() {
            let s = self.to_string_value(val);
            return Ok(self.alloc_string_object(s, self.string_proto));
        }
        if val.is_number() {
            let n = val.to_number().unwrap_or(0.0);
            return Ok(self.alloc_number_object(n, self.number_proto));
        }
        if val.is_boolean() {
            let b = val.as_boolean().unwrap_or(false);
            return Ok(self.alloc_boolean_object(b, self.boolean_proto));
        }
        if val.is_symbol() {
            let desc = if let Some(ptr) = val.as_symbol_ptr() {
                let obj = unsafe { &*(ptr as *mut JsObject) };
                if let ObjectKind::Symbol(d) = &obj.kind {
                    d.clone()
                } else {
                    None
                }
            } else {
                None
            };
            let proto = self.symbol_proto.or(self.object_proto);
            let obj = JsObject::new_symbol(ShapeRegistry::ROOT_SHAPE_ID, proto, desc);
            let ptr = self.allocate_js_object(obj);
            return Ok(unsafe { JsValue::from_object_ptr(ptr as *mut u8) });
        }
        Ok(val)
    }

    pub fn as_bigint_val(&self, val: JsValue) -> Option<BigInt> {
        if let Some(ptr) = val.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::BigInt(bi) = &obj.kind {
                return Some(bi.clone());
            }
        }
        None
    }

    // ── String Coercion and JSON ──

    pub fn to_string_value(&self, val: JsValue) -> String {
        if val.is_sso_string() {
            return val.as_sso_string().unwrap_or_default();
        }
        if val.is_undefined() {
            "undefined".to_string()
        } else if val.is_null() {
            "null".to_string()
        } else if val.is_boolean() {
            if val.as_boolean().unwrap() { "true".to_string() } else { "false".to_string() }
        } else if val.is_int32() {
            format!("{}", val.as_int32().unwrap())
        } else if val.is_double() {
            let d = val.as_double().unwrap();
            if d.is_nan() {
                "NaN".to_string()
            } else if d == 0.0 {
                "0".to_string()
            } else if d.is_infinite() {
                if d > 0.0 { "Infinity".to_string() } else { "-Infinity".to_string() }
            } else {
                let abs_d = d.abs();
                if abs_d >= 1e21 || abs_d < 1e-6 {
                    let s = format!("{:e}", d);
                    if let Some(pos) = s.find('e') {
                        let after = &s[pos + 1..];
                        if !after.starts_with('-') && !after.starts_with('+') {
                            format!("{}e+{}", &s[..pos], after)
                        } else {
                            s
                        }
                    } else {
                        s
                    }
                } else if d.fract() == 0.0 && abs_d < 1e16 {
                    format!("{}", d as i64)
                } else {
                    format!("{}", d)
                }
            }
        } else if val.is_symbol() {
            if let Some(ptr) = val.as_heap_ptr() {
                let obj = unsafe { &*(ptr as *mut JsObject) };
                if let ObjectKind::Symbol(desc) = &obj.kind {
                    return format!("Symbol({})", desc.as_deref().unwrap_or(""));
                }
            }
            "Symbol()".to_string()
        } else if let Some(ptr) = val.as_heap_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            match &mut obj.kind {
                ObjectKind::String(s) | ObjectKind::StringObject(s) => return s.clone(),
                ObjectKind::ConsString { left, right, length } => {
                    let mut result = String::with_capacity(*length);
                    let mut stack = vec![*right, *left];
                    while let Some(node) = stack.pop() {
                        if node.is_sso_string() {
                            if let Some(s) = node.as_sso_string() {
                                result.push_str(&s);
                            }
                        } else if let Some(nptr) = node.as_heap_ptr() {
                            let nobj = unsafe { &*(nptr as *mut JsObject) };
                            match &nobj.kind {
                                ObjectKind::String(s) | ObjectKind::StringObject(s) => {
                                    result.push_str(s);
                                }
                                ObjectKind::ConsString { left: l, right: r, .. } => {
                                    stack.push(*r);
                                    stack.push(*l);
                                }
                                _ => {
                                    result.push_str(&self.to_string_value(node));
                                }
                            }
                        } else {
                            result.push_str(&self.to_string_value(node));
                        }
                    }
                    obj.kind = ObjectKind::String(result.clone());
                    return result;
                }
                ObjectKind::NumberObject(n) => return self.to_string_value(JsValue::from_double(*n)),
                ObjectKind::BooleanObject(b) => return self.to_string_value(JsValue::from_boolean(*b)),
                ObjectKind::Array(elements) => {
                    let parts: Vec<String> = elements.iter().map(|e| self.to_string_value(*e)).collect();
                    parts.join(",")
                }
                ObjectKind::Function(f) => {
                    format!("function {}() {{ [native code] }}", f.name.as_deref().unwrap_or("anonymous"))
                }
                ObjectKind::NativeFunction(n) => {
                    format!("function {}() {{ [native code] }}", n.name)
                }
                ObjectKind::Error { name, message } => {
                    format!("{}: {}", name, message)
                }
                ObjectKind::Date(ms) => format!("Date({})", ms),
                ObjectKind::Symbol(desc) => format!("Symbol({})", desc.as_deref().unwrap_or("")),
                ObjectKind::BigInt(b) => b.to_string(),
                ObjectKind::RegExp { pattern, flags } => format!("/{}/{}", pattern, flags),
                _ => {
                    let has_name_or_msg = {
                        let name = self.shapes.lookup_offset(obj.shape_id, "name").and_then(|offset| obj.get_slot(offset));
                        let msg = self.shapes.lookup_offset(obj.shape_id, "message").and_then(|offset| obj.get_slot(offset));
                        if let Some(msg_val) = msg {
                            let name_str = if let Some(n_val) = name { self.to_string_value(n_val) } else { "Error".to_string() };
                            let msg_str = self.to_string_value(msg_val);
                            Some(format!("{}: {}", name_str, msg_str))
                        } else {
                            None
                        }
                    };
                    has_name_or_msg.unwrap_or_else(|| "[object Object]".to_string())
                }
            }
        } else {
            "undefined".to_string()
        }
    }

    pub fn to_json_string(&self, val: JsValue) -> String {
        let mut visited = std::collections::HashSet::new();
        self.to_json_string_safe(val, &mut visited)
    }

    fn to_json_string_safe(&self, val: JsValue, visited: &mut std::collections::HashSet<*const u8>) -> String {
        if val.is_null() {
            "null".to_string()
        } else if val.is_boolean() {
            if val.as_boolean().unwrap() { "true".to_string() } else { "false".to_string() }
        } else if val.is_number() {
            let n = self.to_string_value(val);
            if n == "NaN" || n == "Infinity" || n == "-Infinity" {
                "null".to_string()
            } else {
                n
            }
        } else if let Some(ptr) = val.as_object_ptr() {
            if !visited.insert(ptr as *const u8) {
                return "null".to_string();
            }
            let obj = unsafe { &*(ptr as *mut JsObject) };
            let res = match &obj.kind {
                ObjectKind::String(s) | ObjectKind::StringObject(s) => {
                    let mut escaped = String::with_capacity(s.len() + 2);
                    escaped.push('"');
                    for c in s.chars() {
                        match c {
                            '"' => escaped.push_str("\\\""),
                            '\\' => escaped.push_str("\\\\"),
                            '\u{08}' => escaped.push_str("\\b"),
                            '\u{0C}' => escaped.push_str("\\f"),
                            '\n' => escaped.push_str("\\n"),
                            '\r' => escaped.push_str("\\r"),
                            '\t' => escaped.push_str("\\t"),
                            _ => escaped.push(c),
                        }
                    }
                    escaped.push('"');
                    escaped
                }
                ObjectKind::Array(elements) => {
                    let items: Vec<String> = elements.iter().map(|e| self.to_json_string_safe(*e, visited)).collect();
                    format!("[{}]", items.join(","))
                }
                ObjectKind::Ordinary => {
                    let mut pairs = Vec::new();
                    for (k, offset) in self.shapes.all_property_entries(obj.shape_id) {
                        if let Some(v) = obj.get_slot(offset) {
                            if !v.is_undefined() && !v.is_symbol() {
                                pairs.push(format!("\"{}\":{}", k, self.to_json_string_safe(v, visited)));
                            }
                        }
                    }
                    format!("{{{}}}", pairs.join(","))
                }
                _ => "null".to_string(),
            };
            visited.remove(&(ptr as *const u8));
            res
        } else {
            "null".to_string()
        }
    }

    pub fn js_string_len(&self, val: JsValue) -> usize {
        if val.is_sso_string() {
            return ((val.0 >> 40) & 0x7) as usize;
        }
        if let Some(ptr) = val.as_heap_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            match &obj.kind {
                ObjectKind::ConsString { length, .. } => return *length,
                ObjectKind::String(s) | ObjectKind::StringObject(s) => return s.encode_utf16().count(),
                _ => {}
            }
        }
        self.to_string_value(val).encode_utf16().count()
    }
}
