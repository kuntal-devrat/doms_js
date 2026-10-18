//! ECMAScript Internationalization API (`Intl`) implementation (ECMA-402).

use super::*;
use crate::interpreter::PreferredType;
use std::collections::HashSet;

/// BCP 47 parsed language tag per RFC 5646 / ECMA-402.
#[derive(Debug, Clone, Default)]
pub struct ParsedLocale {
    pub language: String,
    pub extlang: Option<String>,
    pub script: Option<String>,
    pub region: Option<String>,
    pub variants: Vec<String>,
    pub extensions: Vec<(char, Vec<String>)>,
    pub privateuse: Vec<String>,
}

impl ParsedLocale {
    pub fn parse(tag: &str) -> Result<Self, String> {
        if tag.is_empty() {
            return Err("Empty language tag".into());
        }
        let parts: Vec<&str> = tag.split('-').collect();
        if parts.iter().any(|p| p.is_empty()) {
            return Err("Language tag contains empty subtag".into());
        }

        // Subtag 0: language
        // 2-3 alpha, 4 alpha (reserved unless "root"), or 5-8 alpha
        let lang = parts[0];
        if !lang.chars().all(|c| c.is_ascii_alphabetic())
            || lang.len() < 2
            || (lang.len() == 4 && !lang.eq_ignore_ascii_case("root"))
            || lang.len() > 8
        {
            return Err(format!("Invalid language subtag: {}", lang));
        }

        let mut parsed = ParsedLocale {
            language: lang.to_ascii_lowercase(),
            ..Default::default()
        };

        let mut idx = 1;

        // In UTS 35 / ECMA-402, extlang is not allowed in Unicode locale identifiers
        if idx < parts.len() && parts[idx].len() == 3 && parts[idx].chars().all(|c| c.is_ascii_alphabetic()) {
            return Err(format!("Extlang subtags are not allowed in Unicode locale identifiers: {}", parts[idx]));
        }

        // Optional script (4 alpha)
        if idx < parts.len() && parts[idx].len() == 4 && parts[idx].chars().all(|c| c.is_ascii_alphabetic()) {
            let s = parts[idx];
            let title = format!("{}{}", s[..1].to_ascii_uppercase(), s[1..].to_ascii_lowercase());
            parsed.script = Some(title);
            idx += 1;
        }

        // Optional region (2 alpha or 3 digits)
        if idx < parts.len() {
            let r = parts[idx];
            if (r.len() == 2 && r.chars().all(|c| c.is_ascii_alphabetic()))
                || (r.len() == 3 && r.chars().all(|c| c.is_ascii_digit()))
            {
                parsed.region = Some(r.to_ascii_uppercase());
                idx += 1;
            }
        }

        // Optional variants (5-8 alphanum or 1 digit + 3 alphanum)
        while idx < parts.len() {
            let v = parts[idx];
            let is_variant = (v.len() >= 5 && v.len() <= 8 && v.chars().all(|c| c.is_ascii_alphanumeric()))
                || (v.len() == 4 && v.as_bytes()[0].is_ascii_digit() && v[1..].chars().all(|c| c.is_ascii_alphanumeric()));
            if is_variant {
                let v_lower = v.to_ascii_lowercase();
                if parsed.variants.contains(&v_lower) {
                    return Err(format!("Duplicate variant subtag: {}", v));
                }
                parsed.variants.push(v_lower);
                idx += 1;
            } else {
                break;
            }
        }

        // Extensions / privateuse
        let mut seen_singletons = HashSet::new();
        while idx < parts.len() {
            let ext_key = parts[idx];
            if ext_key.len() != 1 || !ext_key.chars().all(|c| c.is_ascii_alphanumeric()) {
                return Err(format!("Invalid extension singleton: {}", ext_key));
            }
            let key_char = ext_key.chars().next().unwrap().to_ascii_lowercase();
            if !seen_singletons.insert(key_char) {
                return Err(format!("Duplicate extension singleton: {}", key_char));
            }
            idx += 1;

            if key_char == 'x' {
                // Private use subtags (1-8 alphanum each)
                if idx >= parts.len() {
                    return Err("Private use tag missing subtags".into());
                }
                while idx < parts.len() {
                    let sub = parts[idx];
                    if sub.is_empty() || sub.len() > 8 || !sub.chars().all(|c| c.is_ascii_alphanumeric()) {
                        return Err(format!("Invalid private use subtag: {}", sub));
                    }
                    parsed.privateuse.push(sub.to_ascii_lowercase());
                    idx += 1;
                }
                break;
            } else {
                // Normal extension subtags (2-8 alphanum each)
                if idx >= parts.len() {
                    return Err(format!("Extension '{}' missing subtags", key_char));
                }
                let mut ext_subtags = Vec::new();
                while idx < parts.len() {
                    let sub = parts[idx];
                    if sub.len() == 1 && sub.chars().all(|c| c.is_ascii_alphanumeric()) {
                        // start of next singleton extension
                        break;
                    }
                    if sub.len() < 2 || sub.len() > 8 || !sub.chars().all(|c| c.is_ascii_alphanumeric()) {
                        return Err(format!("Invalid extension subtag: {}", sub));
                    }
                    ext_subtags.push(sub.to_ascii_lowercase());
                    idx += 1;
                }
                if ext_subtags.is_empty() {
                    return Err(format!("Extension '{}' missing subtags", key_char));
                }
                parsed.extensions.push((key_char, ext_subtags));
            }
        }

        parsed.canonicalize_aliases();
        Ok(parsed)
    }

