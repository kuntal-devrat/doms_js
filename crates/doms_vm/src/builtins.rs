//! Built-in functions, objects, and prototype methods.

use crate::interpreter::{PropertyKeyVal, Vm, VmError};
use crate::object::{Accessor, JsObject, JsTypedArray, NativeFn, ObjectKind, TypedArrayKind};
use crate::value::JsValue;
use std::sync::Arc;
use num_bigint::BigInt;
use num_traits::{Zero, One, Signed, FromPrimitive};

pub fn register_builtins(vm: &mut Vm) {
    register_symbol(vm);
    register_object_builtins(vm);
    register_function_builtins(vm);
    register_array_builtins(vm);
    register_console(vm);
    register_math(vm);
    register_json(vm);
    register_global_helpers(vm);
    register_promise(vm);
    register_parallel_and_typed_arrays(vm);
    register_phase3_builtins(vm);
    register_bigint(vm);
    register_reflect(vm);
    register_proxy(vm);
    register_test262_harness(vm);
    crate::web_apis::register_web_apis(vm);

    // Ensure all built-in properties on global_object are reflected and non-enumerable
    for (name, val) in &vm.globals.clone() {
        if vm.global_object.is_object() {
            vm.set_property(vm.global_object, name, *val);
            vm.non_enumerable_props.insert((vm.global_object, name.clone()));
            if name == "undefined" || name == "NaN" || name == "Infinity" {
                vm.non_configurable_props.insert((vm.global_object, name.clone()));
                vm.non_writable_props.insert((vm.global_object, name.clone()));
            }
        }
    }

    // Ensure all built-in prototype methods and constructor properties are non-enumerable
    let mut builtin_targets = Vec::new();
    for opt_proto in [
        vm.object_proto,
        vm.array_proto,
        vm.function_proto,
        vm.regexp_proto,
        vm.string_proto,
        vm.number_proto,
        vm.boolean_proto,
        vm.symbol_proto,
        vm.promise_proto,
    ] {
        if let Some(p) = opt_proto {
            builtin_targets.push(p);
        }
    }

    let global_values: Vec<JsValue> = vm.globals.values().copied().collect();
    for global_val in global_values {
        if global_val.is_object() {
            builtin_targets.push(global_val);
            let proto_val = vm.get_property(global_val, "prototype");
            if proto_val.is_object() {
                builtin_targets.push(proto_val);
            }
        }
    }

    for target in builtin_targets {
        if let Some(ptr) = target.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            for prop in vm.shapes.all_properties(obj.shape_id) {
                vm.non_enumerable_props.insert((target, prop));
            }
            if let Some(accs) = &obj.accessors {
                for prop in accs.keys() {
                    vm.non_enumerable_props.insert((target, prop.clone()));
                }
            }
        }
    }
}

fn register_console(vm: &mut Vm) {
    let log_fn = vm.alloc_native_function("log", 1, native_console_log);
    let error_fn = vm.alloc_native_function("error", 1, native_console_error);
    let warn_fn = vm.alloc_native_function("warn", 1, native_console_warn);
    let info_fn = vm.alloc_native_function("info", 1, native_console_info);

    let console_obj = vm.alloc_object();
    vm.set_property(console_obj, "log", log_fn);
    vm.set_property(console_obj, "error", error_fn);
    vm.set_property(console_obj, "warn", warn_fn);
    vm.set_property(console_obj, "info", info_fn);

    vm.globals.insert("console".to_string(), console_obj);
}

fn native_console_log(vm: &mut Vm, _this: JsValue, args: &[JsValue]) -> Result<JsValue, VmError> {
    let parts: Vec<String> = args.iter().map(|arg| vm.to_string_value(*arg)).collect();
    println!("{}", parts.join(" "));
    Ok(JsValue::undefined())
}

fn native_console_error(vm: &mut Vm, _this: JsValue, args: &[JsValue]) -> Result<JsValue, VmError> {
    let parts: Vec<String> = args.iter().map(|arg| vm.to_string_value(*arg)).collect();
    eprintln!("{}", parts.join(" "));
    Ok(JsValue::undefined())
}

fn native_console_warn(vm: &mut Vm, _this: JsValue, args: &[JsValue]) -> Result<JsValue, VmError> {
    let parts: Vec<String> = args.iter().map(|arg| vm.to_string_value(*arg)).collect();
    eprintln!("[WARN] {}", parts.join(" "));
    Ok(JsValue::undefined())
}

fn native_console_info(vm: &mut Vm, _this: JsValue, args: &[JsValue]) -> Result<JsValue, VmError> {
    let parts: Vec<String> = args.iter().map(|arg| vm.to_string_value(*arg)).collect();
    println!("[INFO] {}", parts.join(" "));
    Ok(JsValue::undefined())
}

fn register_math(vm: &mut Vm) {
    let math_obj = vm.alloc_object();

    let abs_fn = vm.alloc_native_function("abs", 1, |_vm, _this, args| {
        let n = args.first().and_then(|v| v.to_number()).unwrap_or(f64::NAN);
        Ok(JsValue::from_double(n.abs()))
    });
    let floor_fn = vm.alloc_native_function("floor", 1, |_vm, _this, args| {
        let n = args.first().and_then(|v| v.to_number()).unwrap_or(f64::NAN);
        Ok(JsValue::from_double(n.floor()))
    });
    let ceil_fn = vm.alloc_native_function("ceil", 1, |_vm, _this, args| {
        let n = args.first().and_then(|v| v.to_number()).unwrap_or(f64::NAN);
        Ok(JsValue::from_double(n.ceil()))
    });
    let round_fn = vm.alloc_native_function("round", 1, |_vm, _this, args| {
        let n = args.first().and_then(|v| v.to_number()).unwrap_or(f64::NAN);
        Ok(JsValue::from_double(n.round()))
    });
    let sqrt_fn = vm.alloc_native_function("sqrt", 1, |_vm, _this, args| {
        let n = args.first().and_then(|v| v.to_number()).unwrap_or(f64::NAN);
        Ok(JsValue::from_double(n.sqrt()))
    });
    let pow_fn = vm.alloc_native_function("pow", 2, |_vm, _this, args| {
        let base = args.first().and_then(|v| v.to_number()).unwrap_or(f64::NAN);
        let exp = args.get(1).and_then(|v| v.to_number()).unwrap_or(f64::NAN);
        Ok(JsValue::from_double(crate::interpreter::js_exponentiate(base, exp)))
    });
    let min_fn = vm.alloc_native_function("min", 2, |_vm, _this, args| {
        let mut min = f64::INFINITY;
        for a in args {
            let n = a.to_number().unwrap_or(f64::NAN);
            if n < min {
                min = n;
            }
        }
        Ok(JsValue::from_double(min))
    });
    let max_fn = vm.alloc_native_function("max", 2, |_vm, _this, args| {
        let mut max = f64::NEG_INFINITY;
        for a in args {
            let n = a.to_number().unwrap_or(f64::NAN);
            if n > max {
                max = n;
            }
        }
        Ok(JsValue::from_double(max))
    });
    let random_fn = vm.alloc_native_function("random", 0, |_vm, _this, _args| {
        // Simple deterministic PRNG based on timestamp / iteration counter
        static mut SEED: u64 = 123456789;
        let rand_val = unsafe {
            SEED = SEED.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (SEED >> 11) as f64 / (1u64 << 53) as f64
        };
        Ok(JsValue::from_double(rand_val))
    });

    vm.set_property(math_obj, "abs", abs_fn);
    vm.set_property(math_obj, "floor", floor_fn);
    vm.set_property(math_obj, "ceil", ceil_fn);
    vm.set_property(math_obj, "round", round_fn);
    vm.set_property(math_obj, "sqrt", sqrt_fn);
    vm.set_property(math_obj, "pow", pow_fn);
    vm.set_property(math_obj, "min", min_fn);
    vm.set_property(math_obj, "max", max_fn);
    vm.set_property(math_obj, "random", random_fn);

    let math_constants = [
        ("E", std::f64::consts::E),
        ("LN10", std::f64::consts::LN_10),
        ("LN2", std::f64::consts::LN_2),
        ("LOG2E", std::f64::consts::LOG2_E),
        ("LOG10E", std::f64::consts::LOG10_E),
        ("PI", std::f64::consts::PI),
        ("SQRT1_2", std::f64::consts::FRAC_1_SQRT_2),
        ("SQRT2", std::f64::consts::SQRT_2),
    ];
    for (name, val) in math_constants {
        vm.set_property(math_obj, name, JsValue::from_double(val));
        vm.non_writable_props.insert((math_obj, name.to_string()));
        vm.non_enumerable_props.insert((math_obj, name.to_string()));
        vm.non_configurable_props.insert((math_obj, name.to_string()));
    }

    vm.globals.insert("Math".to_string(), math_obj);
}

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

fn json_stringify_walk(
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

// ── Strict JSON Lexer & Parser ──

struct JsonParser<'a> {
    chars: Vec<char>,
    pos: usize,
    _phantom: std::marker::PhantomData<&'a ()>,
}

impl<'a> JsonParser<'a> {
    fn new(text: &'a str) -> Self {
        Self {
            chars: text.chars().collect(),
            pos: 0,
            _phantom: std::marker::PhantomData,
        }
    }

    fn skip_whitespace(&mut self) {
        while self.pos < self.chars.len() {
            match self.chars[self.pos] {
                ' ' | '\t' | '\r' | '\n' => self.pos += 1,
                _ => break,
            }
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn advance(&mut self) -> Option<char> {
        if self.pos < self.chars.len() {
            let c = self.chars[self.pos];
            self.pos += 1;
            Some(c)
        } else {
            None
        }
    }

    fn parse_value(&mut self, vm: &mut Vm) -> Result<JsValue, VmError> {
        self.skip_whitespace();
        let c = self.peek().ok_or_else(|| VmError::SyntaxError("Unexpected end of JSON input".into()))?;
        match c {
            'n' => self.parse_null(),
            't' => self.parse_true(),
            'f' => self.parse_false(),
            '"' => self.parse_string(vm),
            '[' => self.parse_array(vm),
            '{' => self.parse_object(vm),
            '-' | '0'..='9' => self.parse_number(),
            _ => Err(VmError::SyntaxError(format!("Unexpected character '{}' in JSON", c))),
        }
    }

    fn parse_null(&mut self) -> Result<JsValue, VmError> {
        if self.pos + 4 <= self.chars.len() && &self.chars[self.pos..self.pos + 4] == &['n', 'u', 'l', 'l'] {
            self.pos += 4;
            Ok(JsValue::null())
        } else {
            Err(VmError::SyntaxError("Invalid JSON: expected null".into()))
        }
    }

    fn parse_true(&mut self) -> Result<JsValue, VmError> {
        if self.pos + 4 <= self.chars.len() && &self.chars[self.pos..self.pos + 4] == &['t', 'r', 'u', 'e'] {
            self.pos += 4;
            Ok(JsValue::from_boolean(true))
        } else {
            Err(VmError::SyntaxError("Invalid JSON: expected true".into()))
        }
    }

    fn parse_false(&mut self) -> Result<JsValue, VmError> {
        if self.pos + 5 <= self.chars.len() && &self.chars[self.pos..self.pos + 5] == &['f', 'a', 'l', 's', 'e'] {
            self.pos += 5;
            Ok(JsValue::from_boolean(false))
        } else {
            Err(VmError::SyntaxError("Invalid JSON: expected false".into()))
        }
    }

    fn parse_string_raw(&mut self) -> Result<String, VmError> {
        self.skip_whitespace();
        if self.advance() != Some('"') {
            return Err(VmError::SyntaxError("Expected string in JSON".into()));
        }
        let mut s = String::new();
        while let Some(c) = self.advance() {
            match c {
                '"' => return Ok(s),
                '\\' => {
                    let esc = self.advance().ok_or_else(|| VmError::SyntaxError("Unterminated escape sequence".into()))?;
                    match esc {
                        '"' => s.push('"'),
                        '\\' => s.push('\\'),
                        '/' => s.push('/'),
                        'b' => s.push('\u{08}'),
                        'f' => s.push('\u{0C}'),
                        'n' => s.push('\n'),
                        'r' => s.push('\r'),
                        't' => s.push('\t'),
                        'u' => {
                            let mut hex = String::new();
                            for _ in 0..4 {
                                let hc = self.advance().ok_or_else(|| VmError::SyntaxError("Invalid unicode escape".into()))?;
                                if !hc.is_ascii_hexdigit() {
                                    return Err(VmError::SyntaxError("Invalid hex digit in unicode escape".into()));
                                }
                                hex.push(hc);
                            }
                            let code = u32::from_str_radix(&hex, 16)
                                .map_err(|_| VmError::SyntaxError("Invalid unicode escape value".into()))?;
                            if (0xD800..=0xDBFF).contains(&code) {
                                // High surrogate: check for low surrogate
                                if self.pos + 6 <= self.chars.len() && self.chars[self.pos] == '\\' && self.chars[self.pos + 1] == 'u' {
                                    let mut low_hex = String::new();
                                    for idx in 0..4 {
                                        let hc = self.chars[self.pos + 2 + idx];
                                        if !hc.is_ascii_hexdigit() { break; }
                                        low_hex.push(hc);
                                    }
                                    if low_hex.len() == 4 {
                                        if let Ok(low_code) = u32::from_str_radix(&low_hex, 16) {
                                            if (0xDC00..=0xDFFF).contains(&low_code) {
                                                self.pos += 6;
                                                let full_code = 0x10000 + ((code - 0xD800) << 10) + (low_code - 0xDC00);
                                                if let Some(ch) = char::from_u32(full_code) {
                                                    s.push(ch);
                                                    continue;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            if let Some(ch) = char::from_u32(code) {
                                s.push(ch);
                            } else {
                                s.push('\u{FFFD}');
                            }
                        }
                        _ => return Err(VmError::SyntaxError(format!("Invalid escape character '{}'", esc))),
                    }
                }
                c if (c as u32) < 0x20 => {
                    return Err(VmError::SyntaxError("Unescaped control character in JSON string".into()));
                }
                other => s.push(other),
            }
        }
        Err(VmError::SyntaxError("Unterminated string in JSON".into()))
    }

    fn parse_string(&mut self, vm: &mut Vm) -> Result<JsValue, VmError> {
        let s = self.parse_string_raw()?;
        Ok(vm.alloc_string(s))
    }

    fn parse_number(&mut self) -> Result<JsValue, VmError> {
        let start = self.pos;
        if self.peek() == Some('-') {
            self.advance();
        }

        let first = self.advance().ok_or_else(|| VmError::SyntaxError("Expected digits for number".into()))?;
        if !first.is_ascii_digit() {
            return Err(VmError::SyntaxError("Expected digit".into()));
        }

        if first == '0' {
            if let Some(next) = self.peek() {
                if next.is_ascii_digit() {
                    return Err(VmError::SyntaxError("Leading zeros are not allowed in JSON numbers".into()));
                }
            }
        } else {
            while let Some(c) = self.peek() {
                if c.is_ascii_digit() {
                    self.advance();
                } else {
                    break;
                }
            }
        }

        // Fractional part
        if self.peek() == Some('.') {
            self.advance();
            let mut has_digit = false;
            while let Some(c) = self.peek() {
                if c.is_ascii_digit() {
                    has_digit = true;
                    self.advance();
                } else {
                    break;
                }
            }
            if !has_digit {
                return Err(VmError::SyntaxError("Decimal point must be followed by digits".into()));
            }
        }

        // Exponent part
        if self.peek() == Some('e') || self.peek() == Some('E') {
            self.advance();
            if self.peek() == Some('+') || self.peek() == Some('-') {
                self.advance();
            }
            let mut has_exp_digit = false;
            while let Some(c) = self.peek() {
                if c.is_ascii_digit() {
                    has_exp_digit = true;
                    self.advance();
                } else {
                    break;
                }
            }
            if !has_exp_digit {
                return Err(VmError::SyntaxError("Exponent must be followed by digits".into()));
            }
        }

        let num_str: String = self.chars[start..self.pos].iter().collect();
        let val = num_str.parse::<f64>().map_err(|_| VmError::SyntaxError("Failed to parse number".into()))?;
        Ok(JsValue::from_double(val))
    }

    fn parse_array(&mut self, vm: &mut Vm) -> Result<JsValue, VmError> {
        self.advance(); // consume '['
        self.skip_whitespace();
        let mut elements = Vec::new();
        if self.peek() == Some(']') {
            self.advance();
            return Ok(vm.alloc_array(elements));
        }

        loop {
            let elem = self.parse_value(vm)?;
            elements.push(elem);
            self.skip_whitespace();
            match self.peek() {
                Some(',') => {
                    self.advance();
                    self.skip_whitespace();
                    if self.peek() == Some(']') {
                        return Err(VmError::SyntaxError("Trailing comma in JSON array".into()));
                    }
                }
                Some(']') => {
                    self.advance();
                    break;
                }
                _ => return Err(VmError::SyntaxError("Expected ',' or ']' in JSON array".into())),
            }
        }

        Ok(vm.alloc_array(elements))
    }

    fn parse_object(&mut self, vm: &mut Vm) -> Result<JsValue, VmError> {
        self.advance(); // consume '{'
        self.skip_whitespace();
        let obj = vm.alloc_object();
        if self.peek() == Some('}') {
            self.advance();
            return Ok(obj);
        }

        loop {
            self.skip_whitespace();
            if self.peek() != Some('"') {
                return Err(VmError::SyntaxError("Property name must be double-quoted in JSON object".into()));
            }
            let key = self.parse_string_raw()?;
            self.skip_whitespace();
            if self.advance() != Some(':') {
                return Err(VmError::SyntaxError("Expected ':' after property name in JSON object".into()));
            }
            let val = self.parse_value(vm)?;
            vm.set_property(obj, &key, val);

            self.skip_whitespace();
            match self.peek() {
                Some(',') => {
                    self.advance();
                    self.skip_whitespace();
                    if self.peek() == Some('}') {
                        return Err(VmError::SyntaxError("Trailing comma in JSON object".into()));
                    }
                }
                Some('}') => {
                    self.advance();
                    break;
                }
                _ => return Err(VmError::SyntaxError("Expected ',' or '}' in JSON object".into())),
            }
        }

        Ok(obj)
    }
}

fn json_walk(vm: &mut Vm, reviver: JsValue, holder: JsValue, name: &str) -> Result<JsValue, VmError> {
    let val = vm.get_property(holder, name);
    if let Some(ptr) = val.as_object_ptr() {
        let obj = unsafe { &*(ptr as *mut JsObject) };
        if let ObjectKind::Array(elements) = &obj.kind {
            let len = elements.len();
            for i in 0..len {
                let prop_str = i.to_string();
                let new_elem = json_walk(vm, reviver, val, &prop_str)?;
                if new_elem.is_undefined() {
                    vm.delete_property(val, &prop_str)?;
                } else {
                    vm.set_property(val, &prop_str, new_elem);
                }
            }
        } else {
            let keys = get_enumerable_keys(vm, val);
            for k in keys {
                let new_val = json_walk(vm, reviver, val, &k)?;
                if new_val.is_undefined() {
                    vm.delete_property(val, &k)?;
                } else {
                    vm.set_property(val, &k, new_val);
                }
            }
        }
    }

    let name_val = vm.alloc_string(name.to_string());
    vm.call(reviver, holder, &[name_val, val])
}

fn register_json(vm: &mut Vm) {
    let json_obj = vm.alloc_object();

    let stringify_fn = vm.alloc_native_function("stringify", 3, |vm, _this, args| {
        let value = args.first().copied().unwrap_or(JsValue::undefined());
        let replacer = args.get(1).copied().filter(|v| v.is_object());
        let space_val = args.get(2).copied().unwrap_or(JsValue::undefined());

        let replacer_fn = replacer.filter(|r| vm.is_callable(*r));
        let replacer_arr: Option<Vec<String>> = replacer
            .filter(|r| !vm.is_callable(*r))
            .and_then(|r| {
                if let Some(ptr) = r.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    if let ObjectKind::Array(elements) = &obj.kind {
                        let mut keys = Vec::new();
                        for elem in elements {
                            if elem.is_string() || elem.is_number() {
                                keys.push(vm.to_string_value(*elem));
                            }
                        }
                        return Some(keys);
                    }
                }
                None
            });

        let gap = if space_val.is_number() {
            let n = space_val.to_number().unwrap_or(0.0).max(0.0).min(10.0) as usize;
            " ".repeat(n)
        } else if space_val.is_string() {
            let s = vm.to_string_value(space_val);
            s[..s.len().min(10)].to_string()
        } else {
            String::new()
        };

        let mut stack = Vec::new();
        let wrapper = vm.alloc_object();
        vm.set_property(wrapper, "", value);

        let res = json_stringify_walk(
            vm,
            wrapper,
            "",
            value,
            &mut stack,
            replacer_fn,
            replacer_arr.as_deref(),
            &gap,
            "",
        )?;

        match res {
            Some(s) => Ok(vm.alloc_string(s)),
            None => Ok(JsValue::undefined()),
        }
    });

    let parse_fn = vm.alloc_native_function("parse", 2, |vm, _this, args| {
        let text_arg = args.first().copied().unwrap_or(JsValue::undefined());
        if text_arg.is_symbol() {
            return Err(VmError::TypeError("Cannot convert a Symbol value to a string".into()));
        }
        let text = vm.to_string_value(text_arg);
        let reviver = args.get(1).copied().filter(|r| vm.is_callable(*r));

        let mut parser = JsonParser::new(&text);
        let val = parser.parse_value(vm)?;
        parser.skip_whitespace();
        if parser.pos != parser.chars.len() {
            return Err(VmError::SyntaxError("Unexpected non-whitespace after JSON input".into()));
        }

        if let Some(rf) = reviver {
            let root = vm.alloc_object();
            vm.set_property(root, "", val);
            json_walk(vm, rf, root, "")
        } else {
            Ok(val)
        }
    });

    vm.set_property(json_obj, "stringify", stringify_fn);
    vm.set_property(json_obj, "parse", parse_fn);

    vm.globals.insert("JSON".to_string(), json_obj);
}

fn get_enumerable_keys(vm: &Vm, arg: JsValue) -> Vec<String> {
    let mut raw_names: Vec<String> = Vec::new();
    if let Some(ptr) = arg.as_object_ptr() {
        let obj = unsafe { &*(ptr as *mut JsObject) };
        if let Some(accs) = &obj.accessors {
            for k in accs.keys() {
                if !vm.non_enumerable_props.contains(&(arg, k.clone())) && !raw_names.contains(k) {
                    raw_names.push(k.clone());
                }
            }
        }
        for prop in vm.shapes.all_properties(obj.shape_id) {
            if !vm.non_enumerable_props.contains(&(arg, prop.clone())) && !raw_names.contains(&prop) {
                raw_names.push(prop);
            }
        }
        if vm.module_namespace_objects.contains(&arg) {
            for ((ns, re_name), _) in &vm.module_reexports {
                if *ns == arg && !raw_names.contains(re_name) {
                    raw_names.push(re_name.clone());
                }
            }
            raw_names.sort();
        }
    }
    let mut int_keys: Vec<(u32, String)> = Vec::new();
    let mut str_keys: Vec<String> = Vec::new();
    for k in raw_names {
        if let Ok(idx) = k.parse::<u32>() {
            if idx < u32::MAX && idx.to_string() == k {
                int_keys.push((idx, k));
                continue;
            }
        }
        str_keys.push(k);
    }
    int_keys.sort_by_key(|(idx, _)| *idx);
    let mut ordered = Vec::new();
    for (_, k) in int_keys {
        ordered.push(k);
    }
    for k in str_keys {
        ordered.push(k);
    }
    ordered
}

fn register_symbol(vm: &mut Vm) {
    let symbol_proto = vm.alloc_object();
    vm.symbol_proto = Some(symbol_proto);

    let sym_value_of = vm.alloc_native_function("valueOf", 0, |_vm, this, _args| {
        if this.is_symbol() {
            return Ok(this);
        }
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if matches!(obj.kind, ObjectKind::Symbol(_)) {
                return Ok(unsafe { JsValue::from_symbol_ptr(ptr) });
            }
        }
        Err(VmError::TypeError("Symbol.prototype.valueOf requires that 'this' be a Symbol".into()))
    });
    let sym_to_string = vm.alloc_native_function("toString", 0, |vm, this, _args| {
        if this.is_symbol() {
            return Ok(vm.alloc_string(vm.to_string_value(this)));
        }
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::Symbol(desc) = &obj.kind {
                return Ok(vm.alloc_string(format!("Symbol({})", desc.as_deref().unwrap_or(""))));
            }
        }
        Err(VmError::TypeError("Symbol.prototype.toString requires that 'this' be a Symbol".into()))
    });
    vm.set_property(symbol_proto, "valueOf", sym_value_of);
    vm.set_property(symbol_proto, "toString", sym_to_string);

    let symbol_ctor = vm.alloc_native_function("Symbol", 1, |vm, _this, args| {
        if vm.is_constructing {
            return Err(VmError::TypeError("Symbol is not a constructor".into()));
        }
        let desc = args.first().map(|v| vm.to_string_value(*v));
        Ok(vm.alloc_symbol(desc))
    });
    vm.set_property(symbol_proto, "constructor", symbol_ctor);
    vm.non_enumerable_props.insert((symbol_proto, "constructor".to_string()));
    vm.set_property(symbol_ctor, "prototype", symbol_proto);
    vm.non_enumerable_props.insert((symbol_ctor, "prototype".to_string()));

    let symbol_for = vm.alloc_native_function("for", 1, |vm, _this, args| {
        let key = args.first().map(|v| vm.to_string_value(*v)).unwrap_or_default();
        if let Some(existing) = vm.symbol_registry.get(&key) {
            return Ok(*existing);
        }
        let sym = vm.alloc_symbol(Some(key.clone()));
        vm.symbol_registry.insert(key, sym);
        Ok(sym)
    });
    vm.set_property(symbol_ctor, "for", symbol_for);

    let symbol_key_for = vm.alloc_native_function("keyFor", 1, |vm, _this, args| {
        let sym = args.first().copied().unwrap_or(JsValue::undefined());
        if !sym.is_symbol() {
            return Err(VmError::TypeError("Symbol.keyFor requires that argument be a Symbol".into()));
        }
        for (k, v) in &vm.symbol_registry {
            if *v == sym {
                return Ok(vm.alloc_string(k.clone()));
            }
        }
        Ok(JsValue::undefined())
    });
    vm.set_property(symbol_ctor, "keyFor", symbol_key_for);

    // Standard well-known symbols
    let sym_to_prim = vm.alloc_symbol(Some("Symbol.toPrimitive".into()));
    let sym_iter = vm.alloc_symbol(Some("Symbol.iterator".into()));
    let sym_async_iter = vm.alloc_symbol(Some("Symbol.asyncIterator".into()));
    let sym_has_instance = vm.alloc_symbol(Some("Symbol.hasInstance".into()));
    let sym_concat_spread = vm.alloc_symbol(Some("Symbol.isConcatSpreadable".into()));
    let sym_match = vm.alloc_symbol(Some("Symbol.match".into()));
    let sym_match_all = vm.alloc_symbol(Some("Symbol.matchAll".into()));
    let sym_replace = vm.alloc_symbol(Some("Symbol.replace".into()));
    let sym_search = vm.alloc_symbol(Some("Symbol.search".into()));
    let sym_species = vm.alloc_symbol(Some("Symbol.species".into()));
    let sym_split = vm.alloc_symbol(Some("Symbol.split".into()));
    let sym_tag = vm.alloc_symbol(Some("Symbol.toStringTag".into()));
    let sym_unscopables = vm.alloc_symbol(Some("Symbol.unscopables".into()));

    vm.set_property(symbol_ctor, "toPrimitive", sym_to_prim);
    vm.set_property(symbol_ctor, "iterator", sym_iter);
    vm.set_property(symbol_ctor, "asyncIterator", sym_async_iter);
    vm.set_property(symbol_ctor, "hasInstance", sym_has_instance);
    vm.set_property(symbol_ctor, "isConcatSpreadable", sym_concat_spread);
    vm.set_property(symbol_ctor, "match", sym_match);
    vm.set_property(symbol_ctor, "matchAll", sym_match_all);
    vm.set_property(symbol_ctor, "replace", sym_replace);
    vm.set_property(symbol_ctor, "search", sym_search);
    vm.set_property(symbol_ctor, "species", sym_species);
    vm.set_property(symbol_ctor, "split", sym_split);
    vm.set_property(symbol_ctor, "toStringTag", sym_tag);
    vm.set_property(symbol_ctor, "unscopables", sym_unscopables);

    vm.symbol_iterator = Some(sym_iter);
    vm.symbol_to_string_tag = Some(sym_tag);
    vm.symbol_is_concat_spreadable = Some(sym_concat_spread);
    vm.symbol_species = Some(sym_species);
    vm.symbol_to_primitive = Some(sym_to_prim);

    vm.globals.insert("Symbol".to_string(), symbol_ctor);
    if vm.global_object.is_object() {
        vm.set_property(vm.global_object, "Symbol", symbol_ctor);
    }
}

pub(crate) fn add_species_getter(vm: &mut Vm, ctor: JsValue) {
    if let Some(sym_species) = vm.symbol_species {
        let species_getter = vm.alloc_native_function("get [Symbol.species]", 0, |_vm, this, _args| Ok(this));
        if let Some(ptr) = ctor.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            obj.insert_symbol_accessor_getter(sym_species, species_getter);
        }
    }
}

pub(crate) fn iterate_to_vec(vm: &mut Vm, iterable: JsValue) -> Result<Vec<JsValue>, VmError> {
    if iterable.is_nullish() {
        return Err(VmError::TypeError("Value is not iterable".into()));
    }
    if let Some(ptr) = iterable.as_object_ptr() {
        let obj = unsafe { &*(ptr as *mut JsObject) };
        if let ObjectKind::Array(arr) = &obj.kind {
            let has_custom_iter = obj.symbol_properties.as_ref().map_or(false, |sp| {
                vm.symbol_iterator.map_or(false, |si| sp.contains_key(&si))
            });
            if !has_custom_iter {
                return Ok(arr.clone());
            }
        }
    }

    let sym_iter = vm.symbol_iterator.ok_or_else(|| VmError::TypeError("No Symbol.iterator".into()))?;
    let method = vm.get_symbol_property(iterable, sym_iter);
    if !vm.is_callable(method) {
        return Err(VmError::TypeError("Value is not iterable".into()));
    }
    let it = vm.call(method, iterable, &[])?;
    let next_fn = vm.get_property(it, "next");
    if !vm.is_callable(next_fn) {
        return Err(VmError::TypeError("Iterator next is not callable".into()));
    }
    let mut res = Vec::new();
    loop {
        let step = vm.call(next_fn, it, &[])?;
        let done = vm.get_property(step, "done");
        if done.is_truthy() {
            break;
        }
        let val = vm.get_property(step, "value");
        res.push(val);
    }
    Ok(res)
}

fn define_property_on_module_namespace(vm: &mut Vm, ns: JsValue, key: &PropertyKeyVal, desc: JsValue) -> Result<bool, VmError> {
    match key {
        PropertyKeyVal::Symbol(sym) => {
            if vm.symbol_to_string_tag == Some(*sym) {
                let has_val = vm.has_property(desc, "value").unwrap_or(false);
                let val = vm.get_property(desc, "value");
                let has_writ = vm.has_property(desc, "writable").unwrap_or(false);
                let writ = vm.get_property(desc, "writable");
                let has_enum = vm.has_property(desc, "enumerable").unwrap_or(false);
                let en = vm.get_property(desc, "enumerable");
                let has_conf = vm.has_property(desc, "configurable").unwrap_or(false);
                let conf = vm.get_property(desc, "configurable");
                if (has_val && vm.to_string_value(val) != "Module")
                    || (has_writ && writ.is_truthy())
                    || (has_enum && en.is_truthy())
                    || (has_conf && conf.is_truthy())
                    || vm.has_property(desc, "get").unwrap_or(false)
                    || vm.has_property(desc, "set").unwrap_or(false)
                {
                    return Ok(false);
                }
                return Ok(true);
            }
            Ok(false)
        }
        PropertyKeyVal::String(name) => {
            let has_prop = if let Some(ptr) = ns.as_object_ptr() {
                let obj = unsafe { &*(ptr as *mut JsObject) };
                vm.shapes.has_property(obj.shape_id, name) || vm.module_reexports.contains_key(&(ns, name.clone()))
            } else {
                false
            };
            if !has_prop {
                return Ok(false);
            }
            if vm.has_property(desc, "get").unwrap_or(false) || vm.has_property(desc, "set").unwrap_or(false) {
                return Ok(false);
            }
            if vm.has_property(desc, "configurable").unwrap_or(false) && vm.get_property(desc, "configurable").is_truthy() {
                return Ok(false);
            }
            if vm.has_property(desc, "enumerable").unwrap_or(false) && !vm.get_property(desc, "enumerable").is_truthy() {
                return Ok(false);
            }
            if vm.has_property(desc, "writable").unwrap_or(false) && !vm.get_property(desc, "writable").is_truthy() {
                return Ok(false);
            }
            if vm.has_property(desc, "value").unwrap_or(false) {
                let req_val = vm.get_property(desc, "value");
                let curr_val = vm.get_property(ns, name);
                if !vm.same_value(curr_val, req_val) {
                    return Ok(false);
                }
            }
            Ok(true)
        }
    }
}

fn register_object_builtins(vm: &mut Vm) {
    let object_constructor = vm.alloc_native_function("Object", 1, |vm, _this, args| {
        if let Some(arg) = args.first() {
            if !arg.is_nullish() {
                return vm.to_object_value(*arg);
            }
        }
        Ok(vm.alloc_object())
    });

    let keys_fn = vm.alloc_native_function("keys", 1, |vm, _this, args| {
        let arg = match args.first() {
            Some(a) if a.is_object() => *a,
            _ => return Err(VmError::TypeError("Object.keys called on non-object".into())),
        };
        let keys_to_alloc = get_enumerable_keys(vm, arg);
        if vm.module_namespace_objects.contains(&arg) {
            for k in &keys_to_alloc {
                if vm.get_property(arg, k) == JsValue::uninitialized() {
                    return Err(VmError::ReferenceError(format!("Cannot access uninitialized export '{}'", k)));
                }
            }
        }
        let keys_vec: Vec<JsValue> = keys_to_alloc.into_iter().map(|k| vm.alloc_string(k)).collect();
        Ok(vm.alloc_array(keys_vec))
    });

    let values_fn = vm.alloc_native_function("values", 1, |vm, _this, args| {
        let arg = match args.first() {
            Some(a) if a.is_object() => *a,
            _ => return Err(VmError::TypeError("Object.values called on non-object".into())),
        };
        let keys_to_alloc = get_enumerable_keys(vm, arg);
        if vm.module_namespace_objects.contains(&arg) {
            for k in &keys_to_alloc {
                if vm.get_property(arg, k) == JsValue::uninitialized() {
                    return Err(VmError::ReferenceError(format!("Cannot access uninitialized export '{}'", k)));
                }
            }
        }
        let vals_vec: Vec<JsValue> = keys_to_alloc.into_iter().map(|k| vm.get_property(arg, &k)).collect();
        Ok(vm.alloc_array(vals_vec))
    });

    let entries_fn = vm.alloc_native_function("entries", 1, |vm, _this, args| {
        let arg = match args.first() {
            Some(a) if a.is_object() => *a,
            _ => return Err(VmError::TypeError("Object.entries called on non-object".into())),
        };
        let keys_to_alloc = get_enumerable_keys(vm, arg);
        if vm.module_namespace_objects.contains(&arg) {
            for k in &keys_to_alloc {
                if vm.get_property(arg, k) == JsValue::uninitialized() {
                    return Err(VmError::ReferenceError(format!("Cannot access uninitialized export '{}'", k)));
                }
            }
        }
        let entries_vec: Vec<JsValue> = keys_to_alloc
            .into_iter()
            .map(|k| {
                let v = vm.get_property(arg, &k);
                let k_val = vm.alloc_string(k);
                vm.alloc_array(vec![k_val, v])
            })
            .collect();
        Ok(vm.alloc_array(entries_vec))
    });

    let assign_fn = vm.alloc_native_function("assign", 2, |vm, _this, args| {
        let target = if let Some(t) = args.first() {
            *t
        } else {
            return Err(VmError::TypeError("Cannot convert undefined or null to object".into()));
        };

        for source in args.iter().skip(1) {
            if let Some(src_ptr) = source.as_object_ptr() {
                let src_obj = unsafe { &*(src_ptr as *mut JsObject) };
                let props: Vec<(String, JsValue)> = vm
                    .shapes
                    .all_property_entries(src_obj.shape_id)
                    .into_iter()
                    .filter_map(|(k, offset)| src_obj.get_slot(offset).map(|v| (k, v)))
                    .collect();

                for (k, v) in props {
                    vm.set_property(target, &k, v);
                }
            }
        }
        Ok(target)
    });

    let object_proto = vm.alloc_object();
    vm.object_proto = Some(object_proto);
    if let Some(ptr) = vm.global_object.as_object_ptr() {
        let obj = unsafe { &mut *(ptr as *mut JsObject) };
        obj.prototype = Some(object_proto);
    }

    vm.set_property(object_proto, "constructor", object_constructor);
    vm.non_enumerable_props.insert((object_proto, "constructor".to_string()));

    let is_prototype_of_fn = vm.alloc_native_function("isPrototypeOf", 1, |vm, this, args| {
        if !this.is_object() {
            return Ok(JsValue::from_boolean(false));
        }
        let target = match args.first() {
            Some(t) if t.is_object() => *t,
            _ => return Ok(JsValue::from_boolean(false)),
        };
        let mut current = vm.get_prototype_of(target);
        while let Some(proto) = current {
            if proto == this {
                return Ok(JsValue::from_boolean(true));
            }
            current = vm.get_prototype_of(proto);
        }
        Ok(JsValue::from_boolean(false))
    });
    vm.set_property(object_proto, "isPrototypeOf", is_prototype_of_fn);

    let value_of_fn = vm.alloc_native_function("valueOf", 0, |_vm, this, _args| {
        Ok(this)
    });
    vm.set_property(object_proto, "valueOf", value_of_fn);

    // Create Function.prototype early so all native functions get Function.prototype
    let native = crate::object::NativeFunction::new_fn("".to_string(), 0, |_vm, _this, _args| Ok(JsValue::undefined()));
    let fn_proto_obj = Box::new(JsObject::new_native_function(crate::shape::ShapeRegistry::ROOT_SHAPE_ID, Some(object_proto), native));
    let fn_proto_ptr = Box::into_raw(fn_proto_obj);
    vm.track_ptr(fn_proto_ptr);
    let function_proto = unsafe { JsValue::from_object_ptr(fn_proto_ptr as *mut u8) };
    vm.function_proto = Some(function_proto);
    let to_string_fn = vm.alloc_native_function("toString", 0, |vm, this, _args| {
        if let Some(ptr) = this.as_object_ptr() {
            if let Some(sym_tag) = vm.symbol_to_string_tag {
                let tag_val = vm.get_symbol_property(this, sym_tag);
                if tag_val.is_string() {
                    let s = vm.to_string_value(tag_val);
                    return Ok(vm.alloc_string(format!("[object {}]", s)));
                }
            }
            let obj = unsafe { &*(ptr as *mut JsObject) };
            let tag = match &obj.kind {
                ObjectKind::Array(_) => "Array",
                ObjectKind::Function(_) | ObjectKind::NativeFunction(_) => "Function",
                ObjectKind::Error { .. } => "Error",
                ObjectKind::RegExp { .. } => "RegExp",
                ObjectKind::Date(_) => "Date",
                ObjectKind::Map(_) => "Map",
                ObjectKind::Set(_) => "Set",
                ObjectKind::WeakMap(_) => "WeakMap",
                ObjectKind::WeakSet(_) => "WeakSet",
                ObjectKind::BigInt(_) => "BigInt",
                ObjectKind::String(_) | ObjectKind::StringObject(_) => "String",
                ObjectKind::NumberObject(_) => "Number",
                ObjectKind::BooleanObject(_) => "Boolean",
                _ => "Object",
            };
            Ok(vm.alloc_string(format!("[object {}]", tag)))
        } else if this.is_null() {
            Ok(vm.alloc_string("[object Null]".into()))
        } else if this.is_undefined() {
            Ok(vm.alloc_string("[object Undefined]".into()))
        } else if this.is_string() {
            Ok(vm.alloc_string("[object String]".into()))
        } else if this.is_number() {
            Ok(vm.alloc_string("[object Number]".into()))
        } else if this.is_boolean() {
            Ok(vm.alloc_string("[object Boolean]".into()))
        } else {
            Ok(vm.alloc_string("[object Object]".into()))
        }
    });
    vm.set_property(object_proto, "toString", to_string_fn);

    let to_locale_string_fn = vm.alloc_native_function("toLocaleString", 0, |vm, this, _args| {
        let to_str = vm.get_property(this, "toString");
        if vm.is_callable(to_str) {
            vm.call(to_str, this, &[])
        } else {
            Err(VmError::TypeError("toString is not callable".into()))
        }
    });
    vm.set_property(object_proto, "toLocaleString", to_locale_string_fn);
    vm.non_enumerable_props.insert((object_proto, "toLocaleString".to_string()));

    let proto_getter = vm.alloc_native_function("get __proto__", 0, |vm, this, _args| {
        if this.is_nullish() {
            return Err(VmError::TypeError("Cannot read property of null or undefined".into()));
        }
        let o = if this.is_object() {
            this
        } else {
            vm.to_object_value(this)?
        };
        if let Some(ptr) = o.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            Ok(obj.prototype.unwrap_or(JsValue::null()))
        } else {
            Ok(JsValue::null())
        }
    });

    let proto_setter = vm.alloc_native_function("set __proto__", 1, |vm, this, args| {
        if this.is_nullish() {
            return Err(VmError::TypeError("Cannot set property on null or undefined".into()));
        }
        let proto = args.first().copied().unwrap_or(JsValue::undefined());
        if !proto.is_object() && !proto.is_null() {
            return Ok(JsValue::undefined());
        }
        if !this.is_object() {
            return Ok(JsValue::undefined());
        }
        let ptr = this.as_object_ptr().unwrap();
        let obj = unsafe { &mut *(ptr as *mut JsObject) };
        let new_proto = if proto.is_null() { None } else { Some(proto) };
        if obj.prototype == new_proto {
            return Ok(JsValue::undefined());
        }
        if vm.non_extensible_objects.contains(&this) {
            return Err(VmError::TypeError("Cannot set prototype of non-extensible object".into()));
        }
        obj.prototype = new_proto;
        Ok(JsValue::undefined())
    });

    if let Some(ptr) = object_proto.as_object_ptr() {
        let obj = unsafe { &mut *(ptr as *mut JsObject) };
        obj.define_accessor("__proto__".to_string(), Some(proto_getter), Some(proto_setter));
    }
    vm.non_enumerable_props.insert((object_proto, "__proto__".to_string()));

    let lookup_getter = vm.alloc_native_function("__lookupGetter__", 1, |vm, this, args| {
        let o = if this.is_object() { this } else { vm.to_object_value(this)? };
        let prop = args.first().copied().unwrap_or(JsValue::undefined());
        let key = vm.to_property_key(prop)?;
        let mut curr = Some(o);
        while let Some(c_val) = curr {
            if let Some(c_ptr) = c_val.as_object_ptr() {
                let c_obj = unsafe { &*(c_ptr as *mut JsObject) };
                match &key {
                    PropertyKeyVal::Symbol(sym) => {
                        if let Some(accs) = &c_obj.symbol_accessors {
                            if let Some(acc) = accs.get(sym) {
                                return Ok(acc.getter.unwrap_or(JsValue::undefined()));
                            }
                        }
                        if let Some(syms) = &c_obj.symbol_properties {
                            if syms.contains_key(sym) {
                                return Ok(JsValue::undefined());
                            }
                        }
                    }
                    PropertyKeyVal::String(s) => {
                        if let Some(accs) = &c_obj.accessors {
                            if let Some(acc) = accs.get(s) {
                                return Ok(acc.getter.unwrap_or(JsValue::undefined()));
                            }
                        }
                        if vm.shapes.has_property(c_obj.shape_id, s) {
                            return Ok(JsValue::undefined());
                        }
                    }
                }
                curr = c_obj.prototype;
            } else {
                break;
            }
        }
        Ok(JsValue::undefined())
    });
    vm.set_property(object_proto, "__lookupGetter__", lookup_getter);
    vm.non_enumerable_props.insert((object_proto, "__lookupGetter__".to_string()));

    let lookup_setter = vm.alloc_native_function("__lookupSetter__", 1, |vm, this, args| {
        let o = if this.is_object() { this } else { vm.to_object_value(this)? };
        let prop = args.first().copied().unwrap_or(JsValue::undefined());
        let key = vm.to_property_key(prop)?;
        let mut curr = Some(o);
        while let Some(c_val) = curr {
            if let Some(c_ptr) = c_val.as_object_ptr() {
                let c_obj = unsafe { &*(c_ptr as *mut JsObject) };
                match &key {
                    PropertyKeyVal::Symbol(sym) => {
                        if let Some(accs) = &c_obj.symbol_accessors {
                            if let Some(acc) = accs.get(sym) {
                                return Ok(acc.setter.unwrap_or(JsValue::undefined()));
                            }
                        }
                        if let Some(syms) = &c_obj.symbol_properties {
                            if syms.contains_key(sym) {
                                return Ok(JsValue::undefined());
                            }
                        }
                    }
                    PropertyKeyVal::String(s) => {
                        if let Some(accs) = &c_obj.accessors {
                            if let Some(acc) = accs.get(s) {
                                return Ok(acc.setter.unwrap_or(JsValue::undefined()));
                            }
                        }
                        if vm.shapes.has_property(c_obj.shape_id, s) {
                            return Ok(JsValue::undefined());
                        }
                    }
                }
                curr = c_obj.prototype;
            } else {
                break;
            }
        }
        Ok(JsValue::undefined())
    });
    vm.set_property(object_proto, "__lookupSetter__", lookup_setter);
    vm.non_enumerable_props.insert((object_proto, "__lookupSetter__".to_string()));

    let define_getter_fn = vm.alloc_native_function("__defineGetter__", 2, |vm, this, args| {
        let o = if this.is_object() { this } else { vm.to_object_value(this)? };
        let getter = args.get(1).copied().unwrap_or(JsValue::undefined());
        if !vm.is_callable(getter) {
            return Err(VmError::TypeError("Getter must be a function".into()));
        }
        let prop = args.first().copied().unwrap_or(JsValue::undefined());
        let key = vm.to_property_key(prop)?;
        if let Some(ptr) = o.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            match key {
                PropertyKeyVal::Symbol(sym) => {
                    let accs = obj.symbol_accessors.get_or_insert_with(|| Box::new(std::collections::HashMap::new()));
                    accs.entry(sym).or_default().getter = Some(getter);
                }
                PropertyKeyVal::String(s) => {
                    obj.define_getter(s, getter);
                }
            }
        }
        Ok(JsValue::undefined())
    });
    vm.set_property(object_proto, "__defineGetter__", define_getter_fn);
    vm.non_enumerable_props.insert((object_proto, "__defineGetter__".to_string()));

    let define_setter_fn = vm.alloc_native_function("__defineSetter__", 2, |vm, this, args| {
        let o = if this.is_object() { this } else { vm.to_object_value(this)? };
        let setter = args.get(1).copied().unwrap_or(JsValue::undefined());
        if !vm.is_callable(setter) {
            return Err(VmError::TypeError("Setter must be a function".into()));
        }
        let prop = args.first().copied().unwrap_or(JsValue::undefined());
        let key = vm.to_property_key(prop)?;
        if let Some(ptr) = o.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            match key {
                PropertyKeyVal::Symbol(sym) => {
                    let accs = obj.symbol_accessors.get_or_insert_with(|| Box::new(std::collections::HashMap::new()));
                    accs.entry(sym).or_default().setter = Some(setter);
                }
                PropertyKeyVal::String(s) => {
                    obj.define_setter(s, setter);
                }
            }
        }
        Ok(JsValue::undefined())
    });
    vm.set_property(object_proto, "__defineSetter__", define_setter_fn);
    vm.non_enumerable_props.insert((object_proto, "__defineSetter__".to_string()));

    let has_own_prop = vm.alloc_native_function("hasOwnProperty", 1, |vm, this, args| {
        if this.is_nullish() {
            return Err(VmError::TypeError("Object.prototype.hasOwnProperty called on null or undefined".into()));
        }
        let prop = args.first().copied().unwrap_or(JsValue::undefined());
        let key = vm.to_property_key(prop)?;
        if vm.module_namespace_objects.contains(&this) {
            if let PropertyKeyVal::String(s) = &key {
                if vm.has_own_property_key(this, &key) && vm.get_property(this, s) == JsValue::uninitialized() {
                    return Err(VmError::ReferenceError(format!("Cannot access uninitialized export '{}'", s)));
                }
            }
        }
        Ok(JsValue::from_boolean(vm.has_own_property_key(this, &key)))
    });
    vm.set_property(object_proto, "hasOwnProperty", has_own_prop);

    let prop_is_enumerable = vm.alloc_native_function("propertyIsEnumerable", 1, |vm, this, args| {
        let key = args.first().map(|v| vm.to_string_value(*v)).unwrap_or_default();
        if vm.module_namespace_objects.contains(&this) {
            if vm.has_property(this, &key)? && vm.get_property(this, &key) == JsValue::uninitialized() {
                return Err(VmError::ReferenceError(format!("Cannot access uninitialized export '{}'", key)));
            }
        }
        if vm.deleted_props.contains(&(this, key.clone())) {
            return Ok(JsValue::from_boolean(false));
        }
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if match &obj.kind {
                ObjectKind::Array(elements) => {
                    if key == "length" {
                        return Ok(JsValue::from_boolean(false));
                    }
                    if key.parse::<usize>().map(|idx| idx < elements.len()).unwrap_or(false) {
                        return Ok(JsValue::from_boolean(true));
                    }
                    false
                }
                ObjectKind::String(s) | ObjectKind::StringObject(s) => {
                    if key == "length" {
                        return Ok(JsValue::from_boolean(false));
                    }
                    if key.parse::<usize>().map(|idx| idx < s.chars().count()).unwrap_or(false) {
                        return Ok(JsValue::from_boolean(true));
                    }
                    false
                }
                ObjectKind::TypedArray(ta) => {
                    if matches!(key.as_str(), "length" | "byteLength" | "byteOffset" | "buffer") {
                        return Ok(JsValue::from_boolean(false));
                    }
                    if key.parse::<usize>().map(|idx| idx < ta.length).unwrap_or(false) {
                        return Ok(JsValue::from_boolean(true));
                    }
                    false
                }
                _ => false,
            } {
                return Ok(JsValue::from_boolean(true));
            }
            let has = vm.shapes.has_property(obj.shape_id, &key)
                || obj.accessors.as_ref().map(|a| a.contains_key(&key)).unwrap_or(false);
            if has {
                let is_enum = !vm.non_enumerable_props.contains(&(this, key));
                Ok(JsValue::from_boolean(is_enum))
            } else {
                Ok(JsValue::from_boolean(false))
            }
        } else {
            Ok(JsValue::from_boolean(false))
        }
    });
    vm.set_property(object_proto, "propertyIsEnumerable", prop_is_enumerable);

