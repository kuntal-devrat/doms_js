//! The `String` global object and its legacy HTML helper methods.

use super::*;

pub(super) fn register_string_global(vm: &mut Vm) {
    let string_proto = vm.alloc_object();
    vm.string_proto = Some(string_proto);

    let str_value_of = vm.alloc_native_function("valueOf", 0, |vm, this, _args| {
        if this.is_js_string() {
            return Ok(this);
        }
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::StringObject(ref s) = obj.kind {
                return Ok(vm.alloc_string(s.clone()));
            }
        }
        Err(VmError::TypeError("String.prototype.valueOf requires that 'this' be a String".into()))
    });
    let str_to_string = vm.alloc_native_function("toString", 0, |vm, this, _args| {
        if this.is_js_string() {
            return Ok(this);
        }
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::StringObject(ref s) = obj.kind {
                return Ok(vm.alloc_string(s.clone()));
            }
        }
        Err(VmError::TypeError("String.prototype.toString requires that 'this' be a String".into()))
    });
    vm.set_property(string_proto, "valueOf", str_value_of);
    vm.set_property(string_proto, "toString", str_to_string);

    let str_ctor = vm.alloc_native_function("String", 1, |vm, _this, args| {
        let arg = args.first().copied();
        if vm.is_constructing {
            let s = match arg {
                Some(v) => vm.to_string_spec(v)?,
                None => String::new(),
            };
            let proto = vm.string_proto;
            Ok(vm.alloc_string_object(s, proto))
        } else {
            let s = match arg {
                Some(v) => {
                    if v.is_symbol()
                        || v.as_object_ptr().is_some_and(|p| {
                            matches!(
                                unsafe { &(*(p as *mut JsObject)).kind },
                                ObjectKind::Symbol(_)
                            )
                        })
                    {
                        vm.to_string_value(v)
                    } else {
                        vm.to_string_spec(v)?
                    }
                }
                None => String::new(),
            };
            Ok(vm.alloc_string(s))
        }
    });

    vm.set_property(string_proto, "constructor", str_ctor);
    vm.set_property_non_enumerable(string_proto, "constructor".to_string());
    vm.set_property(str_ctor, "prototype", string_proto);
    vm.set_property_non_enumerable(str_ctor, "prototype".to_string());
    vm.set_property_non_writable(str_ctor, "prototype".to_string());
    vm.set_property_non_configurable(str_ctor, "prototype".to_string());

    let html_methods: [(&str, &str, &str, bool); 13] = [
        ("anchor", "a", "name", true),
        ("big", "big", "", false),
        ("blink", "blink", "", false),
        ("bold", "b", "", false),
        ("fixed", "tt", "", false),
        ("fontcolor", "font", "color", true),
        ("fontsize", "font", "size", true),
        ("italics", "i", "", false),
        ("link", "a", "href", true),
        ("small", "small", "", false),
        ("strike", "strike", "", false),
        ("sub", "sub", "", false),
        ("sup", "sup", "", false),
    ];
    for (m_name, tag, attr, has_attr) in html_methods {
        let tag_c = tag.to_string();
        let attr_c = attr.to_string();
        let fn_val = vm.alloc_closure_native_function(
            m_name,
            if has_attr { 1 } else { 0 },
            Arc::new(move |vm, this, args| {
                create_html(
                    vm,
                    this,
                    &tag_c,
                    &attr_c,
                    if has_attr { args.first().copied() } else { None },
                )
            }),
        );
        vm.set_property(string_proto, m_name, fn_val);
        vm.set_property_non_enumerable(string_proto, m_name.to_string());
    }

    let str_substr = vm.alloc_native_function("substr", 2, |vm, this, args| {
        if this.is_nullish() {
            return Err(VmError::TypeError(
                "String.prototype.substr called on null or undefined".into(),
            ));
        }
        let s = vm.to_string_spec(this)?;
        let chars: Vec<char> = s.chars().collect();
        let len = chars.len() as i32;
        let raw_start = args.first().and_then(|v| v.to_int32()).unwrap_or(0);
        let start = if raw_start < 0 {
            (len + raw_start).max(0) as usize
        } else {
            raw_start.min(len) as usize
        };
        let length = if let Some(l_arg) = args.get(1) {
            if l_arg.is_undefined() {
                (len as usize).saturating_sub(start)
            } else {
                let l = l_arg.to_int32().unwrap_or(0);
                if l <= 0 { 0 } else { l as usize }
            }
        } else {
            (len as usize).saturating_sub(start)
        };
        let end = (start + length).min(chars.len());
        let sub: String = chars[start..end].iter().collect();
        Ok(vm.alloc_string(sub))
    });
    vm.set_property(string_proto, "substr", str_substr);
    vm.set_property_non_enumerable(string_proto, "substr".to_string());

    let str_trim_start = vm.alloc_native_function("trimStart", 0, |vm, this, _args| {
        if this.is_nullish() {
            return Err(VmError::TypeError(
                "String.prototype.trimStart called on null or undefined".into(),
            ));
        }
        let s = vm.to_string_spec(this)?;
        Ok(vm.alloc_string(s.trim_start().to_string()))
    });
    vm.set_property(string_proto, "trimStart", str_trim_start);
    vm.set_property(string_proto, "trimLeft", str_trim_start);
    vm.set_property_non_enumerable(string_proto, "trimStart".to_string());
    vm.set_property_non_enumerable(string_proto, "trimLeft".to_string());

    let str_trim_end = vm.alloc_native_function("trimEnd", 0, |vm, this, _args| {
        if this.is_nullish() {
            return Err(VmError::TypeError(
                "String.prototype.trimEnd called on null or undefined".into(),
            ));
        }
        let s = vm.to_string_spec(this)?;
        Ok(vm.alloc_string(s.trim_end().to_string()))
    });
    vm.set_property(string_proto, "trimEnd", str_trim_end);
    vm.set_property(string_proto, "trimRight", str_trim_end);
    vm.set_property_non_enumerable(string_proto, "trimEnd".to_string());
    vm.set_property_non_enumerable(string_proto, "trimRight".to_string());

    let string_methods = [
        "indexOf",
        "includes",
        "toLowerCase",
        "toUpperCase",
        "toLocaleLowerCase",
        "toLocaleUpperCase",
        "trim",
        "charAt",
        "charCodeAt",
        "codePointAt",
        "slice",
        "isWellFormed",
        "toWellFormed",
        "replace",
        "match",
        "search",
        "lastIndexOf",
        "split",
        "concat",
        "startsWith",
        "endsWith",
        "repeat",
        "padStart",
        "padEnd",
        "localeCompare",
        "substring",
        "at",
        "replaceAll",
    ];
    for name in string_methods {
        if let Some(m) = vm.get_string_method(name) {
            vm.set_property(string_proto, name, m);
            vm.set_property_non_enumerable(string_proto, name.to_string());
        }
    }

    if let Some(sym_iter) = vm.symbol_iterator {
        let str_iter_fn = vm.alloc_native_function("[Symbol.iterator]", 0, |vm, this, _args| {
            if this.is_nullish() {
                return Err(VmError::TypeError(
                    "String.prototype[Symbol.iterator] called on null or undefined".into(),
                ));
            }
            let s = vm.to_string_spec(this)?;
            let iter_obj = vm.alloc_object_with_exact_proto(vm.string_iterator_proto);
            if let Some(ptr) = iter_obj.as_object_ptr() {
                let obj = unsafe { &mut *(ptr as *mut JsObject) };
                obj.kind = ObjectKind::StringIterator { string: s, index: 0 };
            }
            Ok(iter_obj)
        });
        vm.set_symbol_property(string_proto, sym_iter, str_iter_fn);
    }

    let from_char_code_fn = vm.alloc_native_function("fromCharCode", 1, |vm, _this, args| {
        let mut u16s = Vec::with_capacity(args.len());
        for arg in args {
            let n = vm.to_number_value(*arg)?;
            let u = if n.is_nan() || n.is_infinite() || n == 0.0 {
                0u16
            } else {
                let int = n.trunc();
                let rem = (int % 65536.0) as i64;
                if rem < 0 { (rem + 65536) as u16 } else { rem as u16 }
            };
            u16s.push(u);
        }
        let s = String::from_utf16_lossy(&u16s);
        Ok(vm.alloc_string(s))
    });
    vm.set_property(str_ctor, "fromCharCode", from_char_code_fn);

    let from_code_point_fn = vm.alloc_native_function("fromCodePoint", 1, |vm, _this, args| {
        let mut u16_buf = Vec::with_capacity(args.len());
        for arg in args {
            let num = vm.to_number_value(*arg)?;
            if num < 0.0 || num > 1114111.0 || num.fract() != 0.0 || num.is_nan() {
                return Err(VmError::RangeError(format!("Invalid code point {}", num)));
            }
            let cp = num as u32;
            if cp <= 0xFFFF {
                u16_buf.push(cp as u16);
            } else {
                let cp_sub = cp - 0x10000;
                let high = ((cp_sub >> 10) + 0xD800) as u16;
                let low = ((cp_sub & 0x3FF) + 0xDC00) as u16;
                u16_buf.push(high);
                u16_buf.push(low);
            }
        }
        let s = String::from_utf16_lossy(&u16_buf);
        Ok(vm.alloc_string(s))
    });
    vm.set_property(str_ctor, "fromCodePoint", from_code_point_fn);

    let str_raw_fn = vm.alloc_native_function("raw", 1, |vm, _this, args| {
        let template = args.first().copied().unwrap_or(JsValue::undefined());
        let cooked = vm.to_object_value(template)?;
        let prev_temp = vm.temp_roots.len();
        vm.temp_roots.push(cooked);
        let raw_val = vm.get_property_result(cooked, "raw")?;
        let raw_obj = vm.to_object_value(raw_val)?;
        vm.temp_roots.push(raw_obj);

        let cur_raw = vm.temp_roots[prev_temp + 1];
        let literal_segments = vm.length_of_array_like(cur_raw)?;
        if literal_segments == 0 {
            vm.temp_roots.truncate(prev_temp);
            return Ok(vm.alloc_string(String::new()));
        }

        let substitutions = if args.len() > 1 { &args[1..] } else { &[] };
        let mut result = String::new();

        for i in 0..literal_segments {
            let cur_raw = vm.temp_roots[prev_temp + 1];
            let key = i.to_string();
            let seg_val = vm.get_property_result(cur_raw, &key)?;
            let seg_str = vm.to_string_spec(seg_val)?;
            result.push_str(&seg_str);

            if i + 1 == literal_segments {
                break;
            }

            if i < substitutions.len() {
                let sub_str = vm.to_string_spec(substitutions[i])?;
                result.push_str(&sub_str);
            }
        }

        vm.temp_roots.truncate(prev_temp);
        Ok(vm.alloc_string(result))
    });
    vm.set_property(str_ctor, "raw", str_raw_fn);

    vm.globals.insert("String".to_string(), str_ctor);
    if vm.global_object.is_object() {
        vm.set_property(vm.global_object, "String", str_ctor);
    }
}

fn create_html(
    vm: &mut Vm,
    this: JsValue,
    tag: &str,
    attr: &str,
    val: Option<JsValue>,
) -> Result<JsValue, VmError> {
    if this.is_nullish() {
        return Err(VmError::TypeError(format!(
            "String.prototype.{} called on null or undefined",
            tag
        )));
    }
    let s = vm.to_string_spec(this)?;
    let mut p1 = format!("<{}", tag);
    if !attr.is_empty() {
        let v = match val {
            Some(v) => vm.to_string_spec(v)?,
            None => "undefined".to_string(),
        };
        let escaped = v.replace('"', "&quot;");
        p1.push_str(&format!(" {}=\"{}\"", attr, escaped));
    }
    p1.push('>');
    p1.push_str(&s);
    p1.push_str(&format!("</{}>", tag));
    Ok(vm.alloc_string(p1))
}
