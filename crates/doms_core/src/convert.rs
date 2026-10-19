//! Type conversion traits between Rust types and JavaScript values.

use crate::JsValue;
use doms_vm::Vm;

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
    fn from_js(value: JsValue, _vm: &mut Vm) -> Result<Self, String> {
        value.as_int32().ok_or_else(|| "Expected 32-bit integer".to_string())
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
