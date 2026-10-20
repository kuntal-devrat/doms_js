//! ECMAScript Internationalization API (`Intl`) implementation (ECMA-402).

use super::*;
use crate::shape::PropertyAttributes;

pub mod collator;
pub mod date_time_format;
pub mod display_names;
pub mod list_format;
pub mod locale;
pub mod number_format;
pub mod plural_rules;
pub mod segmenter;

#[allow(unused_imports)]
pub use locale::ParsedLocale;
pub use locale::canonicalize_locale_list;

pub(super) fn setup_prototype_and_constructor(
    vm: &mut Vm,
    ctor: JsValue,
    proto: JsValue,
    tag: &str,
    ctor_name: &str,
    parent_obj: JsValue,
) {
    vm.define_builtin_property(
        ctor,
        "prototype",
        proto,
        PropertyAttributes::NON_WRITABLE_NON_ENUMERABLE,
    );
    vm.define_builtin_property(proto, "constructor", ctor, PropertyAttributes::BUILTIN_METHOD);

    if let Some(sym) = vm.symbol_to_string_tag {
        let tag_val = vm.alloc_string(tag.to_string());
        vm.set_symbol_property(proto, sym, tag_val);
    }

    vm.define_builtin_property(parent_obj, ctor_name, ctor, PropertyAttributes::BUILTIN_METHOD);
}

pub(super) fn register_intl_builtins(vm: &mut Vm) {
    let intl_obj = vm.alloc_object();

    if let Some(sym) = vm.symbol_to_string_tag {
        let tag_val = vm.alloc_string("Intl".to_string());
        vm.set_symbol_property(intl_obj, sym, tag_val);
    }

    // Intl.getCanonicalLocales(locales)
    let get_canon_locales_fn =
        vm.alloc_native_function("getCanonicalLocales", 1, |vm, _this, args| {
            let arg = args.first().copied().unwrap_or(JsValue::undefined());
            let canons = canonicalize_locale_list(vm, arg)?;
            let items: Vec<JsValue> = canons.into_iter().map(|s| vm.alloc_string(s)).collect();
            Ok(vm.alloc_array(items))
        });
    vm.define_builtin_property(
        intl_obj,
        "getCanonicalLocales",
        get_canon_locales_fn,
        PropertyAttributes::BUILTIN_METHOD,
    );

    locale::register_locale(vm, intl_obj);
    number_format::register_number_format(vm, intl_obj);
    date_time_format::register_date_time_format(vm, intl_obj);
    collator::register_collator(vm, intl_obj);
    plural_rules::register_plural_rules(vm, intl_obj);
    list_format::register_list_format(vm, intl_obj);
    segmenter::register_segmenter(vm, intl_obj);
    display_names::register_display_names(vm, intl_obj);

    vm.globals.insert("Intl".to_string(), intl_obj);
    if vm.global_object.is_object() {
        vm.set_property(vm.global_object, "Intl", intl_obj);
    }
}
