//! `Intl.DateTimeFormat` implementation.

use super::*;
use super::setup_prototype_and_constructor;
use super::locale::canonicalize_locale_list;

pub(super) fn register_date_time_format(vm: &mut Vm, intl_obj: JsValue) {
    let date_format_proto = vm.alloc_object();
    let date_format_ctor = vm.alloc_native_function("DateTimeFormat", 0, |vm, this, args| {
        let instance = if vm.is_constructing {
            this
        } else {
            let p = vm.get_property(vm.global_object, "Intl");
            let c = vm.get_property(p, "DateTimeFormat");
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
        let loc_val = vm.alloc_string(locale_str);
        vm.set_property(instance, "_locale", loc_val);

        Ok(instance)
    });

    let df_format_fn = vm.alloc_native_function("format", 1, |vm, _this, args| {
        let s = args.first().map(|v| vm.to_string_value(*v)).unwrap_or_else(|| "2026-10-06T00:00:00Z".to_string());
        Ok(vm.alloc_string(s))
    });
    vm.set_property(date_format_proto, "format", df_format_fn);
    vm.non_enumerable_props.insert((date_format_proto, "format".to_string()));

    let df_resolved_opts = vm.alloc_native_function("resolvedOptions", 0, |vm, this, _| {
        let opts = vm.alloc_object();
        let loc = vm.get_property(this, "_locale");
        let loc_val = if loc.is_js_string() { loc } else { vm.alloc_string("en-US".to_string()) };
        vm.set_property(opts, "locale", loc_val);
        let cal_val = vm.alloc_string("gregory".to_string());
        vm.set_property(opts, "calendar", cal_val);
        let num_val = vm.alloc_string("latn".to_string());
        vm.set_property(opts, "numberingSystem", num_val);
        let tz_val = vm.alloc_string("UTC".to_string());
        vm.set_property(opts, "timeZone", tz_val);
        Ok(opts)
    });
    vm.set_property(date_format_proto, "resolvedOptions", df_resolved_opts);
    vm.non_enumerable_props.insert((date_format_proto, "resolvedOptions".to_string()));

    let supported_locales_fn2 = vm.alloc_native_function("supportedLocalesOf", 1, |vm, _this, args| {
        let loc = args.first().copied().unwrap_or(JsValue::undefined());
        let canons = canonicalize_locale_list(vm, loc)?;
        let items: Vec<JsValue> = canons.into_iter().map(|s| vm.alloc_string(s)).collect();
        Ok(vm.alloc_array(items))
    });
    vm.set_property(date_format_ctor, "supportedLocalesOf", supported_locales_fn2);
    vm.non_enumerable_props.insert((date_format_ctor, "supportedLocalesOf".to_string()));

    setup_prototype_and_constructor(vm, date_format_ctor, date_format_proto, "Intl.DateTimeFormat", "DateTimeFormat", intl_obj);

}