    let group_by_fn = vm.alloc_native_function("groupBy", 2, |vm, _this, args| {
        let items = args.first().copied().unwrap_or(JsValue::undefined());
        let callback = args.get(1).copied().unwrap_or(JsValue::undefined());
        if !vm.is_callable(callback) {
            return Err(VmError::TypeError("Object.groupBy callback must be callable".into()));
        }
        let out_obj = vm.alloc_object();
        let len_val = vm.get_property(items, "length");
        let raw_len = len_val.to_number().unwrap_or(0.0);
        if raw_len > 4_294_967_295.0 || raw_len < 0.0 || raw_len.is_infinite() {
            return Err(VmError::RangeError("Invalid array length".into()));
        }
        let len = raw_len as usize;
        if len > 100_000 {
            return Err(VmError::RangeError("Invalid array length".into()));
        }
        for i in 0..len {
            let item = vm.get_element(items, i);
            let key_val = vm.call(callback, JsValue::undefined(), &[item, JsValue::from_int32(i as i32)])?;
            let key_str = vm.to_string_value(key_val);
            let existing = vm.get_property(out_obj, &key_str);
            if existing.is_undefined() {
                let arr = vm.alloc_array(vec![item]);
                vm.set_property(out_obj, &key_str, arr);
            } else if let Some(ptr) = existing.as_object_ptr() {
                let arr_obj = unsafe { &mut *(ptr as *mut JsObject) };
                if let ObjectKind::Array(ref mut elems) = arr_obj.kind {
                    elems.push(item);
                }
            }
        }
        Ok(out_obj)
    });

    let define_property_fn = vm.alloc_native_function("defineProperty", 3, |vm, _this, args| {
        let target = match args.first() {
            Some(t) if t.is_object() => *t,
            _ => return Err(VmError::TypeError("Object.defineProperty called on non-object".into())),
        };
        let prop_key = match args.get(1) {
            Some(p) => vm.to_property_key(*p)?,
            None => PropertyKeyVal::String("undefined".to_string()),
        };
        let desc = match args.get(2) {
            Some(d) if d.is_object() => *d,
            _ => return Err(VmError::TypeError("Property description must be an object".into())),
        };

        if vm.module_namespace_objects.contains(&target) {
            let success = define_property_on_module_namespace(vm, target, &prop_key, desc)?;
            if success {
                return Ok(target);
            } else {
                return Err(VmError::TypeError("Cannot redefine property on module namespace object".into()));
            }
        }

        let get_val = vm.get_property(desc, "get");
        let set_val = vm.get_property(desc, "set");
        let has_get = !get_val.is_undefined();
        let has_set = !set_val.is_undefined();

        match prop_key {
            PropertyKeyVal::Symbol(sym) => {
                if has_get || has_set {
                    if let Some(ptr) = target.as_object_ptr() {
                        let obj = unsafe { &mut *(ptr as *mut JsObject) };
                        let accs = obj.symbol_accessors.get_or_insert_with(|| Box::new(std::collections::HashMap::new()));
                        let entry = accs.entry(sym).or_insert(Accessor { getter: None, setter: None });
                        if has_get {
                            if !vm.is_callable(get_val) {
                                return Err(VmError::TypeError("Getter must be a function".into()));
                            }
                            entry.getter = Some(get_val);
                        }
                        if has_set {
                            if !vm.is_callable(set_val) {
                                return Err(VmError::TypeError("Setter must be a function".into()));
                            }
                            entry.setter = Some(set_val);
                        }
                        let order = obj.symbol_order.get_or_insert_with(Vec::new);
                        if !order.contains(&sym) {
                            order.push(sym);
                        }
                    }
                } else {
                    let val = vm.get_property(desc, "value");
                    if let Some(ptr) = target.as_object_ptr() {
                        let obj = unsafe { &mut *(ptr as *mut JsObject) };
                        obj.insert_symbol_property(sym, val);
                    }
                }
            }
            PropertyKeyVal::String(prop_name) => {
                let is_non_configurable = vm.non_configurable_props.contains(&(target, prop_name.clone()));
                if is_non_configurable {
                    let conf_val = vm.get_property(desc, "configurable");
                    if !conf_val.is_undefined() && conf_val.is_truthy() {
                        return Err(VmError::TypeError("Cannot redefine property: configurable is false".into()));
                    }
                    let has_accessor = if let Some(ptr) = target.as_object_ptr() {
                        let obj = unsafe { &*(ptr as *mut JsObject) };
                        obj.accessors.as_ref().map_or(false, |accs| accs.contains_key(&prop_name))
                    } else {
                        false
                    };
                    if has_accessor && (vm.has_property(desc, "value").unwrap_or(false) || vm.has_property(desc, "writable").unwrap_or(false)) {
                        return Err(VmError::TypeError("Cannot redefine accessor property as data property".into()));
                    }
                    if !has_accessor && (has_get || has_set) {
                        return Err(VmError::TypeError("Cannot redefine data property as accessor property".into()));
                    }
                    let is_non_writable = vm.non_writable_props.contains(&(target, prop_name.clone()));
                    if is_non_writable {
                        let writ_val = vm.get_property(desc, "writable");
                        if !writ_val.is_undefined() && writ_val.is_truthy() {
                            return Err(VmError::TypeError("Cannot redefine property: writable is false".into()));
                        }
                        if vm.has_property(desc, "value").unwrap_or(false) {
                            let new_val = vm.get_property(desc, "value");
                            let curr_val = vm.get_property(target, &prop_name);
                            if !vm.same_value(curr_val, new_val) {
                                return Err(VmError::TypeError("Cannot change value of non-configurable non-writable property".into()));
                            }
                        }
                    }
                }
                if has_get || has_set {
                    if let Some(ptr) = target.as_object_ptr() {
                        let obj = unsafe { &mut *(ptr as *mut JsObject) };
                        if has_get {
                            if !vm.is_callable(get_val) {
                                return Err(VmError::TypeError("Getter must be a function".into()));
                            }
                            obj.define_getter(prop_name.clone(), get_val);
                        }
                        if has_set {
                            if !vm.is_callable(set_val) {
                                return Err(VmError::TypeError("Setter must be a function".into()));
                            }
                            obj.define_setter(prop_name.clone(), set_val);
                        }
                    }
                } else {
                    let has_val = vm.has_property(desc, "value").unwrap_or(false);
                    if has_val || !vm.has_own_property(target, &prop_name) {
                        let val = vm.get_property(desc, "value");
                        vm.set_own_property(target, &prop_name, val);
                        if target == vm.global_object {
                            vm.globals.insert(prop_name.clone(), val);
                        }
                    }
                }

                let conf_val = vm.get_property(desc, "configurable");
                if !conf_val.is_undefined() {
                    let is_conf = conf_val.is_truthy();
                    if !is_conf {
                        vm.non_configurable_props.insert((target, prop_name.clone()));
                    } else {
                        vm.non_configurable_props.remove(&(target, prop_name.clone()));
                    }
                }

                let enum_val = vm.get_property(desc, "enumerable");
                if !enum_val.is_undefined() {
                    let is_enum = enum_val.is_truthy();
                    if !is_enum {
                        vm.non_enumerable_props.insert((target, prop_name.clone()));
                    } else {
                        vm.non_enumerable_props.remove(&(target, prop_name.clone()));
                    }
                }

                let writ_val = vm.get_property(desc, "writable");
                if !writ_val.is_undefined() {
                    let is_writ = writ_val.is_truthy();
                    if !is_writ {
                        vm.non_writable_props.insert((target, prop_name.clone()));
                        vm.sync_mapped_arguments_delete(target, &prop_name);
                    } else {
                        vm.non_writable_props.remove(&(target, prop_name.clone()));
                    }
                }
            }
        }

        Ok(target)
    });

    let get_own_prop_desc_fn = vm.alloc_native_function("getOwnPropertyDescriptor", 2, |vm, _this, args| {
        let target = match args.first() {
            Some(t) if t.is_object() => *t,
            Some(t) if !t.is_nullish() => *t,
            _ => return Err(VmError::TypeError("Object.getOwnPropertyDescriptor called on non-object".into())),
        };
        let prop_key = match args.get(1) {
            Some(p) => vm.to_property_key(*p)?,
            None => PropertyKeyVal::String("undefined".to_string()),
        };
        if let Some(ptr) = target.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            match prop_key {
                PropertyKeyVal::Symbol(sym) => {
                    if let Some(accs) = &obj.symbol_accessors {
                        if let Some(acc) = accs.get(&sym) {
                            let desc = vm.alloc_object();
                            vm.set_property(desc, "get", acc.getter.unwrap_or(JsValue::undefined()));
                            vm.set_property(desc, "set", acc.setter.unwrap_or(JsValue::undefined()));
                            vm.set_property(desc, "enumerable", JsValue::from_boolean(true));
                            vm.set_property(desc, "configurable", JsValue::from_boolean(true));
                            return Ok(desc);
                        }
                    }
                    if let Some(syms) = &obj.symbol_properties {
                        if let Some(val) = syms.get(&sym) {
                            let desc = vm.alloc_object();
                            vm.set_property(desc, "value", *val);
                            let is_tag = vm.symbol_to_string_tag.is_some_and(|t| t == sym);
                            let is_iter = vm.symbol_iterator.is_some_and(|i| i == sym);
                            let is_writ = !is_tag;
                            let is_enum = !is_tag && !is_iter;
                            let is_conf = if vm.module_namespace_objects.contains(&target) || is_tag { false } else { true };
                            vm.set_property(desc, "writable", JsValue::from_boolean(is_writ));
                            vm.set_property(desc, "enumerable", JsValue::from_boolean(is_enum));
                            vm.set_property(desc, "configurable", JsValue::from_boolean(is_conf));
                            return Ok(desc);
                        }
                    }
                }
                PropertyKeyVal::String(prop_name) => {
                    if let Some(accs) = &obj.accessors {
                        if let Some(acc) = accs.get(&prop_name) {
                            let desc = vm.alloc_object();
                            vm.set_property(desc, "get", acc.getter.unwrap_or(JsValue::undefined()));
                            vm.set_property(desc, "set", acc.setter.unwrap_or(JsValue::undefined()));
                            let is_enum = !vm.non_enumerable_props.contains(&(target, prop_name.clone()));
                            vm.set_property(desc, "enumerable", JsValue::from_boolean(is_enum));
                            let is_conf = !vm.non_configurable_props.contains(&(target, prop_name.clone()));
                            vm.set_property(desc, "configurable", JsValue::from_boolean(is_conf));
                            return Ok(desc);
                        }
                    }
                    if vm.deleted_props.contains(&(target, prop_name.clone())) {
                        return Ok(JsValue::undefined());
                    }
                    match &obj.kind {
                        ObjectKind::Array(elements) => {
                            if prop_name == "length" {
                                let desc = vm.alloc_object();
                                vm.set_property(desc, "value", JsValue::from_int32(elements.len() as i32));
                                vm.set_property(desc, "writable", JsValue::from_boolean(true));
                                vm.set_property(desc, "enumerable", JsValue::from_boolean(false));
                                vm.set_property(desc, "configurable", JsValue::from_boolean(false));
                                return Ok(desc);
                            }
                            if let Ok(idx) = prop_name.parse::<usize>() {
                                if let Some(elem) = elements.get(idx) {
                                    let desc = vm.alloc_object();
                                    vm.set_property(desc, "value", *elem);
                                    vm.set_property(desc, "writable", JsValue::from_boolean(true));
                                    vm.set_property(desc, "enumerable", JsValue::from_boolean(true));
                                    vm.set_property(desc, "configurable", JsValue::from_boolean(true));
                                    return Ok(desc);
                                }
                            }
                        }
                        ObjectKind::String(s) | ObjectKind::StringObject(s) => {
                            if prop_name == "length" {
                                let desc = vm.alloc_object();
                                vm.set_property(desc, "value", JsValue::from_int32(s.encode_utf16().count() as i32));
                                vm.set_property(desc, "writable", JsValue::from_boolean(false));
                                vm.set_property(desc, "enumerable", JsValue::from_boolean(false));
                                vm.set_property(desc, "configurable", JsValue::from_boolean(false));
                                return Ok(desc);
                            }
                            if let Ok(idx) = prop_name.parse::<usize>() {
                                if let Some(ch) = s.chars().nth(idx) {
                                    let ch_val = vm.alloc_string(ch.to_string());
                                    let desc = vm.alloc_object();
                                    vm.set_property(desc, "value", ch_val);
                                    vm.set_property(desc, "writable", JsValue::from_boolean(false));
                                    vm.set_property(desc, "enumerable", JsValue::from_boolean(true));
                                    vm.set_property(desc, "configurable", JsValue::from_boolean(false));
                                    return Ok(desc);
                                }
                            }
                        }
                        ObjectKind::TypedArray(ta) => {
                            if let Ok(idx) = prop_name.parse::<usize>() {
                                if idx < ta.length {
                                    let val = Vm::read_typed_array_element(ta, idx).unwrap_or(JsValue::undefined());
                                    let desc = vm.alloc_object();
                                    vm.set_property(desc, "value", val);
                                    vm.set_property(desc, "writable", JsValue::from_boolean(true));
                                    vm.set_property(desc, "enumerable", JsValue::from_boolean(true));
                                    vm.set_property(desc, "configurable", JsValue::from_boolean(false));
                                    return Ok(desc);
                                }
                            }
                        }
                        ObjectKind::Function(_) | ObjectKind::NativeFunction(_) => {
                            if prop_name == "length" {
                                let len = match &obj.kind {
                                    ObjectKind::Function(f) => f.params.iter().take_while(|p| !p.starts_with("...")).count(),
                                    ObjectKind::NativeFunction(n) => n.length as usize,
                                    _ => 0,
                                };
                                let desc = vm.alloc_object();
                                vm.set_property(desc, "value", JsValue::from_int32(len as i32));
                                vm.set_property(desc, "writable", JsValue::from_boolean(false));
                                vm.set_property(desc, "enumerable", JsValue::from_boolean(false));
                                vm.set_property(desc, "configurable", JsValue::from_boolean(true));
                                return Ok(desc);
                            }
                            if prop_name == "name" {
                                let name_str = match &obj.kind {
                                    ObjectKind::Function(f) => f.name.as_deref().unwrap_or(""),
                                    ObjectKind::NativeFunction(n) => &n.name,
                                    _ => "",
                                };
                                let name_val = vm.alloc_string(name_str.to_string());
                                let desc = vm.alloc_object();
                                vm.set_property(desc, "value", name_val);
                                vm.set_property(desc, "writable", JsValue::from_boolean(false));
                                vm.set_property(desc, "enumerable", JsValue::from_boolean(false));
                                vm.set_property(desc, "configurable", JsValue::from_boolean(true));
                                return Ok(desc);
                            }
                        }
                        _ => {}
                    }
                    if vm.shapes.has_property(obj.shape_id, &prop_name) {
                        let val = vm.get_property(target, &prop_name);
                        if vm.module_namespace_objects.contains(&target) && val == JsValue::uninitialized() {
                            return Err(VmError::ReferenceError(format!("Cannot access uninitialized export '{}'", prop_name)));
                        }
                        let desc = vm.alloc_object();
                        vm.set_property(desc, "value", val);
                        let is_writ = if vm.module_namespace_objects.contains(&target) {
                            true
                        } else {
                            !vm.non_writable_props.contains(&(target, prop_name.clone()))
                        };
                        let is_enum = if vm.module_namespace_objects.contains(&target) {
                            true
                        } else {
                            !vm.non_enumerable_props.contains(&(target, prop_name.clone()))
                        };
                        let is_conf = if vm.module_namespace_objects.contains(&target) {
                            false
                        } else {
                            !vm.non_configurable_props.contains(&(target, prop_name.clone()))
                        };
                        vm.set_property(desc, "writable", JsValue::from_boolean(is_writ));
                        vm.set_property(desc, "enumerable", JsValue::from_boolean(is_enum));
                        vm.set_property(desc, "configurable", JsValue::from_boolean(is_conf));
                        return Ok(desc);
                    }
                    if let Some((tgt_mod, orig_name)) = vm.module_reexports.get(&(target, prop_name.clone())).cloned() {
                        let val = vm.get_property(tgt_mod, &orig_name);
                        if val == JsValue::uninitialized() {
                            return Err(VmError::ReferenceError(format!("Cannot access uninitialized export '{}'", prop_name)));
                        }
                        let desc = vm.alloc_object();
                        vm.set_property(desc, "value", val);
                        vm.set_property(desc, "writable", JsValue::from_boolean(true));
                        vm.set_property(desc, "enumerable", JsValue::from_boolean(true));
                        vm.set_property(desc, "configurable", JsValue::from_boolean(false));
                        return Ok(desc);
                    }
                }
            }
        }
        Ok(JsValue::undefined())
    });

    let get_own_prop_names_fn = vm.alloc_native_function("getOwnPropertyNames", 1, |vm, _this, args| {
        let target = match args.first() {
            Some(t) if t.is_object() => *t,
            _ => return Err(VmError::TypeError("Object.getOwnPropertyNames called on non-object".into())),
        };
        let mut raw_names: Vec<String> = Vec::new();
        if let Some(ptr) = target.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            match &obj.kind {
                ObjectKind::Array(elements) => {
                    for i in 0..elements.len() {
                        raw_names.push(i.to_string());
                    }
                    raw_names.push("length".to_string());
                }
                ObjectKind::String(s) | ObjectKind::StringObject(s) => {
                    for i in 0..s.chars().count() {
                        raw_names.push(i.to_string());
                    }
                    raw_names.push("length".to_string());
                }
                ObjectKind::TypedArray(ta) => {
                    for i in 0..ta.length {
                        raw_names.push(i.to_string());
                    }
                    raw_names.push("length".to_string());
                    raw_names.push("byteLength".to_string());
                    raw_names.push("byteOffset".to_string());
                    raw_names.push("buffer".to_string());
                }
                ObjectKind::Function(_) | ObjectKind::NativeFunction(_) => {
                    raw_names.push("length".to_string());
                    raw_names.push("name".to_string());
                    if matches!(&obj.kind, ObjectKind::Function(_)) {
                        raw_names.push("prototype".to_string());
                    }
                }
                _ => {}
            }
            if let Some(accs) = &obj.accessors {
                for k in accs.keys() {
                    if !raw_names.contains(k) {
                        raw_names.push(k.clone());
                    }
                }
            }
            for prop in vm.shapes.all_properties(obj.shape_id) {
                if !raw_names.contains(&prop) {
                    raw_names.push(prop);
                }
            }
            for ((ns, re_name), _) in &vm.module_reexports {
                if *ns == target && !raw_names.contains(re_name) {
                    raw_names.push(re_name.clone());
                }
            }
            raw_names.retain(|p| !vm.deleted_props.contains(&(target, p.clone())));
            if vm.module_namespace_objects.contains(&target) {
                raw_names.sort();
            }
        }
        let mut int_keys: Vec<(u32, String)> = Vec::new();
        let mut str_keys: Vec<String> = Vec::new();
        for k in raw_names {
            if let Ok(idx) = k.parse::<u32>() {
                if idx < u32::MAX && idx.to_string() == k {
                    int_keys.push((idx, k));
                    continue;
                }
            }
            str_keys.push(k);
        }
        int_keys.sort_by_key(|(idx, _)| *idx);
        let mut names = Vec::new();
        for (_, k) in int_keys {
            names.push(vm.alloc_string(k));
        }
        for k in str_keys {
            names.push(vm.alloc_string(k));
        }
        Ok(vm.alloc_array(names))
    });

    let get_own_prop_symbols_fn = vm.alloc_native_function("getOwnPropertySymbols", 1, |vm, _this, args| {
        let target = match args.first() {
            Some(t) if t.is_object() => *t,
            _ => return Err(VmError::TypeError("Object.getOwnPropertySymbols called on non-object".into())),
        };
        let mut symbols = Vec::new();
        if let Some(ptr) = target.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let Some(order) = &obj.symbol_order {
                for k in order {
                    if !symbols.contains(k) {
                        symbols.push(*k);
                    }
                }
            } else {
                if let Some(sym_accs) = &obj.symbol_accessors {
                    for k in sym_accs.keys() {
                        if !symbols.contains(k) {
                            symbols.push(*k);
                        }
                    }
                }
                if let Some(syms) = &obj.symbol_properties {
                    for k in syms.keys() {
                        if !symbols.contains(k) {
                            symbols.push(*k);
                        }
                    }
                }
            }
        }
        Ok(vm.alloc_array(symbols))
    });

    let prevent_extensions_fn = vm.alloc_native_function("preventExtensions", 1, |vm, _this, args| {
        let target = match args.first() {
            Some(t) if t.is_object() => *t,
            Some(t) if !t.is_nullish() => return Ok(*t),
            _ => return Err(VmError::TypeError("Object.preventExtensions called on non-object".into())),
        };
        vm.non_extensible_objects.insert(target);
        Ok(target)
    });

    let is_extensible_fn = vm.alloc_native_function("isExtensible", 1, |vm, _this, args| {
        let target = match args.first() {
            Some(t) if t.is_object() => *t,
            _ => return Ok(JsValue::from_boolean(false)),
        };
        let ext = !vm.non_extensible_objects.contains(&target);
        Ok(JsValue::from_boolean(ext))
    });

    let get_prototype_of_fn = vm.alloc_native_function("getPrototypeOf", 1, |vm, _this, args| {
        let target = match args.first() {
            Some(t) if !t.is_nullish() => *t,
            _ => return Err(VmError::TypeError("Cannot convert undefined or null to object".into())),
        };
        if let Some(ptr) = target.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            Ok(obj.prototype.unwrap_or(JsValue::null()))
        } else if target.is_string() {
            let string_ctor = vm.globals.get("String").copied().unwrap_or(JsValue::undefined());
            Ok(vm.get_property(string_ctor, "prototype"))
        } else if target.is_number() {
            let num_ctor = vm.globals.get("Number").copied().unwrap_or(JsValue::undefined());
            Ok(vm.get_property(num_ctor, "prototype"))
        } else if target.is_boolean() {
            let bool_ctor = vm.globals.get("Boolean").copied().unwrap_or(JsValue::undefined());
            Ok(vm.get_property(bool_ctor, "prototype"))
        } else {
            Ok(JsValue::null())
        }
    });

    let set_prototype_of_fn = vm.alloc_native_function("setPrototypeOf", 2, |vm, _this, args| {
        let target = match args.first() {
            Some(t) if !t.is_nullish() => *t,
            _ => return Err(VmError::TypeError("Cannot convert undefined or null to object".into())),
        };
        let proto = args.get(1).copied().unwrap_or(JsValue::undefined());
        if !proto.is_object() && !proto.is_null() {
            return Err(VmError::TypeError("Object prototype may only be an Object or null".into()));
        }
        if let Some(ptr) = target.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if vm.module_namespace_objects.contains(&target) {
                if proto.is_null() {
                    return Ok(target);
                }
                return Err(VmError::TypeError("Cannot set prototype of module namespace object".into()));
            }
            if vm.non_extensible_objects.contains(&target) {
                return Err(VmError::TypeError("Cannot set prototype of non-extensible object".into()));
            }
            obj.prototype = if proto.is_null() { None } else { Some(proto) };
        }
        Ok(target)
    });

    let has_own_fn = vm.alloc_native_function("hasOwn", 2, |vm, _this, args| {
        let target = match args.first() {
            Some(t) if !t.is_nullish() => *t,
            _ => return Err(VmError::TypeError("Object.hasOwn called on null or undefined".into())),
        };
        let prop = args.get(1).copied().unwrap_or(JsValue::undefined());
        let prop_key = vm.to_property_key(prop)?;
        Ok(JsValue::from_boolean(vm.has_own_property_key(target, &prop_key)))
    });

    let from_entries_fn = vm.alloc_native_function("fromEntries", 1, |vm, _this, args| {
        let iterable = args.first().copied().unwrap_or(JsValue::undefined());
        if iterable.is_nullish() {
            return Err(VmError::TypeError("Object.fromEntries requires an iterable".into()));
        }
        let items = iterate_to_vec(vm, iterable)?;
        let obj = vm.alloc_object();
        for item in items {
            if !item.is_object() {
                return Err(VmError::TypeError("Iterator value is not an entry object".into()));
            }
            let k = vm.get_property(item, "0");
            let v = vm.get_property(item, "1");
            let key = vm.to_property_key(k)?;
            match key {
                PropertyKeyVal::String(s) => vm.set_property(obj, &s, v),
                PropertyKeyVal::Symbol(sym) => vm.set_symbol_property(obj, sym, v),
            }
        }
        Ok(obj)
    });

    let is_fn = vm.alloc_native_function("is", 2, |vm, _this, args| {
        let a = args.first().copied().unwrap_or(JsValue::undefined());
        let b = args.get(1).copied().unwrap_or(JsValue::undefined());
        if a.is_number() && b.is_number() {
            let na = a.to_number().unwrap();
            let nb = b.to_number().unwrap();
            if na.is_nan() && nb.is_nan() {
                return Ok(JsValue::from_boolean(true));
            }
            if na == 0.0 && nb == 0.0 {
                let a_neg = na.to_bits() == (-0.0f64).to_bits();
                let b_neg = nb.to_bits() == (-0.0f64).to_bits();
                return Ok(JsValue::from_boolean(a_neg == b_neg));
            }
            return Ok(JsValue::from_boolean(na == nb));
        }
        Ok(JsValue::from_boolean(vm.strict_equal(a, b)))
    });
    vm.set_property(object_constructor, "is", is_fn);

    vm.set_property(object_constructor, "defineProperty", define_property_fn);
    vm.set_property(object_constructor, "getOwnPropertyDescriptor", get_own_prop_desc_fn);
    vm.set_property(object_constructor, "getOwnPropertyNames", get_own_prop_names_fn);
    vm.set_property(object_constructor, "getOwnPropertySymbols", get_own_prop_symbols_fn);
    vm.set_property(object_constructor, "preventExtensions", prevent_extensions_fn);
    vm.set_property(object_constructor, "isExtensible", is_extensible_fn);
    vm.set_property(object_constructor, "getPrototypeOf", get_prototype_of_fn);
    vm.set_property(object_constructor, "setPrototypeOf", set_prototype_of_fn);
    vm.set_property(object_constructor, "hasOwn", has_own_fn);
    vm.set_property(object_constructor, "fromEntries", from_entries_fn);
    vm.set_property(object_constructor, "keys", keys_fn);
    vm.set_property(object_constructor, "values", values_fn);
    vm.set_property(object_constructor, "entries", entries_fn);
    vm.set_property(object_constructor, "assign", assign_fn);
    vm.set_property(object_constructor, "groupBy", group_by_fn);
    vm.set_property(object_constructor, "prototype", object_proto);
    vm.non_enumerable_props.insert((object_constructor, "prototype".to_string()));

    vm.globals.insert("Object".to_string(), object_constructor);
    if vm.global_object.is_object() {
        vm.set_property(vm.global_object, "Object", object_constructor);
    }
}

