//! `Default` and standard `From` conversions for `JsValue`.

use super::*;

impl Default for JsValue {
    #[inline]
    fn default() -> Self {
        Self::undefined()
    }
}

impl From<f64> for JsValue {
    #[inline]
    fn from(value: f64) -> Self {
        Self::from_double(value)
    }
}

impl From<i32> for JsValue {
    #[inline]
    fn from(value: i32) -> Self {
        Self::from_int32(value)
    }
}

impl From<bool> for JsValue {
    #[inline]
    fn from(value: bool) -> Self {
        Self::from_boolean(value)
    }
}

impl From<()> for JsValue {
    #[inline]
    fn from(_: ()) -> Self {
        Self::undefined()
    }
}
