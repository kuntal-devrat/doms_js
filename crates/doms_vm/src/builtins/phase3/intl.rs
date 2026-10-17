//! Minimal `Intl` namespace shims.

use super::*;

pub(super) fn register_intl_builtins(vm: &mut Vm) {
    // ── Intl ──
    let intl_obj = vm.alloc_object();

    let num_format_ctor = vm.alloc_native_function("NumberFormat", 0, |vm, _this, _args| {
        let nf_obj = vm.alloc_object();
        let prev_temp = vm.temp_roots.len();
        vm.temp_roots.push(nf_obj);
        let format_fn = vm.alloc_native_function("format", 1, |vm, _this, args| {
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
        let cur_nf = vm.temp_roots[prev_temp];
        vm.set_property(cur_nf, "format", format_fn);
        let resolved_opts = vm.alloc_native_function("resolvedOptions", 0, |vm, _this, _args| {
            let opts = vm.alloc_object();
            let prev = vm.temp_roots.len();
            vm.temp_roots.push(opts);
            let loc = vm.alloc_string("en-US".to_string());
            let cur_opts = vm.temp_roots[prev];
            vm.set_property(cur_opts, "locale", loc);
            let res = vm.temp_roots[prev];
            vm.temp_roots.truncate(prev);
            Ok(res)
        });
        let cur_nf = vm.temp_roots[prev_temp];
        vm.set_property(cur_nf, "resolvedOptions", resolved_opts);
        let res = vm.temp_roots[prev_temp];
        vm.temp_roots.truncate(prev_temp);
        Ok(res)
    });
    let empty_locales_fn = vm.alloc_native_function("supportedLocalesOf", 1, |vm, _this, _args| {
        Ok(vm.alloc_array(Vec::new()))
    });
    vm.set_property(num_format_ctor, "supportedLocalesOf", empty_locales_fn);
    vm.set_property(intl_obj, "NumberFormat", num_format_ctor);

    let date_format_ctor = vm.alloc_native_function("DateTimeFormat", 0, |vm, _this, _args| {
        let df_obj = vm.alloc_object();
        let prev_temp = vm.temp_roots.len();
        vm.temp_roots.push(df_obj);
        let format_fn = vm.alloc_native_function("format", 1, |vm, _this, args| {
            let s = args.first().map(|v| vm.to_string_value(*v)).unwrap_or_else(|| "2026-10-06T00:00:00Z".to_string());
            Ok(vm.alloc_string(s))
        });
        let cur_df = vm.temp_roots[prev_temp];
        vm.set_property(cur_df, "format", format_fn);
        let resolved_opts = vm.alloc_native_function("resolvedOptions", 0, |vm, _this, _args| {
            let opts = vm.alloc_object();
            let prev = vm.temp_roots.len();
            vm.temp_roots.push(opts);
            let loc = vm.alloc_string("en-US".to_string());
            let cur_opts = vm.temp_roots[prev];
            vm.set_property(cur_opts, "locale", loc);
            let res = vm.temp_roots[prev];
            vm.temp_roots.truncate(prev);
            Ok(res)
        });
        let cur_df = vm.temp_roots[prev_temp];
        vm.set_property(cur_df, "resolvedOptions", resolved_opts);
        let res = vm.temp_roots[prev_temp];
        vm.temp_roots.truncate(prev_temp);
        Ok(res)
    });
    let empty_locales_fn2 = vm.alloc_native_function("supportedLocalesOf", 1, |vm, _this, _args| {
        Ok(vm.alloc_array(Vec::new()))
    });
    vm.set_property(date_format_ctor, "supportedLocalesOf", empty_locales_fn2);
    vm.set_property(intl_obj, "DateTimeFormat", date_format_ctor);

    let collator_ctor = vm.alloc_native_function("Collator", 0, |vm, _this, _args| {
        let coll_obj = vm.alloc_object();
        let prev_temp = vm.temp_roots.len();
        vm.temp_roots.push(coll_obj);
        let compare_fn = vm.alloc_native_function("compare", 2, |vm, _this, args| {
            let a = args.first().map(|v| vm.to_string_value(*v)).unwrap_or_default();
            let b = args.get(1).map(|v| vm.to_string_value(*v)).unwrap_or_default();
            let cmp = match a.cmp(&b) {
                std::cmp::Ordering::Less => -1,
                std::cmp::Ordering::Equal => 0,
                std::cmp::Ordering::Greater => 1,
            };
            Ok(JsValue::from_int32(cmp))
        });
        let cur_coll = vm.temp_roots[prev_temp];
        vm.set_property(cur_coll, "compare", compare_fn);
        let resolved_opts = vm.alloc_native_function("resolvedOptions", 0, |vm, _this, _args| {
            let opts = vm.alloc_object();
            let prev = vm.temp_roots.len();
            vm.temp_roots.push(opts);
            let loc = vm.alloc_string("en-US".to_string());
            let cur_opts = vm.temp_roots[prev];
            vm.set_property(cur_opts, "locale", loc);
            let res = vm.temp_roots[prev];
            vm.temp_roots.truncate(prev);
            Ok(res)
        });
        let cur_coll = vm.temp_roots[prev_temp];
        vm.set_property(cur_coll, "resolvedOptions", resolved_opts);
        let res = vm.temp_roots[prev_temp];
        vm.temp_roots.truncate(prev_temp);
        Ok(res)
    });
    let empty_locales_fn3 = vm.alloc_native_function("supportedLocalesOf", 1, |vm, _this, _args| {
        Ok(vm.alloc_array(Vec::new()))
    });
    vm.set_property(collator_ctor, "supportedLocalesOf", empty_locales_fn3);
    vm.set_property(intl_obj, "Collator", collator_ctor);

    vm.globals.insert("Intl".to_string(), intl_obj);

}
