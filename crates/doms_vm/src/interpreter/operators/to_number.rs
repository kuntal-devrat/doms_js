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

    pub fn to_number_value(&mut self, val: JsValue) -> Result<f64, VmError> {
        let prim = self.to_primitive(val, PreferredType::Number)?;
        if prim.is_symbol() {
            return Err(VmError::TypeError("Cannot convert a Symbol value to a number".into()));
        }
        if prim.is_int32() {
            Ok(prim.as_int32().unwrap() as f64)
        } else if prim.is_double() {
            Ok(prim.as_double().unwrap())
        } else if prim.is_boolean() {
            Ok(if prim.as_boolean() == Some(true) { 1.0 } else { 0.0 })
        } else if prim.is_null() {
            Ok(0.0)
        } else if prim.is_undefined() {
            Ok(f64::NAN)
        } else if let Some(ptr) = prim.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            match &obj.kind {
                ObjectKind::String(s) | ObjectKind::StringObject(s) => {
                    let trimmed = s.trim();
                    if trimmed.is_empty() {
                        Ok(0.0)
                    } else if trimmed == "Infinity" || trimmed == "+Infinity" {
                        Ok(f64::INFINITY)
                    } else if trimmed == "-Infinity" {
                        Ok(f64::NEG_INFINITY)
                    } else if trimmed.to_ascii_lowercase().contains("inf") || trimmed.to_ascii_lowercase().contains("nan") {
                        Ok(f64::NAN)
                    } else if let Some(hex_part) = trimmed.strip_prefix("0x").or_else(|| trimmed.strip_prefix("0X")) {
                        Ok(u64::from_str_radix(hex_part, 16).map(|v| v as f64).unwrap_or(f64::NAN))
                    } else if let Some(oct_part) = trimmed.strip_prefix("0o").or_else(|| trimmed.strip_prefix("0O")) {
                        Ok(u64::from_str_radix(oct_part, 8).map(|v| v as f64).unwrap_or(f64::NAN))
                    } else if let Some(bin_part) = trimmed.strip_prefix("0b").or_else(|| trimmed.strip_prefix("0B")) {
                        Ok(u64::from_str_radix(bin_part, 2).map(|v| v as f64).unwrap_or(f64::NAN))
                    } else {
                        Ok(trimmed.parse::<f64>().unwrap_or(f64::NAN))
                    }
                }
                ObjectKind::NumberObject(n) => Ok(*n),
                ObjectKind::BooleanObject(b) => Ok(if *b { 1.0 } else { 0.0 }),
                ObjectKind::BigInt(_) => Err(VmError::TypeError("Cannot convert a BigInt value to a number".into())),
                _ => Ok(f64::NAN),
            }
        } else {
            Ok(f64::NAN)
        }
    }
}