fn register_array_builtins(vm: &mut Vm) {
    let array_constructor = vm.alloc_native_function("Array", 1, |vm, _this, args| {
        if args.len() == 1 {
            if let Some(n) = args[0].to_number() {
                if n < 0.0 || n > 4_294_967_295.0 || n.fract() != 0.0 || n.is_nan() {
                    return Err(VmError::RangeError("Invalid array length".into()));
                }
                let len = n as usize;
                if len > 100_000 {
                    return Err(VmError::RangeError("Invalid array length".into()));
                }
                let mut v = Vec::with_capacity(len.min(1024));
                for _ in 0..len {
                    v.push(JsValue::undefined());
                }
                return Ok(vm.alloc_array(v));
            }
        }
        Ok(vm.alloc_array(args.to_vec()))
    });

    let is_array_fn = vm.alloc_native_function("isArray", 1, |_vm, _this, args| {
        if let Some(arg) = args.first() {
            if let Some(ptr) = arg.as_object_ptr() {
                let obj = unsafe { &*(ptr as *mut JsObject) };
                if matches!(obj.kind, ObjectKind::Array(_)) {
                    return Ok(JsValue::from_boolean(true));
                }
            }
        }
        Ok(JsValue::from_boolean(false))
    });

    let array_proto = vm.alloc_object();
    vm.array_proto = Some(array_proto);

    let map_fn = vm.alloc_native_function("map", 1, |vm, this, args| {
        let cb = args.first().copied().unwrap_or(JsValue::undefined());
        let len_val = vm.get_property(this, "length");
        let raw_len = len_val.to_number().unwrap_or(0.0);
        if raw_len > 4_294_967_295.0 || raw_len < 0.0 || raw_len.is_infinite() {
            return Err(VmError::RangeError("Invalid array length".into()));
        }
        let len = raw_len as usize;
        if len > 100_000 {
            return Err(VmError::RangeError("Invalid array length".into()));
        }
        let mut mapped = Vec::with_capacity(len.min(65536));
        for i in 0..len {
            let el = vm.get_element(this, i);
            let res = vm.call(cb, JsValue::undefined(), &[el, JsValue::from_int32(i as i32), this])?;
            mapped.push(res);
        }
        Ok(vm.alloc_array(mapped))
    });
    vm.set_property(array_proto, "map", map_fn);

    let join_fn = vm.alloc_native_function("join", 1, |vm, this, args| {
        let sep = args.first().map(|v| vm.to_string_value(*v)).unwrap_or_else(|| ",".to_string());
        let len_val = vm.get_property(this, "length");
        let raw_len = len_val.to_number().unwrap_or(0.0);
        if raw_len > 4_294_967_295.0 || raw_len < 0.0 || raw_len.is_infinite() {
            return Err(VmError::RangeError("Invalid array length".into()));
        }
        let len = raw_len as usize;
        if len > 100_000 {
            return Err(VmError::RangeError("Invalid array length".into()));
        }
        let mut parts = Vec::with_capacity(len.min(65536));
        for i in 0..len {
            let el = vm.get_element(this, i);
            parts.push(vm.to_string_value(el));
        }
        Ok(vm.alloc_string(parts.join(&sep)))
    });
    vm.set_property(array_proto, "join", join_fn);

    let push_fn = vm.alloc_native_function("push", 1, |_vm, this, args| {
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::Array(elements) = &mut obj.kind {
                if elements.len() + args.len() > 100_000 {
                    return Err(VmError::RangeError("Invalid array length".into()));
                }
                for arg in args {
                    elements.push(*arg);
                }
                return Ok(JsValue::from_int32(elements.len() as i32));
            }
        }
        Ok(JsValue::undefined())
    });
    vm.set_property(array_proto, "push", push_fn);

    let pop_fn = vm.alloc_native_function("pop", 0, |_vm, this, _args| {
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::Array(elements) = &mut obj.kind {
                return Ok(elements.pop().unwrap_or(JsValue::undefined()));
            }
        }
        Ok(JsValue::undefined())
    });
    vm.set_property(array_proto, "pop", pop_fn);

    let shift_fn = vm.alloc_native_function("shift", 0, |_vm, this, _args| {
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::Array(elements) = &mut obj.kind {
                if !elements.is_empty() {
                    return Ok(elements.remove(0));
                }
            }
        }
        Ok(JsValue::undefined())
    });
    vm.set_property(array_proto, "shift", shift_fn);

    let unshift_fn = vm.alloc_native_function("unshift", 1, |_vm, this, args| {
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::Array(elements) = &mut obj.kind {
                if elements.len() + args.len() > 100_000 {
                    return Err(VmError::RangeError("Invalid array length".into()));
                }
                for (i, arg) in args.iter().enumerate() {
                    elements.insert(i, *arg);
                }
                return Ok(JsValue::from_int32(elements.len() as i32));
            }
        }
        Ok(JsValue::undefined())
    });
    vm.set_property(array_proto, "unshift", unshift_fn);

    let slice_fn = vm.alloc_native_function("slice", 2, |vm, this, args| {
        let len_val = vm.get_property(this, "length");
        let raw_len = len_val.to_number().unwrap_or(0.0);
        if raw_len > 4_294_967_295.0 || raw_len < 0.0 || raw_len.is_infinite() {
            return Err(VmError::RangeError("Invalid array length".into()));
        }
        let len = raw_len as usize;
        let start = args.first().and_then(|v| v.to_number()).unwrap_or(0.0);
        let start_idx = if start < 0.0 {
            (len as f64 + start).max(0.0) as usize
        } else {
            (start as usize).min(len)
        };
        let end = args.get(1).and_then(|v| v.to_number()).unwrap_or(len as f64);
        let end_idx = if end < 0.0 {
            (len as f64 + end).max(0.0) as usize
        } else {
            (end as usize).min(len)
        };
        if end_idx >= start_idx && (end_idx - start_idx) > 100_000 {
            return Err(VmError::RangeError("Invalid array length".into()));
        }
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::Array(elements) = &obj.kind {
                let actual_len = elements.len();
                let s = start_idx.min(actual_len);
                let e = end_idx.min(actual_len);
                let sliced = if s < e { elements[s..e].to_vec() } else { Vec::new() };
                return Ok(vm.alloc_array(sliced));
            }
        }
        let mut sliced = Vec::new();
        if start_idx < end_idx {
            for i in start_idx..end_idx {
                sliced.push(vm.get_element(this, i));
            }
        }
        Ok(vm.alloc_array(sliced))
    });
    vm.set_property(array_proto, "slice", slice_fn);

    let index_of_fn = vm.alloc_native_function("indexOf", 1, |_vm, this, args| {
        let search = args.first().copied().unwrap_or(JsValue::undefined());
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::Array(elements) = &obj.kind {
                for (i, el) in elements.iter().enumerate() {
                    if *el == search {
                        return Ok(JsValue::from_int32(i as i32));
                    }
                }
            }
        }
        Ok(JsValue::from_int32(-1))
    });
    vm.set_property(array_proto, "indexOf", index_of_fn);

    let includes_fn = vm.alloc_native_function("includes", 1, |_vm, this, args| {
        let search = args.first().copied().unwrap_or(JsValue::undefined());
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::Array(elements) = &obj.kind {
                for el in elements {
                    if *el == search {
                        return Ok(JsValue::from_boolean(true));
                    }
                }
            }
        }
        Ok(JsValue::from_boolean(false))
    });
    vm.set_property(array_proto, "includes", includes_fn);

    let for_each_fn = vm.alloc_native_function("forEach", 1, |vm, this, args| {
        let cb = args.first().copied().unwrap_or(JsValue::undefined());
        let len_val = vm.get_property(this, "length");
        let raw_len = len_val.to_number().unwrap_or(0.0);
        if raw_len > 4_294_967_295.0 || raw_len < 0.0 || raw_len.is_infinite() {
            return Err(VmError::RangeError("Invalid array length".into()));
        }
        let len = raw_len as usize;
        if len > 100_000 {
            return Err(VmError::RangeError("Invalid array length".into()));
        }
        for i in 0..len {
            let el = vm.get_element(this, i);
            let _ = vm.call(cb, JsValue::undefined(), &[el, JsValue::from_int32(i as i32), this])?;
        }
        Ok(JsValue::undefined())
    });
    vm.set_property(array_proto, "forEach", for_each_fn);

    let filter_fn = vm.alloc_native_function("filter", 1, |vm, this, args| {
        let cb = args.first().copied().unwrap_or(JsValue::undefined());
        let len_val = vm.get_property(this, "length");
        let raw_len = len_val.to_number().unwrap_or(0.0);
        if raw_len > 4_294_967_295.0 || raw_len < 0.0 || raw_len.is_infinite() {
            return Err(VmError::RangeError("Invalid array length".into()));
        }
        let len = raw_len as usize;
        if len > 100_000 {
            return Err(VmError::RangeError("Invalid array length".into()));
        }
        let mut filtered = Vec::new();
        for i in 0..len {
            let el = vm.get_element(this, i);
            let keep = vm.call(cb, JsValue::undefined(), &[el, JsValue::from_int32(i as i32), this])?;
            if keep.is_truthy() {
                filtered.push(el);
            }
        }
        Ok(vm.alloc_array(filtered))
    });
    vm.set_property(array_proto, "filter", filter_fn);

    let concat_fn = vm.alloc_native_function("concat", 1, |vm, this, args| {
        let mut out = Vec::new();
        let sym_spread = vm.symbol_is_concat_spreadable;
        let mut items = Vec::with_capacity(args.len() + 1);
        items.push(this);
        items.extend_from_slice(args);

        for item in items {
            let is_spreadable = if let Some(sym) = sym_spread {
                let spread_val = vm.get_symbol_property_result(item, sym).unwrap_or(JsValue::undefined());
                if !spread_val.is_undefined() {
                    spread_val.is_truthy()
                } else if let Some(ptr) = item.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    matches!(obj.kind, ObjectKind::Array(_))
                } else {
                    false
                }
            } else if let Some(ptr) = item.as_object_ptr() {
                let obj = unsafe { &*(ptr as *mut JsObject) };
                matches!(obj.kind, ObjectKind::Array(_))
            } else {
                false
            };

            if is_spreadable {
                let len_val = vm.get_property(item, "length");
                let len = len_val.to_number().unwrap_or(0.0) as usize;
                if len > 100_000 {
                    return Err(VmError::RangeError("Invalid array length".into()));
                }
                for i in 0..len {
                    out.push(vm.get_element(item, i));
                }
            } else {
                out.push(item);
            }
        }

        if out.len() > 100_000 {
            return Err(VmError::RangeError("Invalid array length".into()));
        }
        Ok(vm.alloc_array(out))
    });
    vm.set_property(array_proto, "concat", concat_fn);

    let reverse_fn = vm.alloc_native_function("reverse", 0, |vm, this, _args| {
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::Array(ref mut elems) = obj.kind {
                elems.reverse();
                return Ok(this);
            }
        }
        let len_val = vm.get_property(this, "length");
        let raw_len = len_val.to_number().unwrap_or(0.0);
        if raw_len > 4_294_967_295.0 || raw_len < 0.0 || raw_len.is_infinite() {
            return Err(VmError::RangeError("Invalid array length".into()));
        }
        let len = raw_len as usize;
        if len > 100_000 {
            return Err(VmError::RangeError("Invalid array length".into()));
        }
        let mut i = 0;
        let mut j = len.saturating_sub(1);
        while i < j {
            let vi = vm.get_element(this, i);
            let vj = vm.get_element(this, j);
            vm.set_element(this, i, vj);
            vm.set_element(this, j, vi);
            i += 1;
            j = j.saturating_sub(1);
        }
        Ok(this)
    });
    vm.set_property(array_proto, "reverse", reverse_fn);

    let reduce_fn = vm.alloc_native_function("reduce", 1, |vm, this, args| {
        let cb = args.first().copied().unwrap_or(JsValue::undefined());
        if !vm.is_callable(cb) {
            return Err(VmError::TypeError("reduce callback must be callable".into()));
        }
        let len_val = vm.get_property(this, "length");
        let raw_len = len_val.to_number().unwrap_or(0.0);
        if raw_len > 4_294_967_295.0 || raw_len < 0.0 || raw_len.is_infinite() {
            return Err(VmError::RangeError("Invalid array length".into()));
        }
        let len = raw_len as usize;
        if len > 100_000 {
            return Err(VmError::RangeError("Invalid array length".into()));
        }
        let (mut acc, start_i) = if args.len() > 1 {
            (args[1], 0)
        } else if len > 0 {
            (vm.get_element(this, 0), 1)
        } else {
            return Err(VmError::TypeError("Reduce of empty array with no initial value".into()));
        };
        for i in start_i..len {
            let el = vm.get_element(this, i);
            acc = vm.call(cb, JsValue::undefined(), &[acc, el, JsValue::from_int32(i as i32), this])?;
        }
        Ok(acc)
    });
    vm.set_property(array_proto, "reduce", reduce_fn);

    let some_fn = vm.alloc_native_function("some", 1, |vm, this, args| {
        let cb = args.first().copied().unwrap_or(JsValue::undefined());
        let len_val = vm.get_property(this, "length");
        let raw_len = len_val.to_number().unwrap_or(0.0);
        if raw_len > 4_294_967_295.0 || raw_len < 0.0 || raw_len.is_infinite() {
            return Err(VmError::RangeError("Invalid array length".into()));
        }
        let len = raw_len as usize;
        if len > 100_000 {
            return Err(VmError::RangeError("Invalid array length".into()));
        }
        for i in 0..len {
            let el = vm.get_element(this, i);
            let res = vm.call(cb, JsValue::undefined(), &[el, JsValue::from_int32(i as i32), this])?;
            if res.is_truthy() {
                return Ok(JsValue::from_boolean(true));
            }
        }
        Ok(JsValue::from_boolean(false))
    });
    vm.set_property(array_proto, "some", some_fn);

    let every_fn = vm.alloc_native_function("every", 1, |vm, this, args| {
        let cb = args.first().copied().unwrap_or(JsValue::undefined());
        let len_val = vm.get_property(this, "length");
        let raw_len = len_val.to_number().unwrap_or(0.0);
        if raw_len > 4_294_967_295.0 || raw_len < 0.0 || raw_len.is_infinite() {
            return Err(VmError::RangeError("Invalid array length".into()));
        }
        let len = raw_len as usize;
        if len > 100_000 {
            return Err(VmError::RangeError("Invalid array length".into()));
        }
        for i in 0..len {
            let el = vm.get_element(this, i);
            let res = vm.call(cb, JsValue::undefined(), &[el, JsValue::from_int32(i as i32), this])?;
            if !res.is_truthy() {
                return Ok(JsValue::from_boolean(false));
            }
        }
        Ok(JsValue::from_boolean(true))
    });
    vm.set_property(array_proto, "every", every_fn);

    let values_fn = vm.alloc_native_function("values", 0, |vm, this, _args| {
        create_array_iterator(vm, this, IterKind::Values)
    });
    let keys_fn = vm.alloc_native_function("keys", 0, |vm, this, _args| {
        create_array_iterator(vm, this, IterKind::Keys)
    });
    let entries_fn = vm.alloc_native_function("entries", 0, |vm, this, _args| {
        create_array_iterator(vm, this, IterKind::Entries)
    });
    vm.set_property(array_proto, "values", values_fn);
    vm.set_property(array_proto, "keys", keys_fn);
    vm.set_property(array_proto, "entries", entries_fn);
    if let Some(sym_iter) = vm.symbol_iterator {
        vm.set_symbol_property(array_proto, sym_iter, values_fn);
    }

    let of_fn = vm.alloc_native_function("of", 0, |vm, _this, args| {
        Ok(vm.alloc_array(args.to_vec()))
    });
    vm.set_property(array_constructor, "of", of_fn);

    let from_fn = vm.alloc_native_function("from", 1, |vm, _this, args| {
        let items = args.first().copied().unwrap_or(JsValue::undefined());
        let map_fn = args.get(1).copied().unwrap_or(JsValue::undefined());
        let this_arg = args.get(2).copied().unwrap_or(JsValue::undefined());
        let has_map = vm.is_callable(map_fn);

        let mut result_elements = Vec::new();
        let mut handled = false;

        if let Some(sym_iter) = vm.symbol_iterator {
            let iter_method = vm.get_symbol_property_result(items, sym_iter).unwrap_or(JsValue::undefined());
            if vm.is_callable(iter_method) {
                handled = true;
                let iterator = vm.call(iter_method, items, &[])?;
                let next_fn = vm.get_property(iterator, "next");
                let mut idx = 0;
                while vm.is_callable(next_fn) {
                    let step = vm.call(next_fn, iterator, &[])?;
                    let done = vm.get_property(step, "done").is_truthy();
                    if done {
                        break;
                    }
                    let val = vm.get_property(step, "value");
                    let mapped = if has_map {
                        vm.call(map_fn, this_arg, &[val, JsValue::from_int32(idx)])?
                    } else {
                        val
                    };
                    result_elements.push(mapped);
                    idx += 1;
                }
            }
        }

        if !handled {
            let len_val = vm.get_property(items, "length");
            if let Some(n) = len_val.to_number() {
                let len = (n as usize).min(100_000);
                for i in 0..len {
                    let el = vm.get_element(items, i);
                    let mapped = if has_map {
                        vm.call(map_fn, this_arg, &[el, JsValue::from_int32(i as i32)])?
                    } else {
                        el
                    };
                    result_elements.push(mapped);
                }
            }
        }

        Ok(vm.alloc_array(result_elements))
    });
    vm.set_property(array_constructor, "from", from_fn);

    vm.set_property(array_proto, "constructor", array_constructor);

    vm.set_property(array_constructor, "isArray", is_array_fn);
    vm.set_property(array_constructor, "prototype", array_proto);
    add_species_getter(vm, array_constructor);
    vm.globals.insert("Array".to_string(), array_constructor);
    if vm.global_object.is_object() {
        vm.set_property(vm.global_object, "Array", array_constructor);
    }
}

fn register_function_builtins(vm: &mut Vm) {
    let call_fn = vm.alloc_native_function("call", 1, |vm, this, args| {
        if !vm.is_callable(this) {
            return Err(VmError::TypeError("Function.prototype.call called on non-function".into()));
        }
        let this_arg = args.first().copied().unwrap_or(JsValue::undefined());
        let rest_args = if args.len() > 1 { &args[1..] } else { &[] };
        vm.call(this, this_arg, rest_args)
    });

    let apply_fn = vm.alloc_native_function("apply", 2, |vm, this, args| {
        if !vm.is_callable(this) {
            return Err(VmError::TypeError("Function.prototype.apply called on non-function".into()));
        }
        let this_arg = args.first().copied().unwrap_or(JsValue::undefined());
        let mut call_args = Vec::new();
        if let Some(arg_array) = args.get(1) {
            if !arg_array.is_nullish() {
                if let Some(ptr) = arg_array.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    if let ObjectKind::Array(elems) = &obj.kind {
                        call_args.extend_from_slice(elems);
                    } else {
                        let len_val = vm.get_property(*arg_array, "length");
                        let raw_len = len_val.to_number().unwrap_or(0.0);
                        if raw_len > 65536.0 || raw_len < 0.0 || raw_len.is_infinite() {
                            return Err(VmError::RangeError("Maximum call stack / argument limit exceeded".into()));
                        }
                        let len = raw_len as usize;
                        for i in 0..len {
                            call_args.push(vm.get_property(*arg_array, &i.to_string()));
                        }
                    }
                } else {
                    return Err(VmError::TypeError("CreateListFromArrayLike called on non-object".into()));
                }
            }
        }
        vm.call(this, this_arg, &call_args)
    });

    let bind_fn = vm.alloc_native_function("bind", 1, |vm, this, args| {
        if !vm.is_callable(this) {
            return Err(VmError::TypeError("Function.prototype.bind called on non-function".into()));
        }
        let target_fn = this;
        let bound_this = args.first().copied().unwrap_or(JsValue::undefined());
        let bound_args: Vec<JsValue> = if args.len() > 1 { args[1..].to_vec() } else { Vec::new() };

        let bound_closure = vm.alloc_closure_native_function(
            "bound",
            0,
            Arc::new(move |vm, _call_this, call_args| {
                let mut combined_args = bound_args.clone();
                combined_args.extend_from_slice(call_args);
                vm.call(target_fn, bound_this, &combined_args)
            }),
        );
        Ok(bound_closure)
    });

    let to_string_fn = vm.alloc_native_function("toString", 0, |vm, _this, _args| {
        Ok(vm.alloc_string("function () { [native code] }".into()))
    });

    let function_proto = vm.function_proto.unwrap();

    vm.set_property(function_proto, "call", call_fn);
    vm.set_property(function_proto, "apply", apply_fn);
    vm.set_property(function_proto, "bind", bind_fn);
    vm.set_property(function_proto, "toString", to_string_fn);

    let function_constructor = vm.alloc_native_function("Function", 1, |vm, _this, args| {
        let (params_str, body_str) = if args.is_empty() {
            (String::new(), String::new())
        } else if args.len() == 1 {
            (String::new(), vm.to_string_value(args[0]))
        } else {
            let param_parts: Vec<String> = args[..args.len() - 1].iter().map(|a| vm.to_string_value(*a)).collect();
            let p = param_parts.join(", ");
            let b = vm.to_string_value(args[args.len() - 1]);
            (p, b)
        };
        let code = format!("(function({}) {{\n{}\n}})", params_str, body_str);
        let program = doms_parser::parse(&code)
            .map_err(|e| VmError::SyntaxError(e.to_string()))?;
        let chunk = doms_compiler::compile(&program)
            .map_err(|e| VmError::SyntaxError(e.to_string()))?;
        vm.run(&chunk)
    });

    vm.set_property(function_proto, "constructor", function_constructor);
    vm.set_property(function_constructor, "prototype", function_proto);
    if let Some(ptr) = function_constructor.as_object_ptr() {
        let obj = unsafe { &mut *(ptr as *mut JsObject) };
        obj.prototype = Some(function_proto);
    }

    vm.globals.insert("Function".to_string(), function_constructor);
    if vm.global_object.is_object() {
        vm.set_property(vm.global_object, "Function", function_constructor);
    }
}

fn is_uri_mark(c: char) -> bool {
    matches!(c, '-' | '_' | '.' | '!' | '~' | '*' | '\'' | '(' | ')')
}

fn is_uri_reserved(c: char) -> bool {
    matches!(c, ';' | '/' | '?' | ':' | '@' | '&' | '=' | '+' | '$' | ',' | '#')
}

fn is_uri_alpha_num(c: char) -> bool {
    c.is_ascii_alphanumeric()
}

