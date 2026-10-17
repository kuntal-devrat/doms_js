//! Binary, unary, bitwise and type inspection operators implemented in the dispatch loop.

use super::*;

impl Vm {
    pub(crate) fn op_add(&mut self) -> Result<(), VmError> {
        let b = self.pop()?;
        let a = self.pop()?;

        let prev_temp = self.temp_roots.len();
        self.temp_roots.push(a);
        self.temp_roots.push(b);

        let a_prim = match self.to_primitive(a, PreferredType::Default) {
            Ok(v) => v,
            Err(e) => {
                self.temp_roots.truncate(prev_temp);
                return Err(e);
            }
        };
        self.temp_roots.push(a_prim);

        let b_prim = match self.to_primitive(b, PreferredType::Default) {
            Ok(v) => v,
            Err(e) => {
                self.temp_roots.truncate(prev_temp);
                return Err(e);
            }
        };
        self.temp_roots.push(b_prim);

        let a_is_sym = a_prim.is_symbol() || (a_prim.is_object() && a_prim.as_object_ptr().is_some_and(|p| matches!(unsafe { &(*(p as *mut JsObject)).kind }, ObjectKind::Symbol(_))));
        let b_is_sym = b_prim.is_symbol() || (b_prim.is_object() && b_prim.as_object_ptr().is_some_and(|p| matches!(unsafe { &(*(p as *mut JsObject)).kind }, ObjectKind::Symbol(_))));
        if a_is_sym || b_is_sym {
            self.temp_roots.truncate(prev_temp);
            return Err(VmError::TypeError("Cannot convert a Symbol value to a number or string".into()));
        }

        let a_is_str = a_prim.is_string() || (a_prim.is_object() && a_prim.as_object_ptr().is_some_and(|p| matches!(unsafe { &(*(p as *mut JsObject)).kind }, ObjectKind::String(_) | ObjectKind::ConsString { .. })));
        let b_is_str = b_prim.is_string() || (b_prim.is_object() && b_prim.as_object_ptr().is_some_and(|p| matches!(unsafe { &(*(p as *mut JsObject)).kind }, ObjectKind::String(_) | ObjectKind::ConsString { .. })));

        if a_is_str || b_is_str {
            let left_str_val = if a_is_str {
                a_prim
            } else {
                let sa = self.to_string_value(a_prim);
                self.alloc_string(sa)
            };
            self.temp_roots.push(left_str_val);

            let right_str_val = if b_is_str {
                b_prim
            } else {
                let sb = self.to_string_value(b_prim);
                self.alloc_string(sb)
            };
            self.temp_roots.push(right_str_val);

            let len_a = self.js_string_len(left_str_val);
            let len_b = self.js_string_len(right_str_val);
            let total_len = len_a + len_b;

            let res = if len_a == 0 {
                right_str_val
            } else if len_b == 0 {
                left_str_val
            } else if total_len > 32 {
                self.alloc_cons_string(left_str_val, right_str_val, total_len)
            } else {
                let sa = self.to_string_value(left_str_val);
                let sb = self.to_string_value(right_str_val);
                self.alloc_string(format!("{}{}", sa, sb))
            };

            self.temp_roots.truncate(prev_temp);
            self.push(res);
        } else if let (Some(ba), Some(bb)) = (self.as_bigint_val(a_prim), self.as_bigint_val(b_prim)) {
            let res = self.alloc_bigint(&ba + &bb);
            self.temp_roots.truncate(prev_temp);
            self.push(res);
        } else if self.as_bigint_val(a_prim).is_some() || self.as_bigint_val(b_prim).is_some() {
            self.temp_roots.truncate(prev_temp);
            return Err(VmError::TypeError("Cannot mix BigInt and other types, use explicit conversions".into()));
        } else {
            let na = match self.to_number_value(a_prim) {
                Ok(n) => n,
                Err(e) => {
                    self.temp_roots.truncate(prev_temp);
                    return Err(e);
                }
            };
            let nb = match self.to_number_value(b_prim) {
                Ok(n) => n,
                Err(e) => {
                    self.temp_roots.truncate(prev_temp);
                    return Err(e);
                }
            };
            self.temp_roots.truncate(prev_temp);
            self.push(JsValue::from_double(na + nb));
        }
        Ok(())
    }

