//! `JsValue` payload extractors.

use super::*;

impl JsValue {
    // Extractors

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
        if (self.0 & TAG_MASK) == TAG_STRING {
            Some((self.0 & PAYLOAD_MASK) as *mut u8)
        } else {
            None
        }
    }

    #[inline]
    pub fn as_sso_string(&self) -> Option<String> {
        self.with_sso_str(|s| s.to_string())
    }

    #[inline]
    pub fn with_sso_str<R>(&self, f: impl FnOnce(&str) -> R) -> Option<R> {
        if self.is_sso_string() {
            let len = ((self.0 >> 40) & 0x7) as usize;
            let mut bytes = [0u8; 5];
            let payload = self.0 & 0x0000_00FF_FFFF_FFFF;
            for i in 0..len {
                bytes[i] = ((payload >> (i * 8)) & 0xFF) as u8;
            }
            let s = unsafe { std::str::from_utf8_unchecked(&bytes[..len]) };
            Some(f(s))
        } else {
            None
        }
    }

    #[inline]
    pub fn sso_equals_str(&self, other: &str) -> bool {
        if !self.is_sso_string() {
            return false;
        }
        let len = ((self.0 >> 40) & 0x7) as usize;
        if other.len() != len {
            return false;
        }
        let other_bytes = other.as_bytes();
        let payload = self.0 & 0x0000_00FF_FFFF_FFFF;
        for i in 0..len {
            if ((payload >> (i * 8)) & 0xFF) as u8 != other_bytes[i] {
                return false;
            }
        }
        true
    }

    #[inline]
    pub fn as_symbol_ptr(&self) -> Option<*mut u8> {
        if self.is_symbol() { Some((self.0 & PAYLOAD_MASK) as *mut u8) } else { None }
    }
}
