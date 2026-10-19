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
        let s = args.first().map(|v| vm.to_string_value(*v)).unwrap_or_default();
        if vm.is_constructing {
            let proto = vm.string_proto;
            Ok(vm.alloc_string_object(s, proto))
        } else {
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
        let fn_val = vm.alloc_closure_native_function(m_name, if has_attr { 1 } else { 0 }, Arc::new(move |vm, this, args| {
            create_html(vm, this, &tag_c, &attr_c, if has_attr { args.first().copied() } else { None })
        }));
        vm.set_property(string_proto, m_name, fn_val);
        vm.set_property_non_enumerable(string_proto, m_name.to_string());
    }

    let str_substr = vm.alloc_native_function("substr", 2, |vm, this, args| {
        let s = vm.to_string_value(this);
        let chars: Vec<char> = s.chars().collect();
        let len = chars.len() as i32;
        let raw_start = args.first().and_then(|v| v.to_int32()).unwrap_or(0);
        let start = if raw_start < 0 { (len + raw_start).max(0) as usize } else { raw_start.min(len) as usize };
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
        let s = vm.to_string_value(this);
        Ok(vm.alloc_string(s.trim_start().to_string()))
    });
    vm.set_property(string_proto, "trimStart", str_trim_start);
    vm.set_property(string_proto, "trimLeft", str_trim_start);
    vm.set_property_non_enumerable(string_proto, "trimStart".to_string());
    vm.set_property_non_enumerable(string_proto, "trimLeft".to_string());

    let str_trim_end = vm.alloc_native_function("trimEnd", 0, |vm, this, _args| {
        let s = vm.to_string_value(this);
        Ok(vm.alloc_string(s.trim_end().to_string()))
    });
    vm.set_property(string_proto, "trimEnd", str_trim_end);
    vm.set_property(string_proto, "trimRight", str_trim_end);
    vm.set_property_non_enumerable(string_proto, "trimEnd".to_string());
    vm.set_property_non_enumerable(string_proto, "trimRight".to_string());

    let string_methods = [
        "indexOf", "includes", "toLowerCase", "toUpperCase", "toLocaleLowerCase", "toLocaleUpperCase", "trim",
        "charAt", "charCodeAt", "codePointAt", "slice", "isWellFormed", "toWellFormed", "replace",
        "match", "search", "lastIndexOf", "split", "concat", "startsWith", "endsWith",
        "repeat", "padStart", "padEnd", "localeCompare", "substring", "at", "replaceAll",
    ];
    for name in string_methods {
        if let Some(m) = vm.get_string_method(name) {
            vm.set_property(string_proto, name, m);
            vm.set_property_non_enumerable(string_proto, name.to_string());
        }
    }

    if let Some(sym_iter) = vm.symbol_iterator {
        let str_iter_fn = vm.alloc_native_function("[Symbol.iterator]", 0, |vm, this, _args| {
            let s = vm.to_string_value(this);
            let iter_obj = vm.alloc_object_with_exact_proto(vm.string_iterator_proto);
            if let Some(ptr) = iter_obj.as_object_ptr() {
                let obj = unsafe { &mut *(ptr as *mut JsObject) };
                obj.kind = ObjectKind::StringIterator {
                    string: s,
                    index: 0,
                };
            }
            Ok(iter_obj)
        });
        vm.set_symbol_property(string_proto, sym_iter, str_iter_fn);
    }

    let from_char_code_fn = vm.alloc_native_function("fromCharCode", 1, |vm, _this, args| {
        let u16s: Vec<u16> = args.iter().map(|arg| vm.to_number_value(*arg).unwrap_or(0.0) as u32 as u16).collect();
        let s: String = char::decode_utf16(u16s).map(|r| r.unwrap_or('\u{FFFD}')).collect();
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

    vm.globals.insert("String".to_string(), str_ctor);
    if vm.global_object.is_object() {
        vm.set_property(vm.global_object, "String", str_ctor);
    }
}

fn create_html(vm: &mut Vm, this: JsValue, tag: &str, attr: &str, val: Option<JsValue>) -> Result<JsValue, VmError> {
    let s = vm.to_string_value(this);
    let mut p1 = format!("<{}", tag);
    if !attr.is_empty() {
        let v = val.map(|v| vm.to_string_value(v)).unwrap_or_default();
        let escaped = v.replace('"', "&quot;");
        p1.push_str(&format!(" {}=\"{}\"", attr, escaped));
    }
    p1.push('>');
    p1.push_str(&s);
    p1.push_str(&format!("</{}>", tag));
    Ok(vm.alloc_string(p1))
}

