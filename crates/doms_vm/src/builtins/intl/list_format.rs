//! `Intl.ListFormat` implementation.

use super::locale::canonicalize_locale_list;
use super::setup_prototype_and_constructor;
use super::*;

pub(super) fn register_list_format(vm: &mut Vm, intl_obj: JsValue) {
    let lf_proto = vm.alloc_object();
    let lf_ctor = vm.alloc_native_function("ListFormat", 0, |vm, this, args| {
        if !vm.is_constructing {
            return Err(VmError::TypeError("Constructor Intl.ListFormat requires 'new'".into()));
        }
        let loc = args.first().copied().unwrap_or(JsValue::undefined());
        let canons = canonicalize_locale_list(vm, loc).unwrap_or_default();
        let locale_str = canons.into_iter().next().unwrap_or_else(|| "en-US".to_string());
        let loc_val = vm.alloc_string(locale_str);
        vm.set_property(this, "_locale", loc_val);

        let opt = args.get(1).copied().unwrap_or(JsValue::undefined());
        let (lf_type, style) = if opt.is_object() {
            let t = vm.get_property(opt, "type");
            let s = vm.get_property(opt, "style");
            (
                if !t.is_undefined() { vm.to_string_value(t) } else { "conjunction".to_string() },
                if !s.is_undefined() { vm.to_string_value(s) } else { "long".to_string() },
            )
        } else {
            ("conjunction".to_string(), "long".to_string())
        };

        let t_val = vm.alloc_string(lf_type);
        vm.set_property(this, "_type", t_val);
        let s_val = vm.alloc_string(style);
        vm.set_property(this, "_style", s_val);

        Ok(this)
    });

    let lf_format_fn = vm.alloc_native_function("format", 1, |vm, this, args| {
        let list_val = args.first().copied().unwrap_or(JsValue::undefined());
        if !list_val.is_object() {
            return Err(VmError::TypeError(
                "Intl.ListFormat.prototype.format requires an iterable".into(),
            ));
        }
        let len_val = vm.get_property(list_val, "length");
        let len = len_val.as_int32().unwrap_or(0).max(0) as usize;
        let mut strings = Vec::new();
        for i in 0..len {
            let elem = vm.get_property(list_val, &i.to_string());
            strings.push(vm.to_string_value(elem));
        }

        let t_prop = vm.get_property(this, "_type");
        let lf_type = vm.to_string_value(t_prop);
        let conj = if lf_type == "disjunction" { "or" } else { "and" };

        let formatted = match strings.len() {
            0 => String::new(),
            1 => strings[0].clone(),
            2 => format!("{} {} {}", strings[0], conj, strings[1]),
            _ => {
                let initial = strings[..strings.len() - 1].join(", ");
                format!("{}, {} {}", initial, conj, strings.last().unwrap())
            }
        };
        Ok(vm.alloc_string(formatted))
    });
    vm.set_property(lf_proto, "format", lf_format_fn);
    vm.set_property_non_enumerable(lf_proto, "format".to_string());

    let lf_format_to_parts_fn = vm.alloc_native_function("formatToParts", 1, |vm, this, args| {
        let list_val = args.first().copied().unwrap_or(JsValue::undefined());
        if !list_val.is_object() {
            return Err(VmError::TypeError(
                "Intl.ListFormat.prototype.formatToParts requires an iterable".into(),
            ));
        }
        let len_val = vm.get_property(list_val, "length");
        let len = len_val.as_int32().unwrap_or(0).max(0) as usize;
        let mut strings = Vec::new();
        for i in 0..len {
            let elem = vm.get_property(list_val, &i.to_string());
            strings.push(vm.to_string_value(elem));
        }

        let t_prop = vm.get_property(this, "_type");
        let lf_type = vm.to_string_value(t_prop);
        let conj = if lf_type == "disjunction" { "or" } else { "and" };

        let mut parts = Vec::new();
        match strings.len() {
            0 => {}
            1 => {
                let obj = vm.alloc_object();
                let type_val = vm.alloc_string("element".to_string());
                vm.set_property(obj, "type", type_val);
                let val_val = vm.alloc_string(strings[0].clone());
                vm.set_property(obj, "value", val_val);
                parts.push(obj);
            }
            2 => {
                let obj1 = vm.alloc_object();
                let type_val1 = vm.alloc_string("element".to_string());
                vm.set_property(obj1, "type", type_val1);
                let val_val1 = vm.alloc_string(strings[0].clone());
                vm.set_property(obj1, "value", val_val1);
                parts.push(obj1);

                let lit = vm.alloc_object();
                let lit_type = vm.alloc_string("literal".to_string());
                vm.set_property(lit, "type", lit_type);
                let lit_val = vm.alloc_string(format!(" {} ", conj));
                vm.set_property(lit, "value", lit_val);
                parts.push(lit);

                let obj2 = vm.alloc_object();
                let type_val2 = vm.alloc_string("element".to_string());
                vm.set_property(obj2, "type", type_val2);
                let val_val2 = vm.alloc_string(strings[1].clone());
                vm.set_property(obj2, "value", val_val2);
                parts.push(obj2);
            }
            _ => {
                for (i, s) in strings.iter().enumerate() {
                    let obj = vm.alloc_object();
                    let type_val = vm.alloc_string("element".to_string());
                    vm.set_property(obj, "type", type_val);
                    let val_val = vm.alloc_string(s.clone());
                    vm.set_property(obj, "value", val_val);
                    parts.push(obj);

                    if i + 2 == strings.len() {
                        let lit = vm.alloc_object();
                        let lit_type = vm.alloc_string("literal".to_string());
                        vm.set_property(lit, "type", lit_type);
                        let lit_val = vm.alloc_string(format!(", {} ", conj));
                        vm.set_property(lit, "value", lit_val);
                        parts.push(lit);
                    } else if i + 1 < strings.len() {
                        let lit = vm.alloc_object();
                        let lit_type = vm.alloc_string("literal".to_string());
                        vm.set_property(lit, "type", lit_type);
                        let lit_val = vm.alloc_string(", ".to_string());
                        vm.set_property(lit, "value", lit_val);
                        parts.push(lit);
                    }
                }
            }
        }
        Ok(vm.alloc_array(parts))
    });
    vm.set_property(lf_proto, "formatToParts", lf_format_to_parts_fn);
    vm.set_property_non_enumerable(lf_proto, "formatToParts".to_string());

    let lf_resolved_opts = vm.alloc_native_function("resolvedOptions", 0, |vm, this, _| {
        let opts = vm.alloc_object();
        let loc = vm.get_property(this, "_locale");
        let loc_val = if loc.is_js_string() { loc } else { vm.alloc_string("en-US".to_string()) };
        vm.set_property(opts, "locale", loc_val);

        let t = vm.get_property(this, "_type");
        let t_val = if t.is_js_string() { t } else { vm.alloc_string("conjunction".to_string()) };
        vm.set_property(opts, "type", t_val);

        let s = vm.get_property(this, "_style");
        let s_val = if s.is_js_string() { s } else { vm.alloc_string("long".to_string()) };
        vm.set_property(opts, "style", s_val);

        Ok(opts)
    });
    vm.set_property(lf_proto, "resolvedOptions", lf_resolved_opts);
    vm.set_property_non_enumerable(lf_proto, "resolvedOptions".to_string());

    let supported_locales_fn5 =
        vm.alloc_native_function("supportedLocalesOf", 1, |vm, _this, args| {
            let loc = args.first().copied().unwrap_or(JsValue::undefined());
            let canons = canonicalize_locale_list(vm, loc)?;
            let items: Vec<JsValue> = canons.into_iter().map(|s| vm.alloc_string(s)).collect();
            Ok(vm.alloc_array(items))
        });
    vm.set_property(lf_ctor, "supportedLocalesOf", supported_locales_fn5);
    vm.set_property_non_enumerable(lf_ctor, "supportedLocalesOf".to_string());

    setup_prototype_and_constructor(
        vm,
        lf_ctor,
        lf_proto,
        "Intl.ListFormat",
        "ListFormat",
        intl_obj,
    );
}
