//! Calendar math helpers used by the `Date` implementation.

use super::*;

pub(super) const MONTHS: [&str; 12] =
    ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
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

pub(super) fn utc_to_ms(
    year: i32,
    month: i32,
    day: i32,
    hour: i32,
    min: i32,
    sec: i32,
    ms: i32,
) -> f64 {
    let y = if month <= 1 { year - 1 } else { year } as i64;
    let m = (if month <= 1 { month + 10 } else { month - 2 }) as i64;
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u32;
    let doy = (153 * m + 2) / 5 + (day as i64 - 1);
    let doe = (yoe as i64) * 365 + (yoe as i64) / 4 - (yoe as i64) / 100 + doy;
    let days = era * 146097 + doe - 719468;
    (days * 86_400_000
        + (hour as i64) * 3_600_000
        + (min as i64) * 60_000
        + (sec as i64) * 1000
        + (ms as i64)) as f64
}

pub(super) fn time_clip(time: f64) -> f64 {
    if !time.is_finite() || time.abs() > 8.64e15 {
        f64::NAN
    } else if time == 0.0 {
        0.0
    } else {
        time.trunc()
    }
}

pub(super) fn parse_date(s: &str) -> Option<f64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    if let Some(ms) = parse_iso_8601(s) {
        return Some(time_clip(ms));
    }
    if let Some(ms) = parse_rfc2822(s) {
        return Some(time_clip(ms));
    }
    None
}

fn parse_iso_8601(s: &str) -> Option<f64> {
    let bytes = s.as_bytes();
    let mut idx = 0;

    let (year_sign, year_digits) =
        if idx < bytes.len() && (bytes[idx] == b'+' || bytes[idx] == b'-') {
            let sign = if bytes[idx] == b'-' { -1 } else { 1 };
            idx += 1;
            (sign, 6)
        } else {
            (1, 4)
        };

    if idx + year_digits > bytes.len() {
        return None;
    }
    let year_str = std::str::from_utf8(&bytes[idx..idx + year_digits]).ok()?;
    let year_num = year_str.parse::<i32>().ok()?;
    if year_sign == -1 && year_num == 0 {
        return None;
    }
    let year_val = year_num * year_sign;
    idx += year_digits;

    let mut month = 0;
    let mut day = 1;

    if idx < bytes.len() && bytes[idx] == b'-' {
        idx += 1;
        if idx + 2 > bytes.len() {
            return None;
        }
        let m_str = std::str::from_utf8(&bytes[idx..idx + 2]).ok()?;
        month = m_str.parse::<i32>().ok()? - 1;
        if !(0..=11).contains(&month) {
            return None;
        }
        idx += 2;

        if idx < bytes.len() && bytes[idx] == b'-' {
            idx += 1;
            if idx + 2 > bytes.len() {
                return None;
            }
            let d_str = std::str::from_utf8(&bytes[idx..idx + 2]).ok()?;
            day = d_str.parse::<i32>().ok()?;
            if !(1..=31).contains(&day) {
                return None;
            }
            idx += 2;
        }
    }

    let mut hour = 0;
    let mut minute = 0;
    let mut second = 0;
    let mut millisecond = 0;

    if idx < bytes.len() && (bytes[idx] == b'T' || bytes[idx] == b' ') {
        idx += 1;
        if idx + 2 > bytes.len() {
            return None;
        }
        let h_str = std::str::from_utf8(&bytes[idx..idx + 2]).ok()?;
        hour = h_str.parse::<i32>().ok()?;
        if !(0..=24).contains(&hour) {
            return None;
        }
        idx += 2;

        if idx < bytes.len() && bytes[idx] == b':' {
            idx += 1;
            if idx + 2 > bytes.len() {
                return None;
            }
            let min_str = std::str::from_utf8(&bytes[idx..idx + 2]).ok()?;
            minute = min_str.parse::<i32>().ok()?;
            if !(0..=59).contains(&minute) {
                return None;
            }
            idx += 2;

            if idx < bytes.len() && bytes[idx] == b':' {
                idx += 1;
                if idx + 2 > bytes.len() {
                    return None;
                }
                let s_str = std::str::from_utf8(&bytes[idx..idx + 2]).ok()?;
                second = s_str.parse::<i32>().ok()?;
                if !(0..=59).contains(&second) {
                    return None;
                }
                idx += 2;

                if idx < bytes.len() && (bytes[idx] == b'.' || bytes[idx] == b',') {
                    idx += 1;
                    let frac_start = idx;
                    while idx < bytes.len() && bytes[idx].is_ascii_digit() {
                        idx += 1;
                    }
                    let frac_str = std::str::from_utf8(&bytes[frac_start..idx]).ok()?;
                    if frac_str.is_empty() {
                        return None;
                    }
                    let frac_val = format!("{:0<3}", frac_str);
                    millisecond = frac_val[..3].parse::<i32>().ok()?;
                }
            }
        }
    }

    let mut tz_offset_mins = 0;
    if idx < bytes.len() {
        if bytes[idx] == b'Z' || bytes[idx] == b'z' {
            idx += 1;
        } else if bytes[idx] == b'+' || bytes[idx] == b'-' {
            let tz_sign = if bytes[idx] == b'-' { -1 } else { 1 };
            idx += 1;
            if idx + 2 > bytes.len() {
                return None;
            }
            let tz_h = std::str::from_utf8(&bytes[idx..idx + 2]).ok()?.parse::<i32>().ok()?;
            idx += 2;
            let mut tz_m = 0;
            if idx < bytes.len() && bytes[idx] == b':' {
                idx += 1;
            }
            if idx + 2 <= bytes.len()
                && bytes[idx].is_ascii_digit()
                && bytes[idx + 1].is_ascii_digit()
            {
                tz_m = std::str::from_utf8(&bytes[idx..idx + 2]).ok()?.parse::<i32>().ok()?;
                idx += 2;
            }
            tz_offset_mins = tz_sign * (tz_h * 60 + tz_m);
        } else {
            return None;
        }
    }

    if idx != bytes.len() {
        return None;
    }

    let base_ms = utc_to_ms(year_val, month, day, hour, minute, second, millisecond);
    let final_ms = base_ms - (tz_offset_mins as f64) * 60_000.0;
    let clipped = time_clip(final_ms);
    if clipped.is_nan() { None } else { Some(clipped) }
}

