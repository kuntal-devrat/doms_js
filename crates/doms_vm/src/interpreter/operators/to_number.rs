//! The `ToNumeric` and `ToNumber` abstract operations.

use super::*;

impl Vm {
    pub fn to_numeric(&mut self, val: JsValue) -> Result<JsValue, VmError> {
        let prim = self.to_primitive(val, PreferredType::Number)?;
        if self.as_bigint_val(prim).is_some() {
            Ok(prim)
        } else {
            let num = self.to_number_value(prim)?;
            Ok(JsValue::from_double(num))
        }
    }

    pub fn parse_js_number_string(s: &str) -> f64 {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return 0.0;
        }
        if trimmed == "Infinity" || trimmed == "+Infinity" {
            return f64::INFINITY;
        }
        if trimmed == "-Infinity" {
            return f64::NEG_INFINITY;
        }
        if trimmed.to_ascii_lowercase().contains("inf") || trimmed.to_ascii_lowercase().contains("nan") {
            return f64::NAN;
        }
        if let Some(hex_part) = trimmed.strip_prefix("0x").or_else(|| trimmed.strip_prefix("0X")) {
            return u64::from_str_radix(hex_part, 16).map(|v| v as f64).unwrap_or(f64::NAN);
        }
        if let Some(oct_part) = trimmed.strip_prefix("0o").or_else(|| trimmed.strip_prefix("0O")) {
            return u64::from_str_radix(oct_part, 8).map(|v| v as f64).unwrap_or(f64::NAN);
        }
        if let Some(bin_part) = trimmed.strip_prefix("0b").or_else(|| trimmed.strip_prefix("0B")) {
            return u64::from_str_radix(bin_part, 2).map(|v| v as f64).unwrap_or(f64::NAN);
        }
        trimmed.parse::<f64>().unwrap_or(f64::NAN)
    }

    pub fn to_integer_or_infinity(&mut self, val: JsValue) -> Result<f64, VmError> {
        if val.is_undefined() {
            return Ok(0.0);
        }
        let n = self.to_number_value(val)?;
        if n.is_nan() || n == 0.0 {
            Ok(0.0)
        } else if n.is_infinite() {
            Ok(n)
        } else {
            Ok(n.trunc())
        }
    }

    pub fn to_length(&mut self, val: JsValue) -> Result<usize, VmError> {
        let raw_len = self.to_number_value(val)?;
        if raw_len.is_nan() || raw_len <= 0.0 {
            Ok(0)
        } else if raw_len >= 9007199254740991.0 {
            Ok(9007199254740991)
        } else {
            Ok(raw_len.trunc() as usize)
        }
    }

    pub fn length_of_array_like(&mut self, obj: JsValue) -> Result<usize, VmError> {
        let len_val = self.get_property_result(obj, "length")?;
        self.to_length(len_val)
    }

    pub fn to_number_value(&mut self, val: JsValue) -> Result<f64, VmError> {
        let prim = self.to_primitive(val, PreferredType::Number)?;
        if prim.is_symbol() {
            return Err(VmError::TypeError("Cannot convert a Symbol value to a number".into()));
        }
        if prim.is_int32() {
            return Ok(prim.as_int32().unwrap() as f64);
        }
        if prim.is_double() {
            return Ok(prim.as_double().unwrap());
        }
        if prim.is_boolean() {
            return Ok(if prim.as_boolean() == Some(true) { 1.0 } else { 0.0 });
        }
        if prim.is_null() {
            return Ok(0.0);
        }
        if prim.is_undefined() {
            return Ok(f64::NAN);
        }
        if prim.is_sso_string() {
            let s = prim.as_sso_string().unwrap_or_default();
            return Ok(Self::parse_js_number_string(&s));
        }
        if let Some(ptr) = prim.as_heap_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            match &obj.kind {
                ObjectKind::String(s) | ObjectKind::StringObject(s) => {
                    return Ok(Self::parse_js_number_string(s));
                }
                ObjectKind::ConsString { .. } => {
                    let s = self.to_string_value(prim);
                    return Ok(Self::parse_js_number_string(&s));
                }
                ObjectKind::NumberObject(n) => return Ok(*n),
                ObjectKind::BooleanObject(b) => return Ok(if *b { 1.0 } else { 0.0 }),
                ObjectKind::BigInt(_) => return Err(VmError::TypeError("Cannot convert a BigInt value to a number".into())),
                _ => return Ok(f64::NAN),
            }
        }
        Ok(f64::NAN)
    }
}
