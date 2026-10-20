//! `JSON.parse` / `JSON.stringify` with a strict recursive-descent parser.

use super::*;

mod parse;
mod reviver;
mod stringify;

use parse::JsonParser;
use reviver::json_walk;
use stringify::json_stringify_walk;

pub(super) fn register_json(vm: &mut Vm) {
    let json_obj = vm.alloc_object();

    let stringify_fn = vm.alloc_native_function("stringify", 3, |vm, _this, args| {
        let prev_temp = vm.temp_roots.len();
        let value = args.first().copied().unwrap_or(JsValue::undefined());
        vm.temp_roots.push(value);
        let replacer = args.get(1).copied().filter(|v| v.is_object());
        vm.temp_roots.push(replacer.unwrap_or(JsValue::undefined()));
        let space_val = args.get(2).copied().unwrap_or(JsValue::undefined());
        vm.temp_roots.push(space_val);

        let cur_replacer = vm.temp_roots[prev_temp + 1];
        let replacer_fn = if cur_replacer.is_object() && vm.is_callable(cur_replacer) {
            Some(cur_replacer)
        } else {
            None
        };
        let replacer_arr: Option<Vec<String>> =
            if cur_replacer.is_object() && !vm.is_callable(cur_replacer) {
                if vm.is_array_result(cur_replacer)? {
                    let len_val = vm.get_property_result(cur_replacer, "length")?;
                    let len = vm.to_number_value(len_val).unwrap_or(0.0).max(0.0) as usize;
                    let mut keys = Vec::new();
                    for i in 0..len {
                        let mut elem = vm.get_property_result(cur_replacer, &i.to_string())?;
                        if let Some(ptr) = elem.as_object_ptr() {
                            let obj = unsafe { &*(ptr as *mut JsObject) };
                            if matches!(
                                obj.kind,
                                ObjectKind::NumberObject(_) | ObjectKind::StringObject(_)
                            ) {
                                let s = vm.to_string_spec(elem)?;
                                elem = vm.alloc_string(s);
                            }
                        }
                        if elem.is_string() || elem.is_number() {
                            let k = vm.to_string_value(elem);
                            if !keys.contains(&k) {
                                keys.push(k);
                            }
                        }
                    }
                    Some(keys)
                } else {
                    None
                }
            } else {
                None
            };

        let mut cur_space = vm.temp_roots[prev_temp + 2];
        if let Some(ptr) = cur_space.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::NumberObject(n) = obj.kind {
                cur_space = JsValue::from_double(n);
            } else if let ObjectKind::StringObject(_) = obj.kind {
                if let Ok(s) = vm.to_string_spec(cur_space) {
                    cur_space = vm.alloc_string(s);
                }
            }
        }
        let gap = if cur_space.is_number() {
            let num = cur_space.to_number().unwrap_or(0.0);
            if num.is_nan() || num <= 0.0 {
                String::new()
            } else {
                let n = (num.floor() as usize).min(10);
                " ".repeat(n)
            }
        } else if cur_space.is_string() {
            let s = vm.to_string_value(cur_space);
            s.chars().take(10).collect::<String>()
        } else {
            String::new()
        };

        let wrapper = vm.alloc_object();
        vm.temp_roots.push(wrapper);
        let cur_val = vm.temp_roots[prev_temp];
        let cur_wrapper = vm.temp_roots[prev_temp + 3];
        vm.set_property(cur_wrapper, "", cur_val);

        let mut stack = Vec::new();
        let cur_wrapper = vm.temp_roots[prev_temp + 3];
        let res = json_stringify_walk(
            vm,
            cur_wrapper,
            "",
            &mut stack,
            replacer_fn,
            replacer_arr.as_deref(),
            &gap,
            "",
        );

        vm.temp_roots.truncate(prev_temp);
        let s = res?;
        match s {
            Some(s) => Ok(vm.alloc_string(s)),
            None => Ok(JsValue::undefined()),
        }
    });

    let parse_fn = vm.alloc_native_function("parse", 2, |vm, _this, args| {
        let text_arg = args.first().copied().unwrap_or(JsValue::undefined());
        if text_arg.is_symbol() {
            return Err(VmError::TypeError("Cannot convert a Symbol value to a string".into()));
        }
        let text = vm.to_string_spec(text_arg)?;
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
    vm.set_property_non_enumerable(json_obj, "stringify".to_string());
    vm.set_property(json_obj, "parse", parse_fn);
    vm.set_property_non_enumerable(json_obj, "parse".to_string());

    if let Some(tag_sym) = vm.symbol_to_string_tag {
        let tag_val = vm.alloc_string("JSON".into());
        vm.set_symbol_property(json_obj, tag_sym, tag_val);
        if let Some(ptr) = json_obj.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            obj.set_symbol_attributes(
                tag_sym,
                PropertyAttributes { writable: false, enumerable: false, configurable: true },
            );
        }
    }

    vm.globals.insert("JSON".to_string(), json_obj);
    if vm.global_object.is_object() {
        vm.set_property(vm.global_object, "JSON", json_obj);
    }
}
