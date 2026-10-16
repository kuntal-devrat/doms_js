//! Small numeric/string conversion helpers shared by the interpreter.

use super::*;

pub fn double_to_int32(d: f64) -> i32 {
    if d.is_nan() || d.is_infinite() || d == 0.0 {
        return 0;
    }
    let d = d % 4294967296.0;
    let i = d as i64;
    (i as u32) as i32
}

pub fn f64_to_bigint_exact(f: f64) -> Option<(BigInt, bool)> {
    if f.is_nan() || f.is_infinite() {
        return None;
    }
    let bits = f.to_bits();
    let is_neg = (bits >> 63) != 0;
    let raw_exp = ((bits >> 52) & 0x7FF) as i32;
    let mantissa = bits & 0x000F_FFFF_FFFF_FFFF;

    if raw_exp == 0 {
        if mantissa == 0 {
            return Some((BigInt::zero(), false));
        } else {
            return Some((BigInt::zero(), true));
        }
    }

    let significand = (1u128 << 52) | (mantissa as u128);
    let exp = raw_exp - 1023 - 52;

    if exp >= 0 {
        let bi = BigInt::from(significand) << (exp as usize);
        Some((if is_neg { -bi } else { bi }, false))
    } else {
        let shift = (-exp) as u32;
        if shift >= 128 {
            Some((BigInt::zero(), true))
        } else {
            let int_part = significand >> shift;
            let fract_mask = (1u128 << shift) - 1;
            let has_fract = (significand & fract_mask) != 0;
            let bi = BigInt::from(int_part);
            Some((if is_neg { -bi } else { bi }, has_fract))
        }
    }
}

pub fn bigint_cmp_f64(bi: &BigInt, f: f64) -> Option<std::cmp::Ordering> {
    if f.is_nan() {
        return None;
    }
    if f.is_infinite() {
        return Some(if f > 0.0 { std::cmp::Ordering::Less } else { std::cmp::Ordering::Greater });
    }
    if let Some((f_bi, has_fract)) = f64_to_bigint_exact(f) {
        match bi.cmp(&f_bi) {
            std::cmp::Ordering::Equal => {
                if has_fract {
                    if f > 0.0 {
                        Some(std::cmp::Ordering::Less)
                    } else {
                        Some(std::cmp::Ordering::Greater)
                    }
                } else {
                    Some(std::cmp::Ordering::Equal)
                }
            }
            other => Some(other),
        }
    } else {
        None
    }
}

pub fn string_to_bigint(s: &str) -> Option<BigInt> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Some(BigInt::zero());
    }
    if let Some(rest) = trimmed.strip_prefix("0x").or_else(|| trimmed.strip_prefix("0X")) {
        if rest.is_empty() || !rest.chars().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        BigInt::from_str_radix(rest, 16).ok()
    } else if let Some(rest) = trimmed.strip_prefix("0o").or_else(|| trimmed.strip_prefix("0O")) {
        if rest.is_empty() || !rest.chars().all(|c| matches!(c, '0'..='7')) {
            return None;
        }
        BigInt::from_str_radix(rest, 8).ok()
    } else if let Some(rest) = trimmed.strip_prefix("0b").or_else(|| trimmed.strip_prefix("0B")) {
        if rest.is_empty() || !rest.chars().all(|c| matches!(c, '0' | '1')) {
            return None;
        }
        BigInt::from_str_radix(rest, 2).ok()
    } else if let Some(rest) = trimmed.strip_prefix('+') {
        if rest.is_empty() || !rest.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        BigInt::from_str(rest).ok()
    } else if let Some(rest) = trimmed.strip_prefix('-') {
        if rest.is_empty() || !rest.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        BigInt::from_str(rest).ok().map(|b| -b)
    } else {
        if !trimmed.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        BigInt::from_str(trimmed).ok()
    }
}

pub fn js_exponentiate(base: f64, exp: f64) -> f64 {
    if exp.is_nan() {
        return f64::NAN;
    }
    if exp == 0.0 {
        return 1.0;
    }
    if base.abs() == 1.0 && exp.is_infinite() {
        return f64::NAN;
    }
    base.powf(exp)
}

pub fn js_string_code_units(s: &str) -> Vec<u16> {
    let bytes = s.as_bytes();
    let mut units = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b < 0x80 {
            units.push(b as u16);
            i += 1;
        } else if b & 0xE0 == 0xC0 {
            if i + 1 < bytes.len() {
                let cp = (((b & 0x1F) as u16) << 6) | ((bytes[i + 1] & 0x3F) as u16);
                units.push(cp);
                i += 2;
            } else {
                units.push(b as u16);
                i += 1;
            }
        } else if b & 0xF0 == 0xE0 {
            if i + 2 < bytes.len() {
                let cp = (((b & 0x0F) as u16) << 12)
                    | (((bytes[i + 1] & 0x3F) as u16) << 6)
                    | ((bytes[i + 2] & 0x3F) as u16);
                units.push(cp);
                i += 3;
            } else {
                units.push(b as u16);
                i += 1;
            }
        } else if b & 0xF8 == 0xF0 {
            if i + 3 < bytes.len() {
                let cp = (((b & 0x07) as u32) << 18)
                    | (((bytes[i + 1] & 0x3F) as u32) << 12)
                    | (((bytes[i + 2] & 0x3F) as u32) << 6)
                    | ((bytes[i + 3] & 0x3F) as u32);
                let u = cp.saturating_sub(0x10000);
                let hi = ((u >> 10) as u16) + 0xD800;
                let lo = ((u & 0x3FF) as u16) + 0xDC00;
                units.push(hi);
                units.push(lo);
                i += 4;
            } else {
                units.push(b as u16);
                i += 1;
            }
        } else {
            units.push(b as u16);
            i += 1;
        }
    }
    units
}

pub fn js_string_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let ua = js_string_code_units(a);
    let ub = js_string_code_units(b);
    ua.cmp(&ub)
}
