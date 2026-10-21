//! The `ToPrimitive` abstract operation.

use super::*;

impl Vm {
    pub fn ordinary_to_primitive(
        &mut self,
        val: JsValue,
        hint: PreferredType,
    ) -> Result<JsValue, VmError> {
        if self.is_primitive_value(val) {
            return Ok(val);
        }

        if let Some(ptr) = val.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if matches!(obj.kind, ObjectKind::String(_) | ObjectKind::ConsString { .. }) {
                return Ok(val);
            }
            let has_own_props = obj.accessors.as_ref().map_or(false, |a| !a.is_empty())
                || obj.shape_id != ShapeRegistry::ROOT_SHAPE_ID;
            if !has_own_props {
                match &obj.kind {
                    ObjectKind::StringObject(s) => return Ok(self.alloc_string(s.clone())),
                    ObjectKind::NumberObject(n) => return Ok(JsValue::from_double(*n)),
                    ObjectKind::BooleanObject(b) => return Ok(JsValue::from_boolean(*b)),
                    ObjectKind::BigIntObject(b) => return Ok(self.alloc_bigint(b.clone())),
                    _ => {}
                }
            }

            let methods = if hint == PreferredType::String {
                ["toString", "valueOf"]
            } else {
                ["valueOf", "toString"]
            };
            for m in methods {
                let fn_val = self.get_property_result(val, m)?;
                if self.is_callable(fn_val) {
                    let res = self.call(fn_val, val, &[])?;
                    if self.is_primitive_value(res) {
                        return Ok(res);
                    }
                }
            }
            Err(VmError::TypeError("Cannot convert object to primitive value".into()))
        } else {
            Ok(val)
        }
    }

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
                    return Err(VmError::TypeError(
                        "Cannot convert object to primitive value".into(),
                    ));
                }
                return Ok(res);
            }
        }

        let actual_hint = if hint == PreferredType::Default { PreferredType::Number } else { hint };
        self.ordinary_to_primitive(val, actual_hint)
    }

    /// ECMAScript `ToString(argument)` abstract operation.
    pub fn to_string_spec(&mut self, val: JsValue) -> Result<String, VmError> {
        if val.is_sso_string() {
            return Ok(val.as_sso_string().unwrap_or_default());
        }
        if let Some(ptr) = val.as_heap_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::String(ref s) = obj.kind {
                return Ok(s.clone());
            }
        }
        let prim = self.to_primitive(val, PreferredType::String)?;
        if prim.is_symbol()
            || prim.as_object_ptr().is_some_and(|p| {
                matches!(unsafe { &(*(p as *mut JsObject)).kind }, ObjectKind::Symbol(_))
            })
        {
            return Err(VmError::TypeError("Cannot convert a Symbol value to a string".into()));
        }
        Ok(self.to_string_value(prim))
    }
}
