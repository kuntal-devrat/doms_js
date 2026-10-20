//! `Intl.NumberFormat` implementation.

use super::locale::canonicalize_locale_list;
use super::setup_prototype_and_constructor;
use super::*;

pub(super) fn register_number_format(vm: &mut Vm, intl_obj: JsValue) {
    let num_format_proto = vm.alloc_object();
    let num_format_ctor = vm.alloc_native_function("NumberFormat", 0, |vm, this, args| {
        let instance = if vm.is_constructing {
            this
        } else {
            let p = vm.get_property(vm.global_object, "Intl");
            let c = vm.get_property(p, "NumberFormat");
            let proto = vm.get_property(c, "prototype");
            let obj = vm.alloc_object();
            if let Some(ptr) = obj.as_object_ptr() {
                unsafe { (*(ptr as *mut JsObject)).prototype = Some(proto) };
            }
            obj
        };

        let loc = args.first().copied().unwrap_or(JsValue::undefined());
        let canons = canonicalize_locale_list(vm, loc).unwrap_or_default();
        let locale_str = canons.into_iter().next().unwrap_or_else(|| "en-US".to_string());

        let opt = args.get(1).copied().unwrap_or(JsValue::undefined());
        let style = if opt.is_object() {
            let s_val = vm.get_property(opt, "style");
            if !s_val.is_undefined() { vm.to_string_value(s_val) } else { "decimal".to_string() }
        } else {
            "decimal".to_string()
        };

        let loc_val = vm.alloc_string(locale_str);
        vm.set_property(instance, "_locale", loc_val);
        let style_val = vm.alloc_string(style);
        vm.set_property(instance, "_style", style_val);

        Ok(instance)
    });

    let nf_format_fn = vm.alloc_native_function("format", 1, |vm, _this, args| {
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
    vm.set_property(num_format_proto, "format", nf_format_fn);
    vm.set_property_non_enumerable(num_format_proto, "format".to_string());

    let nf_format_to_parts_fn = vm.alloc_native_function("formatToParts", 1, |vm, this, args| {
        let format_fn = vm.get_property(this, "format");
        let formatted = vm.call(format_fn, this, args)?;
        let s = vm.to_string_value(formatted);
        let part_obj = vm.alloc_object();
        let type_val = vm.alloc_string("integer".to_string());
        let val_val = vm.alloc_string(s);
        vm.set_property(part_obj, "type", type_val);
        vm.set_property(part_obj, "value", val_val);
        Ok(vm.alloc_array(vec![part_obj]))
    });
    vm.set_property(num_format_proto, "formatToParts", nf_format_to_parts_fn);
    vm.set_property_non_enumerable(num_format_proto, "formatToParts".to_string());

    let nf_resolved_opts = vm.alloc_native_function("resolvedOptions", 0, |vm, this, _| {
        let opts = vm.alloc_object();
        let loc = vm.get_property(this, "_locale");
        let loc_val = if loc.is_js_string() { loc } else { vm.alloc_string("en-US".to_string()) };
        vm.set_property(opts, "locale", loc_val);
        let num_sys = vm.alloc_string("latn".to_string());
        vm.set_property(opts, "numberingSystem", num_sys);
        let style = vm.get_property(this, "_style");
        let style_val =
            if style.is_js_string() { style } else { vm.alloc_string("decimal".to_string()) };
        vm.set_property(opts, "style", style_val);
        vm.set_property(opts, "minimumIntegerDigits", JsValue::from_int32(1));
        vm.set_property(opts, "minimumFractionDigits", JsValue::from_int32(0));
        vm.set_property(opts, "maximumFractionDigits", JsValue::from_int32(3));
        vm.set_property(opts, "useGrouping", JsValue::from_boolean(true));
        let notation_val = vm.alloc_string("standard".to_string());
        vm.set_property(opts, "notation", notation_val);
        Ok(opts)
    });
    vm.set_property(num_format_proto, "resolvedOptions", nf_resolved_opts);
    vm.set_property_non_enumerable(num_format_proto, "resolvedOptions".to_string());

    let supported_locales_fn =
        vm.alloc_native_function("supportedLocalesOf", 1, |vm, _this, args| {
            let loc = args.first().copied().unwrap_or(JsValue::undefined());
            let canons = canonicalize_locale_list(vm, loc)?;
            let items: Vec<JsValue> = canons.into_iter().map(|s| vm.alloc_string(s)).collect();
            Ok(vm.alloc_array(items))
        });
    vm.set_property(num_format_ctor, "supportedLocalesOf", supported_locales_fn);
    vm.set_property_non_enumerable(num_format_ctor, "supportedLocalesOf".to_string());

    setup_prototype_and_constructor(
        vm,
        num_format_ctor,
        num_format_proto,
        "Intl.NumberFormat",
        "NumberFormat",
        intl_obj,
    );
}
