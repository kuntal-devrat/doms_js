//! Strict, same-value and abstract equality.

use super::*;

impl Vm {
    pub fn strict_equal(&self, a: JsValue, b: JsValue) -> bool {
        if a == b {
            if a.is_double() && a.as_double().unwrap().is_nan() {
                return false;
            }
            return true;
        }
        if a.is_number() && b.is_number() {
            let na = a.to_number().unwrap();
            let nb = b.to_number().unwrap();
            if na.is_nan() || nb.is_nan() {
                return false;
            }
            return na == nb;
        }
        let a_str = if a.is_string() {
            a.as_string_ptr().map(|p| unsafe { &*(p as *mut JsObject) })
        } else if a.is_object() {
            a.as_object_ptr().map(|p| unsafe { &*(p as *mut JsObject) })
        } else {
            None
        };
        let b_str = if b.is_string() {
            b.as_string_ptr().map(|p| unsafe { &*(p as *mut JsObject) })
        } else if b.is_object() {
            b.as_object_ptr().map(|p| unsafe { &*(p as *mut JsObject) })
        } else {
            None
        };
        if let (Some(oa), Some(ob)) = (a_str, b_str) {
            if let (ObjectKind::String(sa), ObjectKind::String(sb)) = (&oa.kind, &ob.kind) {
                return sa == sb;
            }
        }
        let a_ptr = a.as_object_ptr();
        let b_ptr = b.as_object_ptr();
        if let (Some(pa), Some(pb)) = (a_ptr, b_ptr) {
            let oa = unsafe { &*(pa as *mut JsObject) };
            let ob = unsafe { &*(pb as *mut JsObject) };
            if let (ObjectKind::BigInt(ba), ObjectKind::BigInt(bb)) = (&oa.kind, &ob.kind) {
                return ba == bb;
            }
            return pa == pb;
        }
        false
    }

    pub fn same_value(&self, a: JsValue, b: JsValue) -> bool {
        if a.is_number() && b.is_number() {
            let na = a.to_number().unwrap();
            let nb = b.to_number().unwrap();
            if na.is_nan() && nb.is_nan() {
                return true;
            }
            if na == 0.0 && nb == 0.0 {
                return na.to_bits() == nb.to_bits();
            }
            return na == nb;
        }
        self.strict_equal(a, b)
    }

    pub fn abstract_equal(&mut self, a: JsValue, b: JsValue) -> Result<bool, VmError> {
        if self.strict_equal(a, b) {
            return Ok(true);
        }
        if (a.is_null() && b.is_undefined()) || (a.is_undefined() && b.is_null()) {
            return Ok(true);
        }
        if a.is_null() || a.is_undefined() || b.is_null() || b.is_undefined() {
            return Ok(false);
        }

        let a_bi = self.as_bigint_val(a);
        let b_bi = self.as_bigint_val(b);
        let a_is_str = a.is_string() || (a.is_object() && a.as_object_ptr().is_some_and(|p| matches!(unsafe { &(*(p as *mut JsObject)).kind }, ObjectKind::String(_))));
        let b_is_str = b.is_string() || (b.is_object() && b.as_object_ptr().is_some_and(|p| matches!(unsafe { &(*(p as *mut JsObject)).kind }, ObjectKind::String(_))));

        if a.is_number() && b_is_str {
            let nb = self.to_number_value(b)?;
            let na = a.to_number().unwrap();
            return Ok(!na.is_nan() && !nb.is_nan() && na == nb);
        }
        if a_is_str && b.is_number() {
            let na = self.to_number_value(a)?;
            let nb = b.to_number().unwrap();
            return Ok(!na.is_nan() && !nb.is_nan() && na == nb);
        }

        if let Some(ref bi) = a_bi {
            if b_is_str {
                let sb = self.to_string_value(b);
                return Ok(string_to_bigint(&sb).map_or(false, |b_val| *bi == b_val));
            }
        }
        if let Some(ref bi) = b_bi {
            if a_is_str {
                let sa = self.to_string_value(a);
                return Ok(string_to_bigint(&sa).map_or(false, |a_val| a_val == *bi));
            }
        }

        if a.is_boolean() {
            let na = if a.as_boolean() == Some(true) { 1.0 } else { 0.0 };
            return self.abstract_equal(JsValue::from_double(na), b);
        }
        if b.is_boolean() {
            let nb = if b.as_boolean() == Some(true) { 1.0 } else { 0.0 };
            return self.abstract_equal(a, JsValue::from_double(nb));
        }

        if let Some(ref bi) = a_bi {
            if b.is_number() {
                let nb = b.to_number().unwrap();
                return Ok(bigint_cmp_f64(bi, nb) == Some(std::cmp::Ordering::Equal));
            }
        }
        if let Some(ref bi) = b_bi {
            if a.is_number() {
                let na = a.to_number().unwrap();
                return Ok(bigint_cmp_f64(bi, na) == Some(std::cmp::Ordering::Equal));
            }
        }

        let a_is_actual_obj = a.is_object() && a_bi.is_none() && !a_is_str;
        let b_is_actual_obj = b.is_object() && b_bi.is_none() && !b_is_str;

        if !a_is_actual_obj && b_is_actual_obj {
            let b_prim = self.to_primitive(b, PreferredType::Default)?;
            return self.abstract_equal(a, b_prim);
        }
        if a_is_actual_obj && !b_is_actual_obj {
            let a_prim = self.to_primitive(a, PreferredType::Default)?;
            return self.abstract_equal(a_prim, b);
        }

        Ok(false)
    }
}
