//! Fast dispatch for `String.prototype` methods.

use super::*;

impl Vm {
    pub fn get_string_method(&mut self, name: &str) -> Option<JsValue> {
        match name {
            "indexOf" => Some(self.alloc_native_function("indexOf", 1, |vm, this, args| {
                let target = args.first().map(|v| vm.to_string_value(*v)).unwrap_or_default();
                let this_str = vm.to_string_value(this);
                if let Some(idx) = this_str.find(&target) {
                    Ok(JsValue::from_int32(idx as i32))
                } else {
                    Ok(JsValue::from_int32(-1))
                }
            })),
            "includes" => Some(self.alloc_native_function("includes", 1, |vm, this, args| {
                let target = args.first().map(|v| vm.to_string_value(*v)).unwrap_or_default();
                let this_str = vm.to_string_value(this);
                Ok(JsValue::from_boolean(this_str.contains(&target)))
            })),
            "toLowerCase" | "toLocaleLowerCase" => Some(self.alloc_native_function(name, 0, |vm, this, _args| {
                let s = vm.to_string_value(this).to_lowercase();
                Ok(vm.alloc_string(s))
            })),
            "toUpperCase" | "toLocaleUpperCase" => Some(self.alloc_native_function(name, 0, |vm, this, _args| {
                let s = vm.to_string_value(this).to_uppercase();
                Ok(vm.alloc_string(s))
            })),
            "trim" => Some(self.alloc_native_function("trim", 0, |vm, this, _args| {
                let s = vm.to_string_value(this).trim().to_string();
                Ok(vm.alloc_string(s))
            })),
            "charAt" => Some(self.alloc_native_function("charAt", 1, |vm, this, args| {
                let s = vm.to_string_value(this);
                let idx = match args.first() {
                    Some(val) => {
                        let num = vm.to_number_value(*val)?;
                        if num.is_nan() || num.is_infinite() {
                            return Ok(vm.alloc_string("".to_string()));
                        }
                        let int = num.trunc();
                        if int < 0.0 || int >= (usize::MAX as f64) {
                            return Ok(vm.alloc_string("".to_string()));
                        }
                        int as usize
                    }
                    None => 0,
                };
                if let Some(code) = s.encode_utf16().nth(idx) {
                    let s_char = String::from_utf16_lossy(&[code]);
                    Ok(vm.alloc_string(s_char))
                } else {
                    Ok(vm.alloc_string("".to_string()))
                }
            })),
            "charCodeAt" => Some(self.alloc_native_function("charCodeAt", 1, |vm, this, args| {
                let s = vm.to_string_value(this);
                let idx = match args.first() {
                    Some(val) => {
                        let num = vm.to_number_value(*val)?;
                        if num.is_nan() || num.is_infinite() {
                            return Ok(JsValue::from_double(f64::NAN));
                        }
                        let int = num.trunc();
                        if int < 0.0 || int >= (usize::MAX as f64) {
                            return Ok(JsValue::from_double(f64::NAN));
                        }
                        int as usize
                    }
                    None => 0,
                };
                if let Some(code) = s.encode_utf16().nth(idx) {
                    Ok(JsValue::from_int32(code as i32))
                } else {
                    Ok(JsValue::from_double(f64::NAN))
                }
            })),
            "codePointAt" => Some(self.alloc_native_function("codePointAt", 1, |vm, this, args| {
                let s = vm.to_string_value(this);
                let idx = match args.first() {
                    Some(val) => {
                        let num = vm.to_number_value(*val)?;
                        if num.is_nan() || num.is_infinite() {
                            return Ok(JsValue::undefined());
                        }
                        let int = num.trunc();
                        if int < 0.0 || int >= (usize::MAX as f64) {
                            return Ok(JsValue::undefined());
                        }
                        int as usize
                    }
                    None => 0,
                };
                let u16_chars: Vec<u16> = s.encode_utf16().collect();
                if idx >= u16_chars.len() {
                    return Ok(JsValue::undefined());
                }
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
                let s = vm.to_string_value(this);
                let is_ascii = s.is_ascii();
                let len = if is_ascii {
                    s.len()
                } else {
                    s.encode_utf16().count()
                };
                let parse_slice_index = |val_opt: Option<&JsValue>, default: i64, len: usize, vm: &mut Vm| -> Result<usize, VmError> {
                    match val_opt {
                        Some(val) if !val.is_undefined() => {
                            let n = vm.to_number_value(*val)?;
                            if n.is_nan() {
                                Ok(0)
                            } else if n <= -(len as f64) {
                                Ok(0)
                            } else if n < 0.0 {
                                Ok((len as i64 + n.trunc() as i64).max(0) as usize)
                            } else if n >= len as f64 {
                                Ok(len)
                            } else {
                                Ok(n.trunc() as usize)
                            }
                        }
                        _ => {
                            if default < 0 {
                                Ok((len as i64 + default).max(0) as usize)
                            } else {
                                Ok((default as usize).min(len))
                            }
                        }
                    }
                };
                let start = parse_slice_index(args.first(), 0, len, vm)?;
                let end = parse_slice_index(args.get(1), len as i64, len, vm)?;
                let sub = if start < end {
                    if is_ascii {
                        s[start..end].to_string()
                    } else {
                        let u16_chars: Vec<u16> = s.encode_utf16().collect();
                        String::from_utf16_lossy(&u16_chars[start..end])
                    }
                } else {
                    String::new()
                };
                Ok(vm.alloc_string(sub))
            })),
            "isWellFormed" => Some(self.alloc_native_function("isWellFormed", 0, |vm, this, _args| {
                let s = vm.to_string_value(this);
                let mut ok = true;
                for c in s.chars() {
                    let u = c as u32;
                    if (0xD800..=0xDFFF).contains(&u) {
                        ok = false;
                        break;
                    }
                }
                Ok(JsValue::from_boolean(ok))
            })),
            "toWellFormed" => Some(self.alloc_native_function("toWellFormed", 0, |vm, this, _args| {
                let s = vm.to_string_value(this);
                let sanitized: String = s.chars().map(|c| {
                    let u = c as u32;
                    if (0xD800..=0xDFFF).contains(&u) {
                        '\u{FFFD}'
                    } else {
                        c
                    }
                }).collect();
                Ok(vm.alloc_string(sanitized))
            })),
            "replace" => Some(self.alloc_native_function("replace", 2, |vm, this, args| {
                if this.is_nullish() {
                    return Err(VmError::TypeError("String.prototype.replace called on null or undefined".into()));
                }
                let this_str = vm.to_string_value(this);
                let search_arg = args.first().copied().unwrap_or(JsValue::undefined());
                let replace_arg = args.get(1).copied().unwrap_or(JsValue::undefined());

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
                                vm.to_string_value(call_res)
                            } else {
                                vm.to_string_value(replace_arg)
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
                                vm.to_string_value(call_res)
                            } else {
                                vm.to_string_value(replace_arg)
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
                    let search_str = vm.to_string_value(search_arg);
                    if let Some(pos) = this_str.find(&search_str) {
                        let before = &this_str[..pos];
                        let after = &this_str[pos + search_str.len()..];
                        let replacement = if is_replace_fn {
                            let arg0 = vm.alloc_string(search_str.clone());
                            let arg1 = JsValue::from_int32(pos as i32);
                            let arg2 = vm.alloc_string(this_str.clone());
                            let call_res = vm.call(replace_arg, JsValue::undefined(), &[arg0, arg1, arg2])?;
                            vm.to_string_value(call_res)
                        } else {
                            vm.to_string_value(replace_arg)
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
                if this.is_nullish() {
                    return Err(VmError::TypeError("String.prototype.at called on null or undefined".into()));
                }
                let s = vm.to_string_value(this);
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
                let this_str = vm.to_string_value(this);
                let search_arg = args.first().copied().unwrap_or(JsValue::undefined());
                let replace_arg = args.get(1).copied().unwrap_or(JsValue::undefined());

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
                                vm.to_string_value(call_res)
                            } else {
                                vm.to_string_value(replace_arg)
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
                                vm.to_string_value(call_res)
                            } else {
                                vm.to_string_value(replace_arg)
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
                    let search_str = vm.to_string_value(search_arg);
                    let mut result = String::new();
                    let mut search_from = 0;
                    while search_from <= this_str.len() {
                        if search_str.is_empty() {
                            let replacement = if is_replace_fn {
                                let arg0 = vm.alloc_string("".to_string());
                                let arg1 = JsValue::from_int32(search_from as i32);
                                let arg2 = vm.alloc_string(this_str.clone());
                                let call_res = vm.call(replace_arg, JsValue::undefined(), &[arg0, arg1, arg2])?;
                                vm.to_string_value(call_res)
                            } else {
                                vm.to_string_value(replace_arg)
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
                                vm.to_string_value(call_res)
                            } else {
                                vm.to_string_value(replace_arg)
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
                let this_str = vm.to_string_value(this);
                let re_arg = args.first().copied().unwrap_or(JsValue::undefined());
                let re = if let Some(ptr) = re_arg.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    if matches!(obj.kind, ObjectKind::RegExp { .. }) {
                        re_arg
                    } else {
                        let s = if re_arg.is_undefined() { "".to_string() } else { vm.to_string_value(re_arg) };
                        vm.alloc_regexp(s, "".to_string())
                    }
                } else {
                    let s = if re_arg.is_undefined() { "".to_string() } else { vm.to_string_value(re_arg) };
                    vm.alloc_regexp(s, "".to_string())
                };
                let exec_fn = vm.get_property(re, "exec");
                let this_str_val = vm.alloc_string(this_str);
                vm.call(exec_fn, re, &[this_str_val])
            })),
            "search" => Some(self.alloc_native_function("search", 1, |vm, this, args| {
                let this_str = vm.to_string_value(this);
                let re_arg = args.first().copied().unwrap_or(JsValue::undefined());
                let re = if let Some(ptr) = re_arg.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    if matches!(obj.kind, ObjectKind::RegExp { .. }) {
                        re_arg
                    } else {
                        let s = if re_arg.is_undefined() { "".to_string() } else { vm.to_string_value(re_arg) };
                        vm.alloc_regexp(s, "".to_string())
                    }
                } else {
                    let s = if re_arg.is_undefined() { "".to_string() } else { vm.to_string_value(re_arg) };
                    vm.alloc_regexp(s, "".to_string())
                };
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
                let target = args.first().map(|v| vm.to_string_value(*v)).unwrap_or_default();
                let this_str = vm.to_string_value(this);
                if let Some(idx) = this_str.rfind(&target) {
                    Ok(JsValue::from_int32(idx as i32))
                } else {
                    Ok(JsValue::from_int32(-1))
                }
            })),
            "split" => Some(self.alloc_native_function("split", 2, |vm, this, args| {
                let this_str = vm.to_string_value(this);
                let sep = args.first().map(|v| vm.to_string_value(*v)).unwrap_or_default();
                let limit = args.get(1).and_then(|v| v.to_int32()).map(|n| n as usize);
                let parts: Vec<JsValue> = if sep.is_empty() {
                    this_str.chars().map(|c| vm.alloc_string(c.to_string())).collect()
                } else {
                    this_str.split(&sep).map(|p| vm.alloc_string(p.to_string())).collect()
                };
                let parts = if let Some(lim) = limit {
                    parts.into_iter().take(lim).collect()
                } else {
                    parts
                };
                Ok(vm.alloc_array(parts))
            })),
            "substring" => Some(self.alloc_native_function("substring", 2, |vm, this, args| {
                let s = vm.to_string_value(this);
                let u16_chars: Vec<u16> = s.encode_utf16().collect();
                let len = u16_chars.len() as i32;
                let mut start = args.first().and_then(|v| v.to_int32()).unwrap_or(0).clamp(0, len);
                let mut end = args.get(1).and_then(|v| v.to_int32()).unwrap_or(len).clamp(0, len);
                if start > end {
                    std::mem::swap(&mut start, &mut end);
                }
                let sub = String::from_utf16_lossy(&u16_chars[start as usize..end as usize]);
                Ok(vm.alloc_string(sub))
            })),
            "concat" => Some(self.alloc_native_function("concat", 1, |vm, this, args| {
                let mut res = vm.to_string_value(this);
                for arg in args {
                    res.push_str(&vm.to_string_value(*arg));
                }
                Ok(vm.alloc_string(res))
            })),
            "startsWith" => Some(self.alloc_native_function("startsWith", 1, |vm, this, args| {
                let this_str = vm.to_string_value(this);
                let search = args.first().map(|v| vm.to_string_value(*v)).unwrap_or_default();
                let pos = args.get(1).and_then(|v| v.to_int32()).unwrap_or(0).max(0) as usize;
                let starts = if pos <= this_str.len() {
                    this_str[pos..].starts_with(&search)
                } else {
                    false
                };
                Ok(JsValue::from_boolean(starts))
            })),
            "endsWith" => Some(self.alloc_native_function("endsWith", 1, |vm, this, args| {
                let this_str = vm.to_string_value(this);
                let search = args.first().map(|v| vm.to_string_value(*v)).unwrap_or_default();
                let end = args.get(1).and_then(|v| v.to_int32()).unwrap_or(this_str.len() as i32).clamp(0, this_str.len() as i32) as usize;
                let ends = this_str[..end].ends_with(&search);
                Ok(JsValue::from_boolean(ends))
            })),
            "repeat" => Some(self.alloc_native_function("repeat", 1, |vm, this, args| {
                let this_str = vm.to_string_value(this);
                let count_f = args.first().and_then(|v| vm.to_number_value(*v).ok()).unwrap_or(0.0);
                if count_f < 0.0 || count_f.is_infinite() {
                    return Err(VmError::RangeError("Invalid count value".into()));
                }
                let count = count_f as usize;
                Ok(vm.alloc_string(this_str.repeat(count)))
            })),
            "padStart" => Some(self.alloc_native_function("padStart", 1, |vm, this, args| {
                let this_str = vm.to_string_value(this);
                let this_u16: Vec<u16> = this_str.encode_utf16().collect();
                let target_len = args.first().and_then(|v| v.to_int32()).unwrap_or(0).max(0) as usize;
                let pad_str = args.get(1).map(|v| vm.to_string_value(*v)).unwrap_or_else(|| " ".to_string());
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
                let this_str = vm.to_string_value(this);
                let mut this_u16: Vec<u16> = this_str.encode_utf16().collect();
                let target_len = args.first().and_then(|v| v.to_int32()).unwrap_or(0).max(0) as usize;
                let pad_str = args.get(1).map(|v| vm.to_string_value(*v)).unwrap_or_else(|| " ".to_string());
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
                let a = vm.to_string_value(this);
                let b = args.first().map(|v| vm.to_string_value(*v)).unwrap_or_default();
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