fn uri_encode(s: &str, unescape_reserved: bool) -> Result<String, VmError> {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c == '\u{FFFD}' {
            return Err(VmError::UriError("URI malformed".into()));
        }
        if is_uri_alpha_num(c) || is_uri_mark(c) || (unescape_reserved && is_uri_reserved(c)) {
            out.push(c);
        } else {
            let mut buf = [0u8; 4];
            let encoded = c.encode_utf8(&mut buf);
            for b in encoded.bytes() {
                out.push('%');
                out.push_str(&format!("{:02X}", b));
            }
        }
    }
    Ok(out)
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn uri_decode(s: &str, preserve_reserved: bool) -> Result<String, VmError> {
    let bytes = s.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    let mut decoded_bytes = Vec::with_capacity(len);
    let mut out = String::with_capacity(len);

    while i < len {
        let b = bytes[i];
        if b != b'%' {
            if !decoded_bytes.is_empty() {
                let s_part = std::str::from_utf8(&decoded_bytes)
                    .map_err(|_| VmError::UriError("URI malformed".into()))?;
                out.push_str(s_part);
                decoded_bytes.clear();
            }
            out.push(b as char);
            i += 1;
        } else {
            if i + 2 >= len {
                return Err(VmError::UriError("URI malformed".into()));
            }
            let h1 = hex_val(bytes[i + 1]).ok_or_else(|| VmError::UriError("URI malformed".into()))?;
            let h2 = hex_val(bytes[i + 2]).ok_or_else(|| VmError::UriError("URI malformed".into()))?;
            let byte_val = (h1 << 4) | h2;
            i += 3;

            if preserve_reserved && is_uri_reserved(byte_val as char) {
                if !decoded_bytes.is_empty() {
                    let s_part = std::str::from_utf8(&decoded_bytes)
                        .map_err(|_| VmError::UriError("URI malformed".into()))?;
                    out.push_str(s_part);
                    decoded_bytes.clear();
                }
                out.push('%');
                out.push(bytes[i - 2] as char);
                out.push(bytes[i - 1] as char);
            } else {
                decoded_bytes.push(byte_val);
            }
        }
    }
    if !decoded_bytes.is_empty() {
        let s_part = std::str::from_utf8(&decoded_bytes)
            .map_err(|_| VmError::UriError("URI malformed".into()))?;
        out.push_str(s_part);
    }
    Ok(out)
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

fn ms_to_utc(ms: f64) -> Option<(i32, i32, i32, i32, i32, i32, i32, i32)> {
    if ms.is_nan() || ms.is_infinite() {
        return None;
    }
    let total_ms = ms as i64;
    let mut day = total_ms.div_euclid(86_400_000);
    let time_of_day = total_ms.rem_euclid(86_400_000);
    let ms_part = (time_of_day % 1000) as i32;
    let total_secs = (time_of_day / 1000) as i32;
    let sec = total_secs % 60;
    let total_mins = total_secs / 60;
    let min = total_mins % 60;
    let hour = total_mins / 60;
    let weekday = ((day + 4).rem_euclid(7)) as i32;

    day += 719468;
    let era = if day >= 0 { day } else { day - 146096 } / 146097;
    let doe = (day - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y } as i32;
    let month = (m - 1) as i32;
    Some((year, month, d as i32, weekday, hour, min, sec, ms_part))
}

fn utc_to_ms(year: i32, month: i32, day: i32, hour: i32, min: i32, sec: i32, ms: i32) -> f64 {
    let y = if month <= 1 { year - 1 } else { year } as i64;
    let m = (if month <= 1 { month + 10 } else { month - 2 }) as i64;
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u32;
    let doy = (153 * m + 2) / 5 + (day as i64 - 1);
    let doe = (yoe as i64) * 365 + (yoe as i64) / 4 - (yoe as i64) / 100 + doy;
    let days = era * 146097 + doe - 719468;
    (days * 86_400_000 + (hour as i64) * 3_600_000 + (min as i64) * 60_000 + (sec as i64) * 1000 + (ms as i64)) as f64
}

fn parse_iso_date(s: &str) -> Option<(i32, i32, i32)> {
    let parts: Vec<&str> = s.split(|c| c == '-' || c == '/' || c == 'T').collect();
    if parts.len() >= 3 {
        let y = parts[0].parse::<i32>().ok()?;
        let m = parts[1].parse::<i32>().ok()? - 1;
        let d = parts[2].split(|c: char| !c.is_ascii_digit()).next()?.parse::<i32>().ok()?;
        return Some((y, m, d));
    }
    None
}

fn get_date_ms(this: JsValue) -> Result<(usize, f64), VmError> {
    if let Some(ptr) = this.as_object_ptr() {
        let obj = unsafe { &*(ptr as *mut JsObject) };
        if let ObjectKind::Date(ms) = obj.kind {
            return Ok((ptr as usize, ms));
        }
    }
    Err(VmError::TypeError("Method Date.prototype called on non-Date object".into()))
}

fn register_global_helpers(vm: &mut Vm) {
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

    let string_proto = vm.alloc_object();
    vm.string_proto = Some(string_proto);

    let str_value_of = vm.alloc_native_function("valueOf", 0, |vm, this, _args| {
        if this.is_string() {
            return Ok(this);
        }
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::StringObject(ref s) | ObjectKind::String(ref s) = obj.kind {
                return Ok(vm.alloc_string(s.clone()));
            }
        }
        Err(VmError::TypeError("String.prototype.valueOf requires that 'this' be a String".into()))
    });
    let str_to_string = vm.alloc_native_function("toString", 0, |vm, this, _args| {
        if this.is_string() {
            return Ok(this);
        }
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::StringObject(ref s) | ObjectKind::String(ref s) = obj.kind {
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
    vm.non_enumerable_props.insert((string_proto, "constructor".to_string()));
    vm.set_property(str_ctor, "prototype", string_proto);
    vm.non_enumerable_props.insert((str_ctor, "prototype".to_string()));

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
        vm.non_enumerable_props.insert((string_proto, m_name.to_string()));
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
    vm.non_enumerable_props.insert((string_proto, "substr".to_string()));

    let str_trim_start = vm.alloc_native_function("trimStart", 0, |vm, this, _args| {
        let s = vm.to_string_value(this);
        Ok(vm.alloc_string(s.trim_start().to_string()))
    });
    vm.set_property(string_proto, "trimStart", str_trim_start);
    vm.set_property(string_proto, "trimLeft", str_trim_start);
    vm.non_enumerable_props.insert((string_proto, "trimStart".to_string()));
    vm.non_enumerable_props.insert((string_proto, "trimLeft".to_string()));

    let str_trim_end = vm.alloc_native_function("trimEnd", 0, |vm, this, _args| {
        let s = vm.to_string_value(this);
        Ok(vm.alloc_string(s.trim_end().to_string()))
    });
    vm.set_property(string_proto, "trimEnd", str_trim_end);
    vm.set_property(string_proto, "trimRight", str_trim_end);
    vm.non_enumerable_props.insert((string_proto, "trimEnd".to_string()));
    vm.non_enumerable_props.insert((string_proto, "trimRight".to_string()));

    let string_methods = [
        "indexOf", "includes", "toLowerCase", "toUpperCase", "trim",
        "charAt", "charCodeAt", "slice", "isWellFormed", "toWellFormed", "replace",
        "match", "search",
    ];
    for name in string_methods {
        if let Some(m) = vm.get_string_method(name) {
            vm.set_property(string_proto, name, m);
            vm.non_enumerable_props.insert((string_proto, name.to_string()));
        }
    }

    if let Some(sym_iter) = vm.symbol_iterator {
        let str_iter_fn = vm.alloc_native_function("[Symbol.iterator]", 0, |vm, this, _args| {
            let s = vm.to_string_value(this);
            let chars: Vec<JsValue> = s.chars().map(|c| vm.alloc_string(c.to_string())).collect();
            Ok(vm.alloc_iterator(chars))
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
        let mut s = String::new();
        for arg in args {
            let n = vm.to_number_value(*arg).unwrap_or(0.0) as u32;
            if let Some(ch) = char::from_u32(n) {
                s.push(ch);
            } else {
                return Err(VmError::RangeError(format!("Invalid code point {}", n)));
            }
        }
        Ok(vm.alloc_string(s))
    });
    vm.set_property(str_ctor, "fromCodePoint", from_code_point_fn);

    vm.globals.insert("String".to_string(), str_ctor);
    if vm.global_object.is_object() {
        vm.set_property(vm.global_object, "String", str_ctor);
    }

    let number_proto = vm.alloc_object();
    vm.number_proto = Some(number_proto);

    let num_value_of = vm.alloc_native_function("valueOf", 0, |_vm, this, _args| {
        if this.is_number() {
            return Ok(this);
        }
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::NumberObject(n) = obj.kind {
                return Ok(JsValue::from_double(n));
            }
        }
        Err(VmError::TypeError("Number.prototype.valueOf requires that 'this' be a Number".into()))
    });
    let num_to_string = vm.alloc_native_function("toString", 1, |vm, this, args| {
        let n = if this.is_number() {
            this.to_number().unwrap_or(0.0)
        } else if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::NumberObject(num) = obj.kind {
                num
            } else {
                return Err(VmError::TypeError("Number.prototype.toString requires that 'this' be a Number".into()));
            }
        } else {
            return Err(VmError::TypeError("Number.prototype.toString requires that 'this' be a Number".into()));
        };
        let radix = args.first().and_then(|v| v.to_int32()).unwrap_or(10);
        if !(2..=36).contains(&radix) {
            return Err(VmError::RangeError("radix must be between 2 and 36".into()));
        }
        if radix == 10 {
            Ok(vm.alloc_string(vm.to_string_value(JsValue::from_double(n))))
        } else {
            let int_val = n as i64;
            let s = match radix {
                2 => format!("{:b}", int_val),
                8 => format!("{:o}", int_val),
                16 => format!("{:x}", int_val),
                _ => format!("{}", int_val),
            };
            Ok(vm.alloc_string(s))
        }
    });
    vm.set_property(number_proto, "valueOf", num_value_of);
    vm.set_property(number_proto, "toString", num_to_string);

    let num_ctor = vm.alloc_native_function("Number", 1, |vm, _this, args| {
        let n = if let Some(arg) = args.first() {
            vm.to_number_value(*arg).unwrap_or(f64::NAN)
        } else {
            0.0
        };
        if vm.is_constructing {
            let proto = vm.number_proto;
            Ok(vm.alloc_number_object(n, proto))
        } else {
            Ok(JsValue::from_double(n))
        }
    });
    vm.set_property(number_proto, "constructor", num_ctor);
    vm.non_enumerable_props.insert((number_proto, "constructor".to_string()));
    vm.set_property(num_ctor, "prototype", number_proto);
    vm.non_enumerable_props.insert((num_ctor, "prototype".to_string()));

    let num_constants: [(&str, f64); 8] = [
        ("NaN", f64::NAN),
        ("POSITIVE_INFINITY", f64::INFINITY),
        ("NEGATIVE_INFINITY", f64::NEG_INFINITY),
        ("MAX_VALUE", 1.7976931348623157e+308),
        ("MIN_VALUE", 5e-324),
        ("EPSILON", 2.220446049250313e-16),
        ("MAX_SAFE_INTEGER", 9007199254740991.0),
        ("MIN_SAFE_INTEGER", -9007199254740991.0),
    ];
    for (name, val) in num_constants {
        vm.set_property(num_ctor, name, JsValue::from_double(val));
        vm.non_writable_props.insert((num_ctor, name.to_string()));
        vm.non_enumerable_props.insert((num_ctor, name.to_string()));
        vm.non_configurable_props.insert((num_ctor, name.to_string()));
    }
    vm.globals.insert("Number".to_string(), num_ctor);
    if vm.global_object.is_object() {
        vm.set_property(vm.global_object, "Number", num_ctor);
    }

    let boolean_proto = vm.alloc_object();
    vm.boolean_proto = Some(boolean_proto);

    let bool_value_of = vm.alloc_native_function("valueOf", 0, |_vm, this, _args| {
        if this.is_boolean() {
            return Ok(this);
        }
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::BooleanObject(b) = obj.kind {
                return Ok(JsValue::from_boolean(b));
            }
        }
        Err(VmError::TypeError("Boolean.prototype.valueOf requires that 'this' be a Boolean".into()))
    });
    let bool_to_string = vm.alloc_native_function("toString", 0, |vm, this, _args| {
        let b = if this.is_boolean() {
            this.as_boolean().unwrap()
        } else if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::BooleanObject(b) = obj.kind {
                b
            } else {
                return Err(VmError::TypeError("Boolean.prototype.toString requires that 'this' be a Boolean".into()));
            }
        } else {
            return Err(VmError::TypeError("Boolean.prototype.toString requires that 'this' be a Boolean".into()));
        };
        Ok(vm.alloc_string(if b { "true".into() } else { "false".into() }))
    });
    vm.set_property(boolean_proto, "valueOf", bool_value_of);
    vm.set_property(boolean_proto, "toString", bool_to_string);

    let bool_ctor = vm.alloc_native_function("Boolean", 1, |vm, _this, args| {
        let b = args.first().map(|v| v.is_truthy()).unwrap_or(false);
        if vm.is_constructing {
            let proto = vm.boolean_proto;
            Ok(vm.alloc_boolean_object(b, proto))
        } else {
            Ok(JsValue::from_boolean(b))
        }
    });
    vm.set_property(boolean_proto, "constructor", bool_ctor);
    vm.non_enumerable_props.insert((boolean_proto, "constructor".to_string()));
    vm.set_property(bool_ctor, "prototype", boolean_proto);
    vm.non_enumerable_props.insert((bool_ctor, "prototype".to_string()));
    vm.globals.insert("Boolean".to_string(), bool_ctor);
    if vm.global_object.is_object() {
        vm.set_property(vm.global_object, "Boolean", bool_ctor);
    }

    let error_to_string = vm.alloc_native_function("toString", 0, |vm, this, _args| {
        if !this.is_object() {
            return Err(VmError::TypeError("Error.prototype.toString called on non-object".into()));
        }
        let name_val = vm.get_property(this, "name");
        let name = if name_val.is_undefined() {
            "Error".to_string()
        } else {
            let prim = vm.to_primitive(name_val, crate::interpreter::PreferredType::String)?;
            vm.to_string_value(prim)
        };
        let msg_val = vm.get_property(this, "message");
        let msg = if msg_val.is_undefined() {
            "".to_string()
        } else {
            let prim = vm.to_primitive(msg_val, crate::interpreter::PreferredType::String)?;
            vm.to_string_value(prim)
        };
        if name.is_empty() {
            Ok(vm.alloc_string(msg))
        } else if msg.is_empty() {
            Ok(vm.alloc_string(name))
        } else {
            Ok(vm.alloc_string(format!("{}: {}", name, msg)))
        }
    });

    let error_types = ["Error", "TypeError", "RangeError", "ReferenceError", "SyntaxError", "URIError", "EvalError", "AggregateError"];
    let mut error_proto = JsValue::undefined();
    let mut error_ctor = JsValue::undefined();
    for err_type in error_types {
        let err_name = err_type.to_string();
        let proto = vm.alloc_object();
        let name_val = vm.alloc_string(err_name.clone());
        vm.set_property(proto, "name", name_val);
        vm.non_enumerable_props.insert((proto, "name".to_string()));

        let empty_msg = vm.alloc_string("".to_string());
        vm.set_property(proto, "message", empty_msg);
        vm.non_enumerable_props.insert((proto, "message".to_string()));

        if err_type == "Error" {
            error_proto = proto;
            vm.set_property(proto, "toString", error_to_string);
            vm.non_enumerable_props.insert((proto, "toString".to_string()));
        } else if let Some(ptr) = proto.as_object_ptr() {
            let proto_obj = unsafe { &mut *(ptr as *mut JsObject) };
            proto_obj.prototype = Some(error_proto);
        }

        let err_name_c = err_name.clone();
        let length = if err_type == "AggregateError" { 2 } else { 1 };
        let ctor = vm.alloc_closure_native_function(err_type, length, Arc::new(move |vm, _this, args| {
            let (msg_arg, opts_arg) = if err_name_c == "AggregateError" {
                (args.get(1).copied(), args.get(2).copied())
            } else {
                (args.first().copied(), args.get(1).copied())
            };
            let msg = if let Some(arg) = msg_arg {
                if !arg.is_undefined() {
                    if arg.is_symbol() {
                        return Err(VmError::TypeError("Cannot convert a Symbol value to a string".into()));
                    }
                    let prim = vm.to_primitive(arg, crate::interpreter::PreferredType::String)?;
                    if prim.is_symbol() {
                        return Err(VmError::TypeError("Cannot convert a Symbol value to a string".into()));
                    }
                    Some(vm.to_string_value(prim))
                } else {
                    None
                }
            } else {
                None
            };
            let err_val = vm.alloc_error_opt(err_name_c.clone(), msg);
            if err_name_c == "AggregateError" {
                let mut elements = Vec::new();
                if let Some(errs_arg) = args.first().copied() {
                    if let Some(ptr) = errs_arg.as_object_ptr() {
                        let obj = unsafe { &*(ptr as *mut JsObject) };
                        if let ObjectKind::Array(arr) = &obj.kind {
                            elements = arr.clone();
                        } else {
                            // Try iterating via Symbol.iterator
                            let iter_res = (|| -> Result<Vec<JsValue>, VmError> {
                                let sym_iter = vm.symbol_iterator.ok_or_else(|| VmError::TypeError("No Symbol.iterator".into()))?;
                                let method = vm.get_symbol_property_result(errs_arg, sym_iter)?;
                                if !vm.is_callable(method) {
                                    return Err(VmError::TypeError("Symbol.iterator is not callable".into()));
                                }
                                let it = vm.call(method, errs_arg, &[])?;
                                let next_fn = vm.get_property(it, "next");
                                let mut res = Vec::new();
                                loop {
                                    let step = vm.call(next_fn, it, &[])?;
                                    let done = vm.get_property(step, "done");
                                    if done.is_truthy() {
                                        break;
                                    }
                                    let val = vm.get_property(step, "value");
                                    res.push(val);
                                }
                                Ok(res)
                            })();
                            match iter_res {
                                Ok(items) => elements = items,
                                Err(e) => return Err(e),
                            }
                        }
                    } else {
                        return Err(VmError::TypeError("AggregateError errors argument is not iterable".into()));
                    }
                }
                let errors_arr = vm.alloc_array(elements);
                vm.set_property(err_val, "errors", errors_arr);
                vm.non_enumerable_props.insert((err_val, "errors".to_string()));
            }
            if let Some(opts) = opts_arg {
                if opts.is_object() {
                    let has_cause = vm.has_property(opts, "cause")?;
                    if has_cause {
                        let cause = vm.get_property_result(opts, "cause")?;
                        vm.set_property(err_val, "cause", cause);
                        vm.non_enumerable_props.insert((err_val, "cause".to_string()));
                    }
                }
            }
            Ok(err_val)
        }));

        if err_type == "Error" {
            error_ctor = ctor;
        } else if error_ctor.is_object() {
            if let Some(ptr) = ctor.as_object_ptr() {
                let ctor_obj = unsafe { &mut *(ptr as *mut JsObject) };
                ctor_obj.prototype = Some(error_ctor);
            }
        }

        vm.set_property(proto, "constructor", ctor);
        vm.non_enumerable_props.insert((proto, "constructor".to_string()));
        vm.set_property(ctor, "prototype", proto);
        vm.non_enumerable_props.insert((ctor, "prototype".to_string()));
        vm.non_writable_props.insert((ctor, "prototype".to_string()));
        vm.non_configurable_props.insert((ctor, "prototype".to_string()));
        vm.globals.insert(err_type.to_string(), ctor);
    }

    if error_ctor.is_object() {
        let is_error_fn = vm.alloc_native_function("isError", 1, |_vm, _this, args| {
            if let Some(arg) = args.first() {
                if let Some(ptr) = arg.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    return Ok(JsValue::from_boolean(matches!(obj.kind, ObjectKind::Error { .. })));
                }
            }
            Ok(JsValue::from_boolean(false))
        });
        vm.set_property(error_ctor, "isError", is_error_fn);
    }

    vm.globals.insert("parseInt".to_string(), parse_int);
    vm.globals.insert("parseFloat".to_string(), parse_float);
    vm.globals.insert("isNaN".to_string(), is_nan);
    vm.globals.insert("isFinite".to_string(), is_finite);
    vm.globals.insert("encodeURI".to_string(), encode_uri);
    vm.globals.insert("decodeURI".to_string(), decode_uri);
    vm.globals.insert("encodeURIComponent".to_string(), encode_uri_comp);
    vm.globals.insert("decodeURIComponent".to_string(), decode_uri_comp);
    vm.globals.insert("NaN".to_string(), JsValue::from_double(f64::NAN));
    vm.globals.insert("Infinity".to_string(), JsValue::from_double(f64::INFINITY));
    vm.globals.insert("undefined".to_string(), JsValue::undefined());

    // eval
    let eval_fn = vm.alloc_native_function("eval", 1, |vm, _this, args| {
        if args.is_empty() {
            return Ok(JsValue::undefined());
        }
        let code_val = args[0];
        let code_str = if let Some(ptr) = code_val.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::String(ref s) = obj.kind {
                s.clone()
            } else {
                return Ok(code_val);
            }
        } else {
            return Ok(code_val);
        };

        if let Some(max) = vm.max_evals {
            vm.eval_count += 1;
            if vm.eval_count > max {
                return Err(VmError::RangeError(
                    "Execution limit exceeded in eval (too many calls)".to_string(),
                ));
            }
        }
        if let Some(deadline) = vm.deadline {
            if std::time::Instant::now() > deadline {
                return Err(VmError::RuntimeError(
                    "Execution timeout exceeded in eval".to_string(),
                ));
            }
        }
        if vm.oom {
            return Err(VmError::RangeError("Out of memory in eval".to_string()));
        }

        let is_direct = std::mem::take(&mut vm.in_direct_eval);
        let (caller_strict, caller_in_method, caller_in_function, caller_is_fn) = if is_direct {
            if let Some(f) = vm.frames.last() {
                (f.is_strict, f.is_method, f.is_function && !f.is_arrow, f.is_function)
            } else {
                (vm.is_strict, false, false, false)
            }
        } else {
            (false, false, false, false)
        };

        let program = doms_parser::parse_eval(&code_str, caller_strict, caller_in_method, caller_in_function)
            .map_err(|e| VmError::SyntaxError(e.to_string()))?;

        let has_use_strict = program.body.iter()
            .take_while(|s| matches!(s, doms_parser::ast::Statement::Expression(doms_parser::ast::Expression::Literal(doms_parser::ast::Literal::String { .. }))))
            .any(|s| matches!(s, doms_parser::ast::Statement::Expression(doms_parser::ast::Expression::Literal(doms_parser::ast::Literal::String { value, has_escape: false, .. })) if value == "use strict"));

        let is_eval_strict = caller_strict || has_use_strict;

        let chunk = doms_compiler::compile_eval(&program, is_eval_strict)
            .map_err(|e| VmError::SyntaxError(e.to_string()))?;

        vm.in_eval = true;
        let res = (|| {
            let eval_scope = vm.alloc_object();
            let mut has_eval_scope = false;

            for name in &chunk.top_lex_names {
                vm.set_property(eval_scope, name, JsValue::uninitialized());
                has_eval_scope = true;
            }

            let caller_local_names = vm.frames.last().map(|f| f.local_names.clone()).unwrap_or_default();
            let caller_locals = vm.frames.last().map(|f| f.locals.clone()).unwrap_or_default();
            let caller_env_obj = vm.frames.last().and_then(|f| f.env_obj);

            if is_eval_strict {
                for name in &chunk.top_var_names {
                    vm.set_property(eval_scope, name, JsValue::undefined());
                    has_eval_scope = true;
                }
                for name in &chunk.top_func_names {
                    vm.set_property(eval_scope, name, JsValue::undefined());
                    has_eval_scope = true;
                }
            } else {
                if is_direct {
                    let caller_lexical_names = vm.frames.last().map(|f| f.lexical_local_names.clone()).unwrap_or_default();
                    let caller_in_param_init = vm.frames.last().map(|f| f.in_param_init).unwrap_or(false);
                    let caller_is_arrow = vm.frames.last().map(|f| f.is_arrow).unwrap_or(false);
                    let caller_params: Vec<String> = vm.frames.last()
                        .and_then(|f| f.callee_func.as_ref().map(|func| func.params.clone()))
                        .unwrap_or_default();

                    for name in chunk.top_var_names.iter().chain(chunk.top_func_names.iter()) {
                        if caller_in_param_init {
                            let clean_params: Vec<&str> = caller_params.iter().map(|p| {
                                let trimmed = p.split('=').next().unwrap().trim();
                                trimmed.strip_prefix("...").unwrap_or(trimmed)
                            }).collect();
                            if clean_params.contains(&name.as_str()) {
                                return Err(VmError::SyntaxError(format!("Identifier '{}' has already been declared", name)));
                            }
                            if name == "arguments" && !caller_is_arrow {
                                return Err(VmError::SyntaxError("Cannot declare 'arguments' in function with parameter expressions".into()));
                            }
                        } else {
                            if caller_lexical_names.contains(name) {
                                return Err(VmError::SyntaxError(format!("Identifier '{}' has already been declared", name)));
                            }
                        }
                    }
                }
                for name in chunk.top_var_names.iter().chain(chunk.top_func_names.iter()) {
                    if vm.global_lexicals.contains_key(name) {
                        return Err(VmError::SyntaxError(format!("Identifier '{}' has already been declared", name)));
                    }
                }

                let caller_has_fn = is_direct && caller_is_fn;
                if caller_has_fn {
                    let caller_env = if let Some(env) = caller_env_obj {
                        env
                    } else {
                        let env = vm.alloc_object();
                        for (i, name) in caller_local_names.iter().enumerate() {
                            let val = caller_locals.get(i).copied().unwrap_or(JsValue::undefined());
                            vm.set_property(env, name, val);
                            vm.non_configurable_props.insert((env, name.clone()));
                        }
                        if let Some(caller_f) = vm.frames.last_mut() {
                            if !caller_f.in_param_init {
                                caller_f.env_obj = Some(env);
                            }
                            let insert_pos = caller_f.upvalues_count.min(caller_f.scopes.len());
                            caller_f.scopes.insert(insert_pos, env);
                        }
                        env
                    };
                    for name in &chunk.top_var_names {
                        if !vm.has_property(caller_env, name).unwrap_or(false) {
                            vm.set_property(caller_env, name, JsValue::undefined());
                        }
                    }
                    for name in &chunk.top_func_names {
                        if !vm.has_property(caller_env, name).unwrap_or(false) {
                            vm.set_property(caller_env, name, JsValue::undefined());
                        }
                    }
                } else {
                    let is_extensible = !vm.non_extensible_objects.contains(&vm.global_object);
                    if !is_extensible {
                        for name in chunk.top_var_names.iter().chain(chunk.top_func_names.iter()) {
                            let has = if let Some(ptr) = vm.global_object.as_object_ptr() {
                                let obj = unsafe { &*(ptr as *mut JsObject) };
                                vm.shapes.has_property(obj.shape_id, name)
                            } else {
                                false
                            };
                            if !has {
                                return Err(VmError::TypeError(format!("Cannot define variable '{}' on non-extensible global object", name)));
                            }
                        }
                    }
                    for name in &chunk.top_var_names {
                        vm.var_names.insert(name.clone());
                        let has = if let Some(ptr) = vm.global_object.as_object_ptr() {
                            let obj = unsafe { &*(ptr as *mut JsObject) };
                            vm.shapes.has_property(obj.shape_id, name)
                        } else {
                            false
                        };
                        if !has {
                            vm.globals.insert(name.clone(), JsValue::undefined());
                            if vm.global_object.is_object() {
                                vm.set_property(vm.global_object, name, JsValue::undefined());
                            }
                        }
                    }
                    for name in &chunk.top_func_names {
                        vm.var_names.insert(name.clone());
                        if !vm.globals.contains_key(name) {
                            vm.globals.insert(name.clone(), JsValue::undefined());
                            if vm.global_object.is_object() {
                                vm.set_property(vm.global_object, name, JsValue::undefined());
                            }
                        }
                    }
                }
            }

            let effective_this = if is_direct {
                vm.frames.last().map(|f| f.this_val).unwrap_or(vm.global_object)
            } else {
                vm.global_object
            };
            let mut eval_scopes = if is_direct {
                vm.frames.last().map(|f| f.scopes.clone()).unwrap_or_default()
            } else {
                Vec::new()
            };
            if has_eval_scope {
                eval_scopes.push(eval_scope);
            }

            let new_target = if is_direct {
                vm.frames.last().and_then(|f| f.new_target)
            } else {
                None
            };

            let target_depth = vm.frames.len();
            let base_sp = vm.stack.len();
            vm.frames.push(crate::interpreter::CallFrame {
                chunk: Arc::new(chunk),
                ip: 0,
                base_sp,
                this_val: effective_this,
                gen_ptr: None,
                locals: Vec::new(),
                local_names: Vec::new(),
                lexical_local_names: Vec::new(),
                scopes: eval_scopes.clone(),
                upvalues_count: eval_scopes.len(),
                env_obj: None,
                arguments: None,
                is_strict: is_eval_strict,
                new_target,
                is_function: false,
                is_method: false,
                is_arrow: false,
                has_param_exprs: false,
                in_param_init: false,
                is_indirect_eval: !is_direct,
                callee_func: None,
                mapped_arguments: None,
            });
            vm.run_until_depth(target_depth)
        })();
        vm.in_eval = false;
        res
    });
    vm.globals.insert("eval".to_string(), eval_fn);
    if vm.global_object.is_object() {
        vm.set_property(vm.global_object, "eval", eval_fn);
    }

    // Date
    let date_proto = vm.alloc_object();

    const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    const DAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

    let date_get_time = vm.alloc_native_function("getTime", 0, |_vm, this, _args| {
        let (_, ms) = get_date_ms(this)?;
        Ok(JsValue::from_double(ms))
    });
    let date_value_of = vm.alloc_native_function("valueOf", 0, |_vm, this, _args| {
        let (_, ms) = get_date_ms(this)?;
        Ok(JsValue::from_double(ms))
    });

    let date_get_full_year = vm.alloc_native_function("getFullYear", 0, |_vm, this, _args| {
        let (_, ms) = get_date_ms(this)?;
        match ms_to_utc(ms) {
            Some((y, _, _, _, _, _, _, _)) => Ok(JsValue::from_double(y as f64)),
            None => Ok(JsValue::from_double(f64::NAN)),
        }
    });
    let date_get_utc_full_year = vm.alloc_native_function("getUTCFullYear", 0, |_vm, this, _args| {
        let (_, ms) = get_date_ms(this)?;
        match ms_to_utc(ms) {
            Some((y, _, _, _, _, _, _, _)) => Ok(JsValue::from_double(y as f64)),
            None => Ok(JsValue::from_double(f64::NAN)),
        }
    });

    let date_get_month = vm.alloc_native_function("getMonth", 0, |_vm, this, _args| {
        let (_, ms) = get_date_ms(this)?;
        match ms_to_utc(ms) {
            Some((_, m, _, _, _, _, _, _)) => Ok(JsValue::from_double(m as f64)),
            None => Ok(JsValue::from_double(f64::NAN)),
        }
    });
    let date_get_utc_month = vm.alloc_native_function("getUTCMonth", 0, |_vm, this, _args| {
        let (_, ms) = get_date_ms(this)?;
        match ms_to_utc(ms) {
            Some((_, m, _, _, _, _, _, _)) => Ok(JsValue::from_double(m as f64)),
            None => Ok(JsValue::from_double(f64::NAN)),
        }
    });

    let date_get_date = vm.alloc_native_function("getDate", 0, |_vm, this, _args| {
        let (_, ms) = get_date_ms(this)?;
        match ms_to_utc(ms) {
            Some((_, _, d, _, _, _, _, _)) => Ok(JsValue::from_double(d as f64)),
            None => Ok(JsValue::from_double(f64::NAN)),
        }
    });
    let date_get_utc_date = vm.alloc_native_function("getUTCDate", 0, |_vm, this, _args| {
        let (_, ms) = get_date_ms(this)?;
        match ms_to_utc(ms) {
            Some((_, _, d, _, _, _, _, _)) => Ok(JsValue::from_double(d as f64)),
            None => Ok(JsValue::from_double(f64::NAN)),
        }
    });

    let date_get_day = vm.alloc_native_function("getDay", 0, |_vm, this, _args| {
        let (_, ms) = get_date_ms(this)?;
        match ms_to_utc(ms) {
            Some((_, _, _, w, _, _, _, _)) => Ok(JsValue::from_double(w as f64)),
            None => Ok(JsValue::from_double(f64::NAN)),
        }
    });
    let date_get_utc_day = vm.alloc_native_function("getUTCDay", 0, |_vm, this, _args| {
        let (_, ms) = get_date_ms(this)?;
        match ms_to_utc(ms) {
            Some((_, _, _, w, _, _, _, _)) => Ok(JsValue::from_double(w as f64)),
            None => Ok(JsValue::from_double(f64::NAN)),
        }
    });

    let date_get_hours = vm.alloc_native_function("getHours", 0, |_vm, this, _args| {
        let (_, ms) = get_date_ms(this)?;
        match ms_to_utc(ms) {
            Some((_, _, _, _, h, _, _, _)) => Ok(JsValue::from_double(h as f64)),
            None => Ok(JsValue::from_double(f64::NAN)),
        }
    });
    let date_get_utc_hours = vm.alloc_native_function("getUTCHours", 0, |_vm, this, _args| {
        let (_, ms) = get_date_ms(this)?;
        match ms_to_utc(ms) {
            Some((_, _, _, _, h, _, _, _)) => Ok(JsValue::from_double(h as f64)),
            None => Ok(JsValue::from_double(f64::NAN)),
        }
    });

    let date_get_minutes = vm.alloc_native_function("getMinutes", 0, |_vm, this, _args| {
        let (_, ms) = get_date_ms(this)?;
        match ms_to_utc(ms) {
            Some((_, _, _, _, _, min, _, _)) => Ok(JsValue::from_double(min as f64)),
            None => Ok(JsValue::from_double(f64::NAN)),
        }
    });
    let date_get_utc_minutes = vm.alloc_native_function("getUTCMinutes", 0, |_vm, this, _args| {
        let (_, ms) = get_date_ms(this)?;
        match ms_to_utc(ms) {
            Some((_, _, _, _, _, min, _, _)) => Ok(JsValue::from_double(min as f64)),
            None => Ok(JsValue::from_double(f64::NAN)),
        }
    });

    let date_get_seconds = vm.alloc_native_function("getSeconds", 0, |_vm, this, _args| {
        let (_, ms) = get_date_ms(this)?;
        match ms_to_utc(ms) {
            Some((_, _, _, _, _, _, s, _)) => Ok(JsValue::from_double(s as f64)),
            None => Ok(JsValue::from_double(f64::NAN)),
        }
    });
    let date_get_utc_seconds = vm.alloc_native_function("getUTCSeconds", 0, |_vm, this, _args| {
        let (_, ms) = get_date_ms(this)?;
        match ms_to_utc(ms) {
            Some((_, _, _, _, _, _, s, _)) => Ok(JsValue::from_double(s as f64)),
            None => Ok(JsValue::from_double(f64::NAN)),
        }
    });

    let date_get_millis = vm.alloc_native_function("getMilliseconds", 0, |_vm, this, _args| {
        let (_, ms) = get_date_ms(this)?;
        match ms_to_utc(ms) {
            Some((_, _, _, _, _, _, _, m)) => Ok(JsValue::from_double(m as f64)),
            None => Ok(JsValue::from_double(f64::NAN)),
        }
    });
    let date_get_utc_millis = vm.alloc_native_function("getUTCMilliseconds", 0, |_vm, this, _args| {
        let (_, ms) = get_date_ms(this)?;
        match ms_to_utc(ms) {
            Some((_, _, _, _, _, _, _, m)) => Ok(JsValue::from_double(m as f64)),
            None => Ok(JsValue::from_double(f64::NAN)),
        }
    });

    let date_get_tz_offset = vm.alloc_native_function("getTimezoneOffset", 0, |_vm, this, _args| {
        let _ = get_date_ms(this)?;
        Ok(JsValue::from_double(0.0))
    });

    let date_get_year = vm.alloc_native_function("getYear", 0, |_vm, this, _args| {
        let (_, ms) = get_date_ms(this)?;
        match ms_to_utc(ms) {
            Some((y, _, _, _, _, _, _, _)) => Ok(JsValue::from_double((y - 1900) as f64)),
            None => Ok(JsValue::from_double(f64::NAN)),
        }
    });
    let date_set_year = vm.alloc_native_function("setYear", 1, |vm, this, args| {
        let (ptr, ms) = get_date_ms(this)?;
        let arg_y = args.first().and_then(|v| vm.to_number_value(*v).ok()).unwrap_or(f64::NAN);
        if arg_y.is_nan() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            obj.kind = ObjectKind::Date(f64::NAN);
            return Ok(JsValue::from_double(f64::NAN));
        }
        let mut y = arg_y as i32;
        if (0..=99).contains(&y) {
            y += 1900;
        }
        let (_, m, d, _, h, min, s, ms_part) = ms_to_utc(ms).unwrap_or((1970, 0, 1, 0, 0, 0, 0, 0));
        let new_ms = utc_to_ms(y, m, d, h, min, s, ms_part);
        let obj = unsafe { &mut *(ptr as *mut JsObject) };
        obj.kind = ObjectKind::Date(new_ms);
        Ok(JsValue::from_double(new_ms))
    });

    let date_set_time = vm.alloc_native_function("setTime", 1, |vm, this, args| {
        let (ptr, _) = get_date_ms(this)?;
        let new_ms = args.first().and_then(|v| vm.to_number_value(*v).ok()).unwrap_or(f64::NAN);
        let obj = unsafe { &mut *(ptr as *mut JsObject) };
        obj.kind = ObjectKind::Date(new_ms);
        Ok(JsValue::from_double(new_ms))
    });

    let date_to_iso_string = vm.alloc_native_function("toISOString", 0, |vm, this, _args| {
        let (_, ms) = get_date_ms(this)?;
        match ms_to_utc(ms) {
            Some((y, m, d, _, h, min, s, ms_part)) => {
                let iso = format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z", y, m + 1, d, h, min, s, ms_part);
                Ok(vm.alloc_string(iso))
            }
            None => Err(VmError::RangeError("Invalid time value".into())),
        }
    });

    let date_to_utc_string = vm.alloc_native_function("toUTCString", 0, |vm, this, _args| {
        let (_, ms) = get_date_ms(this)?;
        match ms_to_utc(ms) {
            Some((y, m, d, w, h, min, s, _)) => {
                let day_name = DAYS.get(w as usize).unwrap_or(&"Sun");
                let mon_name = MONTHS.get(m as usize).unwrap_or(&"Jan");
                let utc_str = format!("{}, {:02} {} {:04} {:02}:{:02}:{:02} GMT", day_name, d, mon_name, y, h, min, s);
                Ok(vm.alloc_string(utc_str))
            }
            None => Ok(vm.alloc_string("Invalid Date".into())),
        }
    });

    let date_to_date_string = vm.alloc_native_function("toDateString", 0, |vm, this, _args| {
        let (_, ms) = get_date_ms(this)?;
        match ms_to_utc(ms) {
            Some((y, m, d, w, _, _, _, _)) => {
                let day_name = DAYS.get(w as usize).unwrap_or(&"Sun");
                let mon_name = MONTHS.get(m as usize).unwrap_or(&"Jan");
                let s = format!("{} {} {:02} {:04}", day_name, mon_name, d, y);
                Ok(vm.alloc_string(s))
            }
            None => Ok(vm.alloc_string("Invalid Date".into())),
        }
    });

    let date_to_time_string = vm.alloc_native_function("toTimeString", 0, |vm, this, _args| {
        let (_, ms) = get_date_ms(this)?;
        match ms_to_utc(ms) {
            Some((_, _, _, _, h, min, s, _)) => {
                let s = format!("{:02}:{:02}:{:02} GMT+0000 (Coordinated Universal Time)", h, min, s);
                Ok(vm.alloc_string(s))
            }
            None => Ok(vm.alloc_string("Invalid Date".into())),
        }
    });

    let date_to_string = vm.alloc_native_function("toString", 0, |vm, this, _args| {
        let (_, ms) = get_date_ms(this)?;
        match ms_to_utc(ms) {
            Some((y, m, d, w, h, min, s, _)) => {
                let day_name = DAYS.get(w as usize).unwrap_or(&"Sun");
                let mon_name = MONTHS.get(m as usize).unwrap_or(&"Jan");
                let s = format!("{} {} {:02} {:04} {:02}:{:02}:{:02} GMT+0000 (Coordinated Universal Time)", day_name, mon_name, d, y, h, min, s);
                Ok(vm.alloc_string(s))
            }
            None => Ok(vm.alloc_string("Invalid Date".into())),
        }
    });

    let date_to_json = vm.alloc_native_function("toJSON", 1, |vm, this, _args| {
        if let Some(_ptr) = this.as_object_ptr() {
            let to_iso = vm.get_property(this, "toISOString");
            if vm.is_callable(to_iso) {
                return vm.call(to_iso, this, &[]);
            }
        }
        Err(VmError::TypeError("toISOString is not callable".into()))
    });

    let date_methods = [
        ("getTime", date_get_time),
        ("valueOf", date_value_of),
        ("getFullYear", date_get_full_year),
        ("getUTCFullYear", date_get_utc_full_year),
        ("getMonth", date_get_month),
        ("getUTCMonth", date_get_utc_month),
        ("getDate", date_get_date),
        ("getUTCDate", date_get_utc_date),
        ("getDay", date_get_day),
        ("getUTCDay", date_get_utc_day),
        ("getHours", date_get_hours),
        ("getUTCHours", date_get_utc_hours),
        ("getMinutes", date_get_minutes),
        ("getUTCMinutes", date_get_utc_minutes),
        ("getSeconds", date_get_seconds),
        ("getUTCSeconds", date_get_utc_seconds),
        ("getMilliseconds", date_get_millis),
        ("getUTCMilliseconds", date_get_utc_millis),
        ("getTimezoneOffset", date_get_tz_offset),
        ("getYear", date_get_year),
        ("setYear", date_set_year),
        ("setTime", date_set_time),
        ("toISOString", date_to_iso_string),
        ("toUTCString", date_to_utc_string),
        ("toGMTString", date_to_utc_string),
        ("toDateString", date_to_date_string),
        ("toTimeString", date_to_time_string),
        ("toString", date_to_string),
        ("toJSON", date_to_json),
    ];
    for (name, fn_val) in date_methods {
        vm.set_property(date_proto, name, fn_val);
        vm.non_enumerable_props.insert((date_proto, name.to_string()));
    }

    let date_now_fn = vm.alloc_native_function("now", 0, |_vm, _this, _args| {
        let ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as f64)
            .unwrap_or(0.0);
        Ok(JsValue::from_double(ms))
    });
    let date_utc_fn = vm.alloc_native_function("UTC", 7, |vm, _this, args| {
        let y = args.first().and_then(|v| vm.to_number_value(*v).ok()).unwrap_or(0.0) as i32;
        let m = args.get(1).and_then(|v| vm.to_number_value(*v).ok()).unwrap_or(0.0) as i32;
        let d = args.get(2).and_then(|v| vm.to_number_value(*v).ok()).unwrap_or(1.0) as i32;
        let h = args.get(3).and_then(|v| vm.to_number_value(*v).ok()).unwrap_or(0.0) as i32;
        let min = args.get(4).and_then(|v| vm.to_number_value(*v).ok()).unwrap_or(0.0) as i32;
        let s = args.get(5).and_then(|v| vm.to_number_value(*v).ok()).unwrap_or(0.0) as i32;
        let ms = args.get(6).and_then(|v| vm.to_number_value(*v).ok()).unwrap_or(0.0) as i32;
        let final_y = if (0..=99).contains(&y) { y + 1900 } else { y };
        Ok(JsValue::from_double(utc_to_ms(final_y, m, d, h, min, s, ms)))
    });
    let date_parse_fn = vm.alloc_native_function("parse", 1, |vm, _this, args| {
        let s = args.first().map(|v| vm.to_string_value(*v)).unwrap_or_default();
        let trimmed = s.trim();
        if let Some((y, m, d)) = parse_iso_date(trimmed) {
            Ok(JsValue::from_double(utc_to_ms(y, m, d, 0, 0, 0, 0)))
        } else {
            Ok(JsValue::from_double(f64::NAN))
        }
    });

    let date_ctor = vm.alloc_closure_native_function("Date", 7, Arc::new(move |vm, _this, args| {
        if !vm.is_constructing {
            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as f64)
                .unwrap_or(0.0);
            if let Some((y, m, d, w, h, min, s, _)) = ms_to_utc(now_ms) {
                let day_name = DAYS.get(w as usize).unwrap_or(&"Sun");
                let mon_name = MONTHS.get(m as usize).unwrap_or(&"Jan");
                let str_res = format!("{} {} {:02} {:04} {:02}:{:02}:{:02} GMT+0000 (Coordinated Universal Time)", day_name, mon_name, d, y, h, min, s);
                return Ok(vm.alloc_string(str_res));
            }
            return Ok(vm.alloc_string("Invalid Date".into()));
        }

        let time_ms = if args.is_empty() {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as f64)
                .unwrap_or(0.0)
        } else if args.len() == 1 {
            let arg = args[0];
            if arg.is_string() {
                let s = vm.to_string_value(arg);
                if let Some((y, m, d)) = parse_iso_date(&s) {
                    utc_to_ms(y, m, d, 0, 0, 0, 0)
                } else {
                    f64::NAN
                }
            } else {
                vm.to_number_value(arg).unwrap_or(f64::NAN)
            }
        } else {
            let y = args.first().and_then(|v| vm.to_number_value(*v).ok()).unwrap_or(0.0) as i32;
            let m = args.get(1).and_then(|v| vm.to_number_value(*v).ok()).unwrap_or(0.0) as i32;
            let d = args.get(2).and_then(|v| vm.to_number_value(*v).ok()).unwrap_or(1.0) as i32;
            let h = args.get(3).and_then(|v| vm.to_number_value(*v).ok()).unwrap_or(0.0) as i32;
            let min = args.get(4).and_then(|v| vm.to_number_value(*v).ok()).unwrap_or(0.0) as i32;
            let s = args.get(5).and_then(|v| vm.to_number_value(*v).ok()).unwrap_or(0.0) as i32;
            let ms = args.get(6).and_then(|v| vm.to_number_value(*v).ok()).unwrap_or(0.0) as i32;
            let final_y = if (0..=99).contains(&y) { y + 1900 } else { y };
            utc_to_ms(final_y, m, d, h, min, s, ms)
        };
        Ok(vm.alloc_date(Some(date_proto), time_ms))
    }));
    vm.set_property(date_proto, "constructor", date_ctor);
    vm.non_enumerable_props.insert((date_proto, "constructor".to_string()));
    vm.set_property(date_ctor, "prototype", date_proto);
    vm.non_enumerable_props.insert((date_ctor, "prototype".to_string()));

    vm.set_property(date_ctor, "now", date_now_fn);
    vm.non_enumerable_props.insert((date_ctor, "now".to_string()));
    vm.set_property(date_ctor, "UTC", date_utc_fn);
    vm.non_enumerable_props.insert((date_ctor, "UTC".to_string()));
    vm.set_property(date_ctor, "parse", date_parse_fn);
    vm.non_enumerable_props.insert((date_ctor, "parse".to_string()));

    vm.globals.insert("Date".to_string(), date_ctor);

    // Symbol registration moved to register_symbol(vm)
}

