//! Type conversion traits between Rust types and JavaScript values.

use crate::JsValue;
use doms_vm::Vm;
use std::collections::HashMap;

/// Conversion from a Rust type into a JavaScript [`JsValue`].
pub trait IntoJs {
    /// Convert `self` into a [`JsValue`], allocating in the VM heap if necessary.
    fn into_js(self, vm: &mut Vm) -> JsValue;
}

/// Conversion from a JavaScript [`JsValue`] into a Rust type.
pub trait FromJs: Sized {
    /// Attempt to convert a [`JsValue`] into `Self`.
    fn from_js(value: JsValue, vm: &mut Vm) -> Result<Self, String>;
}

impl IntoJs for JsValue {
    #[inline]
    fn into_js(self, _vm: &mut Vm) -> JsValue {
        self
    }
}

impl FromJs for JsValue {
    #[inline]
    fn from_js(value: JsValue, _vm: &mut Vm) -> Result<Self, String> {
        Ok(value)
    }
}

impl IntoJs for bool {
    #[inline]
    fn into_js(self, _vm: &mut Vm) -> JsValue {
        JsValue::from_boolean(self)
    }
}

impl FromJs for bool {
    #[inline]
    fn from_js(value: JsValue, _vm: &mut Vm) -> Result<Self, String> {
        value.as_boolean().ok_or_else(|| "Expected boolean".to_string())
    }
}

impl IntoJs for i32 {
    #[inline]
    fn into_js(self, _vm: &mut Vm) -> JsValue {
        JsValue::from_int32(self)
    }
}

impl FromJs for i32 {
    #[inline]
    fn from_js(value: JsValue, vm: &mut Vm) -> Result<Self, String> {
        if let Some(n) = value.as_int32() {
            Ok(n)
        } else {
            let n = vm.to_number_value(value).map_err(|e| e.to_string())?;
            Ok(n as i32)
        }
    }
}

impl IntoJs for i64 {
    #[inline]
    fn into_js(self, _vm: &mut Vm) -> JsValue {
        JsValue::from_double(self as f64)
    }
}

impl FromJs for i64 {
    #[inline]
    fn from_js(value: JsValue, vm: &mut Vm) -> Result<Self, String> {
        if let Some(n) = value.as_int32() {
            Ok(n as i64)
        } else {
            let n = vm.to_number_value(value).map_err(|e| e.to_string())?;
            Ok(n as i64)
        }
    }
}

impl IntoJs for isize {
    #[inline]
    fn into_js(self, _vm: &mut Vm) -> JsValue {
        JsValue::from_double(self as f64)
    }
}

impl FromJs for isize {
    #[inline]
    fn from_js(value: JsValue, vm: &mut Vm) -> Result<Self, String> {
        let n = vm.to_number_value(value).map_err(|e| e.to_string())?;
        Ok(n as isize)
    }
}

impl IntoJs for u32 {
    #[inline]
    fn into_js(self, _vm: &mut Vm) -> JsValue {
        JsValue::from_double(self as f64)
    }
}

impl FromJs for u32 {
    #[inline]
    fn from_js(value: JsValue, vm: &mut Vm) -> Result<Self, String> {
        let n = vm.to_number_value(value).map_err(|e| e.to_string())?;
        Ok(n as u32)
    }
}

impl IntoJs for usize {
    #[inline]
    fn into_js(self, _vm: &mut Vm) -> JsValue {
        JsValue::from_double(self as f64)
    }
}

impl FromJs for usize {
    #[inline]
    fn from_js(value: JsValue, vm: &mut Vm) -> Result<Self, String> {
        let n = vm.to_number_value(value).map_err(|e| e.to_string())?;
        Ok(n as usize)
    }
}

impl IntoJs for f32 {
    #[inline]
    fn into_js(self, _vm: &mut Vm) -> JsValue {
        JsValue::from_double(self as f64)
    }
}

impl FromJs for f32 {
    #[inline]
    fn from_js(value: JsValue, vm: &mut Vm) -> Result<Self, String> {
        let n = vm.to_number_value(value).map_err(|e| e.to_string())?;
        Ok(n as f32)
    }
}

