//! Calendar math helpers used by the `Date` implementation.

use super::*;

pub(super) const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
pub(super) const DAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
pub(super) fn ms_to_utc(ms: f64) -> Option<(i32, i32, i32, i32, i32, i32, i32, i32)> {
    if ms.is_nan() || ms.is_infinite() {
        return None;
    }
    let total_ms = ms as i64;
    let mut day = total_ms.div_euclid(86_400_000);
    let time_of_day = total_ms.rem_euclid(86_400_000);
    let ms_part = (time_of_day % 1000) as i32;
    let total_secs = (time_of_day / 1000) as i32;
    let sec = total_secs % 60;
    let total_mins = total_secs / 60;
    let min = total_mins % 60;
    let hour = total_mins / 60;
    let weekday = ((day + 4).rem_euclid(7)) as i32;

    day += 719468;
    let era = if day >= 0 { day } else { day - 146096 } / 146097;
    let doe = (day - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y } as i32;
    let month = (m - 1) as i32;
    Some((year, month, d as i32, weekday, hour, min, sec, ms_part))
}

pub(super) fn utc_to_ms(year: i32, month: i32, day: i32, hour: i32, min: i32, sec: i32, ms: i32) -> f64 {
    let y = if month <= 1 { year - 1 } else { year } as i64;
    let m = (if month <= 1 { month + 10 } else { month - 2 }) as i64;
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u32;
    let doy = (153 * m + 2) / 5 + (day as i64 - 1);
    let doe = (yoe as i64) * 365 + (yoe as i64) / 4 - (yoe as i64) / 100 + doy;
    let days = era * 146097 + doe - 719468;
    (days * 86_400_000 + (hour as i64) * 3_600_000 + (min as i64) * 60_000 + (sec as i64) * 1000 + (ms as i64)) as f64
}

pub(super) fn parse_iso_date(s: &str) -> Option<(i32, i32, i32)> {
    let parts: Vec<&str> = s.split(|c| c == '-' || c == '/' || c == 'T').collect();
    if parts.len() >= 3 {
        let y = parts[0].parse::<i32>().ok()?;
        let m = parts[1].parse::<i32>().ok()? - 1;
        let d = parts[2].split(|c: char| !c.is_ascii_digit()).next()?.parse::<i32>().ok()?;
        return Some((y, m, d));
    }
    None
}

pub(super) fn get_date_ms(this: JsValue) -> Result<(usize, f64), VmError> {
    if let Some(ptr) = this.as_object_ptr() {
        let obj = unsafe { &*(ptr as *mut JsObject) };
        if let ObjectKind::Date(ms) = obj.kind {
            return Ok((ptr as usize, ms));
        }
    }
    Err(VmError::TypeError("Method Date.prototype called on non-Date object".into()))
}