fn register_promise(vm: &mut Vm) {
    let promise_ctor = vm.alloc_native_function("Promise", 1, |vm, _this, args| {
        let executor = args.first().copied().unwrap_or(JsValue::undefined());
        if !executor.is_object() {
            return Err(VmError::TypeError("Promise resolver is not a function".into()));
        }
        let promise_val = vm.alloc_promise(crate::object::PromiseState::Pending, JsValue::undefined());
        let p_for_res = promise_val;
        let resolve_fn = vm.alloc_closure_native_function("resolve", 1, Arc::new(move |vm: &mut Vm, _this: JsValue, args: &[JsValue]| -> Result<JsValue, VmError> {
            let res = args.first().copied().unwrap_or(JsValue::undefined());
            vm.resolve_promise(p_for_res, res);
            Ok(JsValue::undefined())
        }));
        let p_for_rej = promise_val;
        let reject_fn = vm.alloc_closure_native_function("reject", 1, Arc::new(move |vm: &mut Vm, _this: JsValue, args: &[JsValue]| -> Result<JsValue, VmError> {
            let rej = args.first().copied().unwrap_or(JsValue::undefined());
            vm.reject_promise(p_for_rej, rej);
            Ok(JsValue::undefined())
        }));

        match vm.call(executor, JsValue::undefined(), &[resolve_fn, reject_fn]) {
            Ok(_) => Ok(promise_val),
            Err(e) => {
                let err_str = vm.alloc_string(e.to_string());
                vm.reject_promise(promise_val, err_str);
                Ok(promise_val)
            }
        }
    });

    let resolve_static = vm.alloc_native_function("resolve", 1, |vm, _this, args| {
        let val = args.first().copied().unwrap_or(JsValue::undefined());
        if let Some(ptr) = val.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if matches!(obj.kind, ObjectKind::Promise(_)) {
                return Ok(val);
            }
        }
        Ok(vm.alloc_promise(crate::object::PromiseState::Fulfilled, val))
    });

    let reject_static = vm.alloc_native_function("reject", 1, |vm, _this, args| {
        let reason = args.first().copied().unwrap_or(JsValue::undefined());
        Ok(vm.alloc_promise(crate::object::PromiseState::Rejected, reason))
    });

    let all_static = vm.alloc_native_function("all", 1, |vm, _this, args| {
        let iterable = args.first().copied().unwrap_or(JsValue::undefined());
        if let Some(ptr) = iterable.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::Array(elements) = &obj.kind {
                let elements_clone = elements.clone();
                let result_promise = vm.alloc_promise(crate::object::PromiseState::Pending, JsValue::undefined());
                if elements_clone.is_empty() {
                    let empty_arr = vm.alloc_array(Vec::new());
                    vm.resolve_promise(result_promise, empty_arr);
                    return Ok(result_promise);
                }
                let count = elements_clone.len();
                let remaining = Arc::new(std::sync::atomic::AtomicUsize::new(count));
                let results = Arc::new(std::sync::Mutex::new(vec![JsValue::undefined(); count]));

                for (idx, elem) in elements_clone.into_iter().enumerate() {
                    let r_p = result_promise;
                    let rem = Arc::clone(&remaining);
                    let res_store = Arc::clone(&results);

                    let resolve_item = vm.alloc_closure_native_function("resolve_item", 1, Arc::new(move |vm: &mut Vm, _this: JsValue, args: &[JsValue]| -> Result<JsValue, VmError> {
                        let v = args.first().copied().unwrap_or(JsValue::undefined());
                        {
                            let mut lock = res_store.lock().unwrap();
                            lock[idx] = v;
                        }
                        if rem.fetch_sub(1, std::sync::atomic::Ordering::SeqCst) == 1 {
                            let final_res = {
                                let lock = res_store.lock().unwrap();
                                lock.clone()
                            };
                            let arr = vm.alloc_array(final_res);
                            vm.resolve_promise(r_p, arr);
                        }
                        Ok(JsValue::undefined())
                    }));

                    let reject_item = vm.alloc_closure_native_function("reject_item", 1, Arc::new(move |vm: &mut Vm, _this: JsValue, args: &[JsValue]| -> Result<JsValue, VmError> {
                        let err = args.first().copied().unwrap_or(JsValue::undefined());
                        vm.reject_promise(r_p, err);
                        Ok(JsValue::undefined())
                    }));

                    if let Some(p_ptr) = elem.as_object_ptr() {
                        let p_obj = unsafe { &*(p_ptr as *mut JsObject) };
                        if matches!(p_obj.kind, ObjectKind::Promise(_)) {
                            let then_fn = vm.get_property(elem, "then");
                            let _ = vm.call(then_fn, elem, &[resolve_item, reject_item]);
                            continue;
                        }
                    }
                    let _ = vm.call(resolve_item, JsValue::undefined(), &[elem]);
                }

                return Ok(result_promise);
            }
        }
        let empty_arr = vm.alloc_array(Vec::new());
        Ok(vm.alloc_promise(crate::object::PromiseState::Fulfilled, empty_arr))
    });

    let race_static = vm.alloc_native_function("race", 1, |vm, _this, args| {
        let iterable = args.first().copied().unwrap_or(JsValue::undefined());
        let result_promise = vm.alloc_promise(crate::object::PromiseState::Pending, JsValue::undefined());
        if let Some(ptr) = iterable.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::Array(elements) = &obj.kind {
                let elements_clone = elements.clone();
                for elem in elements_clone {
                    let r_p = result_promise;
                    let resolve_item = vm.alloc_closure_native_function("race_res", 1, Arc::new(move |vm: &mut Vm, _this: JsValue, args: &[JsValue]| -> Result<JsValue, VmError> {
                        let v = args.first().copied().unwrap_or(JsValue::undefined());
                        vm.resolve_promise(r_p, v);
                        Ok(JsValue::undefined())
                    }));
                    let reject_item = vm.alloc_closure_native_function("race_rej", 1, Arc::new(move |vm: &mut Vm, _this: JsValue, args: &[JsValue]| -> Result<JsValue, VmError> {
                        let err = args.first().copied().unwrap_or(JsValue::undefined());
                        vm.reject_promise(r_p, err);
                        Ok(JsValue::undefined())
                    }));

                    if let Some(p_ptr) = elem.as_object_ptr() {
                        let p_obj = unsafe { &*(p_ptr as *mut JsObject) };
                        if matches!(p_obj.kind, ObjectKind::Promise(_)) {
                            let then_fn = vm.get_property(elem, "then");
                            let _ = vm.call(then_fn, elem, &[resolve_item, reject_item]);
                            continue;
                        }
                    }
                    let _ = vm.call(resolve_item, JsValue::undefined(), &[elem]);
                }
            }
        }
        Ok(result_promise)
    });

    let with_resolvers_static = vm.alloc_native_function("withResolvers", 0, |vm, _this, _args| {
        let promise = vm.alloc_promise(crate::object::PromiseState::Pending, JsValue::undefined());
        let resolve_fn = {
            let p = promise;
            vm.alloc_closure_native_function("resolve", 1, Arc::new(move |vm: &mut Vm, _this, args| {
                let val = args.first().copied().unwrap_or(JsValue::undefined());
                vm.resolve_promise(p, val);
                Ok(JsValue::undefined())
            }))
        };
        let reject_fn = {
            let p = promise;
            vm.alloc_closure_native_function("reject", 1, Arc::new(move |vm: &mut Vm, _this, args| {
                let val = args.first().copied().unwrap_or(JsValue::undefined());
                vm.reject_promise(p, val);
                Ok(JsValue::undefined())
            }))
        };
        let res_obj = vm.alloc_object();
        vm.set_property(res_obj, "promise", promise);
        vm.set_property(res_obj, "resolve", resolve_fn);
        vm.set_property(res_obj, "reject", reject_fn);
        Ok(res_obj)
    });

    let all_settled_static = vm.alloc_native_function("allSettled", 1, |vm, _this, args| {
        let iterable = args.first().copied().unwrap_or(JsValue::undefined());
        if let Some(ptr) = iterable.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::Array(elements) = &obj.kind {
                let elements_clone = elements.clone();
                let result_promise = vm.alloc_promise(crate::object::PromiseState::Pending, JsValue::undefined());
                if elements_clone.is_empty() {
                    let empty_arr = vm.alloc_array(Vec::new());
                    vm.resolve_promise(result_promise, empty_arr);
                    return Ok(result_promise);
                }
                let count = elements_clone.len();
                let remaining = Arc::new(std::sync::atomic::AtomicUsize::new(count));
                let results = Arc::new(std::sync::Mutex::new(vec![JsValue::undefined(); count]));

                for (idx, elem) in elements_clone.into_iter().enumerate() {
                    let r_p = result_promise;
                    let rem = Arc::clone(&remaining);
                    let res_store = Arc::clone(&results);

                    let resolve_item = vm.alloc_closure_native_function("settled_res", 1, Arc::new(move |vm: &mut Vm, _this: JsValue, args: &[JsValue]| -> Result<JsValue, VmError> {
                        let v = args.first().copied().unwrap_or(JsValue::undefined());
                        let item_obj = vm.alloc_object();
                        let status_str = vm.alloc_string("fulfilled".into());
                        vm.set_property(item_obj, "status", status_str);
                        vm.set_property(item_obj, "value", v);
                        {
                            let mut lock = res_store.lock().unwrap();
                            lock[idx] = item_obj;
                        }
                        if rem.fetch_sub(1, std::sync::atomic::Ordering::SeqCst) == 1 {
                            let final_res = {
                                let lock = res_store.lock().unwrap();
                                lock.clone()
                            };
                            let arr = vm.alloc_array(final_res);
                            vm.resolve_promise(r_p, arr);
                        }
                        Ok(JsValue::undefined())
                    }));

                    let rem_rej = Arc::clone(&remaining);
                    let res_store_rej = Arc::clone(&results);
                    let reject_item = vm.alloc_closure_native_function("settled_rej", 1, Arc::new(move |vm: &mut Vm, _this: JsValue, args: &[JsValue]| -> Result<JsValue, VmError> {
                        let err = args.first().copied().unwrap_or(JsValue::undefined());
                        let item_obj = vm.alloc_object();
                        let status_str = vm.alloc_string("rejected".into());
                        vm.set_property(item_obj, "status", status_str);
                        vm.set_property(item_obj, "reason", err);
                        {
                            let mut lock = res_store_rej.lock().unwrap();
                            lock[idx] = item_obj;
                        }
                        if rem_rej.fetch_sub(1, std::sync::atomic::Ordering::SeqCst) == 1 {
                            let final_res = {
                                let lock = res_store_rej.lock().unwrap();
                                lock.clone()
                            };
                            let arr = vm.alloc_array(final_res);
                            vm.resolve_promise(r_p, arr);
                        }
                        Ok(JsValue::undefined())
                    }));

                    if let Some(p_ptr) = elem.as_object_ptr() {
                        let p_obj = unsafe { &*(p_ptr as *mut JsObject) };
                        if matches!(p_obj.kind, ObjectKind::Promise(_)) {
                            let then_fn = vm.get_property(elem, "then");
                            let _ = vm.call(then_fn, elem, &[resolve_item, reject_item]);
                            continue;
                        }
                    }
                    let _ = vm.call(resolve_item, JsValue::undefined(), &[elem]);
                }

                return Ok(result_promise);
            }
        }
        let empty_arr = vm.alloc_array(Vec::new());
        Ok(vm.alloc_promise(crate::object::PromiseState::Fulfilled, empty_arr))
    });

    let any_static = vm.alloc_native_function("any", 1, |vm, _this, args| {
        let iterable = args.first().copied().unwrap_or(JsValue::undefined());
        if let Some(ptr) = iterable.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::Array(elements) = &obj.kind {
                let elements_clone = elements.clone();
                let result_promise = vm.alloc_promise(crate::object::PromiseState::Pending, JsValue::undefined());
                if elements_clone.is_empty() {
                    let err_str = vm.alloc_string("All promises were rejected".into());
                    vm.reject_promise(result_promise, err_str);
                    return Ok(result_promise);
                }
                let count = elements_clone.len();
                let remaining = Arc::new(std::sync::atomic::AtomicUsize::new(count));
                let errors = Arc::new(std::sync::Mutex::new(vec![JsValue::undefined(); count]));

                for (idx, elem) in elements_clone.into_iter().enumerate() {
                    let r_p = result_promise;
                    let resolve_item = vm.alloc_closure_native_function("any_res", 1, Arc::new(move |vm: &mut Vm, _this: JsValue, args: &[JsValue]| -> Result<JsValue, VmError> {
                        let v = args.first().copied().unwrap_or(JsValue::undefined());
                        vm.resolve_promise(r_p, v);
                        Ok(JsValue::undefined())
                    }));

                    let rem = Arc::clone(&remaining);
                    let err_store = Arc::clone(&errors);
                    let reject_item = vm.alloc_closure_native_function("any_rej", 1, Arc::new(move |vm: &mut Vm, _this: JsValue, args: &[JsValue]| -> Result<JsValue, VmError> {
                        let err = args.first().copied().unwrap_or(JsValue::undefined());
                        {
                            let mut lock = err_store.lock().unwrap();
                            lock[idx] = err;
                        }
                        if rem.fetch_sub(1, std::sync::atomic::Ordering::SeqCst) == 1 {
                            let final_errs = {
                                let lock = err_store.lock().unwrap();
                                lock.clone()
                            };
                            let err_arr = vm.alloc_array(final_errs);
                            vm.reject_promise(r_p, err_arr);
                        }
                        Ok(JsValue::undefined())
                    }));

                    if let Some(p_ptr) = elem.as_object_ptr() {
                        let p_obj = unsafe { &*(p_ptr as *mut JsObject) };
                        if matches!(p_obj.kind, ObjectKind::Promise(_)) {
                            let then_fn = vm.get_property(elem, "then");
                            let _ = vm.call(then_fn, elem, &[resolve_item, reject_item]);
                            continue;
                        }
                    }
                    let _ = vm.call(resolve_item, JsValue::undefined(), &[elem]);
                }
                return Ok(result_promise);
            }
        }
        let err_str = vm.alloc_string("All promises were rejected".into());
        Ok(vm.alloc_promise(crate::object::PromiseState::Rejected, err_str))
    });

    vm.set_property(promise_ctor, "resolve", resolve_static);
    vm.set_property(promise_ctor, "reject", reject_static);
    vm.set_property(promise_ctor, "all", all_static);
    vm.set_property(promise_ctor, "race", race_static);
    vm.set_property(promise_ctor, "allSettled", all_settled_static);
    vm.set_property(promise_ctor, "any", any_static);
    vm.set_property(promise_ctor, "withResolvers", with_resolvers_static);

    let promise_proto = vm.alloc_object();
    vm.promise_proto = Some(promise_proto);

    let then_fn = vm.alloc_native_function("then", 2, |vm, this, args| {
        let on_fulfilled = args.first().copied().unwrap_or(JsValue::undefined());
        let on_rejected = args.get(1).copied().unwrap_or(JsValue::undefined());
        vm.promise_then(this, on_fulfilled, on_rejected)
    });
    let catch_fn = vm.alloc_native_function("catch", 1, |vm, this, args| {
        let on_rejected = args.first().copied().unwrap_or(JsValue::undefined());
        let then_val = vm.get_property(this, "then");
        vm.call(then_val, this, &[JsValue::undefined(), on_rejected])
    });
    let finally_fn = vm.alloc_native_function("finally", 1, |vm, this, args| {
        let on_finally = args.first().copied().unwrap_or(JsValue::undefined());
        let then_val = vm.get_property(this, "then");
        if !vm.is_callable(on_finally) {
            return vm.call(then_val, this, &[on_finally, on_finally]);
        }
        let on_fin1 = on_finally;
        let then_fin1 = vm.alloc_closure_native_function("finally_fulfilled", 1, Arc::new(move |vm: &mut Vm, _, args: &[JsValue]| -> Result<JsValue, VmError> {
            let val = args.first().copied().unwrap_or(JsValue::undefined());
            let res = vm.call(on_fin1, JsValue::undefined(), &[])?;
            let p_ctor = vm.globals.get("Promise").copied().unwrap_or(JsValue::undefined());
            let res_fn = vm.get_property(p_ctor, "resolve");
            let p = vm.call(res_fn, p_ctor, &[res])?;
            let ret_cb = vm.alloc_closure_native_function("ret_cb", 0, Arc::new(move |_vm, _, _| Ok(val)));
            let p_then = vm.get_property(p, "then");
            vm.call(p_then, p, &[ret_cb])
        }));
        let on_fin2 = on_finally;
        let then_fin2 = vm.alloc_closure_native_function("finally_rejected", 1, Arc::new(move |vm: &mut Vm, _, args: &[JsValue]| -> Result<JsValue, VmError> {
            let err = args.first().copied().unwrap_or(JsValue::undefined());
            let res = vm.call(on_fin2, JsValue::undefined(), &[])?;
            let p_ctor = vm.globals.get("Promise").copied().unwrap_or(JsValue::undefined());
            let res_fn = vm.get_property(p_ctor, "resolve");
            let p = vm.call(res_fn, p_ctor, &[res])?;
            let throw_cb = vm.alloc_closure_native_function("throw_cb", 0, Arc::new(move |_vm, _, _| Err(VmError::Thrown(err))));
            let p_then = vm.get_property(p, "then");
            vm.call(p_then, p, &[throw_cb])
        }));
        vm.call(then_val, this, &[then_fin1, then_fin2])
    });

    vm.set_property(promise_proto, "then", then_fn);
    vm.set_property(promise_proto, "catch", catch_fn);
    vm.set_property(promise_proto, "finally", finally_fn);
    vm.set_property(promise_proto, "constructor", promise_ctor);
    vm.non_enumerable_props.insert((promise_proto, "then".to_string()));
    vm.non_enumerable_props.insert((promise_proto, "catch".to_string()));
    vm.non_enumerable_props.insert((promise_proto, "finally".to_string()));
    vm.non_enumerable_props.insert((promise_proto, "constructor".to_string()));

    if let Some(sym_tag) = vm.symbol_to_string_tag {
        let tag_val = vm.alloc_string("Promise".to_string());
        vm.set_symbol_property(promise_proto, sym_tag, tag_val);
        vm.non_enumerable_props.insert((promise_proto, "[Symbol.toStringTag]".to_string()));
    }

    vm.set_property(promise_ctor, "prototype", promise_proto);
    vm.non_enumerable_props.insert((promise_ctor, "prototype".to_string()));
    add_species_getter(vm, promise_ctor);

    vm.globals.insert("Promise".to_string(), promise_ctor);
    if vm.global_object.is_object() {
        vm.set_property(vm.global_object, "Promise", promise_ctor);
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum IterKind {
    Keys,
    Values,
    Entries,
}

fn create_array_iterator(vm: &mut Vm, target: JsValue, kind: IterKind) -> Result<JsValue, VmError> {
    let iter_obj = vm.alloc_object();
    let idx = Arc::new(std::sync::Mutex::new(0usize));
    let next_fn = vm.alloc_closure_native_function("next", 0, Arc::new(move |vm, _this, _args| {
        let mut i = idx.lock().unwrap();
        let len = vm.get_property(target, "length").to_number().unwrap_or(0.0) as usize;
        let res = vm.alloc_object();
        if *i < len {
            let curr = *i;
            *i += 1;
            let val = match kind {
                IterKind::Keys => JsValue::from_int32(curr as i32),
                IterKind::Values => vm.get_element(target, curr),
                IterKind::Entries => {
                    let el = vm.get_element(target, curr);
                    vm.alloc_array(vec![JsValue::from_int32(curr as i32), el])
                }
            };
            vm.set_property(res, "value", val);
            vm.set_property(res, "done", JsValue::from_boolean(false));
        } else {
            vm.set_property(res, "value", JsValue::undefined());
            vm.set_property(res, "done", JsValue::from_boolean(true));
        }
        Ok(res)
    }));
    vm.set_property(iter_obj, "next", next_fn);
    if let Some(sym_iter) = vm.symbol_iterator {
        let self_val = iter_obj;
        let self_fn = vm.alloc_closure_native_function("[Symbol.iterator]", 0, Arc::new(move |_vm, _this, _args| Ok(self_val)));
        vm.set_symbol_property(iter_obj, sym_iter, self_fn);
    }
    Ok(iter_obj)
}

fn construct_typed_array_kind(vm: &mut Vm, args: &[JsValue], kind: TypedArrayKind) -> Result<JsValue, VmError> {
    let bpe = kind.bytes_per_element();
    let arg0 = args.first().copied().unwrap_or(JsValue::undefined());
    // 1. ArrayBuffer or SharedArrayBuffer
    if let Some(ptr) = arg0.as_object_ptr() {
        let obj = unsafe { &*(ptr as *mut JsObject) };
        if let ObjectKind::ArrayBuffer(sab) | ObjectKind::SharedArrayBuffer(sab) = &obj.kind {
            let buf_len = sab.byte_length();
            let byte_offset = args.get(1).and_then(|v| v.to_int32()).unwrap_or(0).max(0) as usize;
            if byte_offset % bpe != 0 {
                return Err(VmError::RangeError("Start offset must be a multiple of the element size".into()));
            }
            if byte_offset > buf_len {
                return Err(VmError::RangeError("Start offset is outside the bounds of the buffer".into()));
            }
            let length = if let Some(len_arg) = args.get(2) {
                let l = len_arg.to_int32().unwrap_or(0).max(0) as usize;
                if byte_offset + l * bpe > buf_len {
                    return Err(VmError::RangeError("Invalid typed array length".into()));
                }
                l
            } else {
                (buf_len - byte_offset) / bpe
            };
            return Ok(vm.alloc_typed_array(JsTypedArray {
                buffer: Arc::clone(sab),
                kind,
                length,
                byte_offset,
            }));
        } else if let ObjectKind::TypedArray(src_ta) = &obj.kind {
            // Another TypedArray
            let len = src_ta.length;
            let new_sab = doms_parallel::SharedArrayBuffer::try_new(len * bpe)
                .map_err(|e| VmError::RuntimeError(e.into()))?;
            let mut dst_ta = JsTypedArray {
                buffer: Arc::new(new_sab),
                kind,
                length: len,
                byte_offset: 0,
            };
            for i in 0..len {
                if let Some(val) = Vm::read_typed_array_element(src_ta, i) {
                    Vm::write_typed_array_element(&mut dst_ta, i, val);
                }
            }
            return Ok(vm.alloc_typed_array(dst_ta));
        } else if let ObjectKind::Array(elements) = &obj.kind {
            // Array
            let len = elements.len();
            let new_sab = doms_parallel::SharedArrayBuffer::try_new(len * bpe)
                .map_err(|e| VmError::RuntimeError(e.into()))?;
            let mut dst_ta = JsTypedArray {
                buffer: Arc::new(new_sab),
                kind,
                length: len,
                byte_offset: 0,
            };
            for (i, el) in elements.iter().enumerate() {
                Vm::write_typed_array_element(&mut dst_ta, i, *el);
            }
            return Ok(vm.alloc_typed_array(dst_ta));
        }
    }

    // 2. Length or number argument
    let len = arg0.to_int32().unwrap_or(0).max(0) as usize;
    if len * bpe > 64 * 1024 * 1024 {
        return Err(VmError::RangeError("Invalid typed array length".into()));
    }
    let sab = doms_parallel::SharedArrayBuffer::try_new(len * bpe)
        .map_err(|e| VmError::RuntimeError(e.into()))?;
    Ok(vm.alloc_typed_array(JsTypedArray {
        buffer: Arc::new(sab),
        kind,
        length: len,
        byte_offset: 0,
    }))
}

fn construct_int8_array(vm: &mut Vm, _this: JsValue, args: &[JsValue]) -> Result<JsValue, VmError> {
    construct_typed_array_kind(vm, args, TypedArrayKind::Int8)
}
fn construct_uint8_array(vm: &mut Vm, _this: JsValue, args: &[JsValue]) -> Result<JsValue, VmError> {
    construct_typed_array_kind(vm, args, TypedArrayKind::Uint8)
}
fn construct_uint8_clamped_array(vm: &mut Vm, _this: JsValue, args: &[JsValue]) -> Result<JsValue, VmError> {
    construct_typed_array_kind(vm, args, TypedArrayKind::Uint8Clamped)
}
fn construct_int16_array(vm: &mut Vm, _this: JsValue, args: &[JsValue]) -> Result<JsValue, VmError> {
    construct_typed_array_kind(vm, args, TypedArrayKind::Int16)
}
fn construct_uint16_array(vm: &mut Vm, _this: JsValue, args: &[JsValue]) -> Result<JsValue, VmError> {
    construct_typed_array_kind(vm, args, TypedArrayKind::Uint16)
}
fn construct_int32_array(vm: &mut Vm, _this: JsValue, args: &[JsValue]) -> Result<JsValue, VmError> {
    construct_typed_array_kind(vm, args, TypedArrayKind::Int32)
}
fn construct_uint32_array(vm: &mut Vm, _this: JsValue, args: &[JsValue]) -> Result<JsValue, VmError> {
    construct_typed_array_kind(vm, args, TypedArrayKind::Uint32)
}
fn construct_float32_array(vm: &mut Vm, _this: JsValue, args: &[JsValue]) -> Result<JsValue, VmError> {
    construct_typed_array_kind(vm, args, TypedArrayKind::Float32)
}
fn construct_float64_array(vm: &mut Vm, _this: JsValue, args: &[JsValue]) -> Result<JsValue, VmError> {
    construct_typed_array_kind(vm, args, TypedArrayKind::Float64)
}
fn construct_bigint64_array(vm: &mut Vm, _this: JsValue, args: &[JsValue]) -> Result<JsValue, VmError> {
    construct_typed_array_kind(vm, args, TypedArrayKind::BigInt64)
}
fn construct_biguint64_array(vm: &mut Vm, _this: JsValue, args: &[JsValue]) -> Result<JsValue, VmError> {
    construct_typed_array_kind(vm, args, TypedArrayKind::BigUint64)
}

fn register_single_typed_array(vm: &mut Vm, kind: TypedArrayKind) {
    let name = kind.name();
    let bpe = kind.bytes_per_element();

    let ctor_fn: NativeFn = match kind {
        TypedArrayKind::Int8 => construct_int8_array,
        TypedArrayKind::Uint8 => construct_uint8_array,
        TypedArrayKind::Uint8Clamped => construct_uint8_clamped_array,
        TypedArrayKind::Int16 => construct_int16_array,
        TypedArrayKind::Uint16 => construct_uint16_array,
        TypedArrayKind::Int32 => construct_int32_array,
        TypedArrayKind::Uint32 => construct_uint32_array,
        TypedArrayKind::Float32 => construct_float32_array,
        TypedArrayKind::Float64 => construct_float64_array,
        TypedArrayKind::BigInt64 => construct_bigint64_array,
        TypedArrayKind::BigUint64 => construct_biguint64_array,
    };

    let ctor = vm.alloc_native_function(name, 1, ctor_fn);

    let proto = vm.alloc_object();

    let subarray_fn = vm.alloc_native_function("subarray", 2, |vm, this, args| {
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::TypedArray(ta) = &obj.kind {
                let len = ta.length as i32;
                let start = args.first().and_then(|v| v.to_int32()).unwrap_or(0);
                let end = args.get(1).and_then(|v| v.to_int32()).unwrap_or(len);
                let s = if start < 0 { (len + start).max(0) } else { start.min(len) } as usize;
                let e = if end < 0 { (len + end).max(0) } else { end.min(len) } as usize;
                let count = if e > s { e - s } else { 0 };
                let bpe = ta.kind.bytes_per_element();
                let byte_offset = ta.byte_offset + s * bpe;
                return Ok(vm.alloc_typed_array(JsTypedArray {
                    buffer: Arc::clone(&ta.buffer),
                    kind: ta.kind,
                    length: count,
                    byte_offset,
                }));
            }
        }
        Err(VmError::TypeError("Method called on non-TypedArray".into()))
    });
    vm.set_property(proto, "subarray", subarray_fn);

    let set_fn = vm.alloc_native_function("set", 1, |vm, this, args| {
        let src = args.first().copied().unwrap_or(JsValue::undefined());
        let offset = args.get(1).and_then(|v| v.to_int32()).unwrap_or(0).max(0) as usize;
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::TypedArray(dst_ta) = &mut obj.kind {
                if let Some(src_ptr) = src.as_object_ptr() {
                    let src_obj = unsafe { &*(src_ptr as *mut JsObject) };
                    if let ObjectKind::TypedArray(src_ta) = &src_obj.kind {
                        if offset + src_ta.length > dst_ta.length {
                            return Err(VmError::RangeError("Source is too large for target array".into()));
                        }
                        for i in 0..src_ta.length {
                            if let Some(val) = Vm::read_typed_array_element(src_ta, i) {
                                Vm::write_typed_array_element(dst_ta, offset + i, val);
                            }
                        }
                        return Ok(JsValue::undefined());
                    }
                }
                let len = vm.get_property(src, "length").to_number().unwrap_or(0.0) as usize;
                if offset + len > dst_ta.length {
                    return Err(VmError::RangeError("Source is too large for target array".into()));
                }
                for i in 0..len {
                    let val = vm.get_element(src, i);
                    Vm::write_typed_array_element(dst_ta, offset + i, val);
                }
                return Ok(JsValue::undefined());
            }
        }
        Err(VmError::TypeError("Method called on non-TypedArray".into()))
    });
    vm.set_property(proto, "set", set_fn);

    let slice_fn = vm.alloc_native_function("slice", 2, |vm, this, args| {
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::TypedArray(ta) = &obj.kind {
                let len = ta.length as i32;
                let start = args.first().and_then(|v| v.to_int32()).unwrap_or(0);
                let end = args.get(1).and_then(|v| v.to_int32()).unwrap_or(len);
                let s = if start < 0 { (len + start).max(0) } else { start.min(len) } as usize;
                let e = if end < 0 { (len + end).max(0) } else { end.min(len) } as usize;
                let count = if e > s { e - s } else { 0 };
                let bpe = ta.kind.bytes_per_element();
                let new_sab = doms_parallel::SharedArrayBuffer::try_new(count * bpe)
                    .map_err(|e| VmError::RuntimeError(e.into()))?;
                let mut dst_ta = JsTypedArray {
                    buffer: Arc::new(new_sab),
                    kind: ta.kind,
                    length: count,
                    byte_offset: 0,
                };
                for i in 0..count {
                    if let Some(val) = Vm::read_typed_array_element(ta, s + i) {
                        Vm::write_typed_array_element(&mut dst_ta, i, val);
                    }
                }
                return Ok(vm.alloc_typed_array(dst_ta));
            }
        }
        Err(VmError::TypeError("Method called on non-TypedArray".into()))
    });
    vm.set_property(proto, "slice", slice_fn);

    let fill_fn = vm.alloc_native_function("fill", 1, |_vm, this, args| {
        let value = args.first().copied().unwrap_or(JsValue::undefined());
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::TypedArray(ta) = &mut obj.kind {
                let len = ta.length as i32;
                let start = args.get(1).and_then(|v| v.to_int32()).unwrap_or(0);
                let end = args.get(2).and_then(|v| v.to_int32()).unwrap_or(len);
                let s = if start < 0 { (len + start).max(0) } else { start.min(len) } as usize;
                let e = if end < 0 { (len + end).max(0) } else { end.min(len) } as usize;
                for i in s..e {
                    Vm::write_typed_array_element(ta, i, value);
                }
                return Ok(this);
            }
        }
        Err(VmError::TypeError("Method called on non-TypedArray".into()))
    });
    vm.set_property(proto, "fill", fill_fn);

    let index_of_fn = vm.alloc_native_function("indexOf", 1, |_vm, this, args| {
        let target = args.first().and_then(|v| v.to_number()).unwrap_or(0.0);
        let from_idx = args.get(1).and_then(|v| v.to_int32()).unwrap_or(0).max(0) as usize;
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::TypedArray(ta) = &obj.kind {
                for i in from_idx..ta.length {
                    if let Some(val) = Vm::read_typed_array_element(ta, i) {
                        if let Some(n) = val.to_number() {
                            if n == target {
                                return Ok(JsValue::from_int32(i as i32));
                            }
                        }
                    }
                }
                return Ok(JsValue::from_int32(-1));
            }
        }
        Ok(JsValue::from_int32(-1))
    });
    vm.set_property(proto, "indexOf", index_of_fn);

    let includes_fn = vm.alloc_native_function("includes", 1, |_vm, this, args| {
        let target = args.first().and_then(|v| v.to_number()).unwrap_or(0.0);
        let from_idx = args.get(1).and_then(|v| v.to_int32()).unwrap_or(0).max(0) as usize;
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::TypedArray(ta) = &obj.kind {
                for i in from_idx..ta.length {
                    if let Some(val) = Vm::read_typed_array_element(ta, i) {
                        if let Some(n) = val.to_number() {
                            if n == target || (n.is_nan() && target.is_nan()) {
                                return Ok(JsValue::from_boolean(true));
                            }
                        }
                    }
                }
            }
        }
        Ok(JsValue::from_boolean(false))
    });
    vm.set_property(proto, "includes", includes_fn);

    let join_fn = vm.alloc_native_function("join", 1, |vm, this, args| {
        let sep = args.first().map(|v| vm.to_string_value(*v)).unwrap_or_else(|| ",".to_string());
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::TypedArray(ta) = &obj.kind {
                let mut parts = Vec::with_capacity(ta.length);
                for i in 0..ta.length {
                    if let Some(val) = Vm::read_typed_array_element(ta, i) {
                        parts.push(vm.to_string_value(val));
                    }
                }
                return Ok(vm.alloc_string(parts.join(&sep)));
            }
        }
        Ok(vm.alloc_string("".to_string()))
    });
    vm.set_property(proto, "join", join_fn);

    let values_fn = vm.alloc_native_function("values", 0, |vm, this, _args| {
        create_array_iterator(vm, this, IterKind::Values)
    });
    let keys_fn = vm.alloc_native_function("keys", 0, |vm, this, _args| {
        create_array_iterator(vm, this, IterKind::Keys)
    });
    let entries_fn = vm.alloc_native_function("entries", 0, |vm, this, _args| {
        create_array_iterator(vm, this, IterKind::Entries)
    });
    vm.set_property(proto, "values", values_fn);
    vm.set_property(proto, "keys", keys_fn);
    vm.set_property(proto, "entries", entries_fn);
    if let Some(sym_iter) = vm.symbol_iterator {
        vm.set_symbol_property(proto, sym_iter, values_fn);
    }

    vm.set_property(ctor, "BYTES_PER_ELEMENT", JsValue::from_int32(bpe as i32));
    vm.set_property(proto, "BYTES_PER_ELEMENT", JsValue::from_int32(bpe as i32));
    vm.set_property(proto, "constructor", ctor);
    vm.set_property(ctor, "prototype", proto);

    vm.globals.insert(name.to_string(), ctor);
    if vm.global_object.is_object() {
        vm.set_property(vm.global_object, name, ctor);
    }
}

