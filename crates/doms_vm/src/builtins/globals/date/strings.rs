//! `Date.prototype` string conversion methods.

use super::calendar::{DAYS, MONTHS, get_date_ms, ms_to_utc};
use super::*;

pub(super) fn create_string_methods(vm: &mut Vm) -> Vec<(&'static str, JsValue)> {
    let date_to_iso_string = vm.alloc_native_function("toISOString", 0, |vm, this, _args| {
        let (_, ms) = get_date_ms(this)?;
        match ms_to_utc(ms) {
            Some((y, m, d, _, h, min, s, ms_part)) => {
                let iso = format!(
                    "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
                    y,
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
                    "{}, {:02} {} {:04} {:02}:{:02}:{:02} GMT",
                    day_name, d, mon_name, y, h, min, s
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
                let s = format!("{} {} {:02} {:04}", day_name, mon_name, d, y);
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
                    "{} {} {:02} {:04} {:02}:{:02}:{:02} GMT+0000 (Coordinated Universal Time)",
                    day_name, mon_name, d, y, h, min, s
                );
                Ok(vm.alloc_string(s))
            }
            None => Ok(vm.alloc_string("Invalid Date".into())),
        }
    });

    let date_to_json = vm.alloc_native_function("toJSON", 1, |vm, this, _args| {
        if let Some(_ptr) = this.as_object_ptr() {
            let to_iso = vm.get_property(this, "toISOString");
            if vm.is_callable(to_iso) {
                return vm.call(to_iso, this, &[]);
            }
        }
        Err(VmError::TypeError("toISOString is not callable".into()))
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