    pub fn canonicalize_aliases(&mut self) {
        if self.language == "sgn" && self.region.as_deref() == Some("GR") {
            self.language = "gss".to_string();
            self.region = None;
        } else {
            self.language = match self.language.as_str() {
                "cmn" => "zh".to_string(),
                "ji" => "yi".to_string(),
                "in" => "id".to_string(),
                "iw" => "he".to_string(),
                "mo" => "ro".to_string(),
                _ => self.language.clone(),
            };
        }
        if let Some(ref r) = self.region {
            self.region = Some(match r.as_str() {
                "DD" => "DE".to_string(),
                "SU" => {
                    if self.script.as_deref() == Some("Armn") {
                        "AM".to_string()
                    } else {
                        "RU".to_string()
                    }
                }
                "YU" | "CS" => "RS".to_string(),
                "ZR" => "CD".to_string(),
                "TP" => "TL".to_string(),
                "BU" => "MM".to_string(),
                _ => r.clone(),
            });
        }
    }

    pub fn base_name(&self) -> String {
        let mut base = self.language.clone();
        if let Some(ref ext) = self.extlang {
            base.push('-');
            base.push_str(ext);
        }
        if let Some(ref script) = self.script {
            base.push('-');
            base.push_str(script);
        }
        if let Some(ref region) = self.region {
            base.push('-');
            base.push_str(region);
        }
        for v in &self.variants {
            base.push('-');
            base.push_str(v);
        }
        base
    }

    pub fn canonical_tag(&self) -> String {
        let mut parsed = self.clone();
        parsed.canonicalize_aliases();
        let mut tag = parsed.base_name();
        let mut sorted_exts = parsed.extensions.clone();
        sorted_exts.sort_by_key(|(c, _)| *c);
        for (k, subtags) in &sorted_exts {
            tag.push('-');
            tag.push(*k);
            let mut i = 0;
            while i < subtags.len() {
                let sub = &subtags[i];
                if *k == 'u' && sub == "true" && i > 0 && matches!(subtags[i - 1].as_str(), "kn") {
                    i += 1;
                    continue;
                }
                tag.push('-');
                tag.push_str(sub);
                i += 1;
            }
        }
        if !parsed.privateuse.is_empty() {
            tag.push_str("-x");
            for sub in &parsed.privateuse {
                tag.push('-');
                tag.push_str(sub);
            }
        }
        tag
    }

    pub fn get_unicode_extension(&self, key: &str) -> Option<String> {
        for (singleton, subtags) in &self.extensions {
            if *singleton == 'u' {
                for i in 0..subtags.len() {
                    if subtags[i] == key {
                        if i + 1 < subtags.len() && (subtags[i + 1].len() >= 3 || subtags[i + 1] == "false") {
                            return Some(subtags[i + 1].clone());
                        } else {
                            return Some("true".to_string());
                        }
                    }
                }
            }
        }
        None
    }
}

pub fn canonicalize_locale_list(vm: &mut Vm, locales: JsValue) -> Result<Vec<String>, VmError> {
    if locales.is_undefined() {
        return Ok(Vec::new());
    }
    let mut raw_strings = Vec::new();
    if locales.is_js_string() {
        raw_strings.push(vm.to_string_value(locales));
    } else if locales.is_object() {
        let tag_prop = vm.get_property(locales, "_tag");
        if !tag_prop.is_undefined() {
            raw_strings.push(vm.to_string_value(tag_prop));
        } else {
            let len_val = vm.get_property(locales, "length");
            if let Some(len) = len_val.as_int32() {
                let len = len.max(0) as usize;
                for i in 0..len {
                    let elem = vm.get_property(locales, &i.to_string());
                    if elem.is_undefined() {
                        continue;
                    }
                    if !elem.is_js_string() && !elem.is_object() {
                        return Err(VmError::TypeError("Locale elements must be strings or objects".into()));
                    }
                    if elem.is_object() {
                        let elem_tag = vm.get_property(elem, "_tag");
                        if !elem_tag.is_undefined() {
                            raw_strings.push(vm.to_string_value(elem_tag));
                        } else {
                            let prim = vm.to_primitive(elem, PreferredType::String)?;
                            raw_strings.push(vm.to_string_value(prim));
                        }
                    } else {
                        raw_strings.push(vm.to_string_value(elem));
                    }
                }
            } else {
                let prim = vm.to_primitive(locales, PreferredType::String)?;
                raw_strings.push(vm.to_string_value(prim));
            }
        }
    } else {
        return Err(VmError::TypeError("Locales must be a string or array/object".into()));
    }

    let mut seen = HashSet::new();
    let mut result = Vec::new();
    for s in raw_strings {
        let parsed = ParsedLocale::parse(&s).map_err(|e| VmError::RangeError(format!("Invalid language tag: {}", e)))?;
        let canon = parsed.canonical_tag();
        if seen.insert(canon.clone()) {
            result.push(canon);
        }
    }
    Ok(result)
}