fn register_parallel_and_typed_arrays(vm: &mut Vm) {
    let sab_ctor = vm.alloc_native_function("SharedArrayBuffer", 1, |vm, _this, args| {
        let byte_len = args.first().and_then(|v| v.to_int32()).unwrap_or(0);
        if !(0..=64 * 1024 * 1024).contains(&byte_len) {
            return Err(VmError::RangeError("Invalid SharedArrayBuffer length".into()));
        }
        let sab = doms_parallel::SharedArrayBuffer::try_new(byte_len as usize)
            .map_err(|e| VmError::RuntimeError(e.into()))?;
        Ok(vm.alloc_shared_array_buffer(Arc::new(sab)))
    });
    let sab_proto = vm.alloc_object();
    vm.set_property(sab_ctor, "prototype", sab_proto);
    vm.globals.insert("SharedArrayBuffer".to_string(), sab_ctor);

    let ab_ctor = vm.alloc_native_function("ArrayBuffer", 1, |vm, _this, args| {
        let byte_len = args.first().and_then(|v| v.to_int32()).unwrap_or(0);
        if !(0..=64 * 1024 * 1024).contains(&byte_len) {
            return Err(VmError::RangeError("Invalid ArrayBuffer length".into()));
        }
        let sab = doms_parallel::SharedArrayBuffer::try_new(byte_len as usize)
            .map_err(|e| VmError::RuntimeError(e.into()))?;
        Ok(vm.alloc_array_buffer(Arc::new(sab)))
    });
    let ab_is_view = vm.alloc_native_function("isView", 1, |_vm, _this, args| {
        if let Some(arg) = args.first() {
            if let Some(ptr) = arg.as_object_ptr() {
                let obj = unsafe { &*(ptr as *mut JsObject) };
                if matches!(obj.kind, ObjectKind::TypedArray(_) | ObjectKind::DataView { .. }) {
                    return Ok(JsValue::from_boolean(true));
                }
            }
        }
        Ok(JsValue::from_boolean(false))
    });
    vm.set_property(ab_ctor, "isView", ab_is_view);

    let ab_proto = vm.alloc_object();
    let ab_slice = vm.alloc_native_function("slice", 2, |vm, this, args| {
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::ArrayBuffer(buf) = &obj.kind {
                let len = buf.byte_length() as i32;
                let start = args.first().and_then(|v| v.to_int32()).unwrap_or(0);
                let end = args.get(1).and_then(|v| v.to_int32()).unwrap_or(len);
                let s = if start < 0 { (len + start).max(0) } else { start.min(len) } as usize;
                let e = if end < 0 { (len + end).max(0) } else { end.min(len) } as usize;
                let count = if e > s { e - s } else { 0 };
                let new_buf = doms_parallel::SharedArrayBuffer::try_new(count)
                    .map_err(|e| VmError::RuntimeError(e.into()))?;
                for i in 0..count {
                    if let Some(b) = buf.get(s + i) {
                        new_buf.set(i, b);
                    }
                }
                return Ok(vm.alloc_array_buffer(Arc::new(new_buf)));
            }
        }
        Err(VmError::TypeError("ArrayBuffer.prototype.slice called on non-ArrayBuffer".into()))
    });
    vm.set_property(ab_proto, "slice", ab_slice);
    vm.set_property(ab_ctor, "prototype", ab_proto);
    vm.globals.insert("ArrayBuffer".to_string(), ab_ctor);

    // DataView
    let dv_ctor = vm.alloc_native_function("DataView", 1, |vm, _this, args| {
        let buffer_val = args.first().copied().unwrap_or(JsValue::undefined());
        let (sab, buf_len) = if let Some(ptr) = buffer_val.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            match &obj.kind {
                ObjectKind::ArrayBuffer(b) | ObjectKind::SharedArrayBuffer(b) => (Arc::clone(b), b.byte_length()),
                _ => return Err(VmError::TypeError("DataView requires an ArrayBuffer or SharedArrayBuffer".into())),
            }
        } else {
            return Err(VmError::TypeError("DataView requires an ArrayBuffer or SharedArrayBuffer".into()));
        };

        let byte_offset = args.get(1).and_then(|v| v.to_int32()).unwrap_or(0).max(0) as usize;
        if byte_offset > buf_len {
            return Err(VmError::RangeError("Start offset is outside the bounds of the buffer".into()));
        }
        let byte_length = if let Some(len_arg) = args.get(2) {
            let l = len_arg.to_int32().unwrap_or(0).max(0) as usize;
            if byte_offset + l > buf_len {
                return Err(VmError::RangeError("Invalid DataView length".into()));
            }
            l
        } else {
            buf_len - byte_offset
        };

        Ok(vm.alloc_data_view(sab, byte_offset, byte_length))
    });

    let dv_proto = vm.alloc_object();

    fn dv_bounds(this: JsValue, args: &[JsValue], size: usize, is_setter: bool) -> Result<(Arc<doms_parallel::SharedArrayBuffer>, usize, bool), VmError> {
        let (buf, off, len) = if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::DataView { buffer, byte_offset, byte_length } = &obj.kind {
                (Arc::clone(buffer), *byte_offset, *byte_length)
            } else {
                return Err(VmError::TypeError("DataView method called on non-DataView".into()));
            }
        } else {
            return Err(VmError::TypeError("DataView method called on non-DataView".into()));
        };
        let byte_idx = args.first().and_then(|v| v.to_int32()).unwrap_or(0);
        if byte_idx < 0 || (byte_idx as usize) + size > len {
            return Err(VmError::RangeError("Offset is outside the bounds of the DataView".into()));
        }
        let le_idx = if is_setter { 2 } else { 1 };
        let le = args.get(le_idx).map(|v| v.is_truthy()).unwrap_or(false);
        Ok((buf, off + (byte_idx as usize), le))
    }

    let dv_get_i8 = vm.alloc_native_function("getInt8", 1, |_vm, this, args| {
        let (buf, pos, _) = dv_bounds(this, args, 1, false)?;
        let b = buf.get(pos).unwrap_or(0) as i8;
        Ok(JsValue::from_int32(b as i32))
    });
    let dv_get_u8 = vm.alloc_native_function("getUint8", 1, |_vm, this, args| {
        let (buf, pos, _) = dv_bounds(this, args, 1, false)?;
        let b = buf.get(pos).unwrap_or(0);
        Ok(JsValue::from_int32(b as i32))
    });
    let dv_set_i8 = vm.alloc_native_function("setInt8", 2, |vm, this, args| {
        let (buf, pos, _) = dv_bounds(this, args, 1, true)?;
        let val = args.get(1).and_then(|v| vm.to_number_value(*v).ok()).unwrap_or(0.0) as i32 as i8 as u8;
        buf.set(pos, val);
        Ok(JsValue::undefined())
    });
    let dv_set_u8 = vm.alloc_native_function("setUint8", 2, |vm, this, args| {
        let (buf, pos, _) = dv_bounds(this, args, 1, true)?;
        let val = args.get(1).and_then(|v| vm.to_number_value(*v).ok()).unwrap_or(0.0) as i32 as u8;
        buf.set(pos, val);
        Ok(JsValue::undefined())
    });

    let dv_get_i16 = vm.alloc_native_function("getInt16", 1, |_vm, this, args| {
        let (buf, pos, le) = dv_bounds(this, args, 2, false)?;
        let b0 = buf.get(pos).unwrap_or(0);
        let b1 = buf.get(pos + 1).unwrap_or(0);
        let v = if le { i16::from_le_bytes([b0, b1]) } else { i16::from_be_bytes([b0, b1]) };
        Ok(JsValue::from_int32(v as i32))
    });
    let dv_get_u16 = vm.alloc_native_function("getUint16", 1, |_vm, this, args| {
        let (buf, pos, le) = dv_bounds(this, args, 2, false)?;
        let b0 = buf.get(pos).unwrap_or(0);
        let b1 = buf.get(pos + 1).unwrap_or(0);
        let v = if le { u16::from_le_bytes([b0, b1]) } else { u16::from_be_bytes([b0, b1]) };
        Ok(JsValue::from_int32(v as i32))
    });
    let dv_set_i16 = vm.alloc_native_function("setInt16", 2, |vm, this, args| {
        let (buf, pos, le) = dv_bounds(this, args, 2, true)?;
        let val = args.get(1).and_then(|v| vm.to_number_value(*v).ok()).unwrap_or(0.0) as i32 as i16;
        let bytes = if le { val.to_le_bytes() } else { val.to_be_bytes() };
        buf.set(pos, bytes[0]);
        buf.set(pos + 1, bytes[1]);
        Ok(JsValue::undefined())
    });
    let dv_set_u16 = vm.alloc_native_function("setUint16", 2, |vm, this, args| {
        let (buf, pos, le) = dv_bounds(this, args, 2, true)?;
        let val = args.get(1).and_then(|v| vm.to_number_value(*v).ok()).unwrap_or(0.0) as u32 as u16;
        let bytes = if le { val.to_le_bytes() } else { val.to_be_bytes() };
        buf.set(pos, bytes[0]);
        buf.set(pos + 1, bytes[1]);
        Ok(JsValue::undefined())
    });

    let dv_get_i32 = vm.alloc_native_function("getInt32", 1, |_vm, this, args| {
        let (buf, pos, le) = dv_bounds(this, args, 4, false)?;
        let mut b = [0u8; 4];
        for i in 0..4 { b[i] = buf.get(pos + i).unwrap_or(0); }
        let v = if le { i32::from_le_bytes(b) } else { i32::from_be_bytes(b) };
        Ok(JsValue::from_int32(v))
    });
    let dv_get_u32 = vm.alloc_native_function("getUint32", 1, |_vm, this, args| {
        let (buf, pos, le) = dv_bounds(this, args, 4, false)?;
        let mut b = [0u8; 4];
        for i in 0..4 { b[i] = buf.get(pos + i).unwrap_or(0); }
        let v = if le { u32::from_le_bytes(b) } else { u32::from_be_bytes(b) };
        Ok(JsValue::from_double(v as f64))
    });
    let dv_set_i32 = vm.alloc_native_function("setInt32", 2, |vm, this, args| {
        let (buf, pos, le) = dv_bounds(this, args, 4, true)?;
        let val = args.get(1).and_then(|v| vm.to_number_value(*v).ok()).unwrap_or(0.0) as i32;
        let bytes = if le { val.to_le_bytes() } else { val.to_be_bytes() };
        for (i, &b) in bytes.iter().enumerate() { buf.set(pos + i, b); }
        Ok(JsValue::undefined())
    });
    let dv_set_u32 = vm.alloc_native_function("setUint32", 2, |vm, this, args| {
        let (buf, pos, le) = dv_bounds(this, args, 4, true)?;
        let val = args.get(1).and_then(|v| vm.to_number_value(*v).ok()).unwrap_or(0.0) as u32;
        let bytes = if le { val.to_le_bytes() } else { val.to_be_bytes() };
        for (i, &b) in bytes.iter().enumerate() { buf.set(pos + i, b); }
        Ok(JsValue::undefined())
    });

    let dv_get_f32 = vm.alloc_native_function("getFloat32", 1, |_vm, this, args| {
        let (buf, pos, le) = dv_bounds(this, args, 4, false)?;
        let mut b = [0u8; 4];
        for i in 0..4 { b[i] = buf.get(pos + i).unwrap_or(0); }
        let v = if le { f32::from_le_bytes(b) } else { f32::from_be_bytes(b) };
        Ok(JsValue::from_double(v as f64))
    });
    let dv_set_f32 = vm.alloc_native_function("setFloat32", 2, |vm, this, args| {
        let (buf, pos, le) = dv_bounds(this, args, 4, true)?;
        let val = args.get(1).and_then(|v| vm.to_number_value(*v).ok()).unwrap_or(0.0) as f32;
        let bytes = if le { val.to_le_bytes() } else { val.to_be_bytes() };
        for (i, &b) in bytes.iter().enumerate() { buf.set(pos + i, b); }
        Ok(JsValue::undefined())
    });

    let dv_get_f64 = vm.alloc_native_function("getFloat64", 1, |_vm, this, args| {
        let (buf, pos, le) = dv_bounds(this, args, 8, false)?;
        let mut b = [0u8; 8];
        for i in 0..8 { b[i] = buf.get(pos + i).unwrap_or(0); }
        let v = if le { f64::from_le_bytes(b) } else { f64::from_be_bytes(b) };
        Ok(JsValue::from_double(v))
    });
    let dv_set_f64 = vm.alloc_native_function("setFloat64", 2, |vm, this, args| {
        let (buf, pos, le) = dv_bounds(this, args, 8, true)?;
        let val = args.get(1).and_then(|v| vm.to_number_value(*v).ok()).unwrap_or(0.0);
        let bytes = if le { val.to_le_bytes() } else { val.to_be_bytes() };
        for (i, &b) in bytes.iter().enumerate() { buf.set(pos + i, b); }
        Ok(JsValue::undefined())
    });

    let dv_get_bi64 = vm.alloc_native_function("getBigInt64", 1, |_vm, this, args| {
        let (buf, pos, le) = dv_bounds(this, args, 8, false)?;
        let mut b = [0u8; 8];
        for i in 0..8 { b[i] = buf.get(pos + i).unwrap_or(0); }
        let v = if le { i64::from_le_bytes(b) } else { i64::from_be_bytes(b) };
        Ok(JsValue::from_double(v as f64))
    });
    let dv_set_bi64 = vm.alloc_native_function("setBigInt64", 2, |vm, this, args| {
        let (buf, pos, le) = dv_bounds(this, args, 8, true)?;
        let val = args.get(1).and_then(|v| vm.to_number_value(*v).ok()).unwrap_or(0.0) as i64;
        let bytes = if le { val.to_le_bytes() } else { val.to_be_bytes() };
        for (i, &b) in bytes.iter().enumerate() { buf.set(pos + i, b); }
        Ok(JsValue::undefined())
    });
    let dv_get_bu64 = vm.alloc_native_function("getBigUint64", 1, |_vm, this, args| {
        let (buf, pos, le) = dv_bounds(this, args, 8, false)?;
        let mut b = [0u8; 8];
        for i in 0..8 { b[i] = buf.get(pos + i).unwrap_or(0); }
        let v = if le { u64::from_le_bytes(b) } else { u64::from_be_bytes(b) };
        Ok(JsValue::from_double(v as f64))
    });
    let dv_set_bu64 = vm.alloc_native_function("setBigUint64", 2, |vm, this, args| {
        let (buf, pos, le) = dv_bounds(this, args, 8, true)?;
        let val = args.get(1).and_then(|v| vm.to_number_value(*v).ok()).unwrap_or(0.0) as u64;
        let bytes = if le { val.to_le_bytes() } else { val.to_be_bytes() };
        for (i, &b) in bytes.iter().enumerate() { buf.set(pos + i, b); }
        Ok(JsValue::undefined())
    });

    vm.set_property(dv_proto, "getInt8", dv_get_i8);
    vm.set_property(dv_proto, "getUint8", dv_get_u8);
    vm.set_property(dv_proto, "setInt8", dv_set_i8);
    vm.set_property(dv_proto, "setUint8", dv_set_u8);
    vm.set_property(dv_proto, "getInt16", dv_get_i16);
    vm.set_property(dv_proto, "getUint16", dv_get_u16);
    vm.set_property(dv_proto, "setInt16", dv_set_i16);
    vm.set_property(dv_proto, "setUint16", dv_set_u16);
    vm.set_property(dv_proto, "getInt32", dv_get_i32);
    vm.set_property(dv_proto, "getUint32", dv_get_u32);
    vm.set_property(dv_proto, "setInt32", dv_set_i32);
    vm.set_property(dv_proto, "setUint32", dv_set_u32);
    vm.set_property(dv_proto, "getFloat32", dv_get_f32);
    vm.set_property(dv_proto, "setFloat32", dv_set_f32);
    vm.set_property(dv_proto, "getFloat64", dv_get_f64);
    vm.set_property(dv_proto, "setFloat64", dv_set_f64);
    vm.set_property(dv_proto, "getBigInt64", dv_get_bi64);
    vm.set_property(dv_proto, "setBigInt64", dv_set_bi64);
    vm.set_property(dv_proto, "getBigUint64", dv_get_bu64);
    vm.set_property(dv_proto, "setBigUint64", dv_set_bu64);

    vm.set_property(dv_ctor, "prototype", dv_proto);
    vm.globals.insert("DataView".to_string(), dv_ctor);

    let kinds = [
        TypedArrayKind::Int8,
        TypedArrayKind::Uint8,
        TypedArrayKind::Uint8Clamped,
        TypedArrayKind::Int16,
        TypedArrayKind::Uint16,
        TypedArrayKind::Int32,
        TypedArrayKind::Uint32,
        TypedArrayKind::Float32,
        TypedArrayKind::Float64,
        TypedArrayKind::BigInt64,
        TypedArrayKind::BigUint64,
    ];
    for kind in kinds {
        register_single_typed_array(vm, kind);
    }

    let atomics_obj = vm.alloc_object();

    let atomics_load = vm.alloc_native_function("load", 2, |_vm, _this, args| {
        let ta_val = args.first().copied().unwrap_or(JsValue::undefined());
        let idx = args.get(1).and_then(|v| v.to_int32()).unwrap_or(0).max(0) as usize;
        if let Some(ptr) = ta_val.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::TypedArray(ta) = &obj.kind {
                if let Some(val) = Vm::read_typed_array_element(ta, idx) {
                    return Ok(val);
                }
            }
        }
        Err(VmError::TypeError("Atomics.load requires a TypedArray".into()))
    });

    let atomics_store = vm.alloc_native_function("store", 3, |_vm, _this, args| {
        let ta_val = args.first().copied().unwrap_or(JsValue::undefined());
        let idx = args.get(1).and_then(|v| v.to_int32()).unwrap_or(0).max(0) as usize;
        let val = args.get(2).copied().unwrap_or(JsValue::undefined());
        if let Some(ptr) = ta_val.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::TypedArray(ta) = &mut obj.kind {
                if Vm::write_typed_array_element(ta, idx, val) {
                    return Ok(val);
                }
            }
        }
        Err(VmError::TypeError("Atomics.store requires a TypedArray".into()))
    });

    let atomics_add = vm.alloc_native_function("add", 3, |_vm, _this, args| {
        let ta_val = args.first().copied().unwrap_or(JsValue::undefined());
        let idx = args.get(1).and_then(|v| v.to_int32()).unwrap_or(0).max(0) as usize;
        let value = args.get(2).and_then(|v| v.to_number()).unwrap_or(0.0) as i32;
        if let Some(ptr) = ta_val.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::TypedArray(ta) = &mut obj.kind {
                if let Some(old) = Vm::read_typed_array_element(ta, idx) {
                    let old_n = old.to_number().unwrap_or(0.0) as i32;
                    let new_val = JsValue::from_int32(old_n + value);
                    Vm::write_typed_array_element(ta, idx, new_val);
                    return Ok(old);
                }
            }
        }
        Err(VmError::TypeError("Atomics.add requires a TypedArray".into()))
    });

    let atomics_sub = vm.alloc_native_function("sub", 3, |_vm, _this, args| {
        let ta_val = args.first().copied().unwrap_or(JsValue::undefined());
        let idx = args.get(1).and_then(|v| v.to_int32()).unwrap_or(0).max(0) as usize;
        let value = args.get(2).and_then(|v| v.to_number()).unwrap_or(0.0) as i32;
        if let Some(ptr) = ta_val.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::TypedArray(ta) = &mut obj.kind {
                if let Some(old) = Vm::read_typed_array_element(ta, idx) {
                    let old_n = old.to_number().unwrap_or(0.0) as i32;
                    let new_val = JsValue::from_int32(old_n - value);
                    Vm::write_typed_array_element(ta, idx, new_val);
                    return Ok(old);
                }
            }
        }
        Err(VmError::TypeError("Atomics.sub requires a TypedArray".into()))
    });

    let atomics_and = vm.alloc_native_function("and", 3, |_vm, _this, args| {
        let ta_val = args.first().copied().unwrap_or(JsValue::undefined());
        let idx = args.get(1).and_then(|v| v.to_int32()).unwrap_or(0).max(0) as usize;
        let value = args.get(2).and_then(|v| v.to_number()).unwrap_or(0.0) as i32;
        if let Some(ptr) = ta_val.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::TypedArray(ta) = &mut obj.kind {
                if let Some(old) = Vm::read_typed_array_element(ta, idx) {
                    let old_n = old.to_number().unwrap_or(0.0) as i32;
                    let new_val = JsValue::from_int32(old_n & value);
                    Vm::write_typed_array_element(ta, idx, new_val);
                    return Ok(old);
                }
            }
        }
        Err(VmError::TypeError("Atomics.and requires a TypedArray".into()))
    });

    let atomics_or = vm.alloc_native_function("or", 3, |_vm, _this, args| {
        let ta_val = args.first().copied().unwrap_or(JsValue::undefined());
        let idx = args.get(1).and_then(|v| v.to_int32()).unwrap_or(0).max(0) as usize;
        let value = args.get(2).and_then(|v| v.to_number()).unwrap_or(0.0) as i32;
        if let Some(ptr) = ta_val.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::TypedArray(ta) = &mut obj.kind {
                if let Some(old) = Vm::read_typed_array_element(ta, idx) {
                    let old_n = old.to_number().unwrap_or(0.0) as i32;
                    let new_val = JsValue::from_int32(old_n | value);
                    Vm::write_typed_array_element(ta, idx, new_val);
                    return Ok(old);
                }
            }
        }
        Err(VmError::TypeError("Atomics.or requires a TypedArray".into()))
    });

    let atomics_xor = vm.alloc_native_function("xor", 3, |_vm, _this, args| {
        let ta_val = args.first().copied().unwrap_or(JsValue::undefined());
        let idx = args.get(1).and_then(|v| v.to_int32()).unwrap_or(0).max(0) as usize;
        let value = args.get(2).and_then(|v| v.to_number()).unwrap_or(0.0) as i32;
        if let Some(ptr) = ta_val.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::TypedArray(ta) = &mut obj.kind {
                if let Some(old) = Vm::read_typed_array_element(ta, idx) {
                    let old_n = old.to_number().unwrap_or(0.0) as i32;
                    let new_val = JsValue::from_int32(old_n ^ value);
                    Vm::write_typed_array_element(ta, idx, new_val);
                    return Ok(old);
                }
            }
        }
        Err(VmError::TypeError("Atomics.xor requires a TypedArray".into()))
    });

    let atomics_exchange = vm.alloc_native_function("exchange", 3, |_vm, _this, args| {
        let ta_val = args.first().copied().unwrap_or(JsValue::undefined());
        let idx = args.get(1).and_then(|v| v.to_int32()).unwrap_or(0).max(0) as usize;
        let val = args.get(2).copied().unwrap_or(JsValue::undefined());
        if let Some(ptr) = ta_val.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::TypedArray(ta) = &mut obj.kind {
                if let Some(old) = Vm::read_typed_array_element(ta, idx) {
                    Vm::write_typed_array_element(ta, idx, val);
                    return Ok(old);
                }
            }
        }
        Err(VmError::TypeError("Atomics.exchange requires a TypedArray".into()))
    });

    let atomics_cmpxchg = vm.alloc_native_function("compareExchange", 4, |_vm, _this, args| {
        let ta_val = args.first().copied().unwrap_or(JsValue::undefined());
        let idx = args.get(1).and_then(|v| v.to_int32()).unwrap_or(0).max(0) as usize;
        let expected = args.get(2).and_then(|v| v.to_number()).unwrap_or(0.0) as i32;
        let replacement = args.get(3).copied().unwrap_or(JsValue::undefined());
        if let Some(ptr) = ta_val.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::TypedArray(ta) = &mut obj.kind {
                if let Some(old) = Vm::read_typed_array_element(ta, idx) {
                    let old_n = old.to_number().unwrap_or(0.0) as i32;
                    if old_n == expected {
                        Vm::write_typed_array_element(ta, idx, replacement);
                    }
                    return Ok(old);
                }
            }
        }
        Err(VmError::TypeError("Atomics.compareExchange requires a TypedArray".into()))
    });

    let atomics_lockfree = vm.alloc_native_function("isLockFree", 1, |_vm, _this, args| {
        let size = args.first().and_then(|v| v.to_int32()).unwrap_or(0).max(0) as usize;
        Ok(JsValue::from_boolean(doms_parallel::Atomics::is_lock_free(size)))
    });

    let atomics_wait = vm.alloc_native_function("wait", 4, |vm, _this, args| {
        let ta_val = args.first().copied().unwrap_or(JsValue::undefined());
        let idx = args.get(1).and_then(|v| v.to_int32()).unwrap_or(0).max(0) as usize;
        let expected = args.get(2).and_then(|v| v.to_int32()).unwrap_or(0);
        let timeout = match args.get(3) {
            Some(v) if !v.is_undefined() => {
                let ms = v.to_number().unwrap_or(0.0);
                if ms.is_nan() || ms < 0.0 {
                    Some(std::time::Duration::ZERO)
                } else {
                    Some(std::time::Duration::from_millis(ms as u64))
                }
            }
            _ => None,
        };

        if let Some(ptr) = ta_val.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::TypedArray(ta) = &obj.kind {
                if ta.kind != TypedArrayKind::Int32 {
                    return Err(VmError::TypeError("Atomics.wait requires an Int32Array".into()));
                }
                let byte_off = ta.byte_offset + idx * 4;
                let res = doms_parallel::Atomics::wait(&ta.buffer, byte_off, expected, timeout)
                    .map_err(|e| VmError::RuntimeError(e.to_string()))?;
                return Ok(vm.alloc_string(res.to_string()));
            }
        }
        Err(VmError::TypeError("Atomics.wait requires an Int32Array".into()))
    });

    let atomics_notify = vm.alloc_native_function("notify", 3, |_vm, _this, args| {
        let ta_val = args.first().copied().unwrap_or(JsValue::undefined());
        let idx = args.get(1).and_then(|v| v.to_int32()).unwrap_or(0).max(0) as usize;
        let count = match args.get(2) {
            Some(v) if !v.is_undefined() => {
                let n = v.to_number().unwrap_or(f64::INFINITY);
                if n.is_infinite() || n >= (usize::MAX as f64) {
                    usize::MAX
                } else {
                    n.max(0.0) as usize
                }
            }
            _ => usize::MAX,
        };

        if let Some(ptr) = ta_val.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::TypedArray(ta) = &obj.kind {
                let byte_off = ta.byte_offset + idx * ta.kind.bytes_per_element();
                let woken = doms_parallel::Atomics::notify(&ta.buffer, byte_off, count)
                    .map_err(|e| VmError::RuntimeError(e.to_string()))?;
                return Ok(JsValue::from_int32(woken as i32));
            }
        }
        Err(VmError::TypeError("Atomics.notify requires a TypedArray".into()))
    });

    vm.set_property(atomics_obj, "load", atomics_load);
    vm.set_property(atomics_obj, "store", atomics_store);
    vm.set_property(atomics_obj, "add", atomics_add);
    vm.set_property(atomics_obj, "sub", atomics_sub);
    vm.set_property(atomics_obj, "and", atomics_and);
    vm.set_property(atomics_obj, "or", atomics_or);
    vm.set_property(atomics_obj, "xor", atomics_xor);
    vm.set_property(atomics_obj, "exchange", atomics_exchange);
    vm.set_property(atomics_obj, "compareExchange", atomics_cmpxchg);
    vm.set_property(atomics_obj, "isLockFree", atomics_lockfree);
    vm.set_property(atomics_obj, "wait", atomics_wait);
    vm.set_property(atomics_obj, "notify", atomics_notify);
    vm.globals.insert("Atomics".to_string(), atomics_obj);
    if vm.global_object.is_object() {
        vm.set_property(vm.global_object, "Atomics", atomics_obj);
    }

    let worker_ctor = vm.alloc_native_function("Worker", 1, |vm, _this, args| {
        let script = args.first().map(|v| vm.to_string_value(*v)).unwrap_or_default();
        let channel = doms_parallel::MessageChannel::new();
        let child_port = channel.port2;

        std::thread::spawn(move || {
            let mut worker_vm = Vm::new();
            let parent_port = Arc::new(std::sync::Mutex::new(child_port));
            let post_msg = worker_vm.alloc_closure_native_function("postMessage", 1, Arc::new({
                let p = Arc::clone(&parent_port);
                move |vm: &mut Vm, _this: JsValue, args: &[JsValue]| -> Result<JsValue, VmError> {
                    let msg = args.first().copied().unwrap_or_default();
                    let json = vm.to_json_string(msg);
                    let guard = p.lock().unwrap();
                    guard.post_message(json.into_bytes()).map_err(|e| VmError::RuntimeError(e.to_string()))?;
                    Ok(JsValue::undefined())
                }
            }));
            worker_vm.globals.insert("postMessage".to_string(), post_msg);

            let code = if std::path::Path::new(&script).exists() {
                std::fs::read_to_string(&script).unwrap_or(script)
            } else {
                script
            };

            if let Ok(prog) = doms_parser::parse(&code) {
                if let Ok(chunk) = doms_compiler::compile(&prog) {
                    let _ = worker_vm.run(&chunk);
                }
            }
        });

        let port_arc = Arc::new(std::sync::Mutex::new(channel.port1));
        Ok(vm.alloc_worker(port_arc))
    });

    vm.globals.insert("Worker".to_string(), worker_ctor);
}

