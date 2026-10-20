//! Fast dispatch for `String.prototype` methods conforming to ECMAScript 21.1.3.

use super::*;

#[inline]
pub(crate) fn this_string(vm: &mut Vm, this: JsValue) -> Result<String, VmError> {
    if this.is_nullish() {
        return Err(VmError::TypeError(
            "String.prototype method called on null or undefined".into(),
        ));
    }
    vm.to_string_spec(this)
}

pub(crate) fn is_regexp(vm: &mut Vm, val: JsValue) -> Result<bool, VmError> {
    if let Some(sym_match) = vm.symbol_match {
        if val.is_object() {
            let m = vm.get_symbol_property_result(val, sym_match)?;
            if !m.is_undefined() {
                return Ok(m.is_truthy());
            }
        }
    }
    if let Some(ptr) = val.as_object_ptr() {
        let obj = unsafe { &*(ptr as *mut JsObject) };
        if matches!(obj.kind, ObjectKind::RegExp { .. }) {
            return Ok(true);
        }
    }
    Ok(false)
}

impl Vm {
    pub fn get_string_method(&mut self, name: &str) -> Option<JsValue> {
        match name {
            "indexOf" => Some(self.alloc_native_function("indexOf", 1, |vm, this, args| {
                let this_str = this_string(vm, this)?;
                let target = match args.first() {
                    Some(v) => vm.to_string_spec(*v)?,
                    None => "undefined".to_string(),
                };
                let pos = match args.get(1) {
                    Some(v) => {
                        let num = vm.to_number_value(*v)?;
                        if num.is_nan() { 0 } else { num.trunc() as i64 }
                    }
                    None => 0,
                };
                let this_u16: Vec<u16> = this_str.encode_utf16().collect();
                let target_u16: Vec<u16> = target.encode_utf16().collect();
                let len = this_u16.len() as i64;
                let start = pos.clamp(0, len) as usize;
                if target_u16.is_empty() {
                    return Ok(JsValue::from_int32(start as i32));
                }
                if start + target_u16.len() > this_u16.len() {
                    return Ok(JsValue::from_int32(-1));
                }
                if let Some(idx) = this_u16[start..].windows(target_u16.len()).position(|w| w == target_u16.as_slice()) {
                    Ok(JsValue::from_int32((start + idx) as i32))
                } else {
                    Ok(JsValue::from_int32(-1))
                }
            })),
            "includes" => Some(self.alloc_native_function("includes", 1, |vm, this, args| {
                let this_str = this_string(vm, this)?;
                let search_arg = args.first().copied().unwrap_or(JsValue::undefined());
                if is_regexp(vm, search_arg)? {
                    return Err(VmError::TypeError("First argument to String.prototype.includes must not be a regular expression".into()));
                }
                let target = vm.to_string_spec(search_arg)?;
                let pos = match args.get(1) {
                    Some(v) => {
                        let num = vm.to_number_value(*v)?;
                        if num.is_nan() { 0 } else { num.trunc() as i64 }
                    }
                    None => 0,
                };
                let this_u16: Vec<u16> = this_str.encode_utf16().collect();
                let target_u16: Vec<u16> = target.encode_utf16().collect();
                let len = this_u16.len() as i64;
                let start = pos.clamp(0, len) as usize;
                if target_u16.is_empty() {
                    return Ok(JsValue::from_boolean(true));
                }
                if start + target_u16.len() > this_u16.len() {
                    return Ok(JsValue::from_boolean(false));
                }
                let found = this_u16[start..].windows(target_u16.len()).any(|w| w == target_u16.as_slice());
                Ok(JsValue::from_boolean(found))
            })),
            "toLowerCase" | "toLocaleLowerCase" => Some(self.alloc_native_function(name, 0, |vm, this, _args| {
                let s = this_string(vm, this)?.to_lowercase();
                Ok(vm.alloc_string(s))
            })),
            "toUpperCase" | "toLocaleUpperCase" => Some(self.alloc_native_function(name, 0, |vm, this, _args| {
                let s = this_string(vm, this)?.to_uppercase();
                Ok(vm.alloc_string(s))
            })),
            "trim" => Some(self.alloc_native_function("trim", 0, |vm, this, _args| {
                let s = this_string(vm, this)?.trim().to_string();
                Ok(vm.alloc_string(s))
            })),
            "charAt" => Some(self.alloc_native_function("charAt", 1, |vm, this, args| {
                let s = this_string(vm, this)?;
                let pos = match args.first() {
                    Some(val) => {
                        let num = vm.to_number_value(*val)?;
                        if num.is_nan() { 0 } else { num.trunc() as i64 }
                    }
                    None => 0,
                };
                let u16_chars: Vec<u16> = s.encode_utf16().collect();
                if pos < 0 || (pos as usize) >= u16_chars.len() {
                    return Ok(vm.alloc_string("".to_string()));
                }
                let s_char = String::from_utf16_lossy(&[u16_chars[pos as usize]]);
                Ok(vm.alloc_string(s_char))
            })),
            "charCodeAt" => Some(self.alloc_native_function("charCodeAt", 1, |vm, this, args| {
                let s = this_string(vm, this)?;
                let pos = match args.first() {
                    Some(val) => {
                        let num = vm.to_number_value(*val)?;
                        if num.is_nan() { 0 } else { num.trunc() as i64 }
                    }
                    None => 0,
                };
                let u16_chars: Vec<u16> = s.encode_utf16().collect();
                if pos < 0 || (pos as usize) >= u16_chars.len() {
                    return Ok(JsValue::from_double(f64::NAN));
                }
                Ok(JsValue::from_int32(u16_chars[pos as usize] as i32))
            })),
            "codePointAt" => Some(self.alloc_native_function("codePointAt", 1, |vm, this, args| {
                let s = this_string(vm, this)?;
                let pos = match args.first() {
                    Some(val) => {
                        let num = vm.to_number_value(*val)?;
                        if num.is_nan() { 0 } else { num.trunc() as i64 }
                    }
                    None => 0,
                };
                let u16_chars: Vec<u16> = s.encode_utf16().collect();
                if pos < 0 || (pos as usize) >= u16_chars.len() {
                    return Ok(JsValue::undefined());
                }
                let idx = pos as usize;
                let first = u16_chars[idx];
                if (0xD800..=0xDBFF).contains(&first) && idx + 1 < u16_chars.len() {
                    let second = u16_chars[idx + 1];
                    if (0xDC00..=0xDFFF).contains(&second) {
                        let cp = 0x10000 + (((first as u32 - 0xD800) << 10) | (second as u32 - 0xDC00));
                        return Ok(JsValue::from_int32(cp as i32));
                    }
                }
                Ok(JsValue::from_int32(first as i32))
            })),
            "slice" => Some(self.alloc_native_function("slice", 2, |vm, this, args| {
                let s = this_string(vm, this)?;
                let u16_chars: Vec<u16> = s.encode_utf16().collect();
                let len = u16_chars.len() as i64;
                let parse_slice_index = |val_opt: Option<&JsValue>, default: i64, len: i64, vm: &mut Vm| -> Result<usize, VmError> {
                    match val_opt {
                        Some(val) if !val.is_undefined() => {
                            let n = vm.to_number_value(*val)?;
                            if n.is_nan() {
                                Ok(0)
                            } else if n < 0.0 {
                                Ok((len + n.trunc() as i64).max(0) as usize)
                            } else if n >= (len as f64) {
                                Ok(len as usize)
                            } else {
                                Ok(n.trunc() as usize)
                            }
                        }
                        _ => {
                            if default < 0 {
                                Ok((len + default).max(0) as usize)
                            } else {
                                Ok((default as usize).min(len as usize))
                            }
                        }
                    }
                };
                let start = parse_slice_index(args.first(), 0, len, vm)?;
                let end = parse_slice_index(args.get(1), len, len, vm)?;
                let sub = if start < end {
                    String::from_utf16_lossy(&u16_chars[start..end])
                } else {
                    String::new()
                };
                Ok(vm.alloc_string(sub))
            })),
            "isWellFormed" => Some(self.alloc_native_function("isWellFormed", 0, |vm, this, _args| {
                let s = this_string(vm, this)?;
                let mut ok = true;
                let mut iter = s.encode_utf16().peekable();
                while let Some(c) = iter.next() {
                    if (0xD800..=0xDBFF).contains(&c) {
                        if let Some(&next) = iter.peek() {
                            if (0xDC00..=0xDFFF).contains(&next) {
                                iter.next();
                                continue;
                            }
                        }
                        ok = false;
                        break;
                    } else if (0xDC00..=0xDFFF).contains(&c) {
                        ok = false;
                        break;
                    }
                }
                Ok(JsValue::from_boolean(ok))
            })),
            "toWellFormed" => Some(self.alloc_native_function("toWellFormed", 0, |vm, this, _args| {
                let s = this_string(vm, this)?;
                let u16s: Vec<u16> = s.encode_utf16().collect();
                let mut sanitized = Vec::with_capacity(u16s.len());
                let mut i = 0;
                while i < u16s.len() {
                    let c = u16s[i];
                    if (0xD800..=0xDBFF).contains(&c) {
                        if i + 1 < u16s.len() && (0xDC00..=0xDFFF).contains(&u16s[i + 1]) {
                            sanitized.push(c);
                            sanitized.push(u16s[i + 1]);
                            i += 2;
                        } else {
                            sanitized.push(0xFFFD);
                            i += 1;
                        }
                    } else if (0xDC00..=0xDFFF).contains(&c) {
                        sanitized.push(0xFFFD);
                        i += 1;
                    } else {
                        sanitized.push(c);
                        i += 1;
                    }
                }
                Ok(vm.alloc_string(String::from_utf16_lossy(&sanitized)))
            })),
            "replace" => Some(self.alloc_native_function("replace", 2, |vm, this, args| {
                if this.is_nullish() {
                    return Err(VmError::TypeError("String.prototype.replace called on null or undefined".into()));
                }
                let search_arg = args.first().copied().unwrap_or(JsValue::undefined());
                let replace_arg = args.get(1).copied().unwrap_or(JsValue::undefined());

                if !search_arg.is_undefined() && !search_arg.is_null() {
                    if let Some(sym_replace) = vm.symbol_replace {
                        let replacer = vm.get_symbol_property_result(search_arg, sym_replace)?;
                        if !replacer.is_undefined() && !replacer.is_null() {
                            if !vm.is_callable(replacer) {
                                return Err(VmError::TypeError("RegExp[Symbol.replace] is not a function".into()));
                            }
                            return vm.call(replacer, search_arg, &[this, replace_arg]);
                        }
                    }
                }

                let this_str = vm.to_string_spec(this)?;
                let is_regexp = if let Some(ptr) = search_arg.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    matches!(obj.kind, ObjectKind::RegExp { .. })
                } else {
                    false
                };

                let is_replace_fn = vm.is_callable(replace_arg);

                if is_regexp {
                    let (pattern, flags) = if let Some(ptr) = search_arg.as_object_ptr() {
                        let obj = unsafe { &*(ptr as *mut JsObject) };
                        if let ObjectKind::RegExp { ref pattern, ref flags } = obj.kind {
                            (pattern.clone(), flags.clone())
                        } else {
                            (String::new(), String::new())
                        }
                    } else {
                        (String::new(), String::new())
                    };

                    let is_global = flags.contains('g');
                    let mut result = String::new();
                    let mut search_from = 0;

                    while search_from <= this_str.len() {
                        if pattern.is_empty() {
                            let matched_str = "";
                            let replacement = if is_replace_fn {
                                let arg0 = vm.alloc_string(matched_str.to_string());
                                let arg1 = JsValue::from_int32(search_from as i32);
                                let arg2 = vm.alloc_string(this_str.clone());
                                let call_res = vm.call(replace_arg, JsValue::undefined(), &[arg0, arg1, arg2])?;
                                vm.to_string_spec(call_res)?
                            } else {
                                vm.to_string_spec(replace_arg)?
                            };
                            result.push_str(&replacement);
                            if search_from < this_str.len() {
                                if let Some(ch) = this_str[search_from..].chars().next() {
                                    result.push(ch);
                                    search_from += ch.len_utf8();
                                } else {
                                    break;
                                }
                            } else {
                                break;
                            }
                            if !is_global {
                                result.push_str(&this_str[search_from..]);
                                break;
                            }
                        } else if let Some(found_idx) = this_str[search_from..].find(&pattern) {
                            let match_start = search_from + found_idx;
                            let match_end = match_start + pattern.len();
                            result.push_str(&this_str[search_from..match_start]);

                            let replacement = if is_replace_fn {
                                let arg0 = vm.alloc_string(pattern.clone());
                                let arg1 = JsValue::from_int32(match_start as i32);
                                let arg2 = vm.alloc_string(this_str.clone());
                                let call_res = vm.call(replace_arg, JsValue::undefined(), &[arg0, arg1, arg2])?;
                                vm.to_string_spec(call_res)?
                            } else {
                                vm.to_string_spec(replace_arg)?
                            };
                            result.push_str(&replacement);
                            search_from = match_end;

                            if !is_global {
                                result.push_str(&this_str[search_from..]);
                                break;
                            }
                        } else {
                            result.push_str(&this_str[search_from..]);
                            break;
                        }
                    }
                    Ok(vm.alloc_string(result))
                } else {
                    let search_str = vm.to_string_spec(search_arg)?;
                    if let Some(pos) = this_str.find(&search_str) {
                        let before = &this_str[..pos];
                        let after = &this_str[pos + search_str.len()..];
                        let replacement = if is_replace_fn {
                            let arg0 = vm.alloc_string(search_str.clone());
                            let arg1 = JsValue::from_int32(pos as i32);
                            let arg2 = vm.alloc_string(this_str.clone());
                            let call_res = vm.call(replace_arg, JsValue::undefined(), &[arg0, arg1, arg2])?;
                            vm.to_string_spec(call_res)?
                        } else {
                            vm.to_string_spec(replace_arg)?
                        };
                        let mut result = String::with_capacity(before.len() + replacement.len() + after.len());
                        result.push_str(before);
                        result.push_str(&replacement);
                        result.push_str(after);
                        Ok(vm.alloc_string(result))
                    } else {
                        Ok(vm.alloc_string(this_str))
                    }
                }
            })),
            "at" => Some(self.alloc_native_function("at", 1, |vm, this, args| {
                let s = this_string(vm, this)?;
                let u16_chars: Vec<u16> = s.encode_utf16().collect();
                let len = u16_chars.len() as i64;
                let rel_idx = match args.first() {
                    Some(v) => {
                        let n = vm.to_number_value(*v)?;
                        if n.is_nan() { 0 } else { n.trunc() as i64 }
                    }
                    None => 0,
                };
                let k = if rel_idx >= 0 { rel_idx } else { len + rel_idx };
                if k < 0 || k >= len {
                    return Ok(JsValue::undefined());
                }
                let ch = String::from_utf16_lossy(&[u16_chars[k as usize]]);
                Ok(vm.alloc_string(ch))
            })),
            "replaceAll" => Some(self.alloc_native_function("replaceAll", 2, |vm, this, args| {
                if this.is_nullish() {
                    return Err(VmError::TypeError("String.prototype.replaceAll called on null or undefined".into()));
                }
                let search_arg = args.first().copied().unwrap_or(JsValue::undefined());
                let replace_arg = args.get(1).copied().unwrap_or(JsValue::undefined());

                if !search_arg.is_undefined() && !search_arg.is_null() {
                    if is_regexp(vm, search_arg)? {
                        let flags_prop = vm.get_property(search_arg, "flags");
                        let flags_str = vm.to_string_spec(flags_prop)?;
                        if !flags_str.contains('g') {
                            return Err(VmError::TypeError("String.prototype.replaceAll called with a non-global RegExp argument".into()));
                        }
                    }
                    if let Some(sym_replace) = vm.symbol_replace {
                        let replacer = vm.get_symbol_property_result(search_arg, sym_replace)?;
                        if !replacer.is_undefined() && !replacer.is_null() {
                            if !vm.is_callable(replacer) {
                                return Err(VmError::TypeError("RegExp[Symbol.replace] is not a function".into()));
                            }
                            return vm.call(replacer, search_arg, &[this, replace_arg]);
                        }
                    }
                }

                let this_str = vm.to_string_spec(this)?;
                let is_regexp = if let Some(ptr) = search_arg.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    matches!(obj.kind, ObjectKind::RegExp { .. })
                } else {
                    false
                };

                let is_replace_fn = vm.is_callable(replace_arg);

                if is_regexp {
                    let (pattern, flags) = if let Some(ptr) = search_arg.as_object_ptr() {
                        let obj = unsafe { &*(ptr as *mut JsObject) };
                        if let ObjectKind::RegExp { ref pattern, ref flags } = obj.kind {
                            (pattern.clone(), flags.clone())
                        } else {
                            (String::new(), String::new())
                        }
                    } else {
                        (String::new(), String::new())
                    };

                    if !flags.contains('g') {
                        return Err(VmError::TypeError("String.prototype.replaceAll called with a non-global RegExp argument".into()));
                    }

                    let mut result = String::new();
                    let mut search_from = 0;

                    while search_from <= this_str.len() {
                        if pattern.is_empty() {
                            let matched_str = "";
                            let replacement = if is_replace_fn {
                                let arg0 = vm.alloc_string(matched_str.to_string());
                                let arg1 = JsValue::from_int32(search_from as i32);
                                let arg2 = vm.alloc_string(this_str.clone());
                                let call_res = vm.call(replace_arg, JsValue::undefined(), &[arg0, arg1, arg2])?;
                                vm.to_string_spec(call_res)?
                            } else {
                                vm.to_string_spec(replace_arg)?
                            };
                            result.push_str(&replacement);
                            if search_from < this_str.len() {
                                if let Some(ch) = this_str[search_from..].chars().next() {
                                    result.push(ch);
                                    search_from += ch.len_utf8();
                                } else {
                                    break;
                                }
                            } else {
                                break;
                            }
                        } else if let Some(found_idx) = this_str[search_from..].find(&pattern) {
                            let match_start = search_from + found_idx;
                            let match_end = match_start + pattern.len();
                            result.push_str(&this_str[search_from..match_start]);

                            let replacement = if is_replace_fn {
                                let arg0 = vm.alloc_string(pattern.clone());
                                let arg1 = JsValue::from_int32(match_start as i32);
                                let arg2 = vm.alloc_string(this_str.clone());
                                let call_res = vm.call(replace_arg, JsValue::undefined(), &[arg0, arg1, arg2])?;
                                vm.to_string_spec(call_res)?
                            } else {
                                vm.to_string_spec(replace_arg)?
                            };
                            result.push_str(&replacement);
                            search_from = match_end;
                        } else {
                            result.push_str(&this_str[search_from..]);
                            break;
                        }
                    }
                    Ok(vm.alloc_string(result))
                } else {
                    let search_str = vm.to_string_spec(search_arg)?;
                    let mut result = String::new();
                    let mut search_from = 0;
                    while search_from <= this_str.len() {
                        if search_str.is_empty() {
                            let replacement = if is_replace_fn {
                                let arg0 = vm.alloc_string("".to_string());
                                let arg1 = JsValue::from_int32(search_from as i32);
                                let arg2 = vm.alloc_string(this_str.clone());
                                let call_res = vm.call(replace_arg, JsValue::undefined(), &[arg0, arg1, arg2])?;
                                vm.to_string_spec(call_res)?
                            } else {
                                vm.to_string_spec(replace_arg)?
                            };
                            result.push_str(&replacement);
                            if search_from < this_str.len() {
                                if let Some(ch) = this_str[search_from..].chars().next() {
                                    result.push(ch);
                                    search_from += ch.len_utf8();
                                } else {
                                    break;
                                }
                            } else {
                                break;
                            }
                        } else if let Some(found_idx) = this_str[search_from..].find(&search_str) {
                            let match_start = search_from + found_idx;
                            let match_end = match_start + search_str.len();
                            result.push_str(&this_str[search_from..match_start]);
                            let replacement = if is_replace_fn {
                                let arg0 = vm.alloc_string(search_str.clone());
                                let arg1 = JsValue::from_int32(match_start as i32);
                                let arg2 = vm.alloc_string(this_str.clone());
                                let call_res = vm.call(replace_arg, JsValue::undefined(), &[arg0, arg1, arg2])?;
                                vm.to_string_spec(call_res)?
                            } else {
                                vm.to_string_spec(replace_arg)?
                            };
                            result.push_str(&replacement);
                            search_from = match_end;
                        } else {
                            result.push_str(&this_str[search_from..]);
                            break;
                        }
                    }
                    Ok(vm.alloc_string(result))
                }
            })),
            "match" => Some(self.alloc_native_function("match", 1, |vm, this, args| {
                if this.is_nullish() {
                    return Err(VmError::TypeError("String.prototype.match called on null or undefined".into()));
                }
                let re_arg = args.first().copied().unwrap_or(JsValue::undefined());
                if !re_arg.is_undefined() && !re_arg.is_null() {
                    if let Some(sym_match) = vm.symbol_match {
                        let matcher = vm.get_symbol_property_result(re_arg, sym_match)?;
                        if !matcher.is_undefined() && !matcher.is_null() {
                            if !vm.is_callable(matcher) {
                                return Err(VmError::TypeError("RegExp[Symbol.match] is not a function".into()));
                            }
                            return vm.call(matcher, re_arg, &[this]);
                        }
                    }
                }
                let this_str = vm.to_string_spec(this)?;
                let re = if let Some(ptr) = re_arg.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    if matches!(obj.kind, ObjectKind::RegExp { .. }) {
                        re_arg
                    } else {
                        let s = if re_arg.is_undefined() { "".to_string() } else { vm.to_string_spec(re_arg)? };
                        vm.alloc_regexp(s, "".to_string())
                    }
                } else {
                    let s = if re_arg.is_undefined() { "".to_string() } else { vm.to_string_spec(re_arg)? };
                    vm.alloc_regexp(s, "".to_string())
                };
                if let Some(sym_match) = vm.symbol_match {
                    let matcher = vm.get_symbol_property_result(re, sym_match)?;
                    if vm.is_callable(matcher) {
                        let this_str_val = vm.alloc_string(this_str);
                        return vm.call(matcher, re, &[this_str_val]);
                    }
                }
                let exec_fn = vm.get_property(re, "exec");
                let this_str_val = vm.alloc_string(this_str);
                vm.call(exec_fn, re, &[this_str_val])
            })),
            "search" => Some(self.alloc_native_function("search", 1, |vm, this, args| {
                if this.is_nullish() {
                    return Err(VmError::TypeError("String.prototype.search called on null or undefined".into()));
                }
                let re_arg = args.first().copied().unwrap_or(JsValue::undefined());
                if !re_arg.is_undefined() && !re_arg.is_null() {
                    if let Some(sym_search) = vm.symbol_search {
                        let searcher = vm.get_symbol_property_result(re_arg, sym_search)?;
                        if !searcher.is_undefined() && !searcher.is_null() {
                            if !vm.is_callable(searcher) {
                                return Err(VmError::TypeError("RegExp[Symbol.search] is not a function".into()));
                            }
                            return vm.call(searcher, re_arg, &[this]);
                        }
                    }
                }
                let this_str = vm.to_string_spec(this)?;
                let re = if let Some(ptr) = re_arg.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    if matches!(obj.kind, ObjectKind::RegExp { .. }) {
                        re_arg
                    } else {
                        let s = if re_arg.is_undefined() { "".to_string() } else { vm.to_string_spec(re_arg)? };
                        vm.alloc_regexp(s, "".to_string())
                    }
                } else {
                    let s = if re_arg.is_undefined() { "".to_string() } else { vm.to_string_spec(re_arg)? };
                    vm.alloc_regexp(s, "".to_string())
                };
                if let Some(sym_search) = vm.symbol_search {
                    let searcher = vm.get_symbol_property_result(re, sym_search)?;
                    if vm.is_callable(searcher) {
                        let this_str_val = vm.alloc_string(this_str);
                        return vm.call(searcher, re, &[this_str_val]);
                    }
                }
                let exec_fn = vm.get_property(re, "exec");
                let this_str_val = vm.alloc_string(this_str);
                let res = vm.call(exec_fn, re, &[this_str_val])?;
                if res.is_nullish() || res.is_null() {
                    Ok(JsValue::from_int32(-1))
                } else {
                    let idx = vm.get_property(res, "index");
                    Ok(idx)
                }
            })),
            "lastIndexOf" => Some(self.alloc_native_function("lastIndexOf", 1, |vm, this, args| {
                let this_str = this_string(vm, this)?;
                let target = match args.first() {
                    Some(v) => vm.to_string_spec(*v)?,
                    None => "undefined".to_string(),
                };
                let this_u16: Vec<u16> = this_str.encode_utf16().collect();
                let target_u16: Vec<u16> = target.encode_utf16().collect();
                let len = this_u16.len() as i64;
                let pos = match args.get(1) {
                    Some(v) => {
                        let num = vm.to_number_value(*v)?;
                        if num.is_nan() { len } else { num.trunc() as i64 }
                    }
                    None => len,
                };
                let start = pos.clamp(0, len) as usize;
                if target_u16.is_empty() {
                    return Ok(JsValue::from_int32(start as i32));
                }
                if target_u16.len() > this_u16.len() {
                    return Ok(JsValue::from_int32(-1));
                }
                let max_k = start.min(this_u16.len() - target_u16.len());
                for k in (0..=max_k).rev() {
                    if &this_u16[k..k + target_u16.len()] == target_u16.as_slice() {
                        return Ok(JsValue::from_int32(k as i32));
                    }
                }
                Ok(JsValue::from_int32(-1))
            })),
            "split" => Some(self.alloc_native_function("split", 2, |vm, this, args| {
                if this.is_nullish() {
                    return Err(VmError::TypeError("String.prototype.split called on null or undefined".into()));
                }
                let sep_arg = args.first().copied().unwrap_or(JsValue::undefined());
                let limit_arg = args.get(1).copied().unwrap_or(JsValue::undefined());

                if !sep_arg.is_undefined() && !sep_arg.is_null() {
                    if let Some(sym_split) = vm.symbol_split {
                        let splitter = vm.get_symbol_property_result(sep_arg, sym_split)?;
                        if !splitter.is_undefined() && !splitter.is_null() {
                            if !vm.is_callable(splitter) {
                                return Err(VmError::TypeError("RegExp[Symbol.split] is not a function".into()));
                            }
                            return vm.call(splitter, sep_arg, &[this, limit_arg]);
                        }
                    }
                }

                let this_str = vm.to_string_spec(this)?;
                let sep = if sep_arg.is_undefined() {
                    let str_val = vm.alloc_string(this_str);
                    return Ok(vm.alloc_array(vec![str_val]));
                } else {
                    vm.to_string_spec(sep_arg)?
                };
                let limit = match limit_arg {
                    v if !v.is_undefined() => {
                        let num = vm.to_number_value(v)?;
                        if num.is_nan() || num < 0.0 { 0 } else { (num.trunc() as u32) as usize }
                    }
                    _ => usize::MAX,
                };
                if limit == 0 {
                    return Ok(vm.alloc_array(Vec::new()));
                }
                let parts: Vec<JsValue> = if sep.is_empty() {
                    this_str.chars().map(|c| vm.alloc_string(c.to_string())).collect()
                } else {
                    this_str.split(&sep).map(|p| vm.alloc_string(p.to_string())).collect()
                };
                let parts = parts.into_iter().take(limit).collect();
                Ok(vm.alloc_array(parts))
            })),
            "substring" => Some(self.alloc_native_function("substring", 2, |vm, this, args| {
                let s = this_string(vm, this)?;
                let u16_chars: Vec<u16> = s.encode_utf16().collect();
                let len = u16_chars.len() as i64;
                let mut start = match args.first() {
                    Some(v) => {
                        let n = vm.to_number_value(*v)?;
                        if n.is_nan() { 0 } else { (n.trunc() as i64).clamp(0, len) }
                    }
                    None => 0,
                };
                let mut end = match args.get(1) {
                    Some(v) => {
                        if v.is_undefined() {
                            len
                        } else {
                            let n = vm.to_number_value(*v)?;
                            if n.is_nan() { 0 } else { (n.trunc() as i64).clamp(0, len) }
                        }
                    }
                    None => len,
                };
                if start > end {
                    std::mem::swap(&mut start, &mut end);
                }
                let sub = String::from_utf16_lossy(&u16_chars[start as usize..end as usize]);
                Ok(vm.alloc_string(sub))
            })),
            "concat" => Some(self.alloc_native_function("concat", 1, |vm, this, args| {
                let mut res = this_string(vm, this)?;
                for arg in args {
                    res.push_str(&vm.to_string_spec(*arg)?);
                }
                Ok(vm.alloc_string(res))
            })),
            "startsWith" => Some(self.alloc_native_function("startsWith", 1, |vm, this, args| {
                let this_str = this_string(vm, this)?;
                let search_arg = args.first().copied().unwrap_or(JsValue::undefined());
                if is_regexp(vm, search_arg)? {
                    return Err(VmError::TypeError("First argument to String.prototype.startsWith must not be a regular expression".into()));
                }
                let search = vm.to_string_spec(search_arg)?;
                let this_u16: Vec<u16> = this_str.encode_utf16().collect();
                let search_u16: Vec<u16> = search.encode_utf16().collect();
                let len = this_u16.len() as i64;
                let pos = match args.get(1) {
                    Some(v) => {
                        let n = vm.to_number_value(*v)?;
                        if n.is_nan() { 0 } else { (n.trunc() as i64).clamp(0, len) as usize }
                    }
                    None => 0,
                };
                if pos + search_u16.len() > this_u16.len() {
                    return Ok(JsValue::from_boolean(false));
                }
                let matches = &this_u16[pos..pos + search_u16.len()] == search_u16.as_slice();
                Ok(JsValue::from_boolean(matches))
            })),
            "endsWith" => Some(self.alloc_native_function("endsWith", 1, |vm, this, args| {
                let this_str = this_string(vm, this)?;
                let search_arg = args.first().copied().unwrap_or(JsValue::undefined());
                if is_regexp(vm, search_arg)? {
                    return Err(VmError::TypeError("First argument to String.prototype.endsWith must not be a regular expression".into()));
                }
                let search = vm.to_string_spec(search_arg)?;
                let this_u16: Vec<u16> = this_str.encode_utf16().collect();
                let search_u16: Vec<u16> = search.encode_utf16().collect();
                let len = this_u16.len() as i64;
                let end = match args.get(1) {
                    Some(v) => {
                        if v.is_undefined() {
                            len as usize
                        } else {
                            let n = vm.to_number_value(*v)?;
                            if n.is_nan() { 0 } else { (n.trunc() as i64).clamp(0, len) as usize }
                        }
                    }
                    None => len as usize,
                };
                if search_u16.len() > end {
                    return Ok(JsValue::from_boolean(false));
                }
                let start = end - search_u16.len();
                let matches = &this_u16[start..end] == search_u16.as_slice();
                Ok(JsValue::from_boolean(matches))
            })),
            "repeat" => Some(self.alloc_native_function("repeat", 1, |vm, this, args| {
                let this_str = this_string(vm, this)?;
                let count_f = match args.first() {
                    Some(v) => vm.to_number_value(*v)?,
                    None => 0.0,
                };
                if count_f < 0.0 || count_f.is_infinite() {
                    return Err(VmError::RangeError("Invalid count value".into()));
                }
                let count = count_f as usize;
                Ok(vm.alloc_string(this_str.repeat(count)))
            })),
            "padStart" => Some(self.alloc_native_function("padStart", 1, |vm, this, args| {
                let this_str = this_string(vm, this)?;
                let this_u16: Vec<u16> = this_str.encode_utf16().collect();
                let target_len = match args.first() {
                    Some(v) => {
                        let n = vm.to_number_value(*v)?;
                        if n.is_nan() || n < 0.0 { 0 } else { n.trunc() as usize }
                    }
                    None => 0,
                };
                let pad_str = match args.get(1) {
                    Some(v) if !v.is_undefined() => vm.to_string_spec(*v)?,
                    _ => " ".to_string(),
                };
                if this_u16.len() >= target_len || pad_str.is_empty() {
                    return Ok(vm.alloc_string(this_str));
                }
                let diff = target_len - this_u16.len();
                let pad_u16: Vec<u16> = pad_str.encode_utf16().collect();
                let repeats = (diff / pad_u16.len()) + 1;
                let mut full_pad_u16 = Vec::with_capacity(repeats * pad_u16.len());
                for _ in 0..repeats {
                    full_pad_u16.extend_from_slice(&pad_u16);
                }
                full_pad_u16.truncate(diff);
                full_pad_u16.extend_from_slice(&this_u16);
                let res = String::from_utf16_lossy(&full_pad_u16);
                Ok(vm.alloc_string(res))
            })),
            "padEnd" => Some(self.alloc_native_function("padEnd", 1, |vm, this, args| {
                let this_str = this_string(vm, this)?;
                let mut this_u16: Vec<u16> = this_str.encode_utf16().collect();
                let target_len = match args.first() {
                    Some(v) => {
                        let n = vm.to_number_value(*v)?;
                        if n.is_nan() || n < 0.0 { 0 } else { n.trunc() as usize }
                    }
                    None => 0,
                };
                let pad_str = match args.get(1) {
                    Some(v) if !v.is_undefined() => vm.to_string_spec(*v)?,
                    _ => " ".to_string(),
                };
                if this_u16.len() >= target_len || pad_str.is_empty() {
                    return Ok(vm.alloc_string(this_str));
                }
                let diff = target_len - this_u16.len();
                let pad_u16: Vec<u16> = pad_str.encode_utf16().collect();
                let repeats = (diff / pad_u16.len()) + 1;
                let mut full_pad_u16 = Vec::with_capacity(repeats * pad_u16.len());
                for _ in 0..repeats {
                    full_pad_u16.extend_from_slice(&pad_u16);
                }
                full_pad_u16.truncate(diff);
                this_u16.extend_from_slice(&full_pad_u16);
                let res = String::from_utf16_lossy(&this_u16);
                Ok(vm.alloc_string(res))
            })),
            "localeCompare" => Some(self.alloc_native_function("localeCompare", 1, |vm, this, args| {
                let a = this_string(vm, this)?;
                let b = match args.first() {
                    Some(v) => vm.to_string_spec(*v)?,
                    None => "undefined".to_string(),
                };
                let cmp = match a.cmp(&b) {
                    std::cmp::Ordering::Less => -1,
                    std::cmp::Ordering::Equal => 0,
                    std::cmp::Ordering::Greater => 1,
                };
                Ok(JsValue::from_int32(cmp))
            })),
            _ => None,
        }
    }
}
