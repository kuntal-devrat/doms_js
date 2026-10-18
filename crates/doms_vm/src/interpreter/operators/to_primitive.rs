//! The `ToPrimitive` abstract operation.

use super::*;

impl Vm {
    pub fn to_primitive(&mut self, val: JsValue, hint: PreferredType) -> Result<JsValue, VmError> {
        if self.is_primitive_value(val) {
            return Ok(val);
        }

        // 1. Check Symbol.toPrimitive
        if let Some(sym_prim) = self.symbol_to_primitive {
            let exotic = self.get_symbol_property_result(val, sym_prim)?;
            if !exotic.is_undefined() && !exotic.is_null() {
                if !self.is_callable(exotic) {
                    return Err(VmError::TypeError("Symbol.toPrimitive must be callable".into()));
                }
                let hint_str = match hint {
                    PreferredType::String => "string",
                    PreferredType::Number => "number",
                    PreferredType::Default => "default",
                };
                let hint_val = self.alloc_string(hint_str.to_string());
                let res = self.call(exotic, val, &[hint_val])?;
                if !self.is_primitive_value(res) {
                    return Err(VmError::TypeError("Cannot convert object to primitive value".into()));
                }
                return Ok(res);
            }
        }

        if let Some(ptr) = val.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            match &obj.kind {
                ObjectKind::String(_) | ObjectKind::ConsString { .. } => Ok(val),
                ObjectKind::StringObject(s) => Ok(self.alloc_string(s.clone())),
                ObjectKind::NumberObject(n) => Ok(JsValue::from_double(*n)),
                ObjectKind::BooleanObject(b) => Ok(JsValue::from_boolean(*b)),
                ObjectKind::BigIntObject(b) => Ok(self.alloc_bigint(b.clone())),
                ObjectKind::Date(_) => {
                    let actual_hint = if hint == PreferredType::Default {
                        PreferredType::String
                    } else {
                        hint
                    };
                    let methods = if actual_hint == PreferredType::String {
                        ["toString", "valueOf"]
                    } else {
                        ["valueOf", "toString"]
                    };
                    for m in methods {
                        let fn_val = self.get_property(val, m);
                        if self.is_callable(fn_val) {
                            let res = self.call(fn_val, val, &[])?;
                            if self.is_primitive_value(res) {
                                return Ok(res);
                            }
                        }
                    }
                    Err(VmError::TypeError("Cannot convert Date to primitive value".into()))
                }
                _ => {
                    let methods = if hint == PreferredType::String {
                        ["toString", "valueOf"]
                    } else {
                        ["valueOf", "toString"]
                    };
                    for m in methods {
                        let fn_val = self.get_property(val, m);
                        if self.is_callable(fn_val) {
                            let res = self.call(fn_val, val, &[])?;
                            if self.is_primitive_value(res) {
                                return Ok(res);
                            }
                        }
                    }
                    Err(VmError::TypeError("Cannot convert object to primitive value".into()))
                }
            }
        } else {
            Ok(val)
        }
    }
}
