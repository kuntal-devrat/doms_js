//! `Intl.Locale` and BCP 47 language tag parsing.

use super::*;
use crate::interpreter::PreferredType;
use std::collections::HashSet;
use super::setup_prototype_and_constructor;

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

pub(super) fn register_locale(vm: &mut Vm, intl_obj: JsValue) {
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
            vm.set_property_non_enumerable(locale_proto, $prop.to_string());
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
    vm.set_property_non_enumerable(locale_proto, "toString".to_string());

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
    vm.set_property_non_enumerable(locale_proto, "maximize".to_string());

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
    vm.set_property_non_enumerable(locale_proto, "minimize".to_string());

    setup_prototype_and_constructor(vm, locale_ctor, locale_proto, "Intl.Locale", "Locale", intl_obj);

}