fn setup_prototype_and_constructor(
    vm: &mut Vm,
    ctor: JsValue,
    proto: JsValue,
    tag: &str,
    ctor_name: &str,
    parent_obj: JsValue,
) {
    vm.set_property(ctor, "prototype", proto);
    vm.non_enumerable_props.insert((ctor, "prototype".to_string()));
    vm.non_writable_props.insert((ctor, "prototype".to_string()));
    vm.non_configurable_props.insert((ctor, "prototype".to_string()));

    vm.set_property(proto, "constructor", ctor);
    vm.non_enumerable_props.insert((proto, "constructor".to_string()));

    if let Some(sym) = vm.symbol_to_string_tag {
        let tag_val = vm.alloc_string(tag.to_string());
        vm.set_symbol_property(proto, sym, tag_val);
        vm.non_enumerable_props.insert((proto, "[Symbol.toStringTag]".to_string()));
    }

    vm.set_property(parent_obj, ctor_name, ctor);
    vm.non_enumerable_props.insert((parent_obj, ctor_name.to_string()));
}

pub(super) fn register_intl_builtins(vm: &mut Vm) {
    let intl_obj = vm.alloc_object();

    if let Some(sym) = vm.symbol_to_string_tag {
        let tag_val = vm.alloc_string("Intl".to_string());
        vm.set_symbol_property(intl_obj, sym, tag_val);
        vm.non_enumerable_props.insert((intl_obj, "[Symbol.toStringTag]".to_string()));
    }

    // Intl.getCanonicalLocales(locales)
    let get_canon_locales_fn = vm.alloc_native_function("getCanonicalLocales", 1, |vm, _this, args| {
        let arg = args.first().copied().unwrap_or(JsValue::undefined());
        let canons = canonicalize_locale_list(vm, arg)?;
        let items: Vec<JsValue> = canons.into_iter().map(|s| vm.alloc_string(s)).collect();
        Ok(vm.alloc_array(items))
    });
    vm.set_property(intl_obj, "getCanonicalLocales", get_canon_locales_fn);
    vm.non_enumerable_props.insert((intl_obj, "getCanonicalLocales".to_string()));

    // Intl.Locale
    let locale_proto = vm.alloc_object();
    let locale_ctor = vm.alloc_native_function("Locale", 1, |vm, this, args| {
        if !vm.is_constructing {
            return Err(VmError::TypeError("Constructor Intl.Locale requires 'new'".into()));
        }
        let tag_arg = args.first().copied().unwrap_or(JsValue::undefined());
        if tag_arg.is_undefined() {
            return Err(VmError::TypeError("Intl.Locale requires a tag argument".into()));
        }
        let tag_str = if tag_arg.is_object() {
            let tag_prop = vm.get_property(tag_arg, "_tag");
            if !tag_prop.is_undefined() {
                vm.to_string_value(tag_prop)
            } else {
                let prim = vm.to_primitive(tag_arg, PreferredType::String)?;
                vm.to_string_value(prim)
            }
        } else {
            vm.to_string_value(tag_arg)
        };
        let mut parsed = ParsedLocale::parse(&tag_str)
            .map_err(|e| VmError::RangeError(format!("Invalid language tag: {}", e)))?;

        // Handle options object
        if let Some(opt_val) = args.get(1) {
            if opt_val.is_object() {
                let opt = *opt_val;

                // 1. language
                let lang_val = vm.get_property(opt, "language");
                if !lang_val.is_undefined() {
                    let l_prim = vm.to_primitive(lang_val, PreferredType::String)?;
                    let l_str = vm.to_string_value(l_prim);
                    if !l_str.chars().all(|c| c.is_ascii_alphabetic()) || l_str.len() < 2 || l_str.len() > 8 {
                        return Err(VmError::RangeError(format!("Invalid language subtag: {}", l_str)));
                    }
                    parsed.language = l_str.to_ascii_lowercase();
                }

                // 2. script
                let script_val = vm.get_property(opt, "script");
                if !script_val.is_undefined() {
                    let s_prim = vm.to_primitive(script_val, PreferredType::String)?;
                    let s_str = vm.to_string_value(s_prim);
                    if s_str.len() != 4 || !s_str.chars().all(|c| c.is_ascii_alphabetic()) {
                        return Err(VmError::RangeError(format!("Invalid script subtag: {}", s_str)));
                    }
                    parsed.script = Some(format!("{}{}", s_str[..1].to_ascii_uppercase(), s_str[1..].to_ascii_lowercase()));
                }

                // 3. region
                let region_val = vm.get_property(opt, "region");
                if !region_val.is_undefined() {
                    let r_prim = vm.to_primitive(region_val, PreferredType::String)?;
                    let r_str = vm.to_string_value(r_prim);
                    let is_reg = (r_str.len() == 2 && r_str.chars().all(|c| c.is_ascii_alphabetic()))
                        || (r_str.len() == 3 && r_str.chars().all(|c| c.is_ascii_digit()));
                    if !is_reg {
                        return Err(VmError::RangeError(format!("Invalid region subtag: {}", r_str)));
                    }
                    parsed.region = Some(r_str.to_ascii_uppercase());
                }

                // 4. variants
                let variants_val = vm.get_property(opt, "variants");
                if !variants_val.is_undefined() {
                    let v_prim = vm.to_primitive(variants_val, PreferredType::String)?;
                    let v_str = vm.to_string_value(v_prim);
                    for sub in v_str.split('-') {
                        let is_variant = (sub.len() >= 5 && sub.len() <= 8 && sub.chars().all(|c| c.is_ascii_alphanumeric()))
                            || (sub.len() == 4 && sub.as_bytes().first().map_or(false, |b| b.is_ascii_digit()) && sub[1..].chars().all(|c| c.is_ascii_alphanumeric()));
                        if !is_variant {
                            return Err(VmError::RangeError(format!("Invalid variant subtag: {}", sub)));
                        }
                        let sub_lower = sub.to_ascii_lowercase();
                        if !parsed.variants.contains(&sub_lower) {
                            parsed.variants.push(sub_lower);
                        }
                    }
                }

                // 5-8. calendar, collation, hourCycle, caseFirst
                for (prop, ext_key) in [
                    ("calendar", "ca"),
                    ("collation", "co"),
                    ("hourCycle", "hc"),
                    ("caseFirst", "kf"),
                ] {
                    let v = vm.get_property(opt, prop);
                    if !v.is_undefined() {
                        let v_prim = vm.to_primitive(v, PreferredType::String)?;
                        let v_str = vm.to_string_value(v_prim).to_ascii_lowercase();
                        if let Some((_, subtags)) = parsed.extensions.iter_mut().find(|(c, _)| *c == 'u') {
                            let mut found = false;
                            for i in 0..subtags.len() {
                                if subtags[i] == ext_key {
                                    if i + 1 < subtags.len() {
                                        subtags[i + 1] = v_str.clone();
                                    } else {
                                        subtags.push(v_str.clone());
                                    }
                                    found = true;
                                    break;
                                }
                            }
                            if !found {
                                subtags.push(ext_key.to_string());
                                subtags.push(v_str);
                            }
                        } else {
                            parsed.extensions.push(('u', vec![ext_key.to_string(), v_str]));
                        }
                    }
                }

                // 9. numeric (ToBoolean)
                let num_val = vm.get_property(opt, "numeric");
                if !num_val.is_undefined() {
                    let b = num_val.is_truthy();
                    let v_str = if b { "true" } else { "false" };
                    if let Some((_, subtags)) = parsed.extensions.iter_mut().find(|(c, _)| *c == 'u') {
                        let mut found = false;
                        for i in 0..subtags.len() {
                            if subtags[i] == "kn" {
                                if i + 1 < subtags.len() && (subtags[i + 1] == "true" || subtags[i + 1] == "false") {
                                    subtags[i + 1] = v_str.to_string();
                                } else {
                                    subtags.insert(i + 1, v_str.to_string());
                                }
                                found = true;
                                break;
                            }
                        }
                        if !found {
                            subtags.push("kn".to_string());
                            subtags.push(v_str.to_string());
                        }
                    } else {
                        parsed.extensions.push(('u', vec!["kn".to_string(), v_str.to_string()]));
                    }
                }

                // 10. numberingSystem
                let nu_val = vm.get_property(opt, "numberingSystem");
                if !nu_val.is_undefined() {
                    let nu_prim = vm.to_primitive(nu_val, PreferredType::String)?;
                    let nu_str = vm.to_string_value(nu_prim).to_ascii_lowercase();
                    if let Some((_, subtags)) = parsed.extensions.iter_mut().find(|(c, _)| *c == 'u') {
                        let mut found = false;
                        for i in 0..subtags.len() {
                            if subtags[i] == "nu" {
                                if i + 1 < subtags.len() {
                                    subtags[i + 1] = nu_str.clone();
                                } else {
                                    subtags.push(nu_str.clone());
                                }
                                found = true;
                                break;
                            }
                        }
                        if !found {
                            subtags.push("nu".to_string());
                            subtags.push(nu_str);
                        }
                    } else {
                        parsed.extensions.push(('u', vec!["nu".to_string(), nu_str]));
                    }
                }

                parsed.canonicalize_aliases();
            }
        }

        let base_name = parsed.base_name();
        let canon = parsed.canonical_tag();

        let base_val = vm.alloc_string(base_name);
        vm.set_property(this, "_baseName", base_val);
        let tag_val = vm.alloc_string(canon);
        vm.set_property(this, "_tag", tag_val);

        let lang_val = vm.alloc_string(parsed.language.clone());
        vm.set_property(this, "_language", lang_val);

        let script_val = parsed.script.as_ref().map(|s| vm.alloc_string(s.clone())).unwrap_or(JsValue::undefined());
        vm.set_property(this, "_script", script_val);

        let region_val = parsed.region.as_ref().map(|r| vm.alloc_string(r.clone())).unwrap_or(JsValue::undefined());
        vm.set_property(this, "_region", region_val);

        let cal_val = parsed.get_unicode_extension("ca").map(|s| vm.alloc_string(s)).unwrap_or(JsValue::undefined());
        vm.set_property(this, "_calendar", cal_val);

        let col_val = parsed.get_unicode_extension("co").map(|s| vm.alloc_string(s)).unwrap_or(JsValue::undefined());
        vm.set_property(this, "_collation", col_val);

        let hc_val = parsed.get_unicode_extension("hc").map(|s| vm.alloc_string(s)).unwrap_or(JsValue::undefined());
        vm.set_property(this, "_hourCycle", hc_val);

        let kf_val = parsed.get_unicode_extension("kf").map(|s| vm.alloc_string(s)).unwrap_or(JsValue::undefined());
        vm.set_property(this, "_caseFirst", kf_val);

        let nu_val = parsed.get_unicode_extension("nu").map(|s| vm.alloc_string(s)).unwrap_or(JsValue::undefined());
        vm.set_property(this, "_numberingSystem", nu_val);

        let kn_val = parsed.get_unicode_extension("kn").map(|s| JsValue::from_boolean(s == "true")).unwrap_or(JsValue::undefined());
        vm.set_property(this, "_numeric", kn_val);

        Ok(this)
    });

    // Locale prototype getters
    macro_rules! define_locale_getter {
        ($prop:expr, $field:expr) => {{
            let getter = vm.alloc_native_function(concat!("get ", $prop), 0, |vm, this, _| {
                if !this.is_object() {
                    return Err(VmError::TypeError(concat!("Method Intl.Locale.prototype.", $prop, " called on non-object").into()));
                }
                Ok(vm.get_property(this, $field))
            });
            if let Some(ptr) = locale_proto.as_object_ptr() {
                let obj = unsafe { &mut *(ptr as *mut JsObject) };
                obj.define_getter($prop.to_string(), getter);
            }
            vm.non_enumerable_props.insert((locale_proto, $prop.to_string()));
        }};
    }

    define_locale_getter!("baseName", "_baseName");
    define_locale_getter!("language", "_language");
    define_locale_getter!("script", "_script");
    define_locale_getter!("region", "_region");
    define_locale_getter!("calendar", "_calendar");
    define_locale_getter!("collation", "_collation");
    define_locale_getter!("hourCycle", "_hourCycle");
    define_locale_getter!("caseFirst", "_caseFirst");
    define_locale_getter!("numeric", "_numeric");
    define_locale_getter!("numberingSystem", "_numberingSystem");

    let loc_to_string_fn = vm.alloc_native_function("toString", 0, |vm, this, _| {
        if !this.is_object() {
            return Err(VmError::TypeError("Method Intl.Locale.prototype.toString called on non-object".into()));
        }
        let tag = vm.get_property(this, "_tag");
        if tag.is_js_string() {
            Ok(tag)
        } else {
            Ok(vm.alloc_string("und".to_string()))
        }
    });
    vm.set_property(locale_proto, "toString", loc_to_string_fn);
    vm.non_enumerable_props.insert((locale_proto, "toString".to_string()));

    let loc_maximize_fn = vm.alloc_native_function("maximize", 0, |vm, this, _| {
        if !this.is_object() {
            return Err(VmError::TypeError("Method Intl.Locale.prototype.maximize called on non-object".into()));
        }
        let tag_val = vm.get_property(this, "_tag");
        let tag = vm.to_string_value(tag_val);
        // Common likely subtags
        let maximized = match tag.as_str() {
            "en" => "en-Latn-US",
            "zh" => "zh-Hans-CN",
            "es" => "es-Latn-ES",
            "fr" => "fr-Latn-FR",
            "de" => "de-Latn-DE",
            "ja" => "ja-Jpan-JP",
            "ko" => "ko-Kore-KR",
            "ru" => "ru-Cyrl-RU",
            "ar" => "ar-Arab-EG",
            "hi" => "hi-Deva-IN",
            "pt" => "pt-Latn-BR",
            "it" => "it-Latn-IT",
            _ => tag.as_str(),
        };
        let ctor = vm.get_property(vm.global_object, "Intl");
        let loc_ctor = vm.get_property(ctor, "Locale");
        let str_val = vm.alloc_string(maximized.to_string());
        vm.construct(loc_ctor, &[str_val])
    });
    vm.set_property(locale_proto, "maximize", loc_maximize_fn);
    vm.non_enumerable_props.insert((locale_proto, "maximize".to_string()));

    let loc_minimize_fn = vm.alloc_native_function("minimize", 0, |vm, this, _| {
        if !this.is_object() {
            return Err(VmError::TypeError("Method Intl.Locale.prototype.minimize called on non-object".into()));
        }
        let base_val = vm.get_property(this, "_baseName");
        let base = vm.to_string_value(base_val);
        let minimized = match base.as_str() {
            "en-Latn-US" | "en-US" => "en",
            "zh-Hans-CN" | "zh-CN" => "zh",
            "es-Latn-ES" | "es-ES" => "es",
            "fr-Latn-FR" | "fr-FR" => "fr",
            "de-Latn-DE" | "de-DE" => "de",
            "ja-Jpan-JP" | "ja-JP" => "ja",
            "ko-Kore-KR" | "ko-KR" => "ko",
            "ru-Cyrl-RU" | "ru-RU" => "ru",
            _ => base.as_str(),
        };
        let ctor = vm.get_property(vm.global_object, "Intl");
        let loc_ctor = vm.get_property(ctor, "Locale");
        let str_val = vm.alloc_string(minimized.to_string());
        vm.construct(loc_ctor, &[str_val])
    });
    vm.set_property(locale_proto, "minimize", loc_minimize_fn);
    vm.non_enumerable_props.insert((locale_proto, "minimize".to_string()));

    setup_prototype_and_constructor(vm, locale_ctor, locale_proto, "Intl.Locale", "Locale", intl_obj);

    // Intl.NumberFormat
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
    vm.non_enumerable_props.insert((num_format_proto, "format".to_string()));

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
    vm.non_enumerable_props.insert((num_format_proto, "formatToParts".to_string()));

    let nf_resolved_opts = vm.alloc_native_function("resolvedOptions", 0, |vm, this, _| {
        let opts = vm.alloc_object();
        let loc = vm.get_property(this, "_locale");
        let loc_val = if loc.is_js_string() { loc } else { vm.alloc_string("en-US".to_string()) };
        vm.set_property(opts, "locale", loc_val);
        let num_sys = vm.alloc_string("latn".to_string());
        vm.set_property(opts, "numberingSystem", num_sys);
        let style = vm.get_property(this, "_style");
        let style_val = if style.is_js_string() { style } else { vm.alloc_string("decimal".to_string()) };
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
    vm.non_enumerable_props.insert((num_format_proto, "resolvedOptions".to_string()));

    let supported_locales_fn = vm.alloc_native_function("supportedLocalesOf", 1, |vm, _this, args| {
        let loc = args.first().copied().unwrap_or(JsValue::undefined());
        let canons = canonicalize_locale_list(vm, loc)?;
        let items: Vec<JsValue> = canons.into_iter().map(|s| vm.alloc_string(s)).collect();
        Ok(vm.alloc_array(items))
    });
    vm.set_property(num_format_ctor, "supportedLocalesOf", supported_locales_fn);
    vm.non_enumerable_props.insert((num_format_ctor, "supportedLocalesOf".to_string()));

    setup_prototype_and_constructor(vm, num_format_ctor, num_format_proto, "Intl.NumberFormat", "NumberFormat", intl_obj);

    // Intl.DateTimeFormat
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

    // Intl.Collator
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
    vm.non_enumerable_props.insert((collator_proto, "compare".to_string()));

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
    vm.non_enumerable_props.insert((collator_proto, "resolvedOptions".to_string()));

    let supported_locales_fn3 = vm.alloc_native_function("supportedLocalesOf", 1, |vm, _this, args| {
        let loc = args.first().copied().unwrap_or(JsValue::undefined());
        let canons = canonicalize_locale_list(vm, loc)?;
        let items: Vec<JsValue> = canons.into_iter().map(|s| vm.alloc_string(s)).collect();
        Ok(vm.alloc_array(items))
    });
    vm.set_property(collator_ctor, "supportedLocalesOf", supported_locales_fn3);
    vm.non_enumerable_props.insert((collator_ctor, "supportedLocalesOf".to_string()));

    setup_prototype_and_constructor(vm, collator_ctor, collator_proto, "Intl.Collator", "Collator", intl_obj);

    // Intl.PluralRules
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
    vm.non_enumerable_props.insert((pr_proto, "select".to_string()));

    let pr_resolved_opts = vm.alloc_native_function("resolvedOptions", 0, |vm, this, _| {
        let opts = vm.alloc_object();
        let loc = vm.get_property(this, "_locale");
        let loc_val = if loc.is_js_string() { loc } else { vm.alloc_string("en-US".to_string()) };
        vm.set_property(opts, "locale", loc_val);

        let pr_type = vm.get_property(this, "_type");
        let pr_type_val = if pr_type.is_js_string() { pr_type } else { vm.alloc_string("cardinal".to_string()) };
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
    vm.non_enumerable_props.insert((pr_proto, "resolvedOptions".to_string()));

    let supported_locales_fn4 = vm.alloc_native_function("supportedLocalesOf", 1, |vm, _this, args| {
        let loc = args.first().copied().unwrap_or(JsValue::undefined());
        let canons = canonicalize_locale_list(vm, loc)?;
        let items: Vec<JsValue> = canons.into_iter().map(|s| vm.alloc_string(s)).collect();
        Ok(vm.alloc_array(items))
    });
    vm.set_property(pr_ctor, "supportedLocalesOf", supported_locales_fn4);
    vm.non_enumerable_props.insert((pr_ctor, "supportedLocalesOf".to_string()));

    setup_prototype_and_constructor(vm, pr_ctor, pr_proto, "Intl.PluralRules", "PluralRules", intl_obj);

    // Intl.ListFormat
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
            return Err(VmError::TypeError("Intl.ListFormat.prototype.format requires an iterable".into()));
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
    vm.non_enumerable_props.insert((lf_proto, "format".to_string()));

    let lf_format_to_parts_fn = vm.alloc_native_function("formatToParts", 1, |vm, this, args| {
        let list_val = args.first().copied().unwrap_or(JsValue::undefined());
        if !list_val.is_object() {
            return Err(VmError::TypeError("Intl.ListFormat.prototype.formatToParts requires an iterable".into()));
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
            0 => {},
            1 => {
                let obj = vm.alloc_object();
                let type_val = vm.alloc_string("element".to_string());
                vm.set_property(obj, "type", type_val);
                let val_val = vm.alloc_string(strings[0].clone());
                vm.set_property(obj, "value", val_val);
                parts.push(obj);
            },
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
            },
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
    vm.non_enumerable_props.insert((lf_proto, "formatToParts".to_string()));

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
    vm.non_enumerable_props.insert((lf_proto, "resolvedOptions".to_string()));

    let supported_locales_fn5 = vm.alloc_native_function("supportedLocalesOf", 1, |vm, _this, args| {
        let loc = args.first().copied().unwrap_or(JsValue::undefined());
        let canons = canonicalize_locale_list(vm, loc)?;
        let items: Vec<JsValue> = canons.into_iter().map(|s| vm.alloc_string(s)).collect();
        Ok(vm.alloc_array(items))
    });
    vm.set_property(lf_ctor, "supportedLocalesOf", supported_locales_fn5);
    vm.non_enumerable_props.insert((lf_ctor, "supportedLocalesOf".to_string()));

    setup_prototype_and_constructor(vm, lf_ctor, lf_proto, "Intl.ListFormat", "ListFormat", intl_obj);

    // Intl.Segmenter
    let seg_proto = vm.alloc_object();
    let seg_ctor = vm.alloc_native_function("Segmenter", 0, |vm, this, args| {
        if !vm.is_constructing {
            return Err(VmError::TypeError("Constructor Intl.Segmenter requires 'new'".into()));
        }
        let loc = args.first().copied().unwrap_or(JsValue::undefined());
        let canons = canonicalize_locale_list(vm, loc).unwrap_or_default();
        let locale_str = canons.into_iter().next().unwrap_or_else(|| "en-US".to_string());
        let loc_val = vm.alloc_string(locale_str);
        vm.set_property(this, "_locale", loc_val);

        let opt = args.get(1).copied().unwrap_or(JsValue::undefined());
        let granularity = if opt.is_object() {
            let g = vm.get_property(opt, "granularity");
            if !g.is_undefined() { vm.to_string_value(g) } else { "grapheme".to_string() }
        } else {
            "grapheme".to_string()
        };
        let g_val = vm.alloc_string(granularity);
        vm.set_property(this, "_granularity", g_val);

        Ok(this)
    });

    let segments_proto = vm.alloc_object();
    if let Some(sym) = vm.symbol_to_string_tag {
        let tag_val = vm.alloc_string("Segments".to_string());
        vm.set_symbol_property(segments_proto, sym, tag_val);
        vm.non_enumerable_props.insert((segments_proto, "[Symbol.toStringTag]".to_string()));
    }

    let seg_segment_fn = vm.alloc_native_function("segment", 1, |vm, this, args| {
        let text_val = args.first().copied().unwrap_or(JsValue::undefined());
        let text = vm.to_string_value(text_val);

        let segments_obj = vm.alloc_object();
        if let Some(ptr) = segments_obj.as_object_ptr() {
            let seg_ctor = vm.get_property(vm.global_object, "Intl");
            let s_ctor = vm.get_property(seg_ctor, "Segmenter");
            let s_proto = vm.get_property(s_ctor, "prototype");
            let sp = vm.get_property(s_proto, "_segmentsProto");
            if sp.is_object() {
                unsafe { (*(ptr as *mut JsObject)).prototype = Some(sp) };
            }
        }

        let str_val = vm.alloc_string(text);
        vm.set_property(segments_obj, "_string", str_val);
        let g_val = vm.get_property(this, "_granularity");
        vm.set_property(segments_obj, "_granularity", g_val);
        Ok(segments_obj)
    });
    vm.set_property(seg_proto, "segment", seg_segment_fn);
    vm.non_enumerable_props.insert((seg_proto, "segment".to_string()));
    vm.set_property(seg_proto, "_segmentsProto", segments_proto);

    let segments_containing_fn = vm.alloc_native_function("containing", 1, |vm, this, args| {
        let s_val = vm.get_property(this, "_string");
        let text = vm.to_string_value(s_val);
        let idx = args.first().and_then(|v| v.as_int32()).unwrap_or(0).max(0) as usize;
        let seg_res = vm.alloc_object();
        if idx >= text.len() {
            return Ok(JsValue::undefined());
        }
        let ch = text[idx..].chars().next().unwrap_or(' ');
        let seg_str = vm.alloc_string(ch.to_string());
        vm.set_property(seg_res, "segment", seg_str);
        vm.set_property(seg_res, "index", JsValue::from_int32(idx as i32));
        vm.set_property(seg_res, "input", s_val);
        vm.set_property(seg_res, "isWordLike", JsValue::from_boolean(ch.is_alphanumeric()));
        Ok(seg_res)
    });
    vm.set_property(segments_proto, "containing", segments_containing_fn);
    vm.non_enumerable_props.insert((segments_proto, "containing".to_string()));

    let seg_resolved_opts = vm.alloc_native_function("resolvedOptions", 0, |vm, this, _| {
        let opts = vm.alloc_object();
        let loc = vm.get_property(this, "_locale");
        let loc_val = if loc.is_js_string() { loc } else { vm.alloc_string("en-US".to_string()) };
        vm.set_property(opts, "locale", loc_val);

        let g = vm.get_property(this, "_granularity");
        let g_val = if g.is_js_string() { g } else { vm.alloc_string("grapheme".to_string()) };
        vm.set_property(opts, "granularity", g_val);

        Ok(opts)
    });
    vm.set_property(seg_proto, "resolvedOptions", seg_resolved_opts);
    vm.non_enumerable_props.insert((seg_proto, "resolvedOptions".to_string()));

    let supported_locales_fn6 = vm.alloc_native_function("supportedLocalesOf", 1, |vm, _this, args| {
        let loc = args.first().copied().unwrap_or(JsValue::undefined());
        let canons = canonicalize_locale_list(vm, loc)?;
        let items: Vec<JsValue> = canons.into_iter().map(|s| vm.alloc_string(s)).collect();
        Ok(vm.alloc_array(items))
    });
    vm.set_property(seg_ctor, "supportedLocalesOf", supported_locales_fn6);
    vm.non_enumerable_props.insert((seg_ctor, "supportedLocalesOf".to_string()));

    setup_prototype_and_constructor(vm, seg_ctor, seg_proto, "Intl.Segmenter", "Segmenter", intl_obj);

    // Intl.DisplayNames
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
        if !matches!(type_str.as_str(), "language" | "region" | "script" | "currency" | "calendar" | "dateTimeField") {
            return Err(VmError::RangeError(format!("Invalid DisplayNames type: {}", type_str)));
        }
        let t_val = vm.alloc_string(type_str);
        vm.set_property(this, "_type", t_val);

        let style_val = vm.get_property(opt, "style");
        let style_str = if !style_val.is_undefined() { vm.to_string_value(style_val) } else { "long".to_string() };
        let s_val = vm.alloc_string(style_str);
        vm.set_property(this, "_style", s_val);

        let fallback_val = vm.get_property(opt, "fallback");
        let fallback_str = if !fallback_val.is_undefined() { vm.to_string_value(fallback_val) } else { "code".to_string() };
        let f_val = vm.alloc_string(fallback_str);
        vm.set_property(this, "_fallback", f_val);

        Ok(this)
    });

    let dn_of_fn = vm.alloc_native_function("of", 1, |vm, this, args| {
        let code_val = args.first().copied().unwrap_or(JsValue::undefined());
        if code_val.is_undefined() {
            return Err(VmError::TypeError("Intl.DisplayNames.prototype.of requires a code argument".into()));
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
    vm.non_enumerable_props.insert((dn_proto, "of".to_string()));

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
    vm.non_enumerable_props.insert((dn_proto, "resolvedOptions".to_string()));

    let supported_locales_fn7 = vm.alloc_native_function("supportedLocalesOf", 1, |vm, _this, args| {
        let loc = args.first().copied().unwrap_or(JsValue::undefined());
        let canons = canonicalize_locale_list(vm, loc)?;
        let items: Vec<JsValue> = canons.into_iter().map(|s| vm.alloc_string(s)).collect();
        Ok(vm.alloc_array(items))
    });
    vm.set_property(dn_ctor, "supportedLocalesOf", supported_locales_fn7);
    vm.non_enumerable_props.insert((dn_ctor, "supportedLocalesOf".to_string()));

    setup_prototype_and_constructor(vm, dn_ctor, dn_proto, "Intl.DisplayNames", "DisplayNames", intl_obj);

    vm.globals.insert("Intl".to_string(), intl_obj);
    if vm.global_object.is_object() {
        vm.set_property(vm.global_object, "Intl", intl_obj);
    }
}