    pub(crate) fn op_sub(&mut self) -> Result<(), VmError> {
        let b_val = self.pop()?;
        let a_val = self.pop()?;
        let a_num = self.to_numeric(a_val)?;
        let b_num = self.to_numeric(b_val)?;
        if let (Some(ba), Some(bb)) = (self.as_bigint_val(a_num), self.as_bigint_val(b_num)) {
            let res = self.alloc_bigint(&ba - &bb);
            self.push(res);
        } else if self.as_bigint_val(a_num).is_some() || self.as_bigint_val(b_num).is_some() {
            return Err(VmError::TypeError("Cannot mix BigInt and other types, use explicit conversions".into()));
        } else {
            let a = a_num.to_number().unwrap_or(f64::NAN);
            let b = b_num.to_number().unwrap_or(f64::NAN);
            self.push(JsValue::from_double(a - b));
        }
        Ok(())
    }

    pub(crate) fn op_mul(&mut self) -> Result<(), VmError> {
        let b_val = self.pop()?;
        let a_val = self.pop()?;
        let a_num = self.to_numeric(a_val)?;
        let b_num = self.to_numeric(b_val)?;
        if let (Some(ba), Some(bb)) = (self.as_bigint_val(a_num), self.as_bigint_val(b_num)) {
            let res = self.alloc_bigint(&ba * &bb);
            self.push(res);
        } else if self.as_bigint_val(a_num).is_some() || self.as_bigint_val(b_num).is_some() {
            return Err(VmError::TypeError("Cannot mix BigInt and other types, use explicit conversions".into()));
        } else {
            let a = a_num.to_number().unwrap_or(f64::NAN);
            let b = b_num.to_number().unwrap_or(f64::NAN);
            self.push(JsValue::from_double(a * b));
        }
        Ok(())
    }

    pub(crate) fn op_div(&mut self) -> Result<(), VmError> {
        let b_val = self.pop()?;
        let a_val = self.pop()?;
        let a_num = self.to_numeric(a_val)?;
        let b_num = self.to_numeric(b_val)?;
        if let (Some(ba), Some(bb)) = (self.as_bigint_val(a_num), self.as_bigint_val(b_num)) {
            if bb.is_zero() {
                return Err(VmError::RangeError("Division by zero".into()));
            }
            let res = self.alloc_bigint(&ba / &bb);
            self.push(res);
        } else if self.as_bigint_val(a_num).is_some() || self.as_bigint_val(b_num).is_some() {
            return Err(VmError::TypeError("Cannot mix BigInt and other types, use explicit conversions".into()));
        } else {
            let a = a_num.to_number().unwrap_or(f64::NAN);
            let b = b_num.to_number().unwrap_or(f64::NAN);
            self.push(JsValue::from_double(a / b));
        }
        Ok(())
    }

    pub(crate) fn op_mod(&mut self) -> Result<(), VmError> {
        let b_val = self.pop()?;
        let a_val = self.pop()?;
        let a_num = self.to_numeric(a_val)?;
        let b_num = self.to_numeric(b_val)?;
        if let (Some(ba), Some(bb)) = (self.as_bigint_val(a_num), self.as_bigint_val(b_num)) {
            if bb.is_zero() {
                return Err(VmError::RangeError("Division by zero".into()));
            }
            let res = self.alloc_bigint(&ba % &bb);
            self.push(res);
        } else if self.as_bigint_val(a_num).is_some() || self.as_bigint_val(b_num).is_some() {
            return Err(VmError::TypeError("Cannot mix BigInt and other types, use explicit conversions".into()));
        } else {
            let a = a_num.to_number().unwrap_or(f64::NAN);
            let b = b_num.to_number().unwrap_or(f64::NAN);
            self.push(JsValue::from_double(a % b));
        }
        Ok(())
    }

