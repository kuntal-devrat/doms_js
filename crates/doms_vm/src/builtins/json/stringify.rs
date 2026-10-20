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
    stack: &mut Vec<JsValue>,
    replacer_fn: Option<JsValue>,
    replacer_arr: Option<&[String]>,
    gap: &str,
    indent: &str,
) -> Result<Option<String>, VmError> {
    let prev_temp = vm.temp_roots.len();
    vm.temp_roots.push(holder);

    // 1. Let value be ? Get(holder, key).
    let mut value = vm.get_property_result(holder, key)?;
    vm.temp_roots.push(value);

    // 2. If Type(value) is Object or BigInt, then
    //    a. Let toJSON be ? Get(value, "toJSON").
    //    b. If IsCallable(toJSON) is true, then
    //       i. Set value to ? Call(toJSON, value, « key »).
    if value.is_object() || value.is_bigint() {
        let to_json = vm.get_property_result(value, "toJSON")?;
        if vm.is_callable(to_json) {
            let key_val = vm.alloc_string(key.to_string());
            value = vm.call(to_json, value, &[key_val])?;
            vm.temp_roots[prev_temp + 1] = value;
        }
    }

    // 3. If replacer is not undefined, then
    //    a. Set value to ? Call(replacer, holder, « key, value »).
    if let Some(rf) = replacer_fn {
        let key_val = vm.alloc_string(key.to_string());
        let cur_holder = vm.temp_roots[prev_temp];
        value = vm.call(rf, cur_holder, &[key_val, value])?;
        vm.temp_roots[prev_temp + 1] = value;
    }

    // 4. If Type(value) is Object, then unwrap wrappers:
    if let Some(ptr) = value.as_object_ptr() {
        let obj = unsafe { &*(ptr as *mut JsObject) };
        match &obj.kind {
            ObjectKind::NumberObject(_) => {
                let n = vm.to_number_value(value)?;
                value = JsValue::from_double(n);
                vm.temp_roots[prev_temp + 1] = value;
            }
            ObjectKind::StringObject(_) => {
                let s = vm.to_string_spec(value)?;
                value = vm.alloc_string(s);
                vm.temp_roots[prev_temp + 1] = value;
            }
            ObjectKind::BooleanObject(b) => {
                value = JsValue::from_boolean(*b);
                vm.temp_roots[prev_temp + 1] = value;
            }
            ObjectKind::BigIntObject(bi) => {
                let bi_clone = bi.clone();
                value = vm.alloc_bigint(bi_clone);
                vm.temp_roots[prev_temp + 1] = value;
            }
            _ => {}
        }
    }

    // 5. If value is null, return "null".
    if value.is_null() {
        vm.temp_roots.truncate(prev_temp);
        return Ok(Some("null".to_string()));
    }

    // 6. If value is true, return "true". 7. If value is false, return "false".
    if value.is_boolean() {
        let b = value.as_boolean() == Some(true);
        vm.temp_roots.truncate(prev_temp);
        return Ok(Some(if b { "true".to_string() } else { "false".to_string() }));
    }

    // 8. If Type(value) is String, return QuoteJSONString(value).
    if value.is_js_string() {
        let s = vm.to_string_value(value);
        vm.temp_roots.truncate(prev_temp);
        return Ok(Some(format!("\"{}\"", escape_json_string(&s))));
    }

    // 9. If Type(value) is Number, then
    if value.is_number() {
        let n = vm.to_number_value(value).unwrap_or(0.0);
        vm.temp_roots.truncate(prev_temp);
        if n.is_nan() || n.is_infinite() {
            return Ok(Some("null".to_string()));
        } else {
            return Ok(Some(vm.to_string_value(value)));
        }
    }

    // 10. If Type(value) is BigInt, throw a TypeError exception.
    if value.is_bigint()
        || (value.is_object()
            && value.as_object_ptr().is_some_and(|p| {
                matches!(unsafe { &(*(p as *mut JsObject)).kind }, ObjectKind::BigIntObject(_))
            }))
    {
        vm.temp_roots.truncate(prev_temp);
        return Err(VmError::TypeError("Do not know how to serialize a BigInt".into()));
    }

    // 12. If value is undefined or symbol or callable, return undefined.
    if value.is_undefined() || value.is_symbol() || vm.is_callable(value) {
        vm.temp_roots.truncate(prev_temp);
        return Ok(None);
    }

    // 11. If Type(value) is Object and IsCallable(value) is false:
    if value.is_js_object() {
        if stack.iter().any(|v| v.as_object_ptr() == value.as_object_ptr()) {
            vm.temp_roots.truncate(prev_temp);
            return Err(VmError::TypeError("Converting circular structure to JSON".into()));
        }
        stack.push(value);

        let is_arr = vm.is_array_result(value)?;
        let res = if is_arr {
            let len_val = vm.get_property_result(value, "length")?;
            let len = vm.to_number_value(len_val).unwrap_or(0.0).max(0.0) as usize;
            let mut items = Vec::with_capacity(len.min(1000));
            let next_indent =
                if !gap.is_empty() { format!("{}{}", indent, gap) } else { String::new() };
            for i in 0..len {
                let cur_value = vm.temp_roots[prev_temp + 1];
                let s = json_stringify_walk(
                    vm,
                    cur_value,
                    &i.to_string(),
                    stack,
                    replacer_fn,
                    replacer_arr,
                    gap,
                    &next_indent,
                )?;
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
        } else {
            let cur_value = vm.temp_roots[prev_temp + 1];
            let keys = if let Some(arr_keys) = replacer_arr {
                arr_keys.to_vec()
            } else {
                let key_vals = crate::builtins::object::enumerable_own_property_names(
                    vm,
                    cur_value,
                    crate::builtins::object::EnumerableKind::Key,
                )?;
                key_vals.into_iter().map(|k| vm.to_string_value(k)).collect()
            };

            let mut pairs = Vec::new();
            let next_indent =
                if !gap.is_empty() { format!("{}{}", indent, gap) } else { String::new() };
            for k in keys {
                let cur_value = vm.temp_roots[prev_temp + 1];
                let s_opt = json_stringify_walk(
                    vm,
                    cur_value,
                    &k,
                    stack,
                    replacer_fn,
                    replacer_arr,
                    gap,
                    &next_indent,
                )?;
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
        };

        stack.pop();
        vm.temp_roots.truncate(prev_temp);
        return res;
    }

    vm.temp_roots.truncate(prev_temp);
    Ok(None)
}