fn parse_rfc2822(s: &str) -> Option<f64> {
    let mut month_idx = None;
    for (m_i, m_name) in MONTHS.iter().enumerate() {
        if let Some(pos) = s.to_lowercase().find(&m_name.to_lowercase()) {
            month_idx = Some((m_i as i32, pos));
            break;
        }
    }
    let (month, _) = month_idx?;

    let tokens: Vec<&str> =
        s.split(|c: char| c.is_whitespace() || c == ',').filter(|t| !t.is_empty()).collect();

    let mut year = None;
    let mut day = None;
    let mut hour = 0;
    let mut minute = 0;
    let mut second = 0;
    let mut tz_offset_mins = 0;

    for token in tokens {
        if token.contains(':') {
            let time_parts: Vec<&str> = token.split(':').collect();
            if time_parts.len() >= 2 {
                hour = time_parts[0].parse::<i32>().ok()?;
                minute = time_parts[1].parse::<i32>().ok()?;
                if time_parts.len() >= 3 {
                    second = time_parts[2].parse::<i32>().ok()?;
                }
            }
        } else if let Ok(num) = token.parse::<i32>() {
            if num > 31 && year.is_none() {
                year = Some(num);
            } else if (1..=31).contains(&num) && day.is_none() {
                day = Some(num);
            } else if year.is_none() {
                year = Some(num);
            }
        } else if token.starts_with('+') || token.starts_with('-') {
            if token.len() >= 5 {
                let sign = if token.starts_with('-') { -1 } else { 1 };
                let h = token[1..3].parse::<i32>().ok()?;
                let m = token[3..5].parse::<i32>().ok()?;
                tz_offset_mins = sign * (h * 60 + m);
            }
        } else if token.starts_with("GMT+") || token.starts_with("UTC+") {
            let rest = &token[4..];
            if rest.len() >= 4 {
                let h = rest[..2].parse::<i32>().ok()?;
                let m = rest[2..4].parse::<i32>().ok()?;
                tz_offset_mins = h * 60 + m;
            }
        } else if token.starts_with("GMT-") || token.starts_with("UTC-") {
            let rest = &token[4..];
            if rest.len() >= 4 {
                let h = rest[..2].parse::<i32>().ok()?;
                let m = rest[2..4].parse::<i32>().ok()?;
                tz_offset_mins = -(h * 60 + m);
            }
        }
    }

    let y = year?;
    let d = day.unwrap_or(1);
    let base_ms = utc_to_ms(y, month, d, hour, minute, second, 0);
    let final_ms = base_ms - (tz_offset_mins as f64) * 60_000.0;
    let clipped = time_clip(final_ms);
    if clipped.is_nan() { None } else { Some(clipped) }
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
