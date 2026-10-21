use super::calendar::{DAYS, MONTHS, ms_to_utc, parse_date, time_clip, utc_to_ms};
use super::*;
use crate::interpreter::PreferredType;

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

        if y_num.is_nan()
            || m_num.is_nan()
            || d_num.is_nan()
            || h_num.is_nan()
            || min_num.is_nan()
            || s_num.is_nan()
            || ms_num.is_nan()
        {
            return Ok(JsValue::from_double(f64::NAN));
        }
        let y = y_num as i32;
        let final_y = if (0..=99).contains(&y) && y_num.fract() == 0.0 { y + 1900 } else { y };
        Ok(JsValue::from_double(time_clip(utc_to_ms(
            final_y,
            m_num as i32,
            d_num as i32,
            h_num as i32,
            min_num as i32,
            s_num as i32,
            ms_num as i32,
        ))))
    });
    let date_parse_fn = vm.alloc_native_function("parse", 1, |vm, _this, args| {
        let date_str = args.first().copied().unwrap_or(JsValue::undefined());
        let s = vm.to_string_spec(date_str)?;
        if let Some(ms) = parse_date(&s) {
            Ok(JsValue::from_double(ms))
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
                let str_res = format!(
                    "{} {} {:02} {:04} {:02}:{:02}:{:02} GMT+0000 (Coordinated Universal Time)",
                    day_name, mon_name, d, y, h, min, s
                );
                return Ok(vm.alloc_string(str_res));
            }
            return Ok(vm.alloc_string("Invalid Date".into()));
        }

        let time_ms = if args.is_empty() {
            time_clip(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis() as f64)
                    .unwrap_or(0.0),
            )
        } else if args.len() == 1 {
            let arg = args[0];
            if let Some(ptr) = arg.as_object_ptr() {
                let obj = unsafe { &*(ptr as *mut JsObject) };
                if let ObjectKind::Date(ms) = obj.kind {
                    time_clip(ms)
                } else {
                    let v = vm.to_primitive(arg, PreferredType::Default)?;
                    if v.is_string() {
                        let s = vm.to_string_value(v);
                        parse_date(&s).unwrap_or(f64::NAN)
                    } else {
                        time_clip(vm.to_number_value(v)?)
                    }
                }
            } else if arg.is_string() {
                let s = vm.to_string_value(arg);
                parse_date(&s).unwrap_or(f64::NAN)
            } else {
                time_clip(vm.to_number_value(arg)?)
            }
        } else {
            let y_num = vm.to_number_value(args[0])?;
            let m_num = vm.to_number_value(args[1])?;
            let d_num = if args.len() > 2 { vm.to_number_value(args[2])? } else { 1.0 };
            let h_num = if args.len() > 3 { vm.to_number_value(args[3])? } else { 0.0 };
            let min_num = if args.len() > 4 { vm.to_number_value(args[4])? } else { 0.0 };
            let s_num = if args.len() > 5 { vm.to_number_value(args[5])? } else { 0.0 };
            let ms_num = if args.len() > 6 { vm.to_number_value(args[6])? } else { 0.0 };

            if y_num.is_nan()
                || m_num.is_nan()
                || d_num.is_nan()
                || h_num.is_nan()
                || min_num.is_nan()
                || s_num.is_nan()
                || ms_num.is_nan()
            {
                f64::NAN
            } else {
                let y = y_num as i32;
                let final_y =
                    if (0..=99).contains(&y) && y_num.fract() == 0.0 { y + 1900 } else { y };
                time_clip(utc_to_ms(
                    final_y,
                    m_num as i32,
                    d_num as i32,
                    h_num as i32,
                    min_num as i32,
                    s_num as i32,
                    ms_num as i32,
                ))
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

    if let Some(sym_to_prim) = vm.symbol_to_primitive {
        let date_to_prim_fn =
            vm.alloc_native_function("[Symbol.toPrimitive]", 1, |vm, this, args| {
                if !this.is_js_object() {
                    return Err(VmError::TypeError(
                        "Date.prototype[Symbol.toPrimitive] called on non-object".into(),
                    ));
                }
                let hint_arg = args.first().copied().unwrap_or(JsValue::undefined());
                if !hint_arg.is_js_string() {
                    return Err(VmError::TypeError(
                        "Invalid hint for Date[Symbol.toPrimitive]".into(),
                    ));
                }
                let hint_str = vm.to_string_value(hint_arg);
                let try_first = match hint_str.as_str() {
                    "string" | "default" => PreferredType::String,
                    "number" => PreferredType::Number,
                    _ => {
                        return Err(VmError::TypeError(
                            "Invalid hint for Date[Symbol.toPrimitive]".into(),
                        ));
                    }
                };
                vm.ordinary_to_primitive(this, try_first)
            });
        let non_enum_attrs = crate::shape::PropertyAttributes::new(false, false, true);
        vm.define_symbol_property(date_proto, sym_to_prim, date_to_prim_fn, non_enum_attrs);
    }

    vm.set_property(date_ctor, "now", date_now_fn);
    vm.set_property_non_enumerable(date_ctor, "now".to_string());
    vm.set_property(date_ctor, "UTC", date_utc_fn);
    vm.set_property_non_enumerable(date_ctor, "UTC".to_string());
    vm.set_property(date_ctor, "parse", date_parse_fn);
    vm.set_property_non_enumerable(date_ctor, "parse".to_string());

    vm.globals.insert("Date".to_string(), date_ctor);
}