impl IntoJs for f64 {
    #[inline]
    fn into_js(self, _vm: &mut Vm) -> JsValue {
        JsValue::from_double(self)
    }
}

impl FromJs for f64 {
    #[inline]
    fn from_js(value: JsValue, vm: &mut Vm) -> Result<Self, String> {
        vm.to_number_value(value).map_err(|e| e.to_string())
    }
}

impl IntoJs for String {
    #[inline]
    fn into_js(self, vm: &mut Vm) -> JsValue {
        vm.alloc_string(self)
    }
}

impl IntoJs for &str {
    #[inline]
    fn into_js(self, vm: &mut Vm) -> JsValue {
        vm.alloc_string(self.to_string())
    }
}

impl FromJs for String {
    #[inline]
    fn from_js(value: JsValue, vm: &mut Vm) -> Result<Self, String> {
        Ok(vm.to_string_value(value))
    }
}

impl IntoJs for () {
    #[inline]
    fn into_js(self, _vm: &mut Vm) -> JsValue {
        JsValue::undefined()
    }
}

impl<T: IntoJs> IntoJs for Option<T> {
    #[inline]
    fn into_js(self, vm: &mut Vm) -> JsValue {
        match self {
            Some(v) => v.into_js(vm),
            None => JsValue::null(),
        }
    }
}

impl<T: FromJs> FromJs for Option<T> {
    #[inline]
    fn from_js(value: JsValue, vm: &mut Vm) -> Result<Self, String> {
        if value.is_null() || value.is_undefined() {
            Ok(None)
        } else {
            T::from_js(value, vm).map(Some)
        }
    }
}

impl<T: IntoJs> IntoJs for Vec<T> {
    fn into_js(self, vm: &mut Vm) -> JsValue {
        let elements = self.into_iter().map(|item| item.into_js(vm)).collect();
        vm.alloc_array(elements)
    }
}

impl<T: Clone + IntoJs> IntoJs for &[T] {
    fn into_js(self, vm: &mut Vm) -> JsValue {
        let elements = self.iter().cloned().map(|item| item.into_js(vm)).collect();
        vm.alloc_array(elements)
    }
}

impl<T: FromJs> FromJs for Vec<T> {
    fn from_js(value: JsValue, vm: &mut Vm) -> Result<Self, String> {
        if !value.is_js_object() {
            return Err("Expected array object".to_string());
        }
        let len_val = vm.get_property(value, "length");
        let len = len_val
            .as_int32()
            .map(|n| n.max(0) as usize)
            .or_else(|| len_val.as_double().map(|n| n.max(0.0) as usize))
            .unwrap_or(0);
        let mut vec = Vec::with_capacity(len);
        for i in 0..len {
            let elem = vm.get_element(value, i);
            vec.push(T::from_js(elem, vm)?);
        }
        Ok(vec)
    }
}

impl<T: IntoJs> IntoJs for HashMap<String, T> {
    fn into_js(self, vm: &mut Vm) -> JsValue {
        let obj = vm.alloc_object();
        for (k, v) in self {
            let js_v = v.into_js(vm);
            vm.set_property(obj, &k, js_v);
        }
        obj
    }
}

impl<A: IntoJs, B: IntoJs> IntoJs for (A, B) {
    fn into_js(self, vm: &mut Vm) -> JsValue {
        let a = self.0.into_js(vm);
        let b = self.1.into_js(vm);
        vm.alloc_array(vec![a, b])
    }
}

impl<A: FromJs, B: FromJs> FromJs for (A, B) {
    fn from_js(value: JsValue, vm: &mut Vm) -> Result<Self, String> {
        if !value.is_js_object() {
            return Err("Expected array tuple".to_string());
        }
        let a_val = vm.get_element(value, 0);
        let b_val = vm.get_element(value, 1);
        Ok((A::from_js(a_val, vm)?, B::from_js(b_val, vm)?))
    }
}
