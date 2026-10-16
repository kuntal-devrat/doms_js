//! `JSON.stringify` serialization: the recursive walk over values.

use super::*;

fn escape_json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 4);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0C}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out
}

pub(super) fn json_stringify_walk(
    vm: &mut Vm,
    holder: JsValue,
    key: &str,
    mut val: JsValue,
    stack: &mut Vec<*const JsObject>,
    replacer_fn: Option<JsValue>,
    replacer_arr: Option<&[String]>,
    gap: &str,
    indent: &str,
) -> Result<Option<String>, VmError> {
    // 1. Check toJSON
    if let Some(_ptr) = val.as_object_ptr() {
        let to_json = vm.get_property(val, "toJSON");
        if vm.is_callable(to_json) {
            let key_val = vm.alloc_string(key.to_string());
            val = vm.call(to_json, val, &[key_val])?;
        }
    }

    // 2. Call replacer function if provided
    if let Some(rf) = replacer_fn {
        let key_val = vm.alloc_string(key.to_string());
        val = vm.call(rf, holder, &[key_val, val])?;
    }

    if val.is_null() {
        return Ok(Some("null".to_string()));
    }
    if val.is_boolean() {
        return Ok(Some(if val.as_boolean() == Some(true) { "true".to_string() } else { "false".to_string() }));
    }
    if val.is_number() {
        let n = vm.to_number_value(val).unwrap_or(0.0);
        if n.is_nan() || n.is_infinite() {
            return Ok(Some("null".to_string()));
        } else {
            return Ok(Some(vm.to_string_value(val)));
        }
    }
    if val.is_undefined() || val.is_symbol() {
        return Ok(None);
    }

    if let Some(ptr) = val.as_object_ptr() {
        let obj = unsafe { &*(ptr as *mut JsObject) };
        if matches!(obj.kind, ObjectKind::Function(_) | ObjectKind::NativeFunction(_)) {
            return Ok(None);
        }
        if matches!(obj.kind, ObjectKind::Symbol(_)) {
            return Ok(None);
        }
        if matches!(obj.kind, ObjectKind::BigInt(_)) {
            return Err(VmError::TypeError("Do not know how to serialize a BigInt".into()));
        }

        // Circularity check
        let obj_raw = ptr as *const JsObject;
        if stack.contains(&obj_raw) {
            return Err(VmError::TypeError("Converting circular structure to JSON".into()));
        }
        stack.push(obj_raw);

        let res = match &obj.kind {
            ObjectKind::String(s) | ObjectKind::StringObject(s) => {
                Ok(Some(format!("\"{}\"", escape_json_string(s))))
            }
            ObjectKind::NumberObject(n) => {
                if n.is_nan() || n.is_infinite() {
                    Ok(Some("null".to_string()))
                } else {
                    Ok(Some(vm.to_string_value(JsValue::from_double(*n))))
                }
            }
            ObjectKind::BooleanObject(b) => {
                Ok(Some(if *b { "true".to_string() } else { "false".to_string() }))
            }
            ObjectKind::Array(elements) => {
                let len = elements.len();
                let mut items = Vec::with_capacity(len);
                let next_indent = if !gap.is_empty() { format!("{}{}", indent, gap) } else { String::new() };
                for i in 0..len {
                    let elem = vm.get_property(val, &i.to_string());
                    let s = json_stringify_walk(vm, val, &i.to_string(), elem, stack, replacer_fn, replacer_arr, gap, &next_indent)?;
                    items.push(s.unwrap_or_else(|| "null".to_string()));
                }
                if items.is_empty() {
                    Ok(Some("[]".to_string()))
                } else if gap.is_empty() {
                    Ok(Some(format!("[{}]", items.join(","))))
                } else {
                    let mut out = String::from("[\n");
                    for (idx, item) in items.iter().enumerate() {
                        if idx > 0 {
                            out.push_str(",\n");
                        }
                        out.push_str(&next_indent);
                        out.push_str(item);
                    }
                    out.push('\n');
                    out.push_str(indent);
                    out.push(']');
                    Ok(Some(out))
                }
            }
            _ => {
                let keys = if let Some(arr_keys) = replacer_arr {
                    arr_keys.to_vec()
                } else {
                    get_enumerable_keys(vm, val)
                };

                let mut pairs = Vec::new();
                let next_indent = if !gap.is_empty() { format!("{}{}", indent, gap) } else { String::new() };
                for k in keys {
                    let prop = vm.get_property(val, &k);
                    let s_opt = json_stringify_walk(vm, val, &k, prop, stack, replacer_fn, replacer_arr, gap, &next_indent)?;
                    if let Some(s) = s_opt {
                        if gap.is_empty() {
                            pairs.push(format!("\"{}\":{}", escape_json_string(&k), s));
                        } else {
                            pairs.push(format!("{}\"{}\": {}", next_indent, escape_json_string(&k), s));
                        }
                    }
                }
                if pairs.is_empty() {
                    Ok(Some("{}".to_string()))
                } else if gap.is_empty() {
                    Ok(Some(format!("{{{}}}", pairs.join(","))))
                } else {
                    let mut out = String::from("{\n");
                    out.push_str(&pairs.join(",\n"));
                    out.push('\n');
                    out.push_str(indent);
                    out.push('}');
                    Ok(Some(out))
                }
            }
        };

        stack.pop();
        res
    } else {
        Ok(Some("null".to_string()))
    }
}
