//! Global functions: `parseInt`, `parseFloat`, `isNaN`, `isFinite` and the URI codecs.

use super::*;

pub(super) fn register_global_functions(vm: &mut Vm) {
    let parse_int = vm.alloc_native_function("parseInt", 2, |vm, _this, args| {
        let s = args.first().map(|v| vm.to_string_value(*v)).unwrap_or_else(|| "undefined".to_string());
        let trimmed = s.trim_start();
        if trimmed.is_empty() {
            return Ok(JsValue::from_double(f64::NAN));
        }

        let mut radix = if let Some(r_val) = args.get(1) {
            let r = vm.to_number_value(*r_val).unwrap_or(0.0) as i32;
            if r == 0 { 10 } else { r }
        } else {
            10
        };

        let (sign, rest) = if let Some(stripped) = trimmed.strip_prefix('+') {
            (1.0, stripped)
        } else if let Some(stripped) = trimmed.strip_prefix('-') {
            (-1.0, stripped)
        } else {
            (1.0, trimmed)
        };

        if (radix == 16 || radix == 10) && (rest.starts_with("0x") || rest.starts_with("0X")) {
            radix = 16;
        }

        let rest_num = if radix == 16 && (rest.starts_with("0x") || rest.starts_with("0X")) {
            &rest[2..]
        } else {
            rest
        };

        if !(2..=36).contains(&radix) {
            return Ok(JsValue::from_double(f64::NAN));
        }

        let mut valid_len = 0;
        for c in rest_num.chars() {
            if c.to_digit(radix as u32).is_some() {
                valid_len += c.len_utf8();
            } else {
                break;
            }
        }

        if valid_len == 0 {
            return Ok(JsValue::from_double(f64::NAN));
        }

        let digits_str = &rest_num[..valid_len];
        if let Ok(val) = u64::from_str_radix(digits_str, radix as u32) {
            Ok(JsValue::from_double(sign * (val as f64)))
        } else {
            let mut acc = 0.0;
            let r_f = radix as f64;
            for c in digits_str.chars() {
                if let Some(d) = c.to_digit(radix as u32) {
                    acc = acc * r_f + (d as f64);
                }
            }
            Ok(JsValue::from_double(sign * acc))
        }
    });

    let parse_float = vm.alloc_native_function("parseFloat", 1, |vm, _this, args| {
        let s = args.first().map(|v| vm.to_string_value(*v)).unwrap_or_else(|| "undefined".to_string());
        let trimmed = s.trim_start();
        if trimmed.is_empty() {
            return Ok(JsValue::from_double(f64::NAN));
        }
        if trimmed.starts_with("Infinity") || trimmed.starts_with("+Infinity") {
            return Ok(JsValue::from_double(f64::INFINITY));
        }
        if trimmed.starts_with("-Infinity") {
            return Ok(JsValue::from_double(f64::NEG_INFINITY));
        }
        let mut last_valid = None;
        for i in 1..=trimmed.len() {
            if let Ok(sub) = std::str::from_utf8(&trimmed.as_bytes()[..i]) {
                if let Ok(f) = sub.parse::<f64>() {
                    last_valid = Some(f);
                } else if last_valid.is_some() && !sub.ends_with('e') && !sub.ends_with('E') && !sub.ends_with('+') && !sub.ends_with('-') && !sub.ends_with('.') {
                    break;
                }
            }
        }
        Ok(JsValue::from_double(last_valid.unwrap_or(f64::NAN)))
    });

    let is_nan = vm.alloc_native_function("isNaN", 1, |vm, _this, args| {
        let arg = args.first().copied().unwrap_or(JsValue::undefined());
        let n = vm.to_number_value(arg)?;
        Ok(JsValue::from_boolean(n.is_nan()))
    });

    let is_finite = vm.alloc_native_function("isFinite", 1, |vm, _this, args| {
        let arg = args.first().copied().unwrap_or(JsValue::undefined());
        let n = vm.to_number_value(arg)?;
        Ok(JsValue::from_boolean(n.is_finite()))
    });

    let encode_uri = vm.alloc_native_function("encodeURI", 1, |vm, _this, args| {
        let s = if let Some(v) = args.first() {
            let prim = vm.to_primitive(*v, crate::interpreter::PreferredType::String)?;
            vm.to_string_value(prim)
        } else {
            "undefined".to_string()
        };
        let res = uri_encode(&s, true)?;
        Ok(vm.alloc_string(res))
    });

    let decode_uri = vm.alloc_native_function("decodeURI", 1, |vm, _this, args| {
        let s = if let Some(v) = args.first() {
            let prim = vm.to_primitive(*v, crate::interpreter::PreferredType::String)?;
            vm.to_string_value(prim)
        } else {
            "undefined".to_string()
        };
        let res = uri_decode(&s, true)?;
        Ok(vm.alloc_string(res))
    });

    let encode_uri_comp = vm.alloc_native_function("encodeURIComponent", 1, |vm, _this, args| {
        let s = if let Some(v) = args.first() {
            let prim = vm.to_primitive(*v, crate::interpreter::PreferredType::String)?;
            vm.to_string_value(prim)
        } else {
            "undefined".to_string()
        };
        let res = uri_encode(&s, false)?;
        Ok(vm.alloc_string(res))
    });

    let decode_uri_comp = vm.alloc_native_function("decodeURIComponent", 1, |vm, _this, args| {
        let s = if let Some(v) = args.first() {
            let prim = vm.to_primitive(*v, crate::interpreter::PreferredType::String)?;
            vm.to_string_value(prim)
        } else {
            "undefined".to_string()
        };
        let res = uri_decode(&s, false)?;
        Ok(vm.alloc_string(res))
    });

    let escape_fn = vm.alloc_native_function("escape", 1, |vm, _this, args| {
        let s = if let Some(v) = args.first() {
            vm.to_string_value(*v)
        } else {
            "undefined".to_string()
        };
        let mut r = String::with_capacity(s.len() * 2);
        for ch in s.chars() {
            let cp = ch as u32;
            if cp <= 127 && (ch.is_ascii_alphanumeric() || matches!(ch, '@' | '*' | '_' | '+' | '-' | '.' | '/')) {
                r.push(ch);
            } else if cp < 256 {
                r.push_str(&format!("%{:02X}", cp));
            } else {
                r.push_str(&format!("%u{:04X}", cp));
            }
        }
        Ok(vm.alloc_string(r))
    });

    let unescape_fn = vm.alloc_native_function("unescape", 1, |vm, _this, args| {
        let s = if let Some(v) = args.first() {
            vm.to_string_value(*v)
        } else {
            "undefined".to_string()
        };
        let chars: Vec<char> = s.chars().collect();
        let len = chars.len();
        let mut r = String::with_capacity(len);
        let mut k = 0;
        while k < len {
            let c = chars[k];
            if c == '%' {
                if k + 5 < len && chars[k + 1] == 'u' {
                    let hex_s: String = chars[k + 2..k + 6].iter().collect();
                    if let Ok(val) = u32::from_str_radix(&hex_s, 16) {
                        if let Some(decoded_char) = char::from_u32(val) {
                            r.push(decoded_char);
                            k += 6;
                            continue;
                        }
                    }
                } else if k + 2 < len {
                    let hex_s: String = chars[k + 1..k + 3].iter().collect();
                    if let Ok(val) = u32::from_str_radix(&hex_s, 16) {
                        if let Some(decoded_char) = char::from_u32(val) {
                            r.push(decoded_char);
                            k += 3;
                            continue;
                        }
                    }
                }
            }
            r.push(c);
            k += 1;
        }
        Ok(vm.alloc_string(r))
    });

    vm.globals.insert("parseInt".to_string(), parse_int);
    vm.globals.insert("parseFloat".to_string(), parse_float);
    vm.globals.insert("isNaN".to_string(), is_nan);
    vm.globals.insert("isFinite".to_string(), is_finite);
    vm.globals.insert("encodeURI".to_string(), encode_uri);
    vm.globals.insert("decodeURI".to_string(), decode_uri);
    vm.globals.insert("encodeURIComponent".to_string(), encode_uri_comp);
    vm.globals.insert("decodeURIComponent".to_string(), decode_uri_comp);
    vm.globals.insert("escape".to_string(), escape_fn);
    vm.globals.insert("unescape".to_string(), unescape_fn);
    if vm.global_object.is_object() {
        vm.set_property(vm.global_object, "escape", escape_fn);
        vm.set_property(vm.global_object, "unescape", unescape_fn);
    }
    vm.globals.insert("NaN".to_string(), JsValue::from_double(f64::NAN));
    vm.globals.insert("Infinity".to_string(), JsValue::from_double(f64::INFINITY));
    vm.globals.insert("undefined".to_string(), JsValue::undefined());
}