    pub(crate) fn op_exp(&mut self) -> Result<(), VmError> {
        let b_val = self.pop()?;
        let a_val = self.pop()?;
        let a_num = self.to_numeric(a_val)?;
        let b_num = self.to_numeric(b_val)?;
        if let (Some(ba), Some(bb)) = (self.as_bigint_val(a_num), self.as_bigint_val(b_num)) {
            if bb.is_negative() {
                return Err(VmError::RangeError("Exponent must be non-negative".into()));
            } else if bb.is_zero() {
                let res = self.alloc_bigint(BigInt::one());
                self.push(res);
            } else {
                let exp = bb.to_u32().ok_or_else(|| VmError::RangeError("Maximum BigInt size exceeded".into()))?;
                let res = self.alloc_bigint(ba.pow(exp));
                self.push(res);
            }
        } else if self.as_bigint_val(a_num).is_some() || self.as_bigint_val(b_num).is_some() {
            return Err(VmError::TypeError("Cannot mix BigInt and other types, use explicit conversions".into()));
        } else {
            let a = a_num.to_number().unwrap_or(f64::NAN);
            let b = b_num.to_number().unwrap_or(f64::NAN);
            self.push(JsValue::from_double(js_exponentiate(a, b)));
        }
        Ok(())
    }

    pub(crate) fn op_pos(&mut self) -> Result<(), VmError> {
        let v = self.pop()?;
        if self.as_bigint_val(v).is_some() {
            return Err(VmError::TypeError("Cannot convert a BigInt value to a number".into()));
        }
        let n = self.to_number_value(v)?;
        self.push(JsValue::from_double(n));
        Ok(())
    }

    pub(crate) fn op_neg(&mut self) -> Result<(), VmError> {
        let v = self.pop()?;
        if let Some(bi) = self.as_bigint_val(v) {
            let res = self.alloc_bigint(-&bi);
            self.push(res);
        } else {
            let n = self.to_number_value(v)?;
            self.push(JsValue::from_double(-n));
        }
        Ok(())
    }

    pub(crate) fn op_not(&mut self) -> Result<(), VmError> {
        let v = self.pop()?;
        self.push(JsValue::from_boolean(!v.is_truthy()));
        Ok(())
    }

    pub(crate) fn op_bit_not(&mut self) -> Result<(), VmError> {
        let v = self.pop()?;
        let v_num = self.to_numeric(v)?;
        if let Some(bi) = self.as_bigint_val(v_num) {
            let res = self.alloc_bigint(!bi);
            self.push(res);
        } else {
            let num = self.to_number_value(v_num)?;
            let n = double_to_int32(num);
            self.push(JsValue::from_int32(!n));
        }
        Ok(())
    }

