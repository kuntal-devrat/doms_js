//! `JsValue` payload extractors.

use super::*;

impl JsValue {
    // ── Extractors ──

    #[inline]
    pub fn as_double(&self) -> Option<f64> {
        if self.is_double() { Some(f64::from_bits(self.0)) } else { None }
    }

    #[inline]
    pub fn as_int32(&self) -> Option<i32> {
        if self.is_int32() { Some((self.0 & PAYLOAD32_MASK) as u32 as i32) } else { None }
    }

    #[inline]
    pub fn to_int32(&self) -> Option<i32> {
        if self.is_int32() {
            self.as_int32()
        } else if self.is_double() {
            Some(self.as_double().unwrap() as i32)
        } else {
            None
        }
    }

    #[inline]
    pub fn to_number(&self) -> Option<f64> {
        if self.is_double() {
            Some(f64::from_bits(self.0))
        } else if self.is_int32() {
            Some(self.as_int32().unwrap() as f64)
        } else {
            None
        }
    }

    #[inline]
    pub fn as_number(&self) -> Option<f64> {
        self.to_number()
    }

    #[inline]
    pub fn as_boolean(&self) -> Option<bool> {
        if self.is_boolean() { Some((self.0 & 1) != 0) } else { None }
    }

    #[inline]
    pub fn as_heap_ptr(&self) -> Option<*mut u8> {
        if self.is_heap_ptr() { Some((self.0 & PAYLOAD_MASK) as *mut u8) } else { None }
    }

    #[inline]
    pub fn as_object_ptr(&self) -> Option<*mut u8> {
        if self.is_object() { Some((self.0 & PAYLOAD_MASK) as *mut u8) } else { None }
    }

    #[inline]
    pub fn as_string_ptr(&self) -> Option<*mut u8> {
        if self.is_string() { Some((self.0 & PAYLOAD_MASK) as *mut u8) } else { None }
    }

    #[inline]
    pub fn as_symbol_ptr(&self) -> Option<*mut u8> {
        if self.is_symbol() { Some((self.0 & PAYLOAD_MASK) as *mut u8) } else { None }
    }
}
