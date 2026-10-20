//! `Intl.DisplayNames` implementation.

use super::locale::canonicalize_locale_list;
use super::setup_prototype_and_constructor;
use super::*;

pub(super) fn register_display_names(vm: &mut Vm, intl_obj: JsValue) {
    let dn_proto = vm.alloc_object();
    let dn_ctor = vm.alloc_native_function("DisplayNames", 2, |vm, this, args| {
        if !vm.is_constructing {
            return Err(VmError::TypeError("Constructor Intl.DisplayNames requires 'new'".into()));
        }
        let loc = args.first().copied().unwrap_or(JsValue::undefined());
        let canons = canonicalize_locale_list(vm, loc).unwrap_or_default();
        let locale_str = canons.into_iter().next().unwrap_or_else(|| "en-US".to_string());
        let loc_val = vm.alloc_string(locale_str);
        vm.set_property(this, "_locale", loc_val);

        let opt = args.get(1).copied().unwrap_or(JsValue::undefined());
        if !opt.is_object() {
            return Err(VmError::TypeError("Intl.DisplayNames requires an options object".into()));
        }

        let type_val = vm.get_property(opt, "type");
        if type_val.is_undefined() {
            return Err(VmError::TypeError("Intl.DisplayNames requires options.type".into()));
        }
        let type_str = vm.to_string_value(type_val);
        if !matches!(
            type_str.as_str(),
            "language" | "region" | "script" | "currency" | "calendar" | "dateTimeField"
        ) {
            return Err(VmError::RangeError(format!("Invalid DisplayNames type: {}", type_str)));
        }
        let t_val = vm.alloc_string(type_str);
        vm.set_property(this, "_type", t_val);

        let style_val = vm.get_property(opt, "style");
        let style_str = if !style_val.is_undefined() {
            vm.to_string_value(style_val)
        } else {
            "long".to_string()
        };
        let s_val = vm.alloc_string(style_str);
        vm.set_property(this, "_style", s_val);

        let fallback_val = vm.get_property(opt, "fallback");
        let fallback_str = if !fallback_val.is_undefined() {
            vm.to_string_value(fallback_val)
        } else {
            "code".to_string()
        };
        let f_val = vm.alloc_string(fallback_str);
        vm.set_property(this, "_fallback", f_val);

        Ok(this)
    });

    let dn_of_fn = vm.alloc_native_function("of", 1, |vm, this, args| {
        let code_val = args.first().copied().unwrap_or(JsValue::undefined());
        if code_val.is_undefined() {
            return Err(VmError::TypeError(
                "Intl.DisplayNames.prototype.of requires a code argument".into(),
            ));
        }
        let code = vm.to_string_value(code_val);
        let t_prop = vm.get_property(this, "_type");
        let type_str = vm.to_string_value(t_prop);
        let fb_prop = vm.get_property(this, "_fallback");
        let fallback_str = vm.to_string_value(fb_prop);

        let display = match type_str.as_str() {
            "region" => match code.to_ascii_uppercase().as_str() {
                "US" => Some("United States"),
                "GB" => Some("United Kingdom"),
                "FR" => Some("France"),
                "DE" => Some("Germany"),
                "JP" => Some("Japan"),
                "CN" => Some("China"),
                "CA" => Some("Canada"),
                "IT" => Some("Italy"),
                "ES" => Some("Spain"),
                "RU" => Some("Russia"),
                "IN" => Some("India"),
                "BR" => Some("Brazil"),
                "AU" => Some("Australia"),
                _ => None,
            },
            "language" => match code.to_ascii_lowercase().as_str() {
                "en" => Some("English"),
                "fr" => Some("French"),
                "de" => Some("German"),
                "es" => Some("Spanish"),
                "ja" => Some("Japanese"),
                "zh" => Some("Chinese"),
                "ru" => Some("Russian"),
                "it" => Some("Italian"),
                "pt" => Some("Portuguese"),
                "hi" => Some("Hindi"),
                "ar" => Some("Arabic"),
                _ => None,
            },
            "currency" => match code.to_ascii_uppercase().as_str() {
                "USD" => Some("US Dollar"),
                "EUR" => Some("Euro"),
                "GBP" => Some("British Pound"),
                "JPY" => Some("Japanese Yen"),
                "CNY" => Some("Chinese Yuan"),
                "INR" => Some("Indian Rupee"),
                _ => None,
            },
            "script" => match code.as_str() {
                "Latn" | "latn" => Some("Latin"),
                "Hans" | "hans" => Some("Simplified"),
                "Hant" | "hant" => Some("Traditional"),
                "Cyrl" | "cyrl" => Some("Cyrillic"),
                "Arab" | "arab" => Some("Arabic"),
                "Deva" | "deva" => Some("Devanagari"),
                _ => None,
            },
            _ => None,
        };

        if let Some(d) = display {
            Ok(vm.alloc_string(d.to_string()))
        } else if fallback_str == "none" {
            Ok(JsValue::undefined())
        } else {
            Ok(vm.alloc_string(code))
        }
    });
    vm.set_property(dn_proto, "of", dn_of_fn);
    vm.set_property_non_enumerable(dn_proto, "of".to_string());

    let dn_resolved_opts = vm.alloc_native_function("resolvedOptions", 0, |vm, this, _| {
        let opts = vm.alloc_object();
        let loc = vm.get_property(this, "_locale");
        let loc_val = if loc.is_js_string() { loc } else { vm.alloc_string("en-US".to_string()) };
        vm.set_property(opts, "locale", loc_val);

        let t = vm.get_property(this, "_type");
        vm.set_property(opts, "type", t);

        let s = vm.get_property(this, "_style");
        vm.set_property(opts, "style", s);

        let f = vm.get_property(this, "_fallback");
        vm.set_property(opts, "fallback", f);

        Ok(opts)
    });
    vm.set_property(dn_proto, "resolvedOptions", dn_resolved_opts);
    vm.set_property_non_enumerable(dn_proto, "resolvedOptions".to_string());

    let supported_locales_fn7 =
        vm.alloc_native_function("supportedLocalesOf", 1, |vm, _this, args| {
            let loc = args.first().copied().unwrap_or(JsValue::undefined());
            let canons = canonicalize_locale_list(vm, loc)?;
            let items: Vec<JsValue> = canons.into_iter().map(|s| vm.alloc_string(s)).collect();
            Ok(vm.alloc_array(items))
        });
    vm.set_property(dn_ctor, "supportedLocalesOf", supported_locales_fn7);
    vm.set_property_non_enumerable(dn_ctor, "supportedLocalesOf".to_string());

    setup_prototype_and_constructor(
        vm,
        dn_ctor,
        dn_proto,
        "Intl.DisplayNames",
        "DisplayNames",
        intl_obj,
    );
}
