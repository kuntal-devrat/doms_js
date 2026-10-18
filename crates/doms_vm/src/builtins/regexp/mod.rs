//! The `RegExp` global and `RegExp.prototype`.

use super::*;
use crate::interpreter::PreferredType;

pub(super) fn register_regexp_builtins(vm: &mut Vm) {
    // RegExp prototype and constructor
    let regexp_proto = vm.alloc_object();
    vm.regexp_proto = Some(regexp_proto);

    let regexp_ctor = vm.alloc_native_function("RegExp", 2, |vm, _this, args| {
        let pattern = if let Some(arg0) = args.first() {
            if arg0.is_undefined() {
                String::new()
            } else if let Some(ptr) = arg0.as_object_ptr() {
                let obj = unsafe { &*(ptr as *mut JsObject) };
                if let ObjectKind::RegExp { pattern, flags: orig_flags } = &obj.kind {
                    if args.get(1).map_or(true, |v| v.is_undefined()) {
                        return Ok(vm.alloc_regexp(pattern.clone(), orig_flags.clone()));
                    }
                    pattern.clone()
                } else {
                    vm.to_string_value(*arg0)
                }
            } else {
                vm.to_string_value(*arg0)
            }
        } else {
            String::new()
        };
        let flags = if let Some(arg1) = args.get(1) {
            if arg1.is_undefined() {
                String::new()
            } else {
                vm.to_string_value(*arg1)
            }
        } else {
            String::new()
        };

        // Validate flags: allowed characters and duplicate check
        let mut seen = [false; 256];
        for ch in flags.chars() {
            if !matches!(ch, 'd' | 'g' | 'i' | 'm' | 's' | 'u' | 'v' | 'y') {
                return Err(VmError::SyntaxError(format!("Invalid regular expression flag: {}", ch)));
            }
            let b = ch as usize;
            if b < 256 {
                if seen[b] {
                    return Err(VmError::SyntaxError(format!("Duplicate regular expression flag: {}", ch)));
                }
                seen[b] = true;
            }
        }
        if seen[b'u' as usize] && seen[b'v' as usize] {
            return Err(VmError::SyntaxError("Invalid regular expression flags: 'u' and 'v' cannot both be present".into()));
        }

        // Validate pattern and flags via regress
        if let Err(e) = regress::Regex::with_flags(&pattern, flags.as_str()) {
            return Err(VmError::SyntaxError(format!("Invalid regular expression: {}", e)));
        }

        Ok(vm.alloc_regexp(pattern, flags))
    });
    vm.set_property(regexp_ctor, "prototype", regexp_proto);
    vm.non_enumerable_props.insert((regexp_ctor, "prototype".to_string()));
    vm.non_writable_props.insert((regexp_ctor, "prototype".to_string()));
    vm.non_configurable_props.insert((regexp_ctor, "prototype".to_string()));
    vm.set_property(regexp_proto, "constructor", regexp_ctor);
    vm.non_enumerable_props.insert((regexp_proto, "constructor".to_string()));

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
                    vm.regexp_last_match.input = text.clone();
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
                    vm.regexp_last_match.last_paren = cap_strings.iter()
                        .rev()
                        .filter_map(|c| c.as_ref())
                        .next()
                        .cloned()
                        .unwrap_or_default();

                    let arr_val = vm.alloc_array(elements);
                    vm.set_property(arr_val, "index", JsValue::from_int32(match_start_u16 as i32));
                    let text_val = vm.alloc_string(text.clone());
                    vm.set_property(arr_val, "input", text_val);

                    let named: Vec<_> = m.named_groups().collect();
                    if !named.is_empty() {
                        let groups_obj = vm.alloc_object();
                        for (name, range_opt) in &named {
                            let val = if let Some(range) = range_opt {
                                vm.alloc_string(text[range.clone()].to_string())
                            } else {
                                JsValue::undefined()
                            };
                            vm.set_property(groups_obj, name, val);
                        }
                        vm.set_property(arr_val, "groups", groups_obj);
                    } else {
                        vm.set_property(arr_val, "groups", JsValue::undefined());
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
                            let indices_groups_obj = vm.alloc_object();
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
                                vm.set_property(indices_groups_obj, name, val);
                            }
                            vm.set_property(indices_arr, "groups", indices_groups_obj);
                        } else {
                            vm.set_property(indices_arr, "groups", JsValue::undefined());
                        }

                        vm.set_property(arr_val, "indices", indices_arr);
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
        Err(VmError::TypeError("Method RegExp.prototype.unicodeSets called on incompatible receiver".into()))
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

    // ES2024 22.2.6.4 get RegExp.prototype.flags (generic receiver)
    let re_flags_getter = vm.alloc_native_function("get flags", 0, |vm, this, _| {
        if !this.is_object() {
            return Err(VmError::TypeError("Method RegExp.prototype.flags called on incompatible receiver".into()));
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
                                return Err(VmError::TypeError("Cannot supply flags when pattern is a RegExp".into()));
                            }
                            (p_pat.clone(), p_fl.clone())
                        } else {
                            (if p_arg.is_undefined() { String::new() } else { vm.to_string_value(*p_arg) },
                             args.get(1).map(|v| if v.is_undefined() { String::new() } else { vm.to_string_value(*v) }).unwrap_or_default())
                        }
                    } else {
                        (if p_arg.is_undefined() { String::new() } else { vm.to_string_value(*p_arg) },
                         args.get(1).map(|v| if v.is_undefined() { String::new() } else { vm.to_string_value(*v) }).unwrap_or_default())
                    }
                } else {
                    (String::new(), String::new())
                };

                let mut seen = [false; 256];
                for ch in new_flags.chars() {
                    if !matches!(ch, 'd' | 'g' | 'i' | 'm' | 's' | 'u' | 'v' | 'y') {
                        return Err(VmError::SyntaxError(format!("Invalid regular expression flag: {}", ch)));
                    }
                    let b = ch as usize;
                    if b < 256 {
                        if seen[b] {
                            return Err(VmError::SyntaxError(format!("Duplicate regular expression flag: {}", ch)));
                        }
                        seen[b] = true;
                    }
                }
                if seen[b'u' as usize] && seen[b'v' as usize] {
                    return Err(VmError::SyntaxError("Invalid regular expression flags: 'u' and 'v' cannot both be present".into()));
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
            return Err(VmError::TypeError("RegExp.prototype[Symbol.match] called on non-object".into()));
        }
        let global = vm.get_property(this, "global").is_truthy();
        if !global {
            let text_val = vm.alloc_string(text);
            return vm.call_method(this, "exec", &[text_val]);
        }
        let ok = vm.set_property_with_receiver(this, "lastIndex", JsValue::from_int32(0), this)?;
        if !ok {
            return Err(VmError::TypeError("Cannot assign to read-only property 'lastIndex'".into()));
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
                let ok = vm.set_property_with_receiver(cur_this, "lastIndex", JsValue::from_int32(this_index + 1), cur_this)?;
                if !ok {
                    vm.temp_roots.truncate(prev_temp);
                    return Err(VmError::TypeError("Cannot assign to read-only property 'lastIndex'".into()));
                }
            }
            let str_val = vm.alloc_string(s);
            matches.push(str_val);
        }
        vm.temp_roots.truncate(prev_temp);
        if matches.is_empty() {
            Ok(JsValue::null())
        } else {
            Ok(vm.alloc_array(matches))
        }
    });

    // @@replace ( string, replaceValue )
    let re_replace_fn = vm.alloc_native_function("[Symbol.replace]", 2, |vm, this, args| {
        let string_arg = args.first().copied().unwrap_or(JsValue::undefined());
        let str_prim = vm.to_primitive(string_arg, PreferredType::String)?;
        let string_val = vm.to_string_value(str_prim);
        let replace_val = args.get(1).copied().unwrap_or(JsValue::undefined());
        if !this.is_object() {
            return Err(VmError::TypeError("RegExp.prototype[Symbol.replace] called on non-object".into()));
        }
        let global = vm.get_property(this, "global").is_truthy();
        if global {
            let ok = vm.set_property_with_receiver(this, "lastIndex", JsValue::from_int32(0), this)?;
            if !ok {
                return Err(VmError::TypeError("Cannot assign to read-only property 'lastIndex'".into()));
            }
        }
        let prev_temp = vm.temp_roots.len();
        vm.temp_roots.push(this);
        vm.temp_roots.push(replace_val);

        let is_func = vm.is_callable(replace_val);
        let replace_str = if is_func {
            String::new()
        } else {
            let r_prim = vm.to_primitive(replace_val, PreferredType::String)?;
            vm.to_string_value(r_prim)
        };
        let cur_this = vm.temp_roots[prev_temp];
        let str_arg = vm.alloc_string(string_val.clone());
        let res = vm.call_method(cur_this, "exec", &[str_arg])?;
        if res.is_null() {
            vm.temp_roots.truncate(prev_temp);
            return Ok(vm.alloc_string(string_val));
        }
        let match_val = vm.get_property(res, "0");
        let m_prim = vm.to_primitive(match_val, PreferredType::String)?;
        let matched = vm.to_string_value(m_prim);
        let cur_rep = vm.temp_roots[prev_temp + 1];
        let result = if is_func {
            let m_arg = vm.alloc_string(matched.clone());
            let s_arg = vm.alloc_string(string_val.clone());
            let rep_res = vm.call(cur_rep, JsValue::undefined(), &[m_arg, JsValue::from_int32(0), s_arg])?;
            let rep_prim = vm.to_primitive(rep_res, PreferredType::String)?;
            let rep_s = vm.to_string_value(rep_prim);
            vm.alloc_string(string_val.replacen(&matched, &rep_s, 1))
        } else {
            vm.alloc_string(string_val.replacen(&matched, &replace_str, 1))
        };
        vm.temp_roots.truncate(prev_temp);
        Ok(result)
    });

    // @@search ( string )
    let re_search_fn = vm.alloc_native_function("[Symbol.search]", 1, |vm, this, args| {
        let string_arg = args.first().copied().unwrap_or(JsValue::undefined());
        let str_prim = vm.to_primitive(string_arg, PreferredType::String)?;
        let string_val = vm.to_string_value(str_prim);
        if !this.is_object() {
            return Err(VmError::TypeError("RegExp.prototype[Symbol.search] called on non-object".into()));
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
        let str_prim = vm.to_primitive(string_arg, PreferredType::String)?;
        let string_val = vm.to_string_value(str_prim);
        let limit = args.get(1).and_then(|v| v.as_int32()).map(|l| l.max(0) as usize).unwrap_or(usize::MAX);
        if !this.is_object() {
            return Err(VmError::TypeError("RegExp.prototype[Symbol.split] called on non-object".into()));
        }
        if limit == 0 {
            return Ok(vm.alloc_array(Vec::new()));
        }
        let mut out = Vec::new();
        let mut last_idx = 0;
        let _ = vm.set_property_result(this, "lastIndex", JsValue::from_int32(0));
        let prev_temp = vm.temp_roots.len();
        vm.temp_roots.push(this);
        while out.len() < limit && last_idx <= string_val.len() {
            let slice_str = string_val.get(last_idx..).unwrap_or("");
            let slice_arg = vm.alloc_string(slice_str.to_string());
            let cur_this = vm.temp_roots[prev_temp];
            let res = vm.call_method(cur_this, "exec", &[slice_arg])?;
            if res.is_null() {
                break;
            }
            let match_val = vm.get_property(res, "0");
            let m_prim = vm.to_primitive(match_val, PreferredType::String)?;
            let matched = vm.to_string_value(m_prim);
            let match_len = matched.len();
            let rel_idx = vm.get_property(res, "index").as_int32().unwrap_or(0).max(0) as usize;
            let idx = (last_idx + rel_idx).min(string_val.len());

            if idx >= last_idx {
                let piece = string_val.get(last_idx..idx).unwrap_or("");
                out.push(vm.alloc_string(piece.to_string()));
            }
            last_idx = (idx + match_len.max(1)).min(string_val.len() + 1);
        }
        vm.temp_roots.truncate(prev_temp);
        if out.len() < limit {
            let rem_str = string_val.get(last_idx.min(string_val.len())..).unwrap_or("");
            out.push(vm.alloc_string(rem_str.to_string()));
        }
        Ok(vm.alloc_array(out))
    });

    if let Some(sym) = vm.symbol_match {
        vm.set_symbol_property(regexp_proto, sym, re_match_fn);
        vm.non_enumerable_props.insert((regexp_proto, "[Symbol.match]".to_string()));
    }
    if let Some(sym) = vm.symbol_replace {
        vm.set_symbol_property(regexp_proto, sym, re_replace_fn);
        vm.non_enumerable_props.insert((regexp_proto, "[Symbol.replace]".to_string()));
    }
    if let Some(sym) = vm.symbol_search {
        vm.set_symbol_property(regexp_proto, sym, re_search_fn);
        vm.non_enumerable_props.insert((regexp_proto, "[Symbol.search]".to_string()));
    }
    if let Some(sym) = vm.symbol_split {
        vm.set_symbol_property(regexp_proto, sym, re_split_fn);
        vm.non_enumerable_props.insert((regexp_proto, "[Symbol.split]".to_string()));
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
        "global", "ignoreCase", "multiline", "dotAll", "unicode", "unicodeSets", "sticky",
        "hasIndices", "source", "flags", "exec", "test", "toString", "compile", "constructor"
    ] {
        vm.non_enumerable_props.insert((regexp_proto, prop.to_string()));
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
                vm.non_enumerable_props.insert((regexp_ctor, $name.to_string()));
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
        vm.non_enumerable_props.insert((regexp_ctor, "input".to_string()));
        vm.non_enumerable_props.insert((regexp_ctor, "$_".to_string()));

        // lastMatch / $&
        let last_match_getter = vm.alloc_native_function("get lastMatch", 0, |vm, _this, _| {
            Ok(vm.alloc_string(vm.regexp_last_match.last_match.clone()))
        });
        ctor_obj.define_getter("lastMatch".to_string(), last_match_getter);
        ctor_obj.define_getter("$&".to_string(), last_match_getter);
        vm.non_enumerable_props.insert((regexp_ctor, "lastMatch".to_string()));
        vm.non_enumerable_props.insert((regexp_ctor, "$&".to_string()));

        // lastParen / $+
        let last_paren_getter = vm.alloc_native_function("get lastParen", 0, |vm, _this, _| {
            Ok(vm.alloc_string(vm.regexp_last_match.last_paren.clone()))
        });
        ctor_obj.define_getter("lastParen".to_string(), last_paren_getter);
        ctor_obj.define_getter("$+".to_string(), last_paren_getter);
        vm.non_enumerable_props.insert((regexp_ctor, "lastParen".to_string()));
        vm.non_enumerable_props.insert((regexp_ctor, "$+".to_string()));

        // leftContext / $`
        let left_context_getter = vm.alloc_native_function("get leftContext", 0, |vm, _this, _| {
            Ok(vm.alloc_string(vm.regexp_last_match.left_context.clone()))
        });
        ctor_obj.define_getter("leftContext".to_string(), left_context_getter);
        ctor_obj.define_getter("$`".to_string(), left_context_getter);
        vm.non_enumerable_props.insert((regexp_ctor, "leftContext".to_string()));
        vm.non_enumerable_props.insert((regexp_ctor, "$`".to_string()));

        // rightContext / $'
        let right_context_getter = vm.alloc_native_function("get rightContext", 0, |vm, _this, _| {
            Ok(vm.alloc_string(vm.regexp_last_match.right_context.clone()))
        });
        ctor_obj.define_getter("rightContext".to_string(), right_context_getter);
        ctor_obj.define_getter("$'".to_string(), right_context_getter);
        vm.non_enumerable_props.insert((regexp_ctor, "rightContext".to_string()));
        vm.non_enumerable_props.insert((regexp_ctor, "$'".to_string()));
    }

    vm.globals.insert("RegExp".to_string(), regexp_ctor);
    if vm.global_object.is_object() {
        vm.set_property(vm.global_object, "RegExp", regexp_ctor);
    }

}