fn register_phase3_builtins(vm: &mut Vm) {
    // ── Map ──
    let map_proto = vm.alloc_object();

    let map_get = vm.alloc_native_function("get", 1, |_vm, this, args| {
        let key = args.first().copied().unwrap_or(JsValue::undefined());
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::Map(entries) = &obj.kind {
                for (k, v) in entries {
                    if crate::object::js_values_equal(*k, key) {
                        return Ok(*v);
                    }
                }
                return Ok(JsValue::undefined());
            }
        }
        Err(VmError::TypeError("Method Map.prototype.get called on incompatible receiver".into()))
    });

    let map_set = vm.alloc_native_function("set", 2, |_vm, this, args| {
        let key = args.first().copied().unwrap_or(JsValue::undefined());
        let val = args.get(1).copied().unwrap_or(JsValue::undefined());
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::Map(entries) = &mut obj.kind {
                for entry in entries.iter_mut() {
                    if crate::object::js_values_equal(entry.0, key) {
                        entry.1 = val;
                        return Ok(this);
                    }
                }
                entries.push((key, val));
                return Ok(this);
            }
        }
        Err(VmError::TypeError("Method Map.prototype.set called on incompatible receiver".into()))
    });

    let map_has = vm.alloc_native_function("has", 1, |_vm, this, args| {
        let key = args.first().copied().unwrap_or(JsValue::undefined());
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::Map(entries) = &obj.kind {
                for (k, _) in entries {
                    if crate::object::js_values_equal(*k, key) {
                        return Ok(JsValue::from_boolean(true));
                    }
                }
                return Ok(JsValue::from_boolean(false));
            }
        }
        Err(VmError::TypeError("Method Map.prototype.has called on incompatible receiver".into()))
    });

    let map_delete = vm.alloc_native_function("delete", 1, |_vm, this, args| {
        let key = args.first().copied().unwrap_or(JsValue::undefined());
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::Map(entries) = &mut obj.kind {
                if let Some(idx) = entries.iter().position(|(k, _)| crate::object::js_values_equal(*k, key)) {
                    entries.remove(idx);
                    return Ok(JsValue::from_boolean(true));
                }
                return Ok(JsValue::from_boolean(false));
            }
        }
        Err(VmError::TypeError("Method Map.prototype.delete called on incompatible receiver".into()))
    });

    let map_clear = vm.alloc_native_function("clear", 0, |_vm, this, _args| {
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::Map(entries) = &mut obj.kind {
                entries.clear();
                return Ok(JsValue::undefined());
            }
        }
        Err(VmError::TypeError("Method Map.prototype.clear called on incompatible receiver".into()))
    });

    let map_for_each = vm.alloc_native_function("forEach", 1, |vm, this, args| {
        let cb = args.first().copied().unwrap_or(JsValue::undefined());
        if !vm.is_callable(cb) {
            return Err(VmError::TypeError("Callback is not a function".into()));
        }
        let this_arg = args.get(1).copied().unwrap_or(JsValue::undefined());
        let entries = if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::Map(entries) = &obj.kind {
                entries.clone()
            } else {
                return Err(VmError::TypeError("Method Map.prototype.forEach called on incompatible receiver".into()));
            }
        } else {
            return Err(VmError::TypeError("Method Map.prototype.forEach called on incompatible receiver".into()));
        };
        for (k, v) in entries {
            let _ = vm.call(cb, this_arg, &[v, k, this])?;
        }
        Ok(JsValue::undefined())
    });

    let map_entries = vm.alloc_native_function("entries", 0, |vm, this, _args| {
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::Map(entries) = &obj.kind {
                let items: Vec<JsValue> = entries.iter().map(|(k, v)| vm.alloc_array(vec![*k, *v])).collect();
                return Ok(vm.alloc_iterator(items));
            }
        }
        Err(VmError::TypeError("Method Map.prototype.entries called on incompatible receiver".into()))
    });

    let map_keys = vm.alloc_native_function("keys", 0, |vm, this, _args| {
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::Map(entries) = &obj.kind {
                let items: Vec<JsValue> = entries.iter().map(|(k, _)| *k).collect();
                return Ok(vm.alloc_iterator(items));
            }
        }
        Err(VmError::TypeError("Method Map.prototype.keys called on incompatible receiver".into()))
    });

    let map_values = vm.alloc_native_function("values", 0, |vm, this, _args| {
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::Map(entries) = &obj.kind {
                let items: Vec<JsValue> = entries.iter().map(|(_, v)| *v).collect();
                return Ok(vm.alloc_iterator(items));
            }
        }
        Err(VmError::TypeError("Method Map.prototype.values called on incompatible receiver".into()))
    });

    let map_size_getter = vm.alloc_native_function("get size", 0, |_vm, this, _args| {
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::Map(entries) = &obj.kind {
                return Ok(JsValue::from_int32(entries.len() as i32));
            }
        }
        Err(VmError::TypeError("Method get Map.prototype.size called on incompatible receiver".into()))
    });

    vm.set_property(map_proto, "get", map_get);
    vm.non_enumerable_props.insert((map_proto, "get".to_string()));
    vm.set_property(map_proto, "set", map_set);
    vm.non_enumerable_props.insert((map_proto, "set".to_string()));
    vm.set_property(map_proto, "has", map_has);
    vm.non_enumerable_props.insert((map_proto, "has".to_string()));
    vm.set_property(map_proto, "delete", map_delete);
    vm.non_enumerable_props.insert((map_proto, "delete".to_string()));
    vm.set_property(map_proto, "clear", map_clear);
    vm.non_enumerable_props.insert((map_proto, "clear".to_string()));
    vm.set_property(map_proto, "forEach", map_for_each);
    vm.non_enumerable_props.insert((map_proto, "forEach".to_string()));
    vm.set_property(map_proto, "entries", map_entries);
    vm.non_enumerable_props.insert((map_proto, "entries".to_string()));
    vm.set_property(map_proto, "keys", map_keys);
    vm.non_enumerable_props.insert((map_proto, "keys".to_string()));
    vm.set_property(map_proto, "values", map_values);
    vm.non_enumerable_props.insert((map_proto, "values".to_string()));
    if let Some(ptr) = map_proto.as_object_ptr() {
        let obj = unsafe { &mut *(ptr as *mut JsObject) };
        obj.define_getter("size".to_string(), map_size_getter);
    }
    vm.non_enumerable_props.insert((map_proto, "size".to_string()));
    if let Some(sym_iter) = vm.symbol_iterator {
        vm.set_symbol_property(map_proto, sym_iter, map_entries);
    }
    if let Some(sym_tag) = vm.symbol_to_string_tag {
        let tag_str = vm.alloc_string("Map".into());
        vm.set_symbol_property(map_proto, sym_tag, tag_str);
    }

    let map_group_by = vm.alloc_native_function("groupBy", 2, |vm, _this, args| {
        let items = args.first().copied().unwrap_or(JsValue::undefined());
        let callback = args.get(1).copied().unwrap_or(JsValue::undefined());
        if !vm.is_callable(callback) {
            return Err(VmError::TypeError("Map.groupBy callback must be callable".into()));
        }
        let map_val = vm.alloc_map();
        let len_val = vm.get_property(items, "length");
        let raw_len = len_val.to_number().unwrap_or(0.0);
        if raw_len > 4_294_967_295.0 || raw_len < 0.0 || raw_len.is_infinite() {
            return Err(VmError::RangeError("Invalid array length".into()));
        }
        let len = raw_len as usize;
        if len > 100_000 {
            return Err(VmError::RangeError("Invalid array length".into()));
        }
        for i in 0..len {
            let item = vm.get_element(items, i);
            let key_val = vm.call(callback, JsValue::undefined(), &[item, JsValue::from_int32(i as i32)])?;
            let has_fn = vm.get_property(map_val, "has");
            let has_res = vm.call(has_fn, map_val, &[key_val]).unwrap_or_else(|_| JsValue::from_boolean(false));
            if has_res.is_truthy() {
                let get_fn = vm.get_property(map_val, "get");
                let arr = vm.call(get_fn, map_val, &[key_val])?;
                if let Some(ptr) = arr.as_object_ptr() {
                    let arr_obj = unsafe { &mut *(ptr as *mut JsObject) };
                    if let ObjectKind::Array(ref mut elems) = arr_obj.kind {
                        elems.push(item);
                    }
                }
            } else {
                let arr = vm.alloc_array(vec![item]);
                let set_fn = vm.get_property(map_val, "set");
                let _ = vm.call(set_fn, map_val, &[key_val, arr]);
            }
        }
        Ok(map_val)
    });

    let map_ctor = vm.alloc_native_function("Map", 0, |vm, this, args| {
        if !vm.is_constructing {
            return Err(VmError::TypeError("Constructor Map requires 'new'".into()));
        }
        let map_val = if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if matches!(obj.kind, ObjectKind::Ordinary) {
                obj.kind = ObjectKind::Map(Vec::new());
                this
            } else {
                vm.alloc_map()
            }
        } else {
            vm.alloc_map()
        };

        if let Some(arg) = args.first() {
            if !arg.is_nullish() {
                let adder = vm.get_property(map_val, "set");
                if !vm.is_callable(adder) {
                    return Err(VmError::TypeError("Map.prototype.set is not callable".into()));
                }
                let items = iterate_to_vec(vm, *arg)?;
                for item in items {
                    if !item.is_object() {
                        return Err(VmError::TypeError("Iterator value is not an entry object".into()));
                    }
                    let k = vm.get_property(item, "0");
                    let v = vm.get_property(item, "1");
                    vm.call(adder, map_val, &[k, v])?;
                }
            }
        }
        Ok(map_val)
    });
    vm.set_property(map_ctor, "groupBy", map_group_by);
    vm.set_property(map_ctor, "prototype", map_proto);
    vm.non_enumerable_props.insert((map_ctor, "prototype".to_string()));
    vm.non_writable_props.insert((map_ctor, "prototype".to_string()));
    vm.non_configurable_props.insert((map_ctor, "prototype".to_string()));
    vm.set_property(map_proto, "constructor", map_ctor);
    vm.non_enumerable_props.insert((map_proto, "constructor".to_string()));
    add_species_getter(vm, map_ctor);
    vm.globals.insert("Map".to_string(), map_ctor);

    // ── Set ──
    let set_proto = vm.alloc_object();

    let set_add = vm.alloc_native_function("add", 1, |_vm, this, args| {
        let val = args.first().copied().unwrap_or(JsValue::undefined());
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::Set(elements) = &mut obj.kind {
                if !elements.iter().any(|el| crate::object::js_values_equal(*el, val)) {
                    elements.push(val);
                }
                return Ok(this);
            }
        }
        Err(VmError::TypeError("Method Set.prototype.add called on incompatible receiver".into()))
    });

    let set_has = vm.alloc_native_function("has", 1, |_vm, this, args| {
        let val = args.first().copied().unwrap_or(JsValue::undefined());
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::Set(elements) = &obj.kind {
                return Ok(JsValue::from_boolean(elements.iter().any(|el| crate::object::js_values_equal(*el, val))));
            }
        }
        Err(VmError::TypeError("Method Set.prototype.has called on incompatible receiver".into()))
    });

    let set_delete = vm.alloc_native_function("delete", 1, |_vm, this, args| {
        let val = args.first().copied().unwrap_or(JsValue::undefined());
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::Set(elements) = &mut obj.kind {
                if let Some(idx) = elements.iter().position(|el| crate::object::js_values_equal(*el, val)) {
                    elements.remove(idx);
                    return Ok(JsValue::from_boolean(true));
                }
                return Ok(JsValue::from_boolean(false));
            }
        }
        Err(VmError::TypeError("Method Set.prototype.delete called on incompatible receiver".into()))
    });

    let set_clear = vm.alloc_native_function("clear", 0, |_vm, this, _args| {
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::Set(elements) = &mut obj.kind {
                elements.clear();
                return Ok(JsValue::undefined());
            }
        }
        Err(VmError::TypeError("Method Set.prototype.clear called on incompatible receiver".into()))
    });

    let set_for_each = vm.alloc_native_function("forEach", 1, |vm, this, args| {
        let cb = args.first().copied().unwrap_or(JsValue::undefined());
        if !vm.is_callable(cb) {
            return Err(VmError::TypeError("Callback is not a function".into()));
        }
        let this_arg = args.get(1).copied().unwrap_or(JsValue::undefined());
        let elements = if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::Set(elements) = &obj.kind {
                elements.clone()
            } else {
                return Err(VmError::TypeError("Method Set.prototype.forEach called on incompatible receiver".into()));
            }
        } else {
            return Err(VmError::TypeError("Method Set.prototype.forEach called on incompatible receiver".into()));
        };
        for el in elements {
            let _ = vm.call(cb, this_arg, &[el, el, this])?;
        }
        Ok(JsValue::undefined())
    });

    let set_values = vm.alloc_native_function("values", 0, |vm, this, _args| {
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::Set(elements) = &obj.kind {
                return Ok(vm.alloc_iterator(elements.clone()));
            }
        }
        Err(VmError::TypeError("Method Set.prototype.values called on incompatible receiver".into()))
    });

    let set_entries = vm.alloc_native_function("entries", 0, |vm, this, _args| {
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::Set(elements) = &obj.kind {
                let items: Vec<JsValue> = elements.iter().map(|el| vm.alloc_array(vec![*el, *el])).collect();
                return Ok(vm.alloc_iterator(items));
            }
        }
        Err(VmError::TypeError("Method Set.prototype.entries called on incompatible receiver".into()))
    });

    let set_size_getter = vm.alloc_native_function("get size", 0, |_vm, this, _args| {
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::Set(elements) = &obj.kind {
                return Ok(JsValue::from_int32(elements.len() as i32));
            }
        }
        Err(VmError::TypeError("Method get Set.prototype.size called on incompatible receiver".into()))
    });

    vm.set_property(set_proto, "add", set_add);
    vm.non_enumerable_props.insert((set_proto, "add".to_string()));
    vm.set_property(set_proto, "has", set_has);
    vm.non_enumerable_props.insert((set_proto, "has".to_string()));
    vm.set_property(set_proto, "delete", set_delete);
    vm.non_enumerable_props.insert((set_proto, "delete".to_string()));
    vm.set_property(set_proto, "clear", set_clear);
    vm.non_enumerable_props.insert((set_proto, "clear".to_string()));
    vm.set_property(set_proto, "forEach", set_for_each);
    vm.non_enumerable_props.insert((set_proto, "forEach".to_string()));
    vm.set_property(set_proto, "values", set_values);
    vm.non_enumerable_props.insert((set_proto, "values".to_string()));
    vm.set_property(set_proto, "keys", set_values);
    vm.non_enumerable_props.insert((set_proto, "keys".to_string()));
    vm.set_property(set_proto, "entries", set_entries);
    vm.non_enumerable_props.insert((set_proto, "entries".to_string()));
    if let Some(ptr) = set_proto.as_object_ptr() {
        let obj = unsafe { &mut *(ptr as *mut JsObject) };
        obj.define_getter("size".to_string(), set_size_getter);
    }
    vm.non_enumerable_props.insert((set_proto, "size".to_string()));
    if let Some(sym_iter) = vm.symbol_iterator {
        vm.set_symbol_property(set_proto, sym_iter, set_values);
    }
    if let Some(sym_tag) = vm.symbol_to_string_tag {
        let tag_str = vm.alloc_string("Set".into());
        vm.set_symbol_property(set_proto, sym_tag, tag_str);
    }

    let set_ctor = vm.alloc_native_function("Set", 0, |vm, this, args| {
        if !vm.is_constructing {
            return Err(VmError::TypeError("Constructor Set requires 'new'".into()));
        }
        let set_val = if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if matches!(obj.kind, ObjectKind::Ordinary) {
                obj.kind = ObjectKind::Set(Vec::new());
                this
            } else {
                vm.alloc_set()
            }
        } else {
            vm.alloc_set()
        };

        if let Some(arg) = args.first() {
            if !arg.is_nullish() {
                let adder = vm.get_property(set_val, "add");
                if !vm.is_callable(adder) {
                    return Err(VmError::TypeError("Set.prototype.add is not callable".into()));
                }
                let items = iterate_to_vec(vm, *arg)?;
                for el in items {
                    vm.call(adder, set_val, &[el])?;
                }
            }
        }
        Ok(set_val)
    });
    vm.set_property(set_ctor, "prototype", set_proto);
    vm.non_enumerable_props.insert((set_ctor, "prototype".to_string()));
    vm.non_writable_props.insert((set_ctor, "prototype".to_string()));
    vm.non_configurable_props.insert((set_ctor, "prototype".to_string()));
    vm.set_property(set_proto, "constructor", set_ctor);
    vm.non_enumerable_props.insert((set_proto, "constructor".to_string()));
    add_species_getter(vm, set_ctor);
    vm.globals.insert("Set".to_string(), set_ctor);

    // ── WeakRef & FinalizationRegistry ──
    let weakref_ctor = vm.alloc_native_function("WeakRef", 1, |vm, _this, args| {
        let target = args.first().copied().unwrap_or(JsValue::undefined());
        Ok(vm.alloc_weak_ref(target))
    });
    vm.globals.insert("WeakRef".to_string(), weakref_ctor);

    let fr_ctor = vm.alloc_native_function("FinalizationRegistry", 1, |vm, _this, args| {
        let cb = args.first().copied().unwrap_or(JsValue::undefined());
        Ok(vm.alloc_finalization_registry(cb))
    });
    vm.globals.insert("FinalizationRegistry".to_string(), fr_ctor);

    // ── WeakMap ──
    let wm_proto = vm.alloc_object();

    let wm_get = vm.alloc_native_function("get", 1, |_vm, this, args| {
        let key = args.first().copied().unwrap_or(JsValue::undefined());
        if !key.is_object() {
            return Ok(JsValue::undefined());
        }
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::WeakMap(entries) = &obj.kind {
                for (k, v) in entries {
                    if *k == key {
                        return Ok(*v);
                    }
                }
                return Ok(JsValue::undefined());
            }
        }
        Err(VmError::TypeError("Method WeakMap.prototype.get called on incompatible receiver".into()))
    });

    let wm_set = vm.alloc_native_function("set", 2, |_vm, this, args| {
        let key = args.first().copied().unwrap_or(JsValue::undefined());
        if !key.is_object() {
            return Err(VmError::TypeError("Invalid value used as weak map key".into()));
        }
        let val = args.get(1).copied().unwrap_or(JsValue::undefined());
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::WeakMap(entries) = &mut obj.kind {
                for entry in entries.iter_mut() {
                    if entry.0 == key {
                        entry.1 = val;
                        return Ok(this);
                    }
                }
                entries.push((key, val));
                return Ok(this);
            }
        }
        Err(VmError::TypeError("Method WeakMap.prototype.set called on incompatible receiver".into()))
    });

    let wm_has = vm.alloc_native_function("has", 1, |_vm, this, args| {
        let key = args.first().copied().unwrap_or(JsValue::undefined());
        if !key.is_object() {
            return Ok(JsValue::from_boolean(false));
        }
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::WeakMap(entries) = &obj.kind {
                for (k, _) in entries {
                    if *k == key {
                        return Ok(JsValue::from_boolean(true));
                    }
                }
                return Ok(JsValue::from_boolean(false));
            }
        }
        Err(VmError::TypeError("Method WeakMap.prototype.has called on incompatible receiver".into()))
    });

    let wm_delete = vm.alloc_native_function("delete", 1, |_vm, this, args| {
        let key = args.first().copied().unwrap_or(JsValue::undefined());
        if !key.is_object() {
            return Ok(JsValue::from_boolean(false));
        }
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::WeakMap(entries) = &mut obj.kind {
                if let Some(idx) = entries.iter().position(|(k, _)| *k == key) {
                    entries.remove(idx);
                    return Ok(JsValue::from_boolean(true));
                }
                return Ok(JsValue::from_boolean(false));
            }
        }
        Err(VmError::TypeError("Method WeakMap.prototype.delete called on incompatible receiver".into()))
    });

    vm.set_property(wm_proto, "get", wm_get);
    vm.non_enumerable_props.insert((wm_proto, "get".to_string()));
    vm.set_property(wm_proto, "set", wm_set);
    vm.non_enumerable_props.insert((wm_proto, "set".to_string()));
    vm.set_property(wm_proto, "has", wm_has);
    vm.non_enumerable_props.insert((wm_proto, "has".to_string()));
    vm.set_property(wm_proto, "delete", wm_delete);
    vm.non_enumerable_props.insert((wm_proto, "delete".to_string()));
    if let Some(sym_tag) = vm.symbol_to_string_tag {
        let tag_str = vm.alloc_string("WeakMap".into());
        vm.set_symbol_property(wm_proto, sym_tag, tag_str);
    }

    let weakmap_ctor = vm.alloc_native_function("WeakMap", 0, |vm, this, args| {
        if !vm.is_constructing {
            return Err(VmError::TypeError("Constructor WeakMap requires 'new'".into()));
        }
        let wm_val = if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if matches!(obj.kind, ObjectKind::Ordinary) {
                obj.kind = ObjectKind::WeakMap(Vec::new());
                this
            } else {
                vm.alloc_weak_map()
            }
        } else {
            vm.alloc_weak_map()
        };

        if let Some(arg) = args.first() {
            if !arg.is_nullish() {
                let adder = vm.get_property(wm_val, "set");
                if !vm.is_callable(adder) {
                    return Err(VmError::TypeError("WeakMap.prototype.set is not callable".into()));
                }
                let items = iterate_to_vec(vm, *arg)?;
                for item in items {
                    if !item.is_object() {
                        return Err(VmError::TypeError("Iterator value is not an entry object".into()));
                    }
                    let k = vm.get_property(item, "0");
                    let v = vm.get_property(item, "1");
                    vm.call(adder, wm_val, &[k, v])?;
                }
            }
        }
        Ok(wm_val)
    });
    vm.set_property(weakmap_ctor, "prototype", wm_proto);
    vm.non_enumerable_props.insert((weakmap_ctor, "prototype".to_string()));
    vm.non_writable_props.insert((weakmap_ctor, "prototype".to_string()));
    vm.non_configurable_props.insert((weakmap_ctor, "prototype".to_string()));
    vm.set_property(wm_proto, "constructor", weakmap_ctor);
    vm.non_enumerable_props.insert((wm_proto, "constructor".to_string()));
    vm.globals.insert("WeakMap".to_string(), weakmap_ctor);

    // ── WeakSet ──
    let ws_proto = vm.alloc_object();

    let ws_add = vm.alloc_native_function("add", 1, |_vm, this, args| {
        let val = args.first().copied().unwrap_or(JsValue::undefined());
        if !val.is_object() {
            return Err(VmError::TypeError("Invalid value used in weak set".into()));
        }
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::WeakSet(elements) = &mut obj.kind {
                if !elements.contains(&val) {
                    elements.push(val);
                }
                return Ok(this);
            }
        }
        Err(VmError::TypeError("Method WeakSet.prototype.add called on incompatible receiver".into()))
    });

    let ws_has = vm.alloc_native_function("has", 1, |_vm, this, args| {
        let val = args.first().copied().unwrap_or(JsValue::undefined());
        if !val.is_object() {
            return Ok(JsValue::from_boolean(false));
        }
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::WeakSet(elements) = &obj.kind {
                return Ok(JsValue::from_boolean(elements.contains(&val)));
            }
        }
        Err(VmError::TypeError("Method WeakSet.prototype.has called on incompatible receiver".into()))
    });

    let ws_delete = vm.alloc_native_function("delete", 1, |_vm, this, args| {
        let val = args.first().copied().unwrap_or(JsValue::undefined());
        if !val.is_object() {
            return Ok(JsValue::from_boolean(false));
        }
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::WeakSet(elements) = &mut obj.kind {
                if let Some(idx) = elements.iter().position(|el| *el == val) {
                    elements.remove(idx);
                    return Ok(JsValue::from_boolean(true));
                }
                return Ok(JsValue::from_boolean(false));
            }
        }
        Err(VmError::TypeError("Method WeakSet.prototype.delete called on incompatible receiver".into()))
    });

    vm.set_property(ws_proto, "add", ws_add);
    vm.non_enumerable_props.insert((ws_proto, "add".to_string()));
    vm.set_property(ws_proto, "has", ws_has);
    vm.non_enumerable_props.insert((ws_proto, "has".to_string()));
    vm.set_property(ws_proto, "delete", ws_delete);
    vm.non_enumerable_props.insert((ws_proto, "delete".to_string()));
    if let Some(sym_tag) = vm.symbol_to_string_tag {
        let tag_str = vm.alloc_string("WeakSet".into());
        vm.set_symbol_property(ws_proto, sym_tag, tag_str);
    }

    let weakset_ctor = vm.alloc_native_function("WeakSet", 0, |vm, this, args| {
        if !vm.is_constructing {
            return Err(VmError::TypeError("Constructor WeakSet requires 'new'".into()));
        }
        let ws_val = if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if matches!(obj.kind, ObjectKind::Ordinary) {
                obj.kind = ObjectKind::WeakSet(Vec::new());
                this
            } else {
                vm.alloc_weak_set()
            }
        } else {
            vm.alloc_weak_set()
        };

        if let Some(arg) = args.first() {
            if !arg.is_nullish() {
                let adder = vm.get_property(ws_val, "add");
                if !vm.is_callable(adder) {
                    return Err(VmError::TypeError("WeakSet.prototype.add is not callable".into()));
                }
                let items = iterate_to_vec(vm, *arg)?;
                for el in items {
                    vm.call(adder, ws_val, &[el])?;
                }
            }
        }
        Ok(ws_val)
    });
    vm.set_property(weakset_ctor, "prototype", ws_proto);
    vm.non_enumerable_props.insert((weakset_ctor, "prototype".to_string()));
    vm.non_writable_props.insert((weakset_ctor, "prototype".to_string()));
    vm.non_configurable_props.insert((weakset_ctor, "prototype".to_string()));
    vm.set_property(ws_proto, "constructor", weakset_ctor);
    vm.non_enumerable_props.insert((ws_proto, "constructor".to_string()));
    vm.globals.insert("WeakSet".to_string(), weakset_ctor);

    // ── RegExp ──
    let regexp_proto = vm.alloc_object();
    vm.regexp_proto = Some(regexp_proto);

    let regexp_ctor = vm.alloc_native_function("RegExp", 2, |vm, _this, args| {
        let pattern = args.first().map(|v| vm.to_string_value(*v)).unwrap_or_default();
        let flags = args.get(1).map(|v| vm.to_string_value(*v)).unwrap_or_default();
        Ok(vm.alloc_regexp(pattern, flags))
    });
    vm.set_property(regexp_ctor, "prototype", regexp_proto);
    vm.set_property(regexp_proto, "constructor", regexp_ctor);

    let re_exec_fn = vm.alloc_native_function("exec", 1, |vm, this, args| {
        let text = args.first().map(|v| vm.to_string_value(*v)).unwrap_or_default();
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::RegExp { pattern, flags } = &obj.kind {
                let pattern = pattern.clone();
                let flags = flags.clone();
                let is_global = flags.contains('g');
                let is_sticky = flags.contains('y');

                let last_index_val = vm.get_property(this, "lastIndex");
                let last_index = if is_global || is_sticky {
                    last_index_val.to_int32().unwrap_or(0).max(0) as usize
                } else {
                    0
                };

                let text_u16_len = text.encode_utf16().count();
                if last_index > text_u16_len {
                    if is_global || is_sticky {
                        vm.set_property(this, "lastIndex", JsValue::from_int32(0));
                    }
                    return Ok(JsValue::null());
                }

                let mut byte_offset = text.len();
                let mut curr_u16 = 0;
                for (b_idx, ch) in text.char_indices() {
                    if curr_u16 >= last_index {
                        byte_offset = b_idx;
                        break;
                    }
                    curr_u16 += ch.len_utf16();
                }

                let regex = match regress::Regex::with_flags(&pattern, flags.as_str()) {
                    Ok(r) => r,
                    Err(_) => return Ok(JsValue::null()),
                };

                if let Some(m) = regex.find_from(&text, byte_offset).next() {
                    let match_start_byte = m.range().start;
                    let match_end_byte = m.range().end;

                    if is_sticky && match_start_byte != byte_offset {
                        vm.set_property(this, "lastIndex", JsValue::from_int32(0));
                        return Ok(JsValue::null());
                    }

                    let match_start_u16 = text[..match_start_byte].encode_utf16().count();
                    let match_end_u16 = text[..match_end_byte].encode_utf16().count();

                    if is_global || is_sticky {
                        vm.set_property(this, "lastIndex", JsValue::from_int32(match_end_u16 as i32));
                    }

                    let matched_str = &text[match_start_byte..match_end_byte];
                    let mut elements = vec![vm.alloc_string(matched_str.to_string())];
                    for cap_opt in &m.captures {
                        if let Some(range) = cap_opt {
                            elements.push(vm.alloc_string(text[range.clone()].to_string()));
                        } else {
                            elements.push(JsValue::undefined());
                        }
                    }

                    let arr_val = vm.alloc_array(elements);
                    vm.set_property(arr_val, "index", JsValue::from_int32(match_start_u16 as i32));
                    let text_val = vm.alloc_string(text.clone());
                    vm.set_property(arr_val, "input", text_val);

                    let named: Vec<_> = m.named_groups().collect();
                    if !named.is_empty() {
                        let groups_obj = vm.alloc_object();
                        for (name, range_opt) in named {
                            let val = if let Some(range) = range_opt {
                                vm.alloc_string(text[range].to_string())
                            } else {
                                JsValue::undefined()
                            };
                            vm.set_property(groups_obj, name, val);
                        }
                        vm.set_property(arr_val, "groups", groups_obj);
                    } else {
                        vm.set_property(arr_val, "groups", JsValue::undefined());
                    }

                    return Ok(arr_val);
                } else {
                    if is_global || is_sticky {
                        vm.set_property(this, "lastIndex", JsValue::from_int32(0));
                    }
                    return Ok(JsValue::null());
                }
            }
        }
        Err(VmError::TypeError("RegExp.prototype.exec called on incompatible receiver".into()))
    });

    let re_test_fn = vm.alloc_native_function("test", 1, |vm, this, args| {
        let exec_fn = vm.get_property(this, "exec");
        let res = vm.call(exec_fn, this, args)?;
        Ok(JsValue::from_boolean(!res.is_nullish() && !res.is_null()))
    });

    let re_to_string_fn = vm.alloc_native_function("toString", 0, |vm, this, _args| {
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::RegExp { pattern, flags } = &obj.kind {
                return Ok(vm.alloc_string(format!("/{}/{}", pattern, flags)));
            }
        }
        Ok(vm.alloc_string("/(?:)/".to_string()))
    });

    let re_global_getter = vm.alloc_native_function("get global", 0, |vm, this, _| {
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::RegExp { flags, .. } = &obj.kind {
                return Ok(JsValue::from_boolean(flags.contains('g')));
            }
            if Some(this) == vm.regexp_proto {
                return Ok(JsValue::undefined());
            }
        }
        Err(VmError::TypeError("Method RegExp.prototype.global called on incompatible receiver".into()))
    });

    let re_ignore_case_getter = vm.alloc_native_function("get ignoreCase", 0, |vm, this, _| {
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::RegExp { flags, .. } = &obj.kind {
                return Ok(JsValue::from_boolean(flags.contains('i')));
            }
            if Some(this) == vm.regexp_proto {
                return Ok(JsValue::undefined());
            }
        }
        Err(VmError::TypeError("Method RegExp.prototype.ignoreCase called on incompatible receiver".into()))
    });

    let re_multiline_getter = vm.alloc_native_function("get multiline", 0, |vm, this, _| {
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::RegExp { flags, .. } = &obj.kind {
                return Ok(JsValue::from_boolean(flags.contains('m')));
            }
            if Some(this) == vm.regexp_proto {
                return Ok(JsValue::undefined());
            }
        }
        Err(VmError::TypeError("Method RegExp.prototype.multiline called on incompatible receiver".into()))
    });

    let re_dot_all_getter = vm.alloc_native_function("get dotAll", 0, |vm, this, _| {
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::RegExp { flags, .. } = &obj.kind {
                return Ok(JsValue::from_boolean(flags.contains('s')));
            }
            if Some(this) == vm.regexp_proto {
                return Ok(JsValue::undefined());
            }
        }
        Err(VmError::TypeError("Method RegExp.prototype.dotAll called on incompatible receiver".into()))
    });

    let re_unicode_getter = vm.alloc_native_function("get unicode", 0, |vm, this, _| {
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::RegExp { flags, .. } = &obj.kind {
                return Ok(JsValue::from_boolean(flags.contains('u')));
            }
            if Some(this) == vm.regexp_proto {
                return Ok(JsValue::undefined());
            }
        }
        Err(VmError::TypeError("Method RegExp.prototype.unicode called on incompatible receiver".into()))
    });

    let re_sticky_getter = vm.alloc_native_function("get sticky", 0, |vm, this, _| {
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::RegExp { flags, .. } = &obj.kind {
                return Ok(JsValue::from_boolean(flags.contains('y')));
            }
            if Some(this) == vm.regexp_proto {
                return Ok(JsValue::undefined());
            }
        }
        Err(VmError::TypeError("Method RegExp.prototype.sticky called on incompatible receiver".into()))
    });

    let re_has_indices_getter = vm.alloc_native_function("get hasIndices", 0, |vm, this, _| {
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::RegExp { flags, .. } = &obj.kind {
                return Ok(JsValue::from_boolean(flags.contains('d')));
            }
            if Some(this) == vm.regexp_proto {
                return Ok(JsValue::undefined());
            }
        }
        Err(VmError::TypeError("Method RegExp.prototype.hasIndices called on incompatible receiver".into()))
    });

    let re_source_getter = vm.alloc_native_function("get source", 0, |vm, this, _| {
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::RegExp { pattern, .. } = &obj.kind {
                if pattern.is_empty() {
                    return Ok(vm.alloc_string("(?:)".to_string()));
                }
                return Ok(vm.alloc_string(pattern.clone()));
            }
            if Some(this) == vm.regexp_proto {
                return Ok(vm.alloc_string("(?:)".to_string()));
            }
        }
        Err(VmError::TypeError("Method RegExp.prototype.source called on incompatible receiver".into()))
    });

    let re_flags_getter = vm.alloc_native_function("get flags", 0, |vm, this, _| {
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::RegExp { flags, .. } = &obj.kind {
                let mut sorted = String::new();
                for f in ['d', 'g', 'i', 'm', 's', 'u', 'v', 'y'] {
                    if flags.contains(f) {
                        sorted.push(f);
                    }
                }
                return Ok(vm.alloc_string(sorted));
            }
            if Some(this) == vm.regexp_proto {
                return Ok(vm.alloc_string("".to_string()));
            }
        }
        Err(VmError::TypeError("Method RegExp.prototype.flags called on incompatible receiver".into()))
    });

    vm.set_property(regexp_proto, "exec", re_exec_fn);
    vm.set_property(regexp_proto, "test", re_test_fn);
    vm.set_property(regexp_proto, "toString", re_to_string_fn);

    if let Some(ptr) = regexp_proto.as_object_ptr() {
        let obj = unsafe { &mut *(ptr as *mut JsObject) };
        obj.define_getter("global".to_string(), re_global_getter);
        obj.define_getter("ignoreCase".to_string(), re_ignore_case_getter);
        obj.define_getter("multiline".to_string(), re_multiline_getter);
        obj.define_getter("dotAll".to_string(), re_dot_all_getter);
        obj.define_getter("unicode".to_string(), re_unicode_getter);
        obj.define_getter("sticky".to_string(), re_sticky_getter);
        obj.define_getter("hasIndices".to_string(), re_has_indices_getter);
        obj.define_getter("source".to_string(), re_source_getter);
        obj.define_getter("flags".to_string(), re_flags_getter);
    }

    for prop in [
        "global", "ignoreCase", "multiline", "dotAll", "unicode", "sticky",
        "hasIndices", "source", "flags", "exec", "test", "toString", "constructor"
    ] {
        vm.non_enumerable_props.insert((regexp_proto, prop.to_string()));
    }

    vm.globals.insert("RegExp".to_string(), regexp_ctor);
    if vm.global_object.is_object() {
        vm.set_property(vm.global_object, "RegExp", regexp_ctor);
    }

    // ── Intl ──
    let intl_obj = vm.alloc_object();

    let num_format_ctor = vm.alloc_native_function("NumberFormat", 0, |vm, _this, _args| {
        let nf_obj = vm.alloc_object();
        let format_fn = vm.alloc_native_function("format", 1, |vm, _this, args| {
            let n = args.first().and_then(|v| v.to_number()).unwrap_or(0.0);
            let s = if n.fract() == 0.0 && n.abs() < 1e16 {
                let int_val = n as i64;
                let sign = if int_val < 0 { "-" } else { "" };
                let abs_str = int_val.abs().to_string();
                let mut formatted = String::new();
                for (i, ch) in abs_str.chars().rev().enumerate() {
                    if i > 0 && i % 3 == 0 {
                        formatted.push(',');
                    }
                    formatted.push(ch);
                }
                let rev_res: String = formatted.chars().rev().collect();
                format!("{}{}", sign, rev_res)
            } else {
                format!("{:.3}", n)
            };
            Ok(vm.alloc_string(s))
        });
        let resolved_opts = vm.alloc_native_function("resolvedOptions", 0, |vm, _this, _args| {
            let opts = vm.alloc_object();
            let loc = vm.alloc_string("en-US".to_string());
            vm.set_property(opts, "locale", loc);
            Ok(opts)
        });
        vm.set_property(nf_obj, "format", format_fn);
        vm.set_property(nf_obj, "resolvedOptions", resolved_opts);
        Ok(nf_obj)
    });
    let empty_locales_fn = vm.alloc_native_function("supportedLocalesOf", 1, |vm, _this, _args| {
        Ok(vm.alloc_array(Vec::new()))
    });
    vm.set_property(num_format_ctor, "supportedLocalesOf", empty_locales_fn);
    vm.set_property(intl_obj, "NumberFormat", num_format_ctor);

    let date_format_ctor = vm.alloc_native_function("DateTimeFormat", 0, |vm, _this, _args| {
        let df_obj = vm.alloc_object();
        let format_fn = vm.alloc_native_function("format", 1, |vm, _this, args| {
            let s = args.first().map(|v| vm.to_string_value(*v)).unwrap_or_else(|| "2026-10-06T00:00:00Z".to_string());
            Ok(vm.alloc_string(s))
        });
        let resolved_opts = vm.alloc_native_function("resolvedOptions", 0, |vm, _this, _args| {
            let opts = vm.alloc_object();
            let loc = vm.alloc_string("en-US".to_string());
            vm.set_property(opts, "locale", loc);
            Ok(opts)
        });
        vm.set_property(df_obj, "format", format_fn);
        vm.set_property(df_obj, "resolvedOptions", resolved_opts);
        Ok(df_obj)
    });
    let empty_locales_fn2 = vm.alloc_native_function("supportedLocalesOf", 1, |vm, _this, _args| {
        Ok(vm.alloc_array(Vec::new()))
    });
    vm.set_property(date_format_ctor, "supportedLocalesOf", empty_locales_fn2);
    vm.set_property(intl_obj, "DateTimeFormat", date_format_ctor);

    let collator_ctor = vm.alloc_native_function("Collator", 0, |vm, _this, _args| {
        let coll_obj = vm.alloc_object();
        let compare_fn = vm.alloc_native_function("compare", 2, |vm, _this, args| {
            let a = args.first().map(|v| vm.to_string_value(*v)).unwrap_or_default();
            let b = args.get(1).map(|v| vm.to_string_value(*v)).unwrap_or_default();
            let cmp = match a.cmp(&b) {
                std::cmp::Ordering::Less => -1,
                std::cmp::Ordering::Equal => 0,
                std::cmp::Ordering::Greater => 1,
            };
            Ok(JsValue::from_int32(cmp))
        });
        let resolved_opts = vm.alloc_native_function("resolvedOptions", 0, |vm, _this, _args| {
            let opts = vm.alloc_object();
            let loc = vm.alloc_string("en-US".to_string());
            vm.set_property(opts, "locale", loc);
            Ok(opts)
        });
        vm.set_property(coll_obj, "compare", compare_fn);
        vm.set_property(coll_obj, "resolvedOptions", resolved_opts);
        Ok(coll_obj)
    });
    let empty_locales_fn3 = vm.alloc_native_function("supportedLocalesOf", 1, |vm, _this, _args| {
        Ok(vm.alloc_array(Vec::new()))
    });
    vm.set_property(collator_ctor, "supportedLocalesOf", empty_locales_fn3);
    vm.set_property(intl_obj, "Collator", collator_ctor);

    vm.globals.insert("Intl".to_string(), intl_obj);

    // ── Temporal ──
    let temporal_obj = vm.alloc_object();

    let instant_ctor = vm.alloc_native_function("Instant", 1, |vm, _this, args| {
        let ms = args.first().and_then(|v| v.to_number()).unwrap_or(0.0);
        let inst = vm.alloc_object();
        vm.set_property(inst, "epochMilliseconds", JsValue::from_double(ms));
        vm.set_property(inst, "epochNanoseconds", JsValue::from_double(ms * 1_000_000.0));
        let to_str = vm.alloc_native_function("toString", 0, |vm, this, _args| {
            let ms_val = vm.get_property(this, "epochMilliseconds")
                .to_number()
                .unwrap_or(0.0);
            Ok(vm.alloc_string(format!("{}Z", ms_val)))
        });
        vm.set_property(inst, "toString", to_str);
        Ok(inst)
    });
    vm.set_property(temporal_obj, "Instant", instant_ctor);

    let now_obj = vm.alloc_object();
    let instant_fn = vm.alloc_native_function("instant", 0, |vm, _this, _args| {
        let ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as f64)
            .unwrap_or(0.0);
        let inst = vm.alloc_object();
        vm.set_property(inst, "epochMilliseconds", JsValue::from_double(ms));
        vm.set_property(inst, "epochNanoseconds", JsValue::from_double(ms * 1_000_000.0));
        let to_str = vm.alloc_native_function("toString", 0, |vm, this, _args| {
            let ms_val = vm.get_property(this, "epochMilliseconds")
                .to_number()
                .unwrap_or(0.0);
            Ok(vm.alloc_string(format!("{}Z", ms_val)))
        });
        vm.set_property(inst, "toString", to_str);
        Ok(inst)
    });
    vm.set_property(now_obj, "instant", instant_fn);
    vm.set_property(temporal_obj, "Now", now_obj);

    let plain_date_ctor = vm.alloc_native_function("PlainDate", 3, |vm, _this, args| {
        let y = args.first().and_then(|v| v.to_int32()).unwrap_or(1970);
        let m = args.get(1).and_then(|v| v.to_int32()).unwrap_or(1);
        let d = args.get(2).and_then(|v| v.to_int32()).unwrap_or(1);
        let date_obj = vm.alloc_object();
        vm.set_property(date_obj, "year", JsValue::from_int32(y));
        vm.set_property(date_obj, "month", JsValue::from_int32(m));
        vm.set_property(date_obj, "day", JsValue::from_int32(d));
        let to_str = vm.alloc_native_function("toString", 0, |vm, this, _args| {
            let y_val = vm.get_property(this, "year").to_int32().unwrap_or(0);
            let m_val = vm.get_property(this, "month").to_int32().unwrap_or(0);
            let d_val = vm.get_property(this, "day").to_int32().unwrap_or(0);
            Ok(vm.alloc_string(format!("{:04}-{:02}-{:02}", y_val, m_val, d_val)))
        });
        vm.set_property(date_obj, "toString", to_str);
        Ok(date_obj)
    });
    vm.set_property(temporal_obj, "PlainDate", plain_date_ctor);

    vm.globals.insert("Temporal".to_string(), temporal_obj);
}

