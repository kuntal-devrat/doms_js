//! `Intl.PluralRules` implementation.

use super::locale::canonicalize_locale_list;
use super::setup_prototype_and_constructor;
use super::*;

pub(super) fn register_plural_rules(vm: &mut Vm, intl_obj: JsValue) {
    let pr_proto = vm.alloc_object();
    let pr_ctor = vm.alloc_native_function("PluralRules", 0, |vm, this, args| {
        if !vm.is_constructing {
            return Err(VmError::TypeError("Constructor Intl.PluralRules requires 'new'".into()));
        }
        let loc = args.first().copied().unwrap_or(JsValue::undefined());
        let canons = canonicalize_locale_list(vm, loc).unwrap_or_default();
        let locale_str = canons.into_iter().next().unwrap_or_else(|| "en-US".to_string());
        let loc_val = vm.alloc_string(locale_str);
        vm.set_property(this, "_locale", loc_val);

        let opt = args.get(1).copied().unwrap_or(JsValue::undefined());
        let pr_type = if opt.is_object() {
            let t = vm.get_property(opt, "type");
            if !t.is_undefined() { vm.to_string_value(t) } else { "cardinal".to_string() }
        } else {
            "cardinal".to_string()
        };
        let t_val = vm.alloc_string(pr_type);
        vm.set_property(this, "_type", t_val);

        Ok(this)
    });

    let pr_select_fn = vm.alloc_native_function("select", 1, |vm, _this, args| {
        let n = args.first().and_then(|v| v.to_number()).unwrap_or(0.0);
        let cat = if (n - 1.0).abs() < f64::EPSILON { "one" } else { "other" };
        Ok(vm.alloc_string(cat.to_string()))
    });
    vm.set_property(pr_proto, "select", pr_select_fn);
    vm.set_property_non_enumerable(pr_proto, "select".to_string());

    let pr_resolved_opts = vm.alloc_native_function("resolvedOptions", 0, |vm, this, _| {
        let opts = vm.alloc_object();
        let loc = vm.get_property(this, "_locale");
        let loc_val = if loc.is_js_string() { loc } else { vm.alloc_string("en-US".to_string()) };
        vm.set_property(opts, "locale", loc_val);

        let pr_type = vm.get_property(this, "_type");
        let pr_type_val =
            if pr_type.is_js_string() { pr_type } else { vm.alloc_string("cardinal".to_string()) };
        vm.set_property(opts, "type", pr_type_val);

        let one_str = vm.alloc_string("one".to_string());
        let other_str = vm.alloc_string("other".to_string());
        let cats = vm.alloc_array(vec![one_str, other_str]);
        vm.set_property(opts, "pluralCategories", cats);
        vm.set_property(opts, "minimumIntegerDigits", JsValue::from_int32(1));
        vm.set_property(opts, "minimumFractionDigits", JsValue::from_int32(0));
        vm.set_property(opts, "maximumFractionDigits", JsValue::from_int32(3));
        Ok(opts)
    });
    vm.set_property(pr_proto, "resolvedOptions", pr_resolved_opts);
    vm.set_property_non_enumerable(pr_proto, "resolvedOptions".to_string());

    let supported_locales_fn4 =
        vm.alloc_native_function("supportedLocalesOf", 1, |vm, _this, args| {
            let loc = args.first().copied().unwrap_or(JsValue::undefined());
            let canons = canonicalize_locale_list(vm, loc)?;
            let items: Vec<JsValue> = canons.into_iter().map(|s| vm.alloc_string(s)).collect();
            Ok(vm.alloc_array(items))
        });
    vm.set_property(pr_ctor, "supportedLocalesOf", supported_locales_fn4);
    vm.set_property_non_enumerable(pr_ctor, "supportedLocalesOf".to_string());

    setup_prototype_and_constructor(
        vm,
        pr_ctor,
        pr_proto,
        "Intl.PluralRules",
        "PluralRules",
        intl_obj,
    );
}
