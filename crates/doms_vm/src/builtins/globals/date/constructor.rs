//! The `Date` constructor, its statics and global registration.

use super::*;
use super::calendar::{ms_to_utc, parse_iso_date, utc_to_ms, DAYS, MONTHS};

pub(super) fn register_constructor_and_statics(vm: &mut Vm, date_proto: JsValue) {
    let date_now_fn = vm.alloc_native_function("now", 0, |_vm, _this, _args| {
        let ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as f64)
            .unwrap_or(0.0);
        Ok(JsValue::from_double(ms))
    });
    let date_utc_fn = vm.alloc_native_function("UTC", 7, |vm, _this, args| {
        if args.is_empty() {
            return Ok(JsValue::from_double(f64::NAN));
        }
        let y_num = vm.to_number_value(args[0])?;
        let m_num = if args.len() > 1 { vm.to_number_value(args[1])? } else { 0.0 };
        let d_num = if args.len() > 2 { vm.to_number_value(args[2])? } else { 1.0 };
        let h_num = if args.len() > 3 { vm.to_number_value(args[3])? } else { 0.0 };
        let min_num = if args.len() > 4 { vm.to_number_value(args[4])? } else { 0.0 };
        let s_num = if args.len() > 5 { vm.to_number_value(args[5])? } else { 0.0 };
        let ms_num = if args.len() > 6 { vm.to_number_value(args[6])? } else { 0.0 };

        if y_num.is_nan() || m_num.is_nan() || d_num.is_nan() || h_num.is_nan() || min_num.is_nan() || s_num.is_nan() || ms_num.is_nan() {
            return Ok(JsValue::from_double(f64::NAN));
        }
        let y = y_num as i32;
        let final_y = if (0..=99).contains(&y) && y_num.fract() == 0.0 { y + 1900 } else { y };
        Ok(JsValue::from_double(utc_to_ms(final_y, m_num as i32, d_num as i32, h_num as i32, min_num as i32, s_num as i32, ms_num as i32)))
    });
    let date_parse_fn = vm.alloc_native_function("parse", 1, |vm, _this, args| {
        let s = args.first().map(|v| vm.to_string_value(*v)).unwrap_or_default();
        let trimmed = s.trim();
        if let Some((y, m, d)) = parse_iso_date(trimmed) {
            Ok(JsValue::from_double(utc_to_ms(y, m, d, 0, 0, 0, 0)))
        } else {
            Ok(JsValue::from_double(f64::NAN))
        }
    });

    vm.date_proto = Some(date_proto);
    let date_ctor = vm.alloc_native_function("Date", 7, |vm, _this, args| {
        if !vm.is_constructing {
            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as f64)
                .unwrap_or(0.0);
            if let Some((y, m, d, w, h, min, s, _)) = ms_to_utc(now_ms) {
                let day_name = DAYS.get(w as usize).unwrap_or(&"Sun");
                let mon_name = MONTHS.get(m as usize).unwrap_or(&"Jan");
                let str_res = format!("{} {} {:02} {:04} {:02}:{:02}:{:02} GMT+0000 (Coordinated Universal Time)", day_name, mon_name, d, y, h, min, s);
                return Ok(vm.alloc_string(str_res));
            }
            return Ok(vm.alloc_string("Invalid Date".into()));
        }

        let time_ms = if args.is_empty() {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as f64)
                .unwrap_or(0.0)
        } else if args.len() == 1 {
            let arg = args[0];
            if arg.is_string() {
                let s = vm.to_string_value(arg);
                if let Some((y, m, d)) = parse_iso_date(&s) {
                    utc_to_ms(y, m, d, 0, 0, 0, 0)
                } else {
                    f64::NAN
                }
            } else {
                vm.to_number_value(arg)?
            }
        } else {
            let y_num = vm.to_number_value(args[0])?;
            let m_num = vm.to_number_value(args[1])?;
            let d_num = if args.len() > 2 { vm.to_number_value(args[2])? } else { 1.0 };
            let h_num = if args.len() > 3 { vm.to_number_value(args[3])? } else { 0.0 };
            let min_num = if args.len() > 4 { vm.to_number_value(args[4])? } else { 0.0 };
            let s_num = if args.len() > 5 { vm.to_number_value(args[5])? } else { 0.0 };
            let ms_num = if args.len() > 6 { vm.to_number_value(args[6])? } else { 0.0 };

            if y_num.is_nan() || m_num.is_nan() || d_num.is_nan() || h_num.is_nan() || min_num.is_nan() || s_num.is_nan() || ms_num.is_nan() {
                f64::NAN
            } else {
                let y = y_num as i32;
                let final_y = if (0..=99).contains(&y) && y_num.fract() == 0.0 { y + 1900 } else { y };
                utc_to_ms(final_y, m_num as i32, d_num as i32, h_num as i32, min_num as i32, s_num as i32, ms_num as i32)
            }
        };
        let proto = vm.date_proto.or(vm.object_proto);
        Ok(vm.alloc_date(proto, time_ms))
    });
    vm.set_property(date_proto, "constructor", date_ctor);
    vm.set_property_non_enumerable(date_proto, "constructor".to_string());
    vm.set_property(date_ctor, "prototype", date_proto);
    vm.set_property_non_enumerable(date_ctor, "prototype".to_string());
    vm.set_property_non_writable(date_ctor, "prototype".to_string());
    vm.set_property_non_configurable(date_ctor, "prototype".to_string());

    vm.set_property(date_ctor, "now", date_now_fn);
    vm.set_property_non_enumerable(date_ctor, "now".to_string());
    vm.set_property(date_ctor, "UTC", date_utc_fn);
    vm.set_property_non_enumerable(date_ctor, "UTC".to_string());
    vm.set_property(date_ctor, "parse", date_parse_fn);
    vm.set_property_non_enumerable(date_ctor, "parse".to_string());

    vm.globals.insert("Date".to_string(), date_ctor);
}