    pub(crate) fn op_typeof(&mut self) -> Result<(), VmError> {
        let v = self.pop()?;
        let t = if v.is_symbol() {
            "symbol"
        } else if let Some(ptr) = v.as_heap_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            match &obj.kind {
                ObjectKind::String(_) => "string",
                ObjectKind::Symbol(_) => if v.is_symbol() { "symbol" } else { "object" },
                ObjectKind::BigInt(_) => "bigint",
                ObjectKind::Function(_) | ObjectKind::NativeFunction(_) => "function",
                ObjectKind::Proxy { target, .. } => {
                    let tgt = *target;
                    if self.is_callable(tgt) {
                        "function"
                    } else {
                        "object"
                    }
                }
                _ => "object",
            }
        } else {
            v.js_typeof()
        };
        let s_val = self.alloc_string(t.to_string());
        self.push(s_val);
        Ok(())
    }

    pub(crate) fn op_bit_and(&mut self) -> Result<(), VmError> {
        let b_val = self.pop()?;
        let a_val = self.pop()?;
        let a_num = self.to_numeric(a_val)?;
        let b_num = self.to_numeric(b_val)?;
        if let (Some(ba), Some(bb)) = (self.as_bigint_val(a_num), self.as_bigint_val(b_num)) {
            let res = self.alloc_bigint(&ba & &bb);
            self.push(res);
        } else if self.as_bigint_val(a_num).is_some() || self.as_bigint_val(b_num).is_some() {
            return Err(VmError::TypeError("Cannot mix BigInt and other types, use explicit conversions".into()));
        } else {
            let a_int = double_to_int32(self.to_number_value(a_num)?);
            let b_int = double_to_int32(self.to_number_value(b_num)?);
            self.push(JsValue::from_int32(a_int & b_int));
        }
        Ok(())
    }

    pub(crate) fn op_bit_or(&mut self) -> Result<(), VmError> {
        let b_val = self.pop()?;
        let a_val = self.pop()?;
        let a_num = self.to_numeric(a_val)?;
        let b_num = self.to_numeric(b_val)?;
        if let (Some(ba), Some(bb)) = (self.as_bigint_val(a_num), self.as_bigint_val(b_num)) {
            let res = self.alloc_bigint(&ba | &bb);
            self.push(res);
        } else if self.as_bigint_val(a_num).is_some() || self.as_bigint_val(b_num).is_some() {
            return Err(VmError::TypeError("Cannot mix BigInt and other types, use explicit conversions".into()));
        } else {
            let a_int = double_to_int32(self.to_number_value(a_num)?);
            let b_int = double_to_int32(self.to_number_value(b_num)?);
            self.push(JsValue::from_int32(a_int | b_int));
        }
        Ok(())
    }

    pub(crate) fn op_bit_xor(&mut self) -> Result<(), VmError> {
        let b_val = self.pop()?;
        let a_val = self.pop()?;
        let a_num = self.to_numeric(a_val)?;
        let b_num = self.to_numeric(b_val)?;
        if let (Some(ba), Some(bb)) = (self.as_bigint_val(a_num), self.as_bigint_val(b_num)) {
            let res = self.alloc_bigint(&ba ^ &bb);
            self.push(res);
        } else if self.as_bigint_val(a_num).is_some() || self.as_bigint_val(b_num).is_some() {
            return Err(VmError::TypeError("Cannot mix BigInt and other types, use explicit conversions".into()));
        } else {
            let a_int = double_to_int32(self.to_number_value(a_num)?);
            let b_int = double_to_int32(self.to_number_value(b_num)?);
            self.push(JsValue::from_int32(a_int ^ b_int));
        }
        Ok(())
    }

    pub(crate) fn op_shl(&mut self) -> Result<(), VmError> {
        let b_val = self.pop()?;
        let a_val = self.pop()?;
        let a_num = self.to_numeric(a_val)?;
        let b_num = self.to_numeric(b_val)?;
        if let (Some(ba), Some(bb)) = (self.as_bigint_val(a_num), self.as_bigint_val(b_num)) {
            if bb.is_negative() {
                let shift = (-bb).to_usize().ok_or_else(|| VmError::RangeError("Maximum BigInt size exceeded".into()))?;
                let res = self.alloc_bigint(&ba >> shift);
                self.push(res);
            } else {
                let shift = bb.to_usize().ok_or_else(|| VmError::RangeError("Maximum BigInt size exceeded".into()))?;
                let res = self.alloc_bigint(&ba << shift);
                self.push(res);
            }
        } else if self.as_bigint_val(a_num).is_some() || self.as_bigint_val(b_num).is_some() {
            return Err(VmError::TypeError("Cannot mix BigInt and other types, use explicit conversions".into()));
        } else {
            let a = self.to_number_value(a_num)?;
            let b = self.to_number_value(b_num)?;
            let a_int = double_to_int32(a);
            let b_uint = (double_to_int32(b) as u32) & 0x1F;
            self.push(JsValue::from_int32(a_int.wrapping_shl(b_uint)));
        }
        Ok(())
    }

    pub(crate) fn op_shr(&mut self) -> Result<(), VmError> {
        let b_val = self.pop()?;
        let a_val = self.pop()?;
        let a_num = self.to_numeric(a_val)?;
        let b_num = self.to_numeric(b_val)?;
        if let (Some(ba), Some(bb)) = (self.as_bigint_val(a_num), self.as_bigint_val(b_num)) {
            if bb.is_negative() {
                let shift = (-bb).to_usize().ok_or_else(|| VmError::RangeError("Maximum BigInt size exceeded".into()))?;
                let res = self.alloc_bigint(&ba << shift);
                self.push(res);
            } else {
                let shift = bb.to_usize().ok_or_else(|| VmError::RangeError("Maximum BigInt size exceeded".into()))?;
                let res = self.alloc_bigint(&ba >> shift);
                self.push(res);
            }
        } else if self.as_bigint_val(a_num).is_some() || self.as_bigint_val(b_num).is_some() {
            return Err(VmError::TypeError("Cannot mix BigInt and other types, use explicit conversions".into()));
        } else {
            let a = self.to_number_value(a_num)?;
            let b = self.to_number_value(b_num)?;
            let a_int = double_to_int32(a);
            let b_uint = (double_to_int32(b) as u32) & 0x1F;
            self.push(JsValue::from_int32(a_int.wrapping_shr(b_uint)));
        }
        Ok(())
    }

    pub(crate) fn op_ushr(&mut self) -> Result<(), VmError> {
        let b_val = self.pop()?;
        let a_val = self.pop()?;
        let a_num = self.to_numeric(a_val)?;
        let b_num = self.to_numeric(b_val)?;
        if self.as_bigint_val(a_num).is_some() || self.as_bigint_val(b_num).is_some() {
            return Err(VmError::TypeError("BigInt has no unsigned right shift, use >> instead".into()));
        } else {
            let a = self.to_number_value(a_num)?;
            let b = self.to_number_value(b_num)?;
            let a_u32 = double_to_int32(a) as u32;
            let b_uint = (double_to_int32(b) as u32) & 0x1F;
            let res = a_u32.wrapping_shr(b_uint);
            self.push(JsValue::from_double(res as f64));
        }
        Ok(())
    }

    pub(crate) fn op_instanceof(&mut self) -> Result<(), VmError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        if !lhs.is_object() {
            self.push(JsValue::from_boolean(false));
        } else if let Some(rhs_ptr) = rhs.as_object_ptr() {
            let _rhs_obj = unsafe { &*(rhs_ptr as *mut JsObject) };
            let proto = self.get_property(rhs, "prototype");
            let mut is_instance = false;
            if proto.is_object() {
                let mut curr_val = self.get_prototype_of_val(lhs)?;
                while curr_val.is_object() {
                    if curr_val == proto {
                        is_instance = true;
                        break;
                    }
                    curr_val = self.get_prototype_of_val(curr_val)?;
                }
            }
            self.push(JsValue::from_boolean(is_instance));
        } else {
            return Err(VmError::TypeError("Right-hand side of 'instanceof' is not an object".into()));
        }
        Ok(())
    }

    pub(crate) fn op_in(&mut self) -> Result<(), VmError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        if !rhs.is_js_object() {
            return Err(VmError::TypeError("Cannot use 'in' operator to search for property in non-object".into()));
        }
        let prop_key = self.to_property_key(lhs)?;
        let has = self.has_property_key(rhs, &prop_key)?;
        self.push(JsValue::from_boolean(has));
        Ok(())
    }
}
