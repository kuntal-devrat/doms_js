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
        let new_ms = match args.first() {
            Some(v) => vm.to_number_value(*v)?,
            None => f64::NAN,
        };
        let obj = unsafe { &mut *(ptr as *mut JsObject) };
        obj.kind = ObjectKind::Date(new_ms);
        Ok(JsValue::from_double(new_ms))
    });
    let date_set_full_year = vm.alloc_native_function("setFullYear", 3, |vm, this, args| {
        let (ptr, ms) = get_date_ms(this)?;
        let arg_y = match args.first() {
            Some(v) => vm.to_number_value(*v)?,
            None => f64::NAN,
        };
        let (_cur_y, cur_m, cur_d, _, cur_h, cur_min, cur_s, cur_ms) = if ms.is_nan() {
            (1970, 0, 1, 0, 0, 0, 0, 0)
        } else {
            ms_to_utc(ms).unwrap_or((1970, 0, 1, 0, 0, 0, 0, 0))
        };
        let m_num = if args.len() > 1 { vm.to_number_value(args[1])? } else { cur_m as f64 };
        let d_num = if args.len() > 2 { vm.to_number_value(args[2])? } else { cur_d as f64 };
        let new_ms = if arg_y.is_nan() || m_num.is_nan() || d_num.is_nan() {
            f64::NAN
        } else {
            utc_to_ms(arg_y as i32, m_num as i32, d_num as i32, cur_h, cur_min, cur_s, cur_ms)
        };
        let obj = unsafe { &mut *(ptr as *mut JsObject) };
        obj.kind = ObjectKind::Date(new_ms);
        Ok(JsValue::from_double(new_ms))
    });

    let date_set_month = vm.alloc_native_function("setMonth", 2, |vm, this, args| {
        let (ptr, ms) = get_date_ms(this)?;
        let arg_m = match args.first() {
            Some(v) => vm.to_number_value(*v)?,
            None => f64::NAN,
        };
        let d_num = if args.len() > 1 { Some(vm.to_number_value(args[1])?) } else { None };
        if ms.is_nan() {
            return Ok(JsValue::from_double(f64::NAN));
        }
        let (cur_y, _, cur_d, _, cur_h, cur_min, cur_s, cur_ms) = ms_to_utc(ms).unwrap_or((1970, 0, 1, 0, 0, 0, 0, 0));
        let d = d_num.unwrap_or(cur_d as f64);
        let new_ms = if arg_m.is_nan() || d.is_nan() {
            f64::NAN
        } else {
            utc_to_ms(cur_y, arg_m as i32, d as i32, cur_h, cur_min, cur_s, cur_ms)
        };
        let obj = unsafe { &mut *(ptr as *mut JsObject) };
        obj.kind = ObjectKind::Date(new_ms);
        Ok(JsValue::from_double(new_ms))
    });

    let date_set_date = vm.alloc_native_function("setDate", 1, |vm, this, args| {
        let (ptr, ms) = get_date_ms(this)?;
        let arg_d = match args.first() {
            Some(v) => vm.to_number_value(*v)?,
            None => f64::NAN,
        };
        if ms.is_nan() {
            return Ok(JsValue::from_double(f64::NAN));
        }
        let (cur_y, cur_m, _, _, cur_h, cur_min, cur_s, cur_ms) = ms_to_utc(ms).unwrap_or((1970, 0, 1, 0, 0, 0, 0, 0));
        let new_ms = if arg_d.is_nan() {
            f64::NAN
        } else {
            utc_to_ms(cur_y, cur_m, arg_d as i32, cur_h, cur_min, cur_s, cur_ms)
        };
        let obj = unsafe { &mut *(ptr as *mut JsObject) };
        obj.kind = ObjectKind::Date(new_ms);
        Ok(JsValue::from_double(new_ms))
    });

    let date_set_hours = vm.alloc_native_function("setHours", 4, |vm, this, args| {
        let (ptr, ms) = get_date_ms(this)?;
        let h_num = match args.first() {
            Some(v) => vm.to_number_value(*v)?,
            None => f64::NAN,
        };
        let min_num = if args.len() > 1 { Some(vm.to_number_value(args[1])?) } else { None };
        let s_num = if args.len() > 2 { Some(vm.to_number_value(args[2])?) } else { None };
        let ms_num = if args.len() > 3 { Some(vm.to_number_value(args[3])?) } else { None };
        if ms.is_nan() {
            return Ok(JsValue::from_double(f64::NAN));
        }
        let (cur_y, cur_m, cur_d, _, _cur_h, cur_min, cur_s, cur_ms) = ms_to_utc(ms).unwrap_or((1970, 0, 1, 0, 0, 0, 0, 0));
        let min = min_num.unwrap_or(cur_min as f64);
        let s = s_num.unwrap_or(cur_s as f64);
        let milli = ms_num.unwrap_or(cur_ms as f64);
        let new_ms = if h_num.is_nan() || min.is_nan() || s.is_nan() || milli.is_nan() {
            f64::NAN
        } else {
            utc_to_ms(cur_y, cur_m, cur_d, h_num as i32, min as i32, s as i32, milli as i32)
        };
        let obj = unsafe { &mut *(ptr as *mut JsObject) };
        obj.kind = ObjectKind::Date(new_ms);
        Ok(JsValue::from_double(new_ms))
    });

    let date_set_minutes = vm.alloc_native_function("setMinutes", 3, |vm, this, args| {
        let (ptr, ms) = get_date_ms(this)?;
        let min_num = match args.first() {
            Some(v) => vm.to_number_value(*v)?,
            None => f64::NAN,
        };
        let s_num = if args.len() > 1 { Some(vm.to_number_value(args[1])?) } else { None };
        let ms_num = if args.len() > 2 { Some(vm.to_number_value(args[2])?) } else { None };
        if ms.is_nan() {
            return Ok(JsValue::from_double(f64::NAN));
        }
        let (cur_y, cur_m, cur_d, _, cur_h, _cur_min, cur_s, cur_ms) = ms_to_utc(ms).unwrap_or((1970, 0, 1, 0, 0, 0, 0, 0));
        let s = s_num.unwrap_or(cur_s as f64);
        let milli = ms_num.unwrap_or(cur_ms as f64);
        let new_ms = if min_num.is_nan() || s.is_nan() || milli.is_nan() {
            f64::NAN
        } else {
            utc_to_ms(cur_y, cur_m, cur_d, cur_h, min_num as i32, s as i32, milli as i32)
        };
        let obj = unsafe { &mut *(ptr as *mut JsObject) };
        obj.kind = ObjectKind::Date(new_ms);
        Ok(JsValue::from_double(new_ms))
    });

    let date_set_seconds = vm.alloc_native_function("setSeconds", 2, |vm, this, args| {
        let (ptr, ms) = get_date_ms(this)?;
        let s_num = match args.first() {
            Some(v) => vm.to_number_value(*v)?,
            None => f64::NAN,
        };
        let ms_num = if args.len() > 1 { Some(vm.to_number_value(args[1])?) } else { None };
        if ms.is_nan() {
            return Ok(JsValue::from_double(f64::NAN));
        }
        let (cur_y, cur_m, cur_d, _, cur_h, cur_min, _cur_s, cur_ms) = ms_to_utc(ms).unwrap_or((1970, 0, 1, 0, 0, 0, 0, 0));
        let milli = ms_num.unwrap_or(cur_ms as f64);
        let new_ms = if s_num.is_nan() || milli.is_nan() {
            f64::NAN
        } else {
            utc_to_ms(cur_y, cur_m, cur_d, cur_h, cur_min, s_num as i32, milli as i32)
        };
        let obj = unsafe { &mut *(ptr as *mut JsObject) };
        obj.kind = ObjectKind::Date(new_ms);
        Ok(JsValue::from_double(new_ms))
    });

    let date_set_millis = vm.alloc_native_function("setMilliseconds", 1, |vm, this, args| {
        let (ptr, ms) = get_date_ms(this)?;
        let ms_num = match args.first() {
            Some(v) => vm.to_number_value(*v)?,
            None => f64::NAN,
        };
        if ms.is_nan() {
            return Ok(JsValue::from_double(f64::NAN));
        }
        let (cur_y, cur_m, cur_d, _, cur_h, cur_min, cur_s, _) = ms_to_utc(ms).unwrap_or((1970, 0, 1, 0, 0, 0, 0, 0));
        let new_ms = if ms_num.is_nan() {
            f64::NAN
        } else {
            utc_to_ms(cur_y, cur_m, cur_d, cur_h, cur_min, cur_s, ms_num as i32)
        };
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
