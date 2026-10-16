//! The `Abstract Relational Comparison` used by `<`, `>` and friends.

use super::*;

impl Vm {
    pub fn abstract_relational_comparison(&mut self, x: JsValue, y: JsValue) -> Result<Option<std::cmp::Ordering>, VmError> {
        let px = self.to_primitive(x, PreferredType::Number)?;
        let py = self.to_primitive(y, PreferredType::Number)?;

        let px_is_sym = px.is_symbol() || (px.is_object() && px.as_object_ptr().is_some_and(|p| matches!(unsafe { &(*(p as *mut JsObject)).kind }, ObjectKind::Symbol(_))));
        let py_is_sym = py.is_symbol() || (py.is_object() && py.as_object_ptr().is_some_and(|p| matches!(unsafe { &(*(p as *mut JsObject)).kind }, ObjectKind::Symbol(_))));
        if px_is_sym || py_is_sym {
            return Err(VmError::TypeError("Cannot convert a Symbol value to a number".into()));
        }

        let px_is_str = px.is_string() || (px.is_object() && px.as_object_ptr().is_some_and(|p| matches!(unsafe { &(*(p as *mut JsObject)).kind }, ObjectKind::String(_))));
        let py_is_str = py.is_string() || (py.is_object() && py.as_object_ptr().is_some_and(|p| matches!(unsafe { &(*(p as *mut JsObject)).kind }, ObjectKind::String(_))));

        if px_is_str && py_is_str {
            let sx = self.to_string_value(px);
            let sy = self.to_string_value(py);
            return Ok(Some(js_string_cmp(&sx, &sy)));
        }

        let px_bi = self.as_bigint_val(px);
        let py_bi = self.as_bigint_val(py);

        if let (Some(bx), Some(by)) = (&px_bi, &py_bi) {
            return Ok(Some(bx.cmp(by)));
        }

        if let Some(ref bx) = px_bi {
            if py_is_str {
                let sy = self.to_string_value(py);
                return Ok(string_to_bigint(&sy).map(|by| bx.cmp(&by)));
            }
            let ny = self.to_number_value(py)?;
            return Ok(bigint_cmp_f64(bx, ny));
        }

        if let Some(ref by) = py_bi {
            if px_is_str {
                let sx = self.to_string_value(px);
                return Ok(string_to_bigint(&sx).map(|bx| bx.cmp(by)));
            }
            let nx = self.to_number_value(px)?;
            return Ok(bigint_cmp_f64(by, nx).map(|ord| ord.reverse()));
        }

        let nx = self.to_number_value(px)?;
        let ny = self.to_number_value(py)?;
        if nx.is_nan() || ny.is_nan() {
            return Ok(None);
        }
        if nx < ny {
            Ok(Some(std::cmp::Ordering::Less))
        } else if nx > ny {
            Ok(Some(std::cmp::Ordering::Greater))
        } else {
            Ok(Some(std::cmp::Ordering::Equal))
        }
    }
}
