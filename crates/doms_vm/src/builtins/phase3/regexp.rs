//! The `RegExp` global and `RegExp.prototype`.

use super::*;

pub(super) fn register_regexp_builtins(vm: &mut Vm) {
    // ── RegExp ──
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

        // Validate pattern and flags via regress
        if let Err(e) = regress::Regex::with_flags(&pattern, flags.as_str()) {
            return Err(VmError::SyntaxError(format!("Invalid regular expression: {}", e)));
        }

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

}
