//! `Date.prototype` getters and setters.

use super::*;
use super::calendar::{get_date_ms, ms_to_utc, utc_to_ms};

pub(super) fn create_accessors(vm: &mut Vm) -> Vec<(&'static str, JsValue)> {
    let date_get_time = vm.alloc_native_function("getTime", 0, |_vm, this, _args| {
        let (_, ms) = get_date_ms(this)?;
        Ok(JsValue::from_double(ms))
    });
    let date_value_of = vm.alloc_native_function("valueOf", 0, |_vm, this, _args| {
        let (_, ms) = get_date_ms(this)?;
        Ok(JsValue::from_double(ms))
    });

    let date_get_full_year = vm.alloc_native_function("getFullYear", 0, |_vm, this, _args| {
        let (_, ms) = get_date_ms(this)?;
        match ms_to_utc(ms) {
            Some((y, _, _, _, _, _, _, _)) => Ok(JsValue::from_double(y as f64)),
            None => Ok(JsValue::from_double(f64::NAN)),
        }
    });
    let date_get_utc_full_year = vm.alloc_native_function("getUTCFullYear", 0, |_vm, this, _args| {
        let (_, ms) = get_date_ms(this)?;
        match ms_to_utc(ms) {
            Some((y, _, _, _, _, _, _, _)) => Ok(JsValue::from_double(y as f64)),
            None => Ok(JsValue::from_double(f64::NAN)),
        }
    });

    let date_get_month = vm.alloc_native_function("getMonth", 0, |_vm, this, _args| {
        let (_, ms) = get_date_ms(this)?;
        match ms_to_utc(ms) {
            Some((_, m, _, _, _, _, _, _)) => Ok(JsValue::from_double(m as f64)),
            None => Ok(JsValue::from_double(f64::NAN)),
        }
    });
    let date_get_utc_month = vm.alloc_native_function("getUTCMonth", 0, |_vm, this, _args| {
        let (_, ms) = get_date_ms(this)?;
        match ms_to_utc(ms) {
            Some((_, m, _, _, _, _, _, _)) => Ok(JsValue::from_double(m as f64)),
            None => Ok(JsValue::from_double(f64::NAN)),
        }
    });

    let date_get_date = vm.alloc_native_function("getDate", 0, |_vm, this, _args| {
        let (_, ms) = get_date_ms(this)?;
        match ms_to_utc(ms) {
            Some((_, _, d, _, _, _, _, _)) => Ok(JsValue::from_double(d as f64)),
            None => Ok(JsValue::from_double(f64::NAN)),
        }
    });
    let date_get_utc_date = vm.alloc_native_function("getUTCDate", 0, |_vm, this, _args| {
        let (_, ms) = get_date_ms(this)?;
        match ms_to_utc(ms) {
            Some((_, _, d, _, _, _, _, _)) => Ok(JsValue::from_double(d as f64)),
            None => Ok(JsValue::from_double(f64::NAN)),
        }
    });

    let date_get_day = vm.alloc_native_function("getDay", 0, |_vm, this, _args| {
        let (_, ms) = get_date_ms(this)?;
        match ms_to_utc(ms) {
            Some((_, _, _, w, _, _, _, _)) => Ok(JsValue::from_double(w as f64)),
            None => Ok(JsValue::from_double(f64::NAN)),
        }
    });
    let date_get_utc_day = vm.alloc_native_function("getUTCDay", 0, |_vm, this, _args| {
        let (_, ms) = get_date_ms(this)?;
        match ms_to_utc(ms) {
            Some((_, _, _, w, _, _, _, _)) => Ok(JsValue::from_double(w as f64)),
            None => Ok(JsValue::from_double(f64::NAN)),
        }
    });

    let date_get_hours = vm.alloc_native_function("getHours", 0, |_vm, this, _args| {
        let (_, ms) = get_date_ms(this)?;
        match ms_to_utc(ms) {
            Some((_, _, _, _, h, _, _, _)) => Ok(JsValue::from_double(h as f64)),
            None => Ok(JsValue::from_double(f64::NAN)),
        }
    });
    let date_get_utc_hours = vm.alloc_native_function("getUTCHours", 0, |_vm, this, _args| {
        let (_, ms) = get_date_ms(this)?;
        match ms_to_utc(ms) {
            Some((_, _, _, _, h, _, _, _)) => Ok(JsValue::from_double(h as f64)),
            None => Ok(JsValue::from_double(f64::NAN)),
        }
    });

    let date_get_minutes = vm.alloc_native_function("getMinutes", 0, |_vm, this, _args| {
        let (_, ms) = get_date_ms(this)?;
        match ms_to_utc(ms) {
            Some((_, _, _, _, _, min, _, _)) => Ok(JsValue::from_double(min as f64)),
            None => Ok(JsValue::from_double(f64::NAN)),
        }
    });
    let date_get_utc_minutes = vm.alloc_native_function("getUTCMinutes", 0, |_vm, this, _args| {
        let (_, ms) = get_date_ms(this)?;
        match ms_to_utc(ms) {
            Some((_, _, _, _, _, min, _, _)) => Ok(JsValue::from_double(min as f64)),
            None => Ok(JsValue::from_double(f64::NAN)),
        }
    });

    let date_get_seconds = vm.alloc_native_function("getSeconds", 0, |_vm, this, _args| {
        let (_, ms) = get_date_ms(this)?;
        match ms_to_utc(ms) {
            Some((_, _, _, _, _, _, s, _)) => Ok(JsValue::from_double(s as f64)),
            None => Ok(JsValue::from_double(f64::NAN)),
        }
    });
    let date_get_utc_seconds = vm.alloc_native_function("getUTCSeconds", 0, |_vm, this, _args| {
        let (_, ms) = get_date_ms(this)?;
        match ms_to_utc(ms) {
            Some((_, _, _, _, _, _, s, _)) => Ok(JsValue::from_double(s as f64)),
            None => Ok(JsValue::from_double(f64::NAN)),
        }
    });

    let date_get_millis = vm.alloc_native_function("getMilliseconds", 0, |_vm, this, _args| {
        let (_, ms) = get_date_ms(this)?;
        match ms_to_utc(ms) {
            Some((_, _, _, _, _, _, _, m)) => Ok(JsValue::from_double(m as f64)),
            None => Ok(JsValue::from_double(f64::NAN)),
        }
    });
    let date_get_utc_millis = vm.alloc_native_function("getUTCMilliseconds", 0, |_vm, this, _args| {
        let (_, ms) = get_date_ms(this)?;
        match ms_to_utc(ms) {
            Some((_, _, _, _, _, _, _, m)) => Ok(JsValue::from_double(m as f64)),
            None => Ok(JsValue::from_double(f64::NAN)),
        }
    });

    let date_get_tz_offset = vm.alloc_native_function("getTimezoneOffset", 0, |_vm, this, _args| {
        let _ = get_date_ms(this)?;
        Ok(JsValue::from_double(0.0))
    });

    let date_get_year = vm.alloc_native_function("getYear", 0, |_vm, this, _args| {
        let (_, ms) = get_date_ms(this)?;
        match ms_to_utc(ms) {
            Some((y, _, _, _, _, _, _, _)) => Ok(JsValue::from_double((y - 1900) as f64)),
            None => Ok(JsValue::from_double(f64::NAN)),
        }
    });
    let date_set_year = vm.alloc_native_function("setYear", 1, |vm, this, args| {
        let (ptr, ms) = get_date_ms(this)?;
        let arg_y = args.first().and_then(|v| vm.to_number_value(*v).ok()).unwrap_or(f64::NAN);
        if arg_y.is_nan() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            obj.kind = ObjectKind::Date(f64::NAN);
            return Ok(JsValue::from_double(f64::NAN));
        }
        let mut y = arg_y as i32;
        if (0..=99).contains(&y) {
            y += 1900;
        }
        let (_, m, d, _, h, min, s, ms_part) = ms_to_utc(ms).unwrap_or((1970, 0, 1, 0, 0, 0, 0, 0));
        let new_ms = utc_to_ms(y, m, d, h, min, s, ms_part);
        let obj = unsafe { &mut *(ptr as *mut JsObject) };
        obj.kind = ObjectKind::Date(new_ms);
        Ok(JsValue::from_double(new_ms))
    });

    let date_set_time = vm.alloc_native_function("setTime", 1, |vm, this, args| {
        let (ptr, _) = get_date_ms(this)?;
        let new_ms = args.first().and_then(|v| vm.to_number_value(*v).ok()).unwrap_or(f64::NAN);
        let obj = unsafe { &mut *(ptr as *mut JsObject) };
        obj.kind = ObjectKind::Date(new_ms);
        Ok(JsValue::from_double(new_ms))
    });
    let date_set_full_year = vm.alloc_native_function("setFullYear", 3, |vm, this, args| {
        let (ptr, ms) = get_date_ms(this)?;
        let arg_y = args.first().and_then(|v| vm.to_number_value(*v).ok()).unwrap_or(f64::NAN);
        let (cur_y, cur_m, cur_d, _, cur_h, cur_min, cur_s, cur_ms) = ms_to_utc(ms).unwrap_or((1970, 0, 1, 0, 0, 0, 0, 0));
        let y = if arg_y.is_nan() { cur_y } else { arg_y as i32 };
        let m = args.get(1).and_then(|v| vm.to_number_value(*v).ok()).map(|n| n as i32).unwrap_or(cur_m);
        let d = args.get(2).and_then(|v| vm.to_number_value(*v).ok()).map(|n| n as i32).unwrap_or(cur_d);
        let new_ms = utc_to_ms(y, m, d, cur_h, cur_min, cur_s, cur_ms);
        let obj = unsafe { &mut *(ptr as *mut JsObject) };
        obj.kind = ObjectKind::Date(new_ms);
        Ok(JsValue::from_double(new_ms))
    });

    let date_set_month = vm.alloc_native_function("setMonth", 2, |vm, this, args| {
        let (ptr, ms) = get_date_ms(this)?;
        let arg_m = args.first().and_then(|v| vm.to_number_value(*v).ok()).unwrap_or(f64::NAN);
        let (cur_y, cur_m, cur_d, _, cur_h, cur_min, cur_s, cur_ms) = ms_to_utc(ms).unwrap_or((1970, 0, 1, 0, 0, 0, 0, 0));
        let m = if arg_m.is_nan() { cur_m } else { arg_m as i32 };
        let d = args.get(1).and_then(|v| vm.to_number_value(*v).ok()).map(|n| n as i32).unwrap_or(cur_d);
        let new_ms = utc_to_ms(cur_y, m, d, cur_h, cur_min, cur_s, cur_ms);
        let obj = unsafe { &mut *(ptr as *mut JsObject) };
        obj.kind = ObjectKind::Date(new_ms);
        Ok(JsValue::from_double(new_ms))
    });

    let date_set_date = vm.alloc_native_function("setDate", 1, |vm, this, args| {
        let (ptr, ms) = get_date_ms(this)?;
        let arg_d = args.first().and_then(|v| vm.to_number_value(*v).ok()).unwrap_or(f64::NAN);
        let (cur_y, cur_m, cur_d, _, cur_h, cur_min, cur_s, cur_ms) = ms_to_utc(ms).unwrap_or((1970, 0, 1, 0, 0, 0, 0, 0));
        let d = if arg_d.is_nan() { cur_d } else { arg_d as i32 };
        let new_ms = utc_to_ms(cur_y, cur_m, d, cur_h, cur_min, cur_s, cur_ms);
        let obj = unsafe { &mut *(ptr as *mut JsObject) };
        obj.kind = ObjectKind::Date(new_ms);
        Ok(JsValue::from_double(new_ms))
    });

    let date_set_hours = vm.alloc_native_function("setHours", 4, |vm, this, args| {
        let (ptr, ms) = get_date_ms(this)?;
        let arg_h = args.first().and_then(|v| vm.to_number_value(*v).ok()).unwrap_or(f64::NAN);
        let (cur_y, cur_m, cur_d, _, cur_h, cur_min, cur_s, cur_ms) = ms_to_utc(ms).unwrap_or((1970, 0, 1, 0, 0, 0, 0, 0));
        let h = if arg_h.is_nan() { cur_h } else { arg_h as i32 };
        let min = args.get(1).and_then(|v| vm.to_number_value(*v).ok()).map(|n| n as i32).unwrap_or(cur_min);
        let s = args.get(2).and_then(|v| vm.to_number_value(*v).ok()).map(|n| n as i32).unwrap_or(cur_s);
        let ms_part = args.get(3).and_then(|v| vm.to_number_value(*v).ok()).map(|n| n as i32).unwrap_or(cur_ms);
        let new_ms = utc_to_ms(cur_y, cur_m, cur_d, h, min, s, ms_part);
        let obj = unsafe { &mut *(ptr as *mut JsObject) };
        obj.kind = ObjectKind::Date(new_ms);
        Ok(JsValue::from_double(new_ms))
    });

    let date_set_minutes = vm.alloc_native_function("setMinutes", 3, |vm, this, args| {
        let (ptr, ms) = get_date_ms(this)?;
        let arg_min = args.first().and_then(|v| vm.to_number_value(*v).ok()).unwrap_or(f64::NAN);
        let (cur_y, cur_m, cur_d, _, cur_h, cur_min, cur_s, cur_ms) = ms_to_utc(ms).unwrap_or((1970, 0, 1, 0, 0, 0, 0, 0));
        let min = if arg_min.is_nan() { cur_min } else { arg_min as i32 };
        let s = args.get(1).and_then(|v| vm.to_number_value(*v).ok()).map(|n| n as i32).unwrap_or(cur_s);
        let ms_part = args.get(2).and_then(|v| vm.to_number_value(*v).ok()).map(|n| n as i32).unwrap_or(cur_ms);
        let new_ms = utc_to_ms(cur_y, cur_m, cur_d, cur_h, min, s, ms_part);
        let obj = unsafe { &mut *(ptr as *mut JsObject) };
        obj.kind = ObjectKind::Date(new_ms);
        Ok(JsValue::from_double(new_ms))
    });

    let date_set_seconds = vm.alloc_native_function("setSeconds", 2, |vm, this, args| {
        let (ptr, ms) = get_date_ms(this)?;
        let arg_s = args.first().and_then(|v| vm.to_number_value(*v).ok()).unwrap_or(f64::NAN);
        let (cur_y, cur_m, cur_d, _, cur_h, cur_min, cur_s, cur_ms) = ms_to_utc(ms).unwrap_or((1970, 0, 1, 0, 0, 0, 0, 0));
        let s = if arg_s.is_nan() { cur_s } else { arg_s as i32 };
        let ms_part = args.get(1).and_then(|v| vm.to_number_value(*v).ok()).map(|n| n as i32).unwrap_or(cur_ms);
        let new_ms = utc_to_ms(cur_y, cur_m, cur_d, cur_h, cur_min, s, ms_part);
        let obj = unsafe { &mut *(ptr as *mut JsObject) };
        obj.kind = ObjectKind::Date(new_ms);
        Ok(JsValue::from_double(new_ms))
    });

    let date_set_millis = vm.alloc_native_function("setMilliseconds", 1, |vm, this, args| {
        let (ptr, ms) = get_date_ms(this)?;
        let arg_ms = args.first().and_then(|v| vm.to_number_value(*v).ok()).unwrap_or(f64::NAN);
        let (cur_y, cur_m, cur_d, _, cur_h, cur_min, cur_s, cur_ms) = ms_to_utc(ms).unwrap_or((1970, 0, 1, 0, 0, 0, 0, 0));
        let ms_part = if arg_ms.is_nan() { cur_ms } else { arg_ms as i32 };
        let new_ms = utc_to_ms(cur_y, cur_m, cur_d, cur_h, cur_min, cur_s, ms_part);
        let obj = unsafe { &mut *(ptr as *mut JsObject) };
        obj.kind = ObjectKind::Date(new_ms);
        Ok(JsValue::from_double(new_ms))
    });

    vec![
        ("getTime", date_get_time),
        ("valueOf", date_value_of),
        ("getFullYear", date_get_full_year),
        ("getUTCFullYear", date_get_utc_full_year),
        ("getMonth", date_get_month),
        ("getUTCMonth", date_get_utc_month),
        ("getDate", date_get_date),
        ("getUTCDate", date_get_utc_date),
        ("getDay", date_get_day),
        ("getUTCDay", date_get_utc_day),
        ("getHours", date_get_hours),
        ("getUTCHours", date_get_utc_hours),
        ("getMinutes", date_get_minutes),
        ("getUTCMinutes", date_get_utc_minutes),
        ("getSeconds", date_get_seconds),
        ("getUTCSeconds", date_get_utc_seconds),
        ("getMilliseconds", date_get_millis),
        ("getUTCMilliseconds", date_get_utc_millis),
        ("getTimezoneOffset", date_get_tz_offset),
        ("getYear", date_get_year),
        ("setYear", date_set_year),
        ("setTime", date_set_time),
        ("setFullYear", date_set_full_year),
        ("setUTCFullYear", date_set_full_year),
        ("setMonth", date_set_month),
        ("setUTCMonth", date_set_month),
        ("setDate", date_set_date),
        ("setUTCDate", date_set_date),
        ("setHours", date_set_hours),
        ("setUTCHours", date_set_hours),
        ("setMinutes", date_set_minutes),
        ("setUTCMinutes", date_set_minutes),
        ("setSeconds", date_set_seconds),
        ("setUTCSeconds", date_set_seconds),
        ("setMilliseconds", date_set_millis),
        ("setUTCMilliseconds", date_set_millis),
    ]
}
