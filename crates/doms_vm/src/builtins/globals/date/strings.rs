//! `Date.prototype` string conversion methods.

use super::calendar::{DAYS, MONTHS, get_date_ms, ms_to_utc};
use super::*;

fn format_year(y: i32) -> String {
    if y >= 0 { format!("{:04}", y) } else { format!("-{:04}", y.unsigned_abs()) }
}

pub(super) fn create_string_methods(vm: &mut Vm) -> Vec<(&'static str, JsValue)> {
    let date_to_iso_string = vm.alloc_native_function("toISOString", 0, |vm, this, _args| {
        let (_, ms) = get_date_ms(this)?;
        match ms_to_utc(ms) {
            Some((y, m, d, _, h, min, s, ms_part)) => {
                let y_str = if (0..=9999).contains(&y) {
                    format!("{:04}", y)
                } else if y < 0 {
                    format!("-{:06}", y.unsigned_abs())
                } else {
                    format!("+{:06}", y)
                };
                let iso = format!(
                    "{}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
                    y_str,
                    m + 1,
                    d,
                    h,
                    min,
                    s,
                    ms_part
                );
                Ok(vm.alloc_string(iso))
            }
            None => Err(VmError::RangeError("Invalid time value".into())),
        }
    });

    let date_to_utc_string = vm.alloc_native_function("toUTCString", 0, |vm, this, _args| {
        let (_, ms) = get_date_ms(this)?;
        match ms_to_utc(ms) {
            Some((y, m, d, w, h, min, s, _)) => {
                let day_name = DAYS.get(w as usize).unwrap_or(&"Sun");
                let mon_name = MONTHS.get(m as usize).unwrap_or(&"Jan");
                let utc_str = format!(
                    "{}, {:02} {} {} {:02}:{:02}:{:02} GMT",
                    day_name,
                    d,
                    mon_name,
                    format_year(y),
                    h,
                    min,
                    s
                );
                Ok(vm.alloc_string(utc_str))
            }
            None => Ok(vm.alloc_string("Invalid Date".into())),
        }
    });

    let date_to_date_string = vm.alloc_native_function("toDateString", 0, |vm, this, _args| {
        let (_, ms) = get_date_ms(this)?;
        match ms_to_utc(ms) {
            Some((y, m, d, w, _, _, _, _)) => {
                let day_name = DAYS.get(w as usize).unwrap_or(&"Sun");
                let mon_name = MONTHS.get(m as usize).unwrap_or(&"Jan");
                let s = format!("{} {} {:02} {}", day_name, mon_name, d, format_year(y));
                Ok(vm.alloc_string(s))
            }
            None => Ok(vm.alloc_string("Invalid Date".into())),
        }
    });

    let date_to_time_string = vm.alloc_native_function("toTimeString", 0, |vm, this, _args| {
        let (_, ms) = get_date_ms(this)?;
        match ms_to_utc(ms) {
            Some((_, _, _, _, h, min, s, _)) => {
                let s =
                    format!("{:02}:{:02}:{:02} GMT+0000 (Coordinated Universal Time)", h, min, s);
                Ok(vm.alloc_string(s))
            }
            None => Ok(vm.alloc_string("Invalid Date".into())),
        }
    });

    let date_to_string = vm.alloc_native_function("toString", 0, |vm, this, _args| {
        let (_, ms) = get_date_ms(this)?;
        match ms_to_utc(ms) {
            Some((y, m, d, w, h, min, s, _)) => {
                let day_name = DAYS.get(w as usize).unwrap_or(&"Sun");
                let mon_name = MONTHS.get(m as usize).unwrap_or(&"Jan");
                let s = format!(
                    "{} {} {:02} {} {:02}:{:02}:{:02} GMT+0000 (Coordinated Universal Time)",
                    day_name,
                    mon_name,
                    d,
                    format_year(y),
                    h,
                    min,
                    s
                );
                Ok(vm.alloc_string(s))
            }
            None => Ok(vm.alloc_string("Invalid Date".into())),
        }
    });

    let date_to_json = vm.alloc_native_function("toJSON", 1, |vm, this, _args| {
        if this.is_nullish() {
            return Err(VmError::TypeError(
                "Date.prototype.toJSON called on null or undefined".into(),
            ));
        }
        let o = vm.to_object_value(this)?;
        let tv = vm.to_primitive(o, crate::interpreter::PreferredType::Number)?;
        if let Some(n) = tv.as_double() {
            if !n.is_finite() {
                return Ok(JsValue::null());
            }
        }
        let to_iso = vm.get_property_result(o, "toISOString")?;
        if !vm.is_callable(to_iso) {
            return Err(VmError::TypeError("toISOString is not callable".into()));
        }
        vm.call(to_iso, o, &[])
    });
    let date_to_locale_string = vm.alloc_native_function("toLocaleString", 0, |vm, this, _args| {
        let (_, ms) = get_date_ms(this)?;
        match ms_to_utc(ms) {
            Some((y, m, d, _, h, min, s, _)) => {
                let s = format!("{:02}/{:02}/{:04}, {:02}:{:02}:{:02}", m + 1, d, y, h, min, s);
                Ok(vm.alloc_string(s))
            }
            None => Ok(vm.alloc_string("Invalid Date".into())),
        }
    });

    let date_to_locale_date_string =
        vm.alloc_native_function("toLocaleDateString", 0, |vm, this, _args| {
            let (_, ms) = get_date_ms(this)?;
            match ms_to_utc(ms) {
                Some((y, m, d, _, _, _, _, _)) => {
                    let s = format!("{:02}/{:02}/{:04}", m + 1, d, y);
                    Ok(vm.alloc_string(s))
                }
                None => Ok(vm.alloc_string("Invalid Date".into())),
            }
        });

    let date_to_locale_time_string =
        vm.alloc_native_function("toLocaleTimeString", 0, |vm, this, _args| {
            let (_, ms) = get_date_ms(this)?;
            match ms_to_utc(ms) {
                Some((_, _, _, _, h, min, s, _)) => {
                    let s = format!("{:02}:{:02}:{:02}", h, min, s);
                    Ok(vm.alloc_string(s))
                }
                None => Ok(vm.alloc_string("Invalid Date".into())),
            }
        });

    vec![
        ("toISOString", date_to_iso_string),
        ("toUTCString", date_to_utc_string),
        ("toGMTString", date_to_utc_string),
        ("toDateString", date_to_date_string),
        ("toTimeString", date_to_time_string),
        ("toString", date_to_string),
        ("toJSON", date_to_json),
        ("toLocaleString", date_to_locale_string),
        ("toLocaleDateString", date_to_locale_date_string),
        ("toLocaleTimeString", date_to_locale_time_string),
    ]
}