fn register_test262_harness(vm: &mut Vm) {
    let print_fn = vm.alloc_native_function("print", 1, native_console_log);
    vm.globals.insert("print".to_string(), print_fn);

    let test262_obj = vm.alloc_object();
    vm.set_property(test262_obj, "global", vm.global_object);

    let eval_script_fn = vm.alloc_native_function("evalScript", 1, |vm, _this, args| {
        let code = args.first().map(|v| vm.to_string_value(*v)).unwrap_or_default();
        let program = doms_parser::parse(&code).map_err(|e| VmError::SyntaxError(e.to_string()))?;
        let chunk = doms_compiler::compile(&program).map_err(|e| VmError::SyntaxError(e.to_string()))?;
        vm.in_eval = false;
        vm.run(&chunk)
    });
    vm.set_property(test262_obj, "evalScript", eval_script_fn);

    let gc_fn = vm.alloc_native_function("gc", 0, |vm, _this, _args| {
        vm.heap.collect_nursery();
        let _ = vm.heap.collect_old_gen_compact();
        Ok(JsValue::undefined())
    });
    vm.set_property(test262_obj, "gc", gc_fn);

    let detach_fn = vm.alloc_native_function("detachArrayBuffer", 1, |_vm, _this, _args| {
        Ok(JsValue::undefined())
    });
    vm.set_property(test262_obj, "detachArrayBuffer", detach_fn);

    let create_realm_fn = vm.alloc_native_function("createRealm", 0, |vm, _this, _args| {
        let realm_obj = vm.alloc_object();
        let realm_glob = vm.alloc_object();
        if let Some(ptr) = realm_glob.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            obj.prototype = Some(vm.global_object);
        }
        vm.set_property(realm_obj, "global", realm_glob);

        let realm_eval_fn = vm.alloc_closure_native_function("eval", 1, std::sync::Arc::new(move |vm, _this, args| {
            let code_val = args.first().copied().unwrap_or(JsValue::undefined());
            let code_str = if let Some(ptr) = code_val.as_object_ptr() {
                let obj = unsafe { &*(ptr as *mut JsObject) };
                if let ObjectKind::String(ref s) = obj.kind {
                    s.clone()
                } else {
                    return Ok(code_val);
                }
            } else {
                return Ok(code_val);
            };
            let program = doms_parser::parse_eval(&code_str, false, false, false)
                .map_err(|e| VmError::SyntaxError(e.to_string()))?;
            let has_use_strict = program.body.iter()
                .take_while(|s| matches!(s, doms_parser::ast::Statement::Expression(doms_parser::ast::Expression::Literal(doms_parser::ast::Literal::String { .. }))))
                .any(|s| matches!(s, doms_parser::ast::Statement::Expression(doms_parser::ast::Expression::Literal(doms_parser::ast::Literal::String { value, has_escape: false, .. })) if value == "use strict"));
            let chunk = doms_compiler::compile_eval(&program, has_use_strict)
                .map_err(|e| VmError::SyntaxError(e.to_string()))?;

            let eval_scope = vm.alloc_object();
            let mut has_eval_scope = false;
            for name in &chunk.top_lex_names {
                vm.set_property(eval_scope, name, JsValue::uninitialized());
                has_eval_scope = true;
            }
            if has_use_strict {
                for name in &chunk.top_var_names {
                    vm.set_property(eval_scope, name, JsValue::undefined());
                    has_eval_scope = true;
                }
                for name in &chunk.top_func_names {
                    vm.set_property(eval_scope, name, JsValue::undefined());
                    has_eval_scope = true;
                }
            } else {
                for name in &chunk.top_var_names {
                    vm.set_property(realm_glob, name, JsValue::undefined());
                }
                for name in &chunk.top_func_names {
                    vm.set_property(realm_glob, name, JsValue::undefined());
                }
            }

            let mut eval_scopes = Vec::new();
            eval_scopes.push(realm_glob);
            if has_eval_scope {
                eval_scopes.push(eval_scope);
            }

            let root_chunk = Arc::new(chunk);
            let target_depth = vm.frames.len();
            vm.frames.push(crate::interpreter::CallFrame {
                chunk: root_chunk,
                ip: 0,
                base_sp: vm.stack.len(),
                this_val: realm_glob,
                gen_ptr: None,
                locals: Vec::new(),
                local_names: Vec::new(),
                lexical_local_names: Vec::new(),
                scopes: eval_scopes.clone(),
                upvalues_count: eval_scopes.len(),
                env_obj: None,
                arguments: None,
                is_strict: has_use_strict,
                new_target: None,
                is_function: false,
                is_method: false,
                is_arrow: false,
                has_param_exprs: false,
                in_param_init: false,
                is_indirect_eval: true,
                callee_func: None,
                mapped_arguments: None,
            });
            vm.run_until_depth(target_depth)
        }));

        vm.set_property(realm_glob, "eval", realm_eval_fn);
        vm.set_property(realm_obj, "evalScript", realm_eval_fn);
        Ok(realm_obj)
    });
    vm.set_property(test262_obj, "createRealm", create_realm_fn);
    vm.set_property(test262_obj, "global", vm.global_object);
    let agent_obj = vm.alloc_object();
    vm.set_property(test262_obj, "agent", agent_obj);

    vm.globals.insert("$262".to_string(), test262_obj);

    let do_not_eval = vm.alloc_native_function("$DONOTEVALUATE", 0, |_vm, _this, _args| {
        Err(VmError::ReferenceError("Test262: This statement should not be evaluated.".into()))
    });
    vm.globals.insert("$DONOTEVALUATE".to_string(), do_not_eval);

    let t262_err = vm.alloc_native_function("Test262Error", 1, |vm, this, args| {
        let msg = args.first().map(|v| vm.to_string_value(*v)).unwrap_or_default();
        let target = if this.is_object() { this } else { vm.alloc_object() };
        let name_val = vm.alloc_string("Test262Error".into());
        let msg_val = vm.alloc_string(msg);
        vm.set_property(target, "name", name_val);
        vm.set_property(target, "message", msg_val);
        Ok(target)
    });
    vm.globals.insert("Test262Error".to_string(), t262_err);
}

fn register_reflect(vm: &mut Vm) {
    let reflect_obj = vm.alloc_object();

    let construct_fn = vm.alloc_native_function("construct", 2, |vm, _this, args| {
        let target = args.first().copied().unwrap_or(JsValue::undefined());
        let args_list = if let Some(arg_arr) = args.get(1) {
            if let Some(ptr) = arg_arr.as_object_ptr() {
                let obj = unsafe { &*(ptr as *mut JsObject) };
                if let ObjectKind::Array(elements) = &obj.kind {
                    elements.clone()
                } else {
                    Vec::new()
                }
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        let new_target = args.get(2).copied().unwrap_or(target);
        if !vm.is_constructor(target) || !vm.is_constructor(new_target) {
            return Err(VmError::TypeError("Target is not a constructor".into()));
        }
        let this_obj = vm.alloc_object();
        let proto = vm.get_property(new_target, "prototype");
        if proto.is_object() {
            let this_ptr = this_obj.as_object_ptr().unwrap();
            let jsobj = unsafe { &mut *(this_ptr as *mut JsObject) };
            jsobj.prototype = Some(proto);
        }
        let res = vm.call(target, this_obj, &args_list)?;
        if res.is_object() {
            if proto.is_object() {
                if let Some(r_ptr) = res.as_object_ptr() {
                    let r_obj = unsafe { &mut *(r_ptr as *mut JsObject) };
                    r_obj.prototype = Some(proto);
                }
            }
            Ok(res)
        } else {
            Ok(this_obj)
        }
    });
    vm.set_property(reflect_obj, "construct", construct_fn);

    let apply_fn = vm.alloc_native_function("apply", 3, |vm, _this, args| {
        let target = args.first().copied().unwrap_or(JsValue::undefined());
        let this_arg = args.get(1).copied().unwrap_or(JsValue::undefined());
        let args_list = if let Some(arg_arr) = args.get(2) {
            if let Some(ptr) = arg_arr.as_object_ptr() {
                let obj = unsafe { &*(ptr as *mut JsObject) };
                if let ObjectKind::Array(elements) = &obj.kind {
                    elements.clone()
                } else {
                    Vec::new()
                }
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };
        vm.call(target, this_arg, &args_list)
    });
    vm.set_property(reflect_obj, "apply", apply_fn);

    let get_fn = vm.alloc_native_function("get", 2, |vm, _this, args| {
        let target = args.first().copied().unwrap_or(JsValue::undefined());
        let key = args.get(1).map(|v| vm.to_string_value(*v)).unwrap_or_default();
        Ok(vm.get_property(target, &key))
    });
    vm.set_property(reflect_obj, "get", get_fn);

    let set_fn = vm.alloc_native_function("set", 3, |vm, _this, args| {
        let target = match args.first() {
            Some(t) if t.is_object() => *t,
            _ => return Err(VmError::TypeError("Reflect.set called on non-object".into())),
        };
        if vm.module_namespace_objects.contains(&target) {
            return Ok(JsValue::from_boolean(false));
        }
        let key = args.get(1).map(|v| vm.to_string_value(*v)).unwrap_or_default();
        let val = args.get(2).copied().unwrap_or(JsValue::undefined());
        if vm.non_writable_props.contains(&(target, key.clone())) {
            return Ok(JsValue::from_boolean(false));
        }
        vm.set_property(target, &key, val);
        Ok(JsValue::from_boolean(true))
    });
    vm.set_property(reflect_obj, "set", set_fn);

    let has_fn = vm.alloc_native_function("has", 2, |vm, _this, args| {
        let target = match args.first() {
            Some(t) if t.is_object() => *t,
            _ => return Err(VmError::TypeError("Reflect.has called on non-object".into())),
        };
        let key_arg = args.get(1).copied().unwrap_or(JsValue::undefined());
        let has = if key_arg.is_symbol() {
            let prop_key = PropertyKeyVal::Symbol(key_arg);
            let mut curr = Some(target);
            let mut found = false;
            while let Some(c_val) = curr {
                if vm.has_own_property_key(c_val, &prop_key) {
                    found = true;
                    break;
                }
                if let Some(c_ptr) = c_val.as_object_ptr() {
                    let c_obj = unsafe { &*(c_ptr as *mut JsObject) };
                    curr = c_obj.prototype;
                } else {
                    break;
                }
            }
            found
        } else {
            let key = vm.to_string_value(key_arg);
            vm.has_property(target, &key)?
        };
        Ok(JsValue::from_boolean(has))
    });
    vm.set_property(reflect_obj, "has", has_fn);

    let define_prop_fn = vm.alloc_native_function("defineProperty", 3, |vm, _this, args| {
        let target = match args.first() {
            Some(t) if t.is_object() => *t,
            _ => return Err(VmError::TypeError("Reflect.defineProperty called on non-object".into())),
        };
        let prop_key = match args.get(1) {
            Some(p) => vm.to_property_key(*p)?,
            None => PropertyKeyVal::String("undefined".to_string()),
        };
        let desc = match args.get(2) {
            Some(d) if d.is_object() => *d,
            _ => return Err(VmError::TypeError("Property description must be an object".into())),
        };
        if vm.module_namespace_objects.contains(&target) {
            let success = define_property_on_module_namespace(vm, target, &prop_key, desc)?;
            return Ok(JsValue::from_boolean(success));
        }
        let obj_ctor = vm.globals.get("Object").copied().unwrap_or(JsValue::undefined());
        let def_fn = vm.get_property(obj_ctor, "defineProperty");
        let key_val = match &prop_key {
            PropertyKeyVal::Symbol(s) => *s,
            PropertyKeyVal::String(s) => vm.alloc_string(s.clone()),
        };
        let res = vm.call(def_fn, obj_ctor, &[target, key_val, desc]);
        Ok(JsValue::from_boolean(res.is_ok()))
    });
    vm.set_property(reflect_obj, "defineProperty", define_prop_fn);

    let delete_prop_fn = vm.alloc_native_function("deleteProperty", 2, |vm, _this, args| {
        let target = match args.first() {
            Some(t) if t.is_object() => *t,
            _ => return Err(VmError::TypeError("Reflect.deleteProperty called on non-object".into())),
        };
        let prop_key = match args.get(1) {
            Some(p) => vm.to_property_key(*p)?,
            None => PropertyKeyVal::String("undefined".to_string()),
        };
        if vm.module_namespace_objects.contains(&target) {
            match prop_key {
                PropertyKeyVal::Symbol(sym) => {
                    if vm.symbol_to_string_tag == Some(sym) {
                        return Ok(JsValue::from_boolean(false));
                    }
                    return Ok(JsValue::from_boolean(true));
                }
                PropertyKeyVal::String(name) => {
                    if let Some(ptr) = target.as_object_ptr() {
                        let obj = unsafe { &*(ptr as *mut JsObject) };
                        if vm.shapes.has_property(obj.shape_id, &name) {
                            return Ok(JsValue::from_boolean(false));
                        }
                    }
                    return Ok(JsValue::from_boolean(true));
                }
            }
        }
        match prop_key {
            PropertyKeyVal::Symbol(sym) => {
                if let Some(ptr) = target.as_object_ptr() {
                    let obj = unsafe { &mut *(ptr as *mut JsObject) };
                    if let Some(syms) = &mut obj.symbol_properties {
                        syms.remove(&sym);
                    }
                }
                Ok(JsValue::from_boolean(true))
            }
            PropertyKeyVal::String(name) => {
                if vm.non_configurable_props.contains(&(target, name.clone())) {
                    return Ok(JsValue::from_boolean(false));
                }
                vm.deleted_props.insert((target, name));
                Ok(JsValue::from_boolean(true))
            }
        }
    });
    vm.set_property(reflect_obj, "deleteProperty", delete_prop_fn);

    let get_proto_fn = vm.alloc_native_function("getPrototypeOf", 1, |_vm, _this, args| {
        let target = match args.first() {
            Some(t) if t.is_object() => *t,
            _ => return Err(VmError::TypeError("Reflect.getPrototypeOf called on non-object".into())),
        };
        if let Some(ptr) = target.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            Ok(obj.prototype.unwrap_or(JsValue::null()))
        } else {
            Ok(JsValue::null())
        }
    });
    vm.set_property(reflect_obj, "getPrototypeOf", get_proto_fn);

    let set_proto_fn = vm.alloc_native_function("setPrototypeOf", 2, |vm, _this, args| {
        let target = match args.first() {
            Some(t) if t.is_object() => *t,
            _ => return Err(VmError::TypeError("Reflect.setPrototypeOf called on non-object".into())),
        };
        let proto = args.get(1).copied().unwrap_or(JsValue::undefined());
        if !proto.is_object() && !proto.is_null() {
            return Err(VmError::TypeError("Object prototype may only be an Object or null".into()));
        }
        if vm.module_namespace_objects.contains(&target) {
            return Ok(JsValue::from_boolean(proto.is_null()));
        }
        if vm.non_extensible_objects.contains(&target) {
            return Ok(JsValue::from_boolean(false));
        }
        if let Some(ptr) = target.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            obj.prototype = if proto.is_null() { None } else { Some(proto) };
            Ok(JsValue::from_boolean(true))
        } else {
            Ok(JsValue::from_boolean(false))
        }
    });
    vm.set_property(reflect_obj, "setPrototypeOf", set_proto_fn);

    let is_ext_fn = vm.alloc_native_function("isExtensible", 1, |vm, _this, args| {
        let target = match args.first() {
            Some(t) if t.is_object() => *t,
            _ => return Err(VmError::TypeError("Reflect.isExtensible called on non-object".into())),
        };
        Ok(JsValue::from_boolean(!vm.non_extensible_objects.contains(&target)))
    });
    vm.set_property(reflect_obj, "isExtensible", is_ext_fn);

    let prev_ext_fn = vm.alloc_native_function("preventExtensions", 1, |vm, _this, args| {
        let target = match args.first() {
            Some(t) if t.is_object() => *t,
            _ => return Err(VmError::TypeError("Reflect.preventExtensions called on non-object".into())),
        };
        vm.non_extensible_objects.insert(target);
        Ok(JsValue::from_boolean(true))
    });
    vm.set_property(reflect_obj, "preventExtensions", prev_ext_fn);

    let get_own_desc_fn = vm.alloc_native_function("getOwnPropertyDescriptor", 2, |vm, _this, args| {
        let target = match args.first() {
            Some(t) if t.is_object() => *t,
            _ => return Err(VmError::TypeError("Reflect.getOwnPropertyDescriptor called on non-object".into())),
        };
        let prop = args.get(1).copied().unwrap_or(JsValue::undefined());
        let obj_ctor = vm.globals.get("Object").copied().unwrap_or(JsValue::undefined());
        let desc_fn = vm.get_property(obj_ctor, "getOwnPropertyDescriptor");
        vm.call(desc_fn, obj_ctor, &[target, prop])
    });
    vm.set_property(reflect_obj, "getOwnPropertyDescriptor", get_own_desc_fn);

    let own_keys_fn = vm.alloc_native_function("ownKeys", 1, |vm, _this, args| {
        let target = match args.first() {
            Some(t) if t.is_object() => *t,
            _ => return Err(VmError::TypeError("Reflect.ownKeys called on non-object".into())),
        };
        let obj_ctor = vm.globals.get("Object").copied().unwrap_or(JsValue::undefined());
        let keys_fn = vm.get_property(obj_ctor, "getOwnPropertyNames");
        let syms_fn = vm.get_property(obj_ctor, "getOwnPropertySymbols");
        let str_keys = vm.call(keys_fn, obj_ctor, &[target])?;
        let sym_keys = vm.call(syms_fn, obj_ctor, &[target])?;
        let mut all = Vec::new();
        if let Some(ptr) = str_keys.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::Array(elems) = &obj.kind {
                all.extend_from_slice(elems);
            }
        }
        if let Some(ptr) = sym_keys.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::Array(elems) = &obj.kind {
                all.extend_from_slice(elems);
            }
        }
        let out = vm.alloc_array(all);
        Ok(out)
    });
    vm.set_property(reflect_obj, "ownKeys", own_keys_fn);

    vm.globals.insert("Reflect".to_string(), reflect_obj);
}

fn register_proxy(vm: &mut Vm) {
    let proxy_ctor = vm.alloc_native_function("Proxy", 2, |vm, _this, args| {
        if !vm.is_constructing {
            return Err(VmError::TypeError("Constructor Proxy requires 'new'".into()));
        }
        let target = args.first().copied().unwrap_or(JsValue::undefined());
        let handler = args.get(1).copied().unwrap_or(JsValue::undefined());
        if !target.is_object() || !handler.is_object() {
            return Err(VmError::TypeError("Cannot create proxy with a non-object target or handler".into()));
        }
        Ok(vm.alloc_proxy(target, handler))
    });
    let revocable_fn = vm.alloc_native_function("revocable", 2, |vm, _this, args| {
        let target = args.first().copied().unwrap_or(JsValue::undefined());
        let handler = args.get(1).copied().unwrap_or(JsValue::undefined());
        if !target.is_object() || !handler.is_object() {
            return Err(VmError::TypeError("Cannot create proxy with a non-object target or handler".into()));
        }
        let proxy = vm.alloc_proxy(target, handler);
        let revoke = vm.alloc_native_function("revoke", 0, |_vm, _this, _args| {
            Ok(JsValue::undefined())
        });
        let result = vm.alloc_object();
        vm.set_property(result, "proxy", proxy);
        vm.set_property(result, "revoke", revoke);
        Ok(result)
    });
    vm.set_property(proxy_ctor, "revocable", revocable_fn);
    vm.globals.insert("Proxy".to_string(), proxy_ctor);
    if vm.global_object.is_object() {
        vm.set_property(vm.global_object, "Proxy", proxy_ctor);
    }
}

fn to_bigint_value(vm: &mut Vm, val: JsValue) -> Result<BigInt, VmError> {
    if val.is_nullish() {
        return Err(VmError::TypeError("Cannot convert undefined or null to a BigInt".into()));
    }
    if val.is_symbol() {
        return Err(VmError::TypeError("Cannot convert a Symbol value to a BigInt".into()));
    }
    let prim = vm.to_primitive(val, crate::interpreter::PreferredType::Number)?;
    if let Some(ptr) = prim.as_object_ptr() {
        let obj = unsafe { &*(ptr as *mut JsObject) };
        if let ObjectKind::BigInt(b) | ObjectKind::BigIntObject(b) = &obj.kind {
            return Ok(b.clone());
        }
    }
    if prim.is_boolean() {
        return Ok(if prim.as_boolean() == Some(true) { BigInt::one() } else { BigInt::zero() });
    }
    if prim.is_number() {
        let n = prim.to_number().unwrap_or(0.0);
        if !n.is_finite() {
            return Err(VmError::RangeError(format!("Cannot convert {} to a BigInt", n)));
        }
        if n.fract() != 0.0 {
            return Err(VmError::RangeError(format!("The number {} cannot be converted to a BigInt because it is not an integer", n)));
        }
        return BigInt::from_f64(n).ok_or_else(|| VmError::RangeError(format!("Cannot convert {} to a BigInt", n)));
    }
    if prim.is_string() || (prim.is_object() && prim.as_object_ptr().is_some_and(|p| matches!(unsafe { &(*(p as *mut JsObject)).kind }, ObjectKind::String(_)))) {
        let s = vm.to_string_value(prim);
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return Ok(BigInt::zero());
        }
        let (is_neg, rest) = if let Some(stripped) = trimmed.strip_prefix('+') {
            (false, stripped)
        } else if let Some(stripped) = trimmed.strip_prefix('-') {
            (true, stripped)
        } else {
            (false, trimmed)
        };
        let bi_res = if let Some(hex) = rest.strip_prefix("0x").or_else(|| rest.strip_prefix("0X")) {
            if is_neg {
                return Err(VmError::SyntaxError("Cannot convert string with sign and hex prefix to a BigInt".into()));
            }
            BigInt::parse_bytes(hex.as_bytes(), 16)
        } else if let Some(bin) = rest.strip_prefix("0b").or_else(|| rest.strip_prefix("0B")) {
            if is_neg {
                return Err(VmError::SyntaxError("Cannot convert string with sign and binary prefix to a BigInt".into()));
            }
            BigInt::parse_bytes(bin.as_bytes(), 2)
        } else if let Some(oct) = rest.strip_prefix("0o").or_else(|| rest.strip_prefix("0O")) {
            if is_neg {
                return Err(VmError::SyntaxError("Cannot convert string with sign and octal prefix to a BigInt".into()));
            }
            BigInt::parse_bytes(oct.as_bytes(), 8)
        } else {
            if rest.contains('.') || rest.contains('e') || rest.contains('E') || rest.contains('n') {
                return Err(VmError::SyntaxError("Cannot convert string with decimal or exponent to a BigInt".into()));
            }
            BigInt::parse_bytes(rest.as_bytes(), 10).map(|mut bi| {
                if is_neg {
                    bi = -bi;
                }
                bi
            })
        };
        return bi_res.ok_or_else(|| VmError::SyntaxError(format!("Cannot convert string '{}' to a BigInt", s)));
    }
    Err(VmError::TypeError("Cannot convert value to a BigInt".into()))
}

fn register_bigint(vm: &mut Vm) {
    let bigint_proto = vm.alloc_object();

    let to_string_fn = vm.alloc_native_function("toString", 0, |vm, this, args| {
        let bi = if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::BigInt(val) = &obj.kind {
                val.clone()
            } else {
                return Err(VmError::TypeError("BigInt.prototype.toString requires that 'this' be a BigInt".into()));
            }
        } else {
            return Err(VmError::TypeError("BigInt.prototype.toString requires that 'this' be a BigInt".into()));
        };
        let radix = if let Some(r_val) = args.first() {
            if r_val.is_undefined() {
                10
            } else {
                if r_val.is_symbol() || (r_val.is_object() && r_val.as_object_ptr().is_some_and(|p| matches!(unsafe { &(*(p as *mut JsObject)).kind }, ObjectKind::BigInt(_)))) {
                    return Err(VmError::TypeError("Cannot convert Symbol or BigInt to number".into()));
                }
                let r = vm.to_number_value(*r_val).map_err(|_| VmError::TypeError("radix must be convertible to number".into()))? as i32;
                if !(2..=36).contains(&r) {
                    return Err(VmError::RangeError("radix must be an integer at least 2 and no greater than 36".into()));
                }
                r
            }
        } else {
            10
        };
        let s = bi.to_str_radix(radix as u32);
        Ok(vm.alloc_string(s))
    });

    let value_of_fn = vm.alloc_native_function("valueOf", 0, |_vm, this, _args| {
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if matches!(obj.kind, ObjectKind::BigInt(_)) {
                return Ok(this);
            }
        }
        Err(VmError::TypeError("BigInt.prototype.valueOf requires that 'this' be a BigInt".into()))
    });

    let to_locale_string_fn = vm.alloc_native_function("toLocaleString", 0, |vm, this, args| {
        let to_str = vm.get_property(this, "toString");
        if vm.is_callable(to_str) {
            vm.call(to_str, this, args)
        } else {
            Err(VmError::TypeError("toString is not callable".into()))
        }
    });

    vm.set_property(bigint_proto, "toString", to_string_fn);
    vm.non_enumerable_props.insert((bigint_proto, "toString".to_string()));
    vm.set_property(bigint_proto, "valueOf", value_of_fn);
    vm.non_enumerable_props.insert((bigint_proto, "valueOf".to_string()));
    vm.set_property(bigint_proto, "toLocaleString", to_locale_string_fn);
    vm.non_enumerable_props.insert((bigint_proto, "toLocaleString".to_string()));

    if let Some(tag_sym) = vm.symbol_to_string_tag {
        let tag_val = vm.alloc_string("BigInt".to_string());
        vm.set_symbol_property(bigint_proto, tag_sym, tag_val);
    }

    let bigint_ctor = vm.alloc_native_function("BigInt", 1, |vm, _this, args| {
        if vm.is_constructing {
            return Err(VmError::TypeError("BigInt is not a constructor".into()));
        }
        let val = args.first().copied().unwrap_or(JsValue::undefined());
        let bi_val = to_bigint_value(vm, val)?;
        Ok(vm.alloc_bigint(bi_val))
    });

    vm.set_property(bigint_proto, "constructor", bigint_ctor);
    vm.non_enumerable_props.insert((bigint_proto, "constructor".to_string()));
    vm.set_property(bigint_ctor, "prototype", bigint_proto);
    vm.non_enumerable_props.insert((bigint_ctor, "prototype".to_string()));
    vm.non_writable_props.insert((bigint_ctor, "prototype".to_string()));
    vm.non_configurable_props.insert((bigint_ctor, "prototype".to_string()));

    let as_int_n = vm.alloc_native_function("asIntN", 2, |vm, _this, args| {
        let bits_arg = args.first().copied().unwrap_or(JsValue::undefined());
        let bits_f = vm.to_number_value(bits_arg).unwrap_or(0.0);
        let bits = if bits_f <= 0.0 { 0 } else { bits_f as usize };
        let bi_arg = args.get(1).copied().unwrap_or(JsValue::undefined());
        let bi = to_bigint_value(vm, bi_arg)?;
        if bits == 0 {
            return Ok(vm.alloc_bigint(BigInt::zero()));
        }
        let two_to_bits = BigInt::one() << bits;
        let mut mod_val = bi % &two_to_bits;
        if mod_val.is_negative() {
            mod_val += &two_to_bits;
        }
        let half = &two_to_bits >> 1;
        if mod_val >= half {
            mod_val -= two_to_bits;
        }
        Ok(vm.alloc_bigint(mod_val))
    });

    let as_uint_n = vm.alloc_native_function("asUintN", 2, |vm, _this, args| {
        let bits_arg = args.first().copied().unwrap_or(JsValue::undefined());
        let bits_f = vm.to_number_value(bits_arg).unwrap_or(0.0);
        let bits = if bits_f <= 0.0 { 0 } else { bits_f as usize };
        let bi_arg = args.get(1).copied().unwrap_or(JsValue::undefined());
        let bi = to_bigint_value(vm, bi_arg)?;
        if bits == 0 {
            return Ok(vm.alloc_bigint(BigInt::zero()));
        }
        let two_to_bits = BigInt::one() << bits;
        let mut mod_val = bi % &two_to_bits;
        if mod_val.is_negative() {
            mod_val += &two_to_bits;
        }
        Ok(vm.alloc_bigint(mod_val))
    });

    vm.set_property(bigint_ctor, "asIntN", as_int_n);
    vm.non_enumerable_props.insert((bigint_ctor, "asIntN".to_string()));
    vm.set_property(bigint_ctor, "asUintN", as_uint_n);
    vm.non_enumerable_props.insert((bigint_ctor, "asUintN".to_string()));

    vm.globals.insert("BigInt".to_string(), bigint_ctor);
    if vm.global_object.is_object() {
        vm.set_property(vm.global_object, "BigInt", bigint_ctor);
    }
}


