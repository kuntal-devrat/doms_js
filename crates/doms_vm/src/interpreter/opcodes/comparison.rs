//! Comparison operator dispatch arms (`StrictEq`, `StrictNotEq`, `Eq`, `NotEq`, `Lt`, `Lte`, `Gt`, `Gte`).

use super::*;

impl Vm {
    pub(crate) fn op_strict_eq(&mut self) -> Result<(), VmError> {
        let b = self.pop()?;
        let a = self.pop()?;
        self.push(JsValue::from_boolean(self.strict_equal(a, b)));
        Ok(())
    }

    pub(crate) fn op_strict_not_eq(&mut self) -> Result<(), VmError> {
        let b = self.pop()?;
        let a = self.pop()?;
        self.push(JsValue::from_boolean(!self.strict_equal(a, b)));
        Ok(())
    }

    pub(crate) fn op_eq(&mut self) -> Result<(), VmError> {
        let b = self.pop()?;
        let a = self.pop()?;
        let eq = self.abstract_equal(a, b)?;
        self.push(JsValue::from_boolean(eq));
        Ok(())
    }

    pub(crate) fn op_not_eq(&mut self) -> Result<(), VmError> {
        let b = self.pop()?;
        let a = self.pop()?;
        let eq = self.abstract_equal(a, b)?;
        self.push(JsValue::from_boolean(!eq));
        Ok(())
    }

    pub(crate) fn op_lt(&mut self) -> Result<(), VmError> {
        let b_val = self.pop()?;
        let a_val = self.pop()?;
        let ord = self.abstract_relational_comparison(a_val, b_val)?;
        self.push(JsValue::from_boolean(ord == Some(std::cmp::Ordering::Less)));
        Ok(())
    }

    pub(crate) fn op_lte(&mut self) -> Result<(), VmError> {
        let b_val = self.pop()?;
        let a_val = self.pop()?;
        let ord = self.abstract_relational_comparison(a_val, b_val)?;
        self.push(JsValue::from_boolean(matches!(ord, Some(std::cmp::Ordering::Less | std::cmp::Ordering::Equal))));
        Ok(())
    }

    pub(crate) fn op_gt(&mut self) -> Result<(), VmError> {
        let b_val = self.pop()?;
        let a_val = self.pop()?;
        let ord = self.abstract_relational_comparison(a_val, b_val)?;
        self.push(JsValue::from_boolean(ord == Some(std::cmp::Ordering::Greater)));
        Ok(())
    }

    pub(crate) fn op_gte(&mut self) -> Result<(), VmError> {
        let b_val = self.pop()?;
        let a_val = self.pop()?;
        let ord = self.abstract_relational_comparison(a_val, b_val)?;
        self.push(JsValue::from_boolean(matches!(ord, Some(std::cmp::Ordering::Greater | std::cmp::Ordering::Equal))));
        Ok(())
    }
}
