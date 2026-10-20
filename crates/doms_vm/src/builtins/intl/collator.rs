//! `Intl.Collator` implementation.

use super::locale::canonicalize_locale_list;
use super::setup_prototype_and_constructor;
use super::*;

pub(super) fn register_collator(vm: &mut Vm, intl_obj: JsValue) {
    let collator_proto = vm.alloc_object();
    let collator_ctor = vm.alloc_native_function("Collator", 0, |vm, this, args| {
        let instance = if vm.is_constructing {
            this
        } else {
            let p = vm.get_property(vm.global_object, "Intl");
            let c = vm.get_property(p, "Collator");
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

    let coll_compare_fn = vm.alloc_native_function("compare", 2, |vm, _this, args| {
        let a = args.first().map(|v| vm.to_string_value(*v)).unwrap_or_default();
        let b = args.get(1).map(|v| vm.to_string_value(*v)).unwrap_or_default();
        let cmp = match a.cmp(&b) {
            std::cmp::Ordering::Less => -1,
            std::cmp::Ordering::Equal => 0,
            std::cmp::Ordering::Greater => 1,
        };
        Ok(JsValue::from_int32(cmp))
    });
    vm.set_property(collator_proto, "compare", coll_compare_fn);
    vm.set_property_non_enumerable(collator_proto, "compare".to_string());

    let coll_resolved_opts = vm.alloc_native_function("resolvedOptions", 0, |vm, this, _| {
        let opts = vm.alloc_object();
        let loc = vm.get_property(this, "_locale");
        let loc_val = if loc.is_js_string() { loc } else { vm.alloc_string("en-US".to_string()) };
        vm.set_property(opts, "locale", loc_val);
        let usage_val = vm.alloc_string("sort".to_string());
        vm.set_property(opts, "usage", usage_val);
        let sens_val = vm.alloc_string("variant".to_string());
        vm.set_property(opts, "sensitivity", sens_val);
        vm.set_property(opts, "ignorePunctuation", JsValue::from_boolean(false));
        vm.set_property(opts, "numeric", JsValue::from_boolean(false));
        let cf_val = vm.alloc_string("false".to_string());
        vm.set_property(opts, "caseFirst", cf_val);
        let col_val = vm.alloc_string("default".to_string());
        vm.set_property(opts, "collation", col_val);
        Ok(opts)
    });
    vm.set_property(collator_proto, "resolvedOptions", coll_resolved_opts);
    vm.set_property_non_enumerable(collator_proto, "resolvedOptions".to_string());

    let supported_locales_fn3 =
        vm.alloc_native_function("supportedLocalesOf", 1, |vm, _this, args| {
            let loc = args.first().copied().unwrap_or(JsValue::undefined());
            let canons = canonicalize_locale_list(vm, loc)?;
            let items: Vec<JsValue> = canons.into_iter().map(|s| vm.alloc_string(s)).collect();
            Ok(vm.alloc_array(items))
        });
    vm.set_property(collator_ctor, "supportedLocalesOf", supported_locales_fn3);
    vm.set_property_non_enumerable(collator_ctor, "supportedLocalesOf".to_string());

    setup_prototype_and_constructor(
        vm,
        collator_ctor,
        collator_proto,
        "Intl.Collator",
        "Collator",
        intl_obj,
    );
}
