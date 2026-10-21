//! The `RegExp` global and `RegExp.prototype`.

use super::*;
use crate::interpreter::PreferredType;

pub fn is_regexp(vm: &mut Vm, val: JsValue) -> Result<bool, VmError> {
    if let Some(sym_match) = vm.symbol_match {
        if val.is_js_object() {
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

fn utf16_slice(s: &str, start_u16: usize, end_u16: usize) -> String {
    let mut curr_u16 = 0;
    let mut out = String::new();
    for ch in s.chars() {
        let u16_len = ch.len_utf16();
        if curr_u16 + u16_len > start_u16 && curr_u16 < end_u16 {
            out.push(ch);
        }
        curr_u16 += u16_len;
        if curr_u16 >= end_u16 {
            break;
        }
    }
    out
}

fn utf16_len(s: &str) -> usize {
    s.encode_utf16().count()
}

fn to_uint32_val(vm: &mut Vm, val: JsValue) -> Result<u32, VmError> {
    let num = vm.to_number_value(val)?;
    if num.is_nan() || num.is_infinite() || num == 0.0 {
        Ok(0)
    } else {
        let int_val = num.trunc() as i64;
        Ok(int_val.rem_euclid(1i64 << 32) as u32)
    }
}

fn get_substitution(
    vm: &mut Vm,
    matched: &str,
    str_val: &str,
    position: usize,
    captures: &[JsValue],
    named_captures: Option<JsValue>,
    template: &str,
) -> Result<String, VmError> {
    let mut result = String::with_capacity(template.len());
    let chars: Vec<char> = template.chars().collect();
    let mut i = 0;
    let n_captures = captures.len();
    let length_s = utf16_len(str_val);
    let match_length = utf16_len(matched);

    while i < chars.len() {
        let ch = chars[i];
        if ch == '$' && i + 1 < chars.len() {
            let next = chars[i + 1];
            match next {
                '$' => {
                    result.push('$');
                    i += 2;
                }
                '&' => {
                    result.push_str(matched);
                    i += 2;
                }
                '`' => {
                    let before = utf16_slice(str_val, 0, position);
                    result.push_str(&before);
                    i += 2;
                }
                '\'' => {
                    let tail_index = (position + match_length).min(length_s);
                    let after = utf16_slice(str_val, tail_index, length_s);
                    result.push_str(&after);
                    i += 2;
                }
                '0'..='9' => {
                    let d1 = (next as u32 - '0' as u32) as usize;
                    if i + 2 < chars.len() && chars[i + 2].is_ascii_digit() {
                        let d2 = (chars[i + 2] as u32 - '0' as u32) as usize;
                        let two_digit = d1 * 10 + d2;
                        if two_digit > 0 && two_digit <= n_captures {
                            let cap = captures[two_digit - 1];
                            if !cap.is_undefined() {
                                result.push_str(&vm.to_string_spec(cap)?);
                            }
                            i += 3;
                            continue;
                        }
                    }
                    if d1 > 0 && d1 <= n_captures {
                        let cap = captures[d1 - 1];
                        if !cap.is_undefined() {
                            result.push_str(&vm.to_string_spec(cap)?);
                        }
                        i += 2;
                    } else {
                        result.push('$');
                        result.push(next);
                        i += 2;
                    }
                }
                '<' => {
                    if let Some(named_caps) = named_captures {
                        if !named_caps.is_undefined() {
                            if let Some(close_idx) = chars[i + 2..].iter().position(|&c| c == '>') {
                                let group_name: String =
                                    chars[i + 2..i + 2 + close_idx].iter().collect();
                                let cap_val = vm.get_property_result(named_caps, &group_name)?;
                                if !cap_val.is_undefined() {
                                    result.push_str(&vm.to_string_spec(cap_val)?);
                                }
                                i += 2 + close_idx + 1;
                                continue;
                            }
                        }
                    }
                    result.push('$');
                    result.push('<');
                    i += 2;
                }
                _ => {
                    result.push('$');
                    result.push(next);
                    i += 2;
                }
            }
        } else {
            result.push(ch);
            i += 1;
        }
    }
    Ok(result)
}

pub(super) fn register_regexp_builtins(vm: &mut Vm) {
    // RegExp prototype and constructor
    let regexp_proto = vm.alloc_object();
    vm.regexp_proto = Some(regexp_proto);

    let regexp_ctor = vm.alloc_native_function("RegExp", 2, |vm, _this, args| {
        let pattern_arg = args.first().copied().unwrap_or(JsValue::undefined());
        let flags_arg = args.get(1).copied().unwrap_or(JsValue::undefined());
        let pattern_is_re = is_regexp(vm, pattern_arg)?;

        let (pattern, flags): (String, String) = if pattern_is_re {
            if let Some(ptr) = pattern_arg.as_object_ptr() {
                let obj = unsafe { &*(ptr as *mut JsObject) };
                if let ObjectKind::RegExp { pattern: p, flags: f } = &obj.kind {
                    if flags_arg.is_undefined() {
                        if !vm.is_constructing {
                            return Ok(pattern_arg);
                        }
                        return Ok(vm.alloc_regexp(p.clone(), f.clone()));
                    }
                }
            }
            let p_source = vm.get_property_result(pattern_arg, "source")?;
            let pattern_str =
                if p_source.is_undefined() { String::new() } else { vm.to_string_spec(p_source)? };
            let flags_str = if flags_arg.is_undefined() {
                let p_flags = vm.get_property_result(pattern_arg, "flags")?;
                if p_flags.is_undefined() { String::new() } else { vm.to_string_spec(p_flags)? }
            } else {
                vm.to_string_spec(flags_arg)?
            };
            (pattern_str, flags_str)
        } else {
            let pattern_str = if pattern_arg.is_undefined() {
                String::new()
            } else {
                vm.to_string_spec(pattern_arg)?
            };
            let flags_str = if flags_arg.is_undefined() {
                String::new()
            } else {
                vm.to_string_spec(flags_arg)?
            };
            (pattern_str, flags_str)
        };

        // Validate flags: allowed characters and duplicate check
        let mut seen = [false; 256];
        for ch in flags.chars() {
            if !matches!(ch, 'd' | 'g' | 'i' | 'm' | 's' | 'u' | 'v' | 'y') {
                return Err(VmError::SyntaxError(format!(
                    "Invalid regular expression flag: {}",
                    ch
                )));
            }
            let b = ch as usize;
            if b < 256 {
                if seen[b] {
                    return Err(VmError::SyntaxError(format!(
                        "Duplicate regular expression flag: {}",
                        ch
                    )));
                }
                seen[b] = true;
            }
        }
        if seen[b'u' as usize] && seen[b'v' as usize] {
            return Err(VmError::SyntaxError(
                "Invalid regular expression flags: 'u' and 'v' cannot both be present".into(),
            ));
        }

        // Validate pattern and flags via regress
        if let Err(e) = regress::Regex::with_flags(&pattern, flags.as_str()) {
            return Err(VmError::SyntaxError(format!("Invalid regular expression: {}", e)));
        }

        Ok(vm.alloc_regexp(pattern, flags))
    });
    vm.set_property(regexp_ctor, "prototype", regexp_proto);
    vm.set_property_non_enumerable(regexp_ctor, "prototype".to_string());
    vm.set_property_non_writable(regexp_ctor, "prototype".to_string());
    vm.set_property_non_configurable(regexp_ctor, "prototype".to_string());
    vm.set_property(regexp_proto, "constructor", regexp_ctor);
    vm.set_property_non_enumerable(regexp_proto, "constructor".to_string());

    let re_exec_fn = vm.alloc_native_function("exec", 1, |vm, this, args| {
        let text_arg = args.first().copied().unwrap_or(JsValue::undefined());
        let text_prim = vm.to_primitive(text_arg, PreferredType::String)?;
        let text = vm.to_string_value(text_prim);
        reg_exp_builtin_exec(vm, this, &text)
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
        Err(VmError::TypeError(
            "Method RegExp.prototype.global called on incompatible receiver".into(),
        ))
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
        Err(VmError::TypeError(
            "Method RegExp.prototype.ignoreCase called on incompatible receiver".into(),
        ))
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
        Err(VmError::TypeError(
            "Method RegExp.prototype.multiline called on incompatible receiver".into(),
        ))
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
        Err(VmError::TypeError(
            "Method RegExp.prototype.dotAll called on incompatible receiver".into(),
        ))
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
        Err(VmError::TypeError(
            "Method RegExp.prototype.unicode called on incompatible receiver".into(),
        ))
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
        Err(VmError::TypeError(
            "Method RegExp.prototype.sticky called on incompatible receiver".into(),
        ))
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
        Err(VmError::TypeError(
            "Method RegExp.prototype.hasIndices called on incompatible receiver".into(),
        ))
    });

    let re_unicode_sets_getter = vm.alloc_native_function("get unicodeSets", 0, |vm, this, _| {
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::RegExp { flags, .. } = &obj.kind {
                return Ok(JsValue::from_boolean(flags.contains('v')));
            }
            if Some(this) == vm.regexp_proto {
                return Ok(JsValue::undefined());
            }
        }
        Err(VmError::TypeError(
            "Method RegExp.prototype.unicodeSets called on incompatible receiver".into(),
        ))
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
        Err(VmError::TypeError(
            "Method RegExp.prototype.source called on incompatible receiver".into(),
        ))
    });

    // ES2024 22.2.6.4 get RegExp.prototype.flags (generic receiver)
    let re_flags_getter = vm.alloc_native_function("get flags", 0, |vm, this, _| {
        if !this.is_object() {
            return Err(VmError::TypeError(
                "Method RegExp.prototype.flags called on incompatible receiver".into(),
            ));
        }
        let mut flags_str = String::new();
        if vm.get_property(this, "hasIndices").is_truthy() {
            flags_str.push('d');
        }
        if vm.get_property(this, "global").is_truthy() {
            flags_str.push('g');
        }
        if vm.get_property(this, "ignoreCase").is_truthy() {
            flags_str.push('i');
        }
        if vm.get_property(this, "multiline").is_truthy() {
            flags_str.push('m');
        }
        if vm.get_property(this, "dotAll").is_truthy() {
            flags_str.push('s');
        }
        if vm.get_property(this, "unicode").is_truthy() {
            flags_str.push('u');
        }
        if vm.get_property(this, "unicodeSets").is_truthy() {
            flags_str.push('v');
        }
        if vm.get_property(this, "sticky").is_truthy() {
            flags_str.push('y');
        }
        Ok(vm.alloc_string(flags_str))
    });

    // Annex B.2.5.1 RegExp.prototype.compile ( pattern, flags )
    let re_compile_fn = vm.alloc_native_function("compile", 2, |vm, this, args| {
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::RegExp { ref mut pattern, ref mut flags } = obj.kind {
                let (new_pat, new_flags) = if let Some(p_arg) = args.first() {
                    if let Some(p_ptr) = p_arg.as_object_ptr() {
                        let p_obj = unsafe { &*(p_ptr as *mut JsObject) };
                        if let ObjectKind::RegExp { pattern: p_pat, flags: p_fl } = &p_obj.kind {
                            if args.get(1).map_or(false, |v| !v.is_undefined()) {
                                return Err(VmError::TypeError(
                                    "Cannot supply flags when pattern is a RegExp".into(),
                                ));
                            }
                            (p_pat.clone(), p_fl.clone())
                        } else {
                            (
                                if p_arg.is_undefined() {
                                    String::new()
                                } else {
                                    vm.to_string_value(*p_arg)
                                },
                                args.get(1)
                                    .map(|v| {
                                        if v.is_undefined() {
                                            String::new()
                                        } else {
                                            vm.to_string_value(*v)
                                        }
                                    })
                                    .unwrap_or_default(),
                            )
                        }
                    } else {
                        (
                            if p_arg.is_undefined() {
                                String::new()
                            } else {
                                vm.to_string_value(*p_arg)
                            },
                            args.get(1)
                                .map(|v| {
                                    if v.is_undefined() {
                                        String::new()
                                    } else {
                                        vm.to_string_value(*v)
                                    }
                                })
                                .unwrap_or_default(),
                        )
                    }
                } else {
                    (String::new(), String::new())
                };

                let mut seen = [false; 256];
                for ch in new_flags.chars() {
                    if !matches!(ch, 'd' | 'g' | 'i' | 'm' | 's' | 'u' | 'v' | 'y') {
                        return Err(VmError::SyntaxError(format!(
                            "Invalid regular expression flag: {}",
                            ch
                        )));
                    }
                    let b = ch as usize;
                    if b < 256 {
                        if seen[b] {
                            return Err(VmError::SyntaxError(format!(
                                "Duplicate regular expression flag: {}",
                                ch
                            )));
                        }
                        seen[b] = true;
                    }
                }
                if seen[b'u' as usize] && seen[b'v' as usize] {
                    return Err(VmError::SyntaxError(
                        "Invalid regular expression flags: 'u' and 'v' cannot both be present"
                            .into(),
                    ));
                }

                if let Err(e) = regress::Regex::with_flags(&new_pat, new_flags.as_str()) {
                    return Err(VmError::SyntaxError(format!("Invalid regular expression: {}", e)));
                }

                *pattern = new_pat;
                *flags = new_flags;
                vm.set_property(this, "lastIndex", JsValue::from_int32(0));
                return Ok(this);
            }
        }
        Err(VmError::TypeError("RegExp.prototype.compile called on incompatible receiver".into()))
    });

    vm.set_property(regexp_proto, "exec", re_exec_fn);
    vm.set_property(regexp_proto, "test", re_test_fn);
    vm.set_property(regexp_proto, "toString", re_to_string_fn);

    // @@match ( string )
    let re_match_fn = vm.alloc_native_function("[Symbol.match]", 1, |vm, this, args| {
        let text_arg = args.first().copied().unwrap_or(JsValue::undefined());
        let text_prim = vm.to_primitive(text_arg, PreferredType::String)?;
        let text = vm.to_string_value(text_prim);
        if !this.is_object() {
            return Err(VmError::TypeError(
                "RegExp.prototype[Symbol.match] called on non-object".into(),
            ));
        }
        let global = vm.get_property(this, "global").is_truthy();
        if !global {
            let text_val = vm.alloc_string(text);
            return vm.call_method(this, "exec", &[text_val]);
        }
        let ok = vm.set_property_with_receiver(this, "lastIndex", JsValue::from_int32(0), this)?;
        if !ok {
            return Err(VmError::TypeError(
                "Cannot assign to read-only property 'lastIndex'".into(),
            ));
        }
        let prev_temp = vm.temp_roots.len();
        vm.temp_roots.push(this);
        let mut matches = Vec::new();
        while matches.len() < 50_000 {
            let cur_this = vm.temp_roots[prev_temp];
            let text_val = vm.alloc_string(text.clone());
            let res = vm.call_method(cur_this, "exec", &[text_val])?;
            if res.is_null() {
                break;
            }
            let match_str_val = vm.get_property(res, "0");
            let prim = vm.to_primitive(match_str_val, PreferredType::String)?;
            let s = vm.to_string_value(prim);
            let cur_this = vm.temp_roots[prev_temp];
            if s.is_empty() {
                let this_index = vm.get_property(cur_this, "lastIndex").as_int32().unwrap_or(0);
                let ok = vm.set_property_with_receiver(
                    cur_this,
                    "lastIndex",
                    JsValue::from_int32(this_index + 1),
                    cur_this,
                )?;
                if !ok {
                    vm.temp_roots.truncate(prev_temp);
                    return Err(VmError::TypeError(
                        "Cannot assign to read-only property 'lastIndex'".into(),
                    ));
                }
            }
            let str_val = vm.alloc_string(s);
            matches.push(str_val);
        }
        vm.temp_roots.truncate(prev_temp);
        if matches.is_empty() { Ok(JsValue::null()) } else { Ok(vm.alloc_array(matches)) }
    });

    // @@replace ( string, replaceValue )
    let re_replace_fn = vm.alloc_native_function("[Symbol.replace]", 2, |vm, this, args| {
        let string_arg = args.first().copied().unwrap_or(JsValue::undefined());
        let replace_val = args.get(1).copied().unwrap_or(JsValue::undefined());
        if !this.is_js_object() {
            return Err(VmError::TypeError(
                "RegExp.prototype[Symbol.replace] called on non-object".into(),
            ));
        }
        let s = vm.to_string_spec(string_arg)?;
        let length_s = utf16_len(&s);
        let functional_replace = vm.is_callable(replace_val);
        let replace_str =
            if functional_replace { String::new() } else { vm.to_string_spec(replace_val)? };

        let global = vm.get_property_result(this, "global")?.is_truthy();
        let full_unicode = if global {
            let u = vm.get_property_result(this, "unicode")?.is_truthy();
            let _ = vm.set_property_result(this, "lastIndex", JsValue::from_int32(0))?;
            u
        } else {
            false
        };

        let mut results = Vec::new();
        let s_val = vm.alloc_string(s.clone());

        loop {
            let result = reg_exp_exec(vm, this, &s)?;
            if result.is_null() {
                break;
            }
            results.push(result);
            if !global {
                break;
            }
            let match_str_val = vm.get_property_result(result, "0")?;
            let match_str = vm.to_string_spec(match_str_val)?;
            if match_str.is_empty() {
                let this_index_val = vm.get_property_result(this, "lastIndex")?;
                let this_index = vm.to_length(this_index_val)?;
                let next_index = advance_string_index(&s, this_index, full_unicode);
                let _ = vm.set_property_result(
                    this,
                    "lastIndex",
                    JsValue::from_double(next_index as f64),
                )?;
            }
        }

        let mut accumulated_result = String::new();
        let mut next_source_position = 0;

        for result in results {
            let n_captures_val = vm.get_property_result(result, "length")?;
            let len = vm.to_length(n_captures_val)?;
            let n_captures = len.saturating_sub(1);

            let matched_val = vm.get_property_result(result, "0")?;
            let matched = vm.to_string_spec(matched_val)?;
            let match_length = utf16_len(&matched);

            let pos_val = vm.get_property_result(result, "index")?;
            let position = if let Some(i) = pos_val.as_int32() {
                (i.max(0) as usize).min(length_s)
            } else if let Some(n) = pos_val.as_double() {
                (n.max(0.0) as usize).min(length_s)
            } else {
                0
            };

            let mut captures = Vec::with_capacity(n_captures);
            for n in 1..=n_captures {
                let cap_n = vm.get_property_result(result, &n.to_string())?;
                captures.push(cap_n);
            }

            let named_captures = vm.get_property_result(result, "groups")?;
            let named_caps_opt =
                if named_captures.is_undefined() { None } else { Some(named_captures) };

            let replacement = if functional_replace {
                let mut replacer_args = Vec::with_capacity(captures.len() + 4);
                replacer_args.push(matched_val);
                replacer_args.extend_from_slice(&captures);
                replacer_args.push(JsValue::from_int32(position as i32));
                replacer_args.push(s_val);
                if let Some(nc) = named_caps_opt {
                    replacer_args.push(nc);
                }
                let repl_res = vm.call(replace_val, JsValue::undefined(), &replacer_args)?;
                vm.to_string_spec(repl_res)?
            } else {
                get_substitution(
                    vm,
                    &matched,
                    &s,
                    position,
                    &captures,
                    named_caps_opt,
                    &replace_str,
                )?
            };

            if position >= next_source_position {
                let prefix = utf16_slice(&s, next_source_position, position);
                accumulated_result.push_str(&prefix);
                accumulated_result.push_str(&replacement);
                next_source_position = position + match_length;
            }
        }

        if next_source_position < length_s {
            let suffix = utf16_slice(&s, next_source_position, length_s);
            accumulated_result.push_str(&suffix);
        }

        Ok(vm.alloc_string(accumulated_result))
    });

    // @@search ( string )
    let re_search_fn = vm.alloc_native_function("[Symbol.search]", 1, |vm, this, args| {
        let string_arg = args.first().copied().unwrap_or(JsValue::undefined());
        let str_prim = vm.to_primitive(string_arg, PreferredType::String)?;
        let string_val = vm.to_string_value(str_prim);
        if !this.is_object() {
            return Err(VmError::TypeError(
                "RegExp.prototype[Symbol.search] called on non-object".into(),
            ));
        }
        let _ = vm.set_property_result(this, "lastIndex", JsValue::from_int32(0));
        let prev_temp = vm.temp_roots.len();
        vm.temp_roots.push(this);
        let str_arg = vm.alloc_string(string_val);
        let cur_this = vm.temp_roots[prev_temp];
        let res = vm.call_method(cur_this, "exec", &[str_arg])?;
        vm.temp_roots.truncate(prev_temp);
        if res.is_null() {
            Ok(JsValue::from_int32(-1))
        } else {
            let index_val = vm.get_property(res, "index");
            Ok(index_val)
        }
    });

    // @@split ( string, limit )
    let re_split_fn = vm.alloc_native_function("[Symbol.split]", 2, |vm, this, args| {
        let string_arg = args.first().copied().unwrap_or(JsValue::undefined());
        let limit_arg = args.get(1).copied().unwrap_or(JsValue::undefined());
        if !this.is_js_object() {
            return Err(VmError::TypeError(
                "RegExp.prototype[Symbol.split] called on non-object".into(),
            ));
        }
        let s = vm.to_string_spec(string_arg)?;
        let size = utf16_len(&s);

        let default_ctor = vm.globals.get("RegExp").copied().unwrap_or(JsValue::undefined());
        let c = vm.species_constructor(this, default_ctor)?;

        let flags_val = vm.get_property_result(this, "flags")?;
        let flags = vm.to_string_spec(flags_val)?;
        let unicode_matching = flags.contains('u') || flags.contains('v');
        let new_flags = if flags.contains('y') { flags } else { format!("{}y", flags) };

        let new_flags_val = vm.alloc_string(new_flags);
        let splitter = vm.construct(c, &[this, new_flags_val])?;

        let lim = if limit_arg.is_undefined() { u32::MAX } else { to_uint32_val(vm, limit_arg)? };

        if lim == 0 {
            return Ok(vm.alloc_array(Vec::new()));
        }

        let s_val = vm.alloc_string(s.clone());

        if size == 0 {
            let z = reg_exp_exec(vm, splitter, &s)?;
            if !z.is_null() {
                return Ok(vm.alloc_array(Vec::new()));
            }
            return Ok(vm.alloc_array(vec![s_val]));
        }

        let mut a = Vec::new();
        let mut p = 0;
        let mut q = p;

        while q < size {
            let _ = vm.set_property_result(splitter, "lastIndex", JsValue::from_int32(q as i32))?;
            let z = reg_exp_exec(vm, splitter, &s)?;
            if z.is_null() {
                q = advance_string_index(&s, q, unicode_matching);
            } else {
                let e_val = vm.get_property_result(splitter, "lastIndex")?;
                let e = vm.to_length(e_val)?.min(size);
                if e == p {
                    q = advance_string_index(&s, q, unicode_matching);
                } else {
                    let sub = utf16_slice(&s, p, q);
                    a.push(vm.alloc_string(sub));
                    if a.len() == lim as usize {
                        return Ok(vm.alloc_array(a));
                    }
                    p = e;

                    let len_val = vm.get_property_result(z, "length")?;
                    let n_captures = vm.to_length(len_val)?.saturating_sub(1);
                    for i in 1..=n_captures {
                        let next_cap = vm.get_property_result(z, &i.to_string())?;
                        a.push(next_cap);
                        if a.len() == lim as usize {
                            return Ok(vm.alloc_array(a));
                        }
                    }
                    q = p;
                }
            }
        }

        let rem = utf16_slice(&s, p, size);
        a.push(vm.alloc_string(rem));
        Ok(vm.alloc_array(a))
    });

    // @@matchAll ( string )
    let re_match_all_fn = vm.alloc_native_function("[Symbol.matchAll]", 1, |vm, this, args| {
        if !this.is_object() {
            return Err(VmError::TypeError(
                "RegExp.prototype[Symbol.matchAll] called on non-object".into(),
            ));
        }
        let string_arg = args.first().copied().unwrap_or(JsValue::undefined());
        let s_prim = vm.to_primitive(string_arg, PreferredType::String)?;
        let s = vm.to_string_value(s_prim);

        let default_ctor = vm.globals.get("RegExp").copied().unwrap_or(JsValue::undefined());
        let c = vm.species_constructor(this, default_ctor)?;

        let flags_val = vm.get_property_result(this, "flags")?;
        let flags_prim = vm.to_primitive(flags_val, PreferredType::String)?;
        let flags = vm.to_string_value(flags_prim);

        let flags_js = vm.alloc_string(flags.clone());
        let matcher = vm.construct(c, &[this, flags_js])?;

        let last_index_val = vm.get_property_result(this, "lastIndex")?;
        let last_index = vm.to_length(last_index_val)?;
        let last_idx_js = if last_index <= i32::MAX as usize {
            JsValue::from_int32(last_index as i32)
        } else {
            JsValue::from_double(last_index as f64)
        };
        let ok = vm.set_property_with_receiver(matcher, "lastIndex", last_idx_js, matcher)?;
        if !ok {
            return Err(VmError::TypeError("Cannot set lastIndex on matcher".into()));
        }

        let global = flags.contains('g');
        let full_unicode = flags.contains('u') || flags.contains('v');

        Ok(vm.alloc_regexp_string_iterator(matcher, s, global, full_unicode))
    });

    let non_enum_attrs =
        crate::shape::PropertyAttributes { writable: true, enumerable: false, configurable: true };
    if let Some(sym) = vm.symbol_match {
        vm.define_symbol_property(regexp_proto, sym, re_match_fn, non_enum_attrs);
        let key_name = format!("[Symbol:{:x}]", sym.raw_bits());
        vm.set_property_non_enumerable(regexp_proto, key_name);
    }
    if let Some(sym) = vm.symbol_match_all {
        vm.define_symbol_property(regexp_proto, sym, re_match_all_fn, non_enum_attrs);
        let key_name = format!("[Symbol:{:x}]", sym.raw_bits());
        vm.set_property_non_enumerable(regexp_proto, key_name);
    }
    if let Some(sym) = vm.symbol_replace {
        vm.define_symbol_property(regexp_proto, sym, re_replace_fn, non_enum_attrs);
        let key_name = format!("[Symbol:{:x}]", sym.raw_bits());
        vm.set_property_non_enumerable(regexp_proto, key_name);
    }
    if let Some(sym) = vm.symbol_search {
        vm.define_symbol_property(regexp_proto, sym, re_search_fn, non_enum_attrs);
        let key_name = format!("[Symbol:{:x}]", sym.raw_bits());
        vm.set_property_non_enumerable(regexp_proto, key_name);
    }
    if let Some(sym) = vm.symbol_split {
        vm.define_symbol_property(regexp_proto, sym, re_split_fn, non_enum_attrs);
        let key_name = format!("[Symbol:{:x}]", sym.raw_bits());
        vm.set_property_non_enumerable(regexp_proto, key_name);
    }

    vm.set_property(regexp_proto, "compile", re_compile_fn);

    if let Some(ptr) = regexp_proto.as_object_ptr() {
        let obj = unsafe { &mut *(ptr as *mut JsObject) };
        obj.define_getter("global".to_string(), re_global_getter);
        obj.define_getter("ignoreCase".to_string(), re_ignore_case_getter);
        obj.define_getter("multiline".to_string(), re_multiline_getter);
        obj.define_getter("dotAll".to_string(), re_dot_all_getter);
        obj.define_getter("unicode".to_string(), re_unicode_getter);
        obj.define_getter("unicodeSets".to_string(), re_unicode_sets_getter);
        obj.define_getter("sticky".to_string(), re_sticky_getter);
        obj.define_getter("hasIndices".to_string(), re_has_indices_getter);
        obj.define_getter("source".to_string(), re_source_getter);
        obj.define_getter("flags".to_string(), re_flags_getter);
    }

    for prop in [
        "global",
        "ignoreCase",
        "multiline",
        "dotAll",
        "unicode",
        "unicodeSets",
        "sticky",
        "hasIndices",
        "source",
        "flags",
        "exec",
        "test",
        "toString",
        "compile",
        "constructor",
    ] {
        vm.set_property_non_enumerable(regexp_proto, prop.to_string());
    }

    // Annex B.2.5.2 - B.2.5.8 Legacy RegExp constructor properties
    if let Some(ptr) = regexp_ctor.as_object_ptr() {
        let ctor_obj = unsafe { &mut *(ptr as *mut JsObject) };

        macro_rules! define_paren {
            ($idx:expr, $name:expr) => {{
                let getter = vm.alloc_native_function(concat!("get ", $name), 0, |vm, _this, _| {
                    Ok(vm.alloc_string(vm.regexp_last_match.parens[$idx].clone()))
                });
                ctor_obj.define_getter($name.to_string(), getter);
                vm.set_property_non_enumerable(regexp_ctor, $name.to_string());
            }};
        }

        define_paren!(0, "$1");
        define_paren!(1, "$2");
        define_paren!(2, "$3");
        define_paren!(3, "$4");
        define_paren!(4, "$5");
        define_paren!(5, "$6");
        define_paren!(6, "$7");
        define_paren!(7, "$8");
        define_paren!(8, "$9");

        // input / $_
        let input_getter = vm.alloc_native_function("get input", 0, |vm, _this, _| {
            Ok(vm.alloc_string(vm.regexp_last_match.input.clone()))
        });
        let input_setter = vm.alloc_native_function("set input", 1, |vm, _this, args| {
            if let Some(v) = args.first() {
                vm.regexp_last_match.input = vm.to_string_value(*v);
            }
            Ok(JsValue::undefined())
        });
        ctor_obj.define_getter("input".to_string(), input_getter);
        ctor_obj.define_setter("input".to_string(), input_setter);
        ctor_obj.define_getter("$_".to_string(), input_getter);
        ctor_obj.define_setter("$_".to_string(), input_setter);
        vm.set_property_non_enumerable(regexp_ctor, "input".to_string());
        vm.set_property_non_enumerable(regexp_ctor, "$_".to_string());

        // lastMatch / $&
        let last_match_getter = vm.alloc_native_function("get lastMatch", 0, |vm, _this, _| {
            Ok(vm.alloc_string(vm.regexp_last_match.last_match.clone()))
        });
        ctor_obj.define_getter("lastMatch".to_string(), last_match_getter);
        ctor_obj.define_getter("$&".to_string(), last_match_getter);
        vm.set_property_non_enumerable(regexp_ctor, "lastMatch".to_string());
        vm.set_property_non_enumerable(regexp_ctor, "$&".to_string());

        // lastParen / $+
        let last_paren_getter = vm.alloc_native_function("get lastParen", 0, |vm, _this, _| {
            Ok(vm.alloc_string(vm.regexp_last_match.last_paren.clone()))
        });
        ctor_obj.define_getter("lastParen".to_string(), last_paren_getter);
        ctor_obj.define_getter("$+".to_string(), last_paren_getter);
        vm.set_property_non_enumerable(regexp_ctor, "lastParen".to_string());
        vm.set_property_non_enumerable(regexp_ctor, "$+".to_string());

        // leftContext / $`
        let left_context_getter = vm.alloc_native_function("get leftContext", 0, |vm, _this, _| {
            Ok(vm.alloc_string(vm.regexp_last_match.left_context.clone()))
        });
        ctor_obj.define_getter("leftContext".to_string(), left_context_getter);
        ctor_obj.define_getter("$`".to_string(), left_context_getter);
        vm.set_property_non_enumerable(regexp_ctor, "leftContext".to_string());
        vm.set_property_non_enumerable(regexp_ctor, "$`".to_string());

        // rightContext / $'
        let right_context_getter =
            vm.alloc_native_function("get rightContext", 0, |vm, _this, _| {
                Ok(vm.alloc_string(vm.regexp_last_match.right_context.clone()))
            });
        ctor_obj.define_getter("rightContext".to_string(), right_context_getter);
        ctor_obj.define_getter("$'".to_string(), right_context_getter);
        vm.set_property_non_enumerable(regexp_ctor, "rightContext".to_string());
        vm.set_property_non_enumerable(regexp_ctor, "$'".to_string());
    }

    helpers::add_species_getter(vm, regexp_ctor);

    vm.globals.insert("RegExp".to_string(), regexp_ctor);
    if vm.global_object.is_object() {
        vm.set_property(vm.global_object, "RegExp", regexp_ctor);
    }
}

pub fn reg_exp_builtin_exec(vm: &mut Vm, this: JsValue, text: &str) -> Result<JsValue, VmError> {
    if let Some(ptr) = this.as_object_ptr() {
        let obj = unsafe { &*(ptr as *mut JsObject) };
        if let ObjectKind::RegExp { pattern, flags } = &obj.kind {
            let pattern = pattern.clone();
            let flags = flags.clone();
            let is_global = flags.contains('g');
            let is_sticky = flags.contains('y');

            let last_index = if is_global || is_sticky {
                let last_index_val = vm.get_property_result(this, "lastIndex")?;
                vm.to_length(last_index_val)?
            } else {
                0
            };

            let text_u16_len = text.encode_utf16().count();
            if last_index > text_u16_len {
                if is_global || is_sticky {
                    let ok = vm.set_property_with_receiver(
                        this,
                        "lastIndex",
                        JsValue::from_int32(0),
                        this,
                    )?;
                    if !ok {
                        return Err(VmError::TypeError("Cannot set lastIndex on RegExp".into()));
                    }
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

            if let Some(m) = regex.find_from(text, byte_offset).next() {
                let match_start_byte = m.range().start;
                let match_end_byte = m.range().end;

                if is_sticky && match_start_byte != byte_offset {
                    let ok = vm.set_property_with_receiver(
                        this,
                        "lastIndex",
                        JsValue::from_int32(0),
                        this,
                    )?;
                    if !ok {
                        return Err(VmError::TypeError("Cannot set lastIndex on RegExp".into()));
                    }
                    return Ok(JsValue::null());
                }

                let match_start_u16 = text[..match_start_byte].encode_utf16().count();
                let match_end_u16 = text[..match_end_byte].encode_utf16().count();

                if is_global || is_sticky {
                    let last_idx_js = if match_end_u16 <= i32::MAX as usize {
                        JsValue::from_int32(match_end_u16 as i32)
                    } else {
                        JsValue::from_double(match_end_u16 as f64)
                    };
                    let ok = vm.set_property_with_receiver(this, "lastIndex", last_idx_js, this)?;
                    if !ok {
                        return Err(VmError::TypeError("Cannot set lastIndex on RegExp".into()));
                    }
                }

                let matched_str = &text[match_start_byte..match_end_byte];
                let mut elements = vec![vm.alloc_string(matched_str.to_string())];
                let mut cap_strings = Vec::new();
                for cap_opt in &m.captures {
                    if let Some(range) = cap_opt {
                        let s = text[range.clone()].to_string();
                        elements.push(vm.alloc_string(s.clone()));
                        cap_strings.push(Some(s));
                    } else {
                        elements.push(JsValue::undefined());
                        cap_strings.push(None);
                    }
                }

                // Update Annex B legacy RegExp last match state
                vm.regexp_last_match.input = text.to_string();
                vm.regexp_last_match.last_match = matched_str.to_string();
                vm.regexp_last_match.left_context = text[..match_start_byte].to_string();
                vm.regexp_last_match.right_context = text[match_end_byte..].to_string();
                for i in 0..9 {
                    vm.regexp_last_match.parens[i] = if let Some(Some(s)) = cap_strings.get(i) {
                        s.clone()
                    } else {
                        String::new()
                    };
                }
                vm.regexp_last_match.last_paren = cap_strings
                    .iter()
                    .rev()
                    .filter_map(|c| c.as_ref())
                    .next()
                    .cloned()
                    .unwrap_or_default();

                let arr_val = vm.alloc_array(elements);
                vm.create_data_property_or_throw(
                    arr_val,
                    "index",
                    JsValue::from_int32(match_start_u16 as i32),
                )?;
                let text_val = vm.alloc_string(text.to_string());
                vm.create_data_property_or_throw(arr_val, "input", text_val)?;

                let named: Vec<_> = m.named_groups().collect();
                if !named.is_empty() {
                    let groups_obj = vm.alloc_object_with_exact_proto(None);
                    for (name, range_opt) in &named {
                        let val = if let Some(range) = range_opt {
                            vm.alloc_string(text[range.clone()].to_string())
                        } else {
                            JsValue::undefined()
                        };
                        vm.create_data_property_or_throw(groups_obj, name, val)?;
                    }
                    vm.create_data_property_or_throw(arr_val, "groups", groups_obj)?;
                } else {
                    vm.create_data_property_or_throw(arr_val, "groups", JsValue::undefined())?;
                }

                // Match indices support for /d flag (ES2022)
                if flags.contains('d') {
                    let mut indices_list = Vec::with_capacity(1 + m.captures.len());
                    let match_indices = vm.alloc_array(vec![
                        JsValue::from_int32(match_start_u16 as i32),
                        JsValue::from_int32(match_end_u16 as i32),
                    ]);
                    indices_list.push(match_indices);

                    for cap_opt in &m.captures {
                        if let Some(range) = cap_opt {
                            let c_start_u16 = text[..range.start].encode_utf16().count();
                            let c_end_u16 = text[..range.end].encode_utf16().count();
                            let cap_indices = vm.alloc_array(vec![
                                JsValue::from_int32(c_start_u16 as i32),
                                JsValue::from_int32(c_end_u16 as i32),
                            ]);
                            indices_list.push(cap_indices);
                        } else {
                            indices_list.push(JsValue::undefined());
                        }
                    }

                    let indices_arr = vm.alloc_array(indices_list);
                    if !named.is_empty() {
                        let indices_groups_obj = vm.alloc_object_with_exact_proto(None);
                        for (name, range_opt) in &named {
                            let val = if let Some(range) = range_opt {
                                let c_start_u16 = text[..range.start].encode_utf16().count();
                                let c_end_u16 = text[..range.end].encode_utf16().count();
                                vm.alloc_array(vec![
                                    JsValue::from_int32(c_start_u16 as i32),
                                    JsValue::from_int32(c_end_u16 as i32),
                                ])
                            } else {
                                JsValue::undefined()
                            };
                            vm.create_data_property_or_throw(indices_groups_obj, name, val)?;
                        }
                        vm.create_data_property_or_throw(
                            indices_arr,
                            "groups",
                            indices_groups_obj,
                        )?;
                    } else {
                        vm.create_data_property_or_throw(
                            indices_arr,
                            "groups",
                            JsValue::undefined(),
                        )?;
                    }

                    vm.create_data_property_or_throw(arr_val, "indices", indices_arr)?;
                }

                return Ok(arr_val);
            } else {
                if is_global || is_sticky {
                    let ok = vm.set_property_with_receiver(
                        this,
                        "lastIndex",
                        JsValue::from_int32(0),
                        this,
                    )?;
                    if !ok {
                        return Err(VmError::TypeError("Cannot set lastIndex on RegExp".into()));
                    }
                }
                return Ok(JsValue::null());
            }
        }
    }
    Err(VmError::TypeError("RegExp.prototype.exec called on incompatible receiver".into()))
}

pub fn reg_exp_exec(vm: &mut Vm, r: JsValue, s: &str) -> Result<JsValue, VmError> {
    let exec = vm.get_property_result(r, "exec")?;
    if vm.is_callable(exec) {
        let prev_temp = vm.temp_roots.len();
        vm.temp_roots.push(r);
        let s_val = vm.alloc_string(s.to_string());
        let cur_r = vm.temp_roots[prev_temp];
        let res = vm.call(exec, cur_r, &[s_val])?;
        vm.temp_roots.truncate(prev_temp);
        if !res.is_object() && !res.is_null() {
            return Err(VmError::TypeError("RegExp exec did not return an Object or null".into()));
        }
        return Ok(res);
    }
    let is_regexp = if let Some(ptr) = r.as_object_ptr() {
        let obj = unsafe { &*(ptr as *mut JsObject) };
        matches!(obj.kind, ObjectKind::RegExp { .. })
    } else {
        false
    };
    if !is_regexp {
        return Err(VmError::TypeError("RegExpExec called on incompatible receiver".into()));
    }
    reg_exp_builtin_exec(vm, r, s)
}

pub fn advance_string_index(s: &str, index: usize, unicode: bool) -> usize {
    let u16_chars: Vec<u16> = s.encode_utf16().collect();
    if !unicode || index + 1 >= u16_chars.len() {
        return index + 1;
    }
    let first = u16_chars[index];
    if (0xD800..=0xDBFF).contains(&first) {
        let second = u16_chars[index + 1];
        if (0xDC00..=0xDFFF).contains(&second) {
            return index + 2;
        }
    }
    index + 1
}
