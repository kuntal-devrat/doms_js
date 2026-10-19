//! `Intl.Segmenter` implementation.

use super::*;
use super::setup_prototype_and_constructor;
use super::locale::canonicalize_locale_list;

pub(super) fn register_segmenter(vm: &mut Vm, intl_obj: JsValue) {
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
        vm.set_property_non_enumerable(segments_proto, "[Symbol.toStringTag]".to_string());
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
    vm.set_property_non_enumerable(seg_proto, "segment".to_string());
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
    vm.set_property_non_enumerable(segments_proto, "containing".to_string());

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
    vm.set_property_non_enumerable(seg_proto, "resolvedOptions".to_string());

    let supported_locales_fn6 = vm.alloc_native_function("supportedLocalesOf", 1, |vm, _this, args| {
        let loc = args.first().copied().unwrap_or(JsValue::undefined());
        let canons = canonicalize_locale_list(vm, loc)?;
        let items: Vec<JsValue> = canons.into_iter().map(|s| vm.alloc_string(s)).collect();
        Ok(vm.alloc_array(items))
    });
    vm.set_property(seg_ctor, "supportedLocalesOf", supported_locales_fn6);
    vm.set_property_non_enumerable(seg_ctor, "supportedLocalesOf".to_string());

    setup_prototype_and_constructor(vm, seg_ctor, seg_proto, "Intl.Segmenter", "Segmenter", intl_obj);

}
