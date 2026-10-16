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
