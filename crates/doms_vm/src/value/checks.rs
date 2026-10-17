//! Type checks, raw access, truthiness testing and `typeof` support.

use super::*;

impl JsValue {
    // ── Type checks ──

    #[inline]
    pub fn is_double(&self) -> bool {
        self.0 < DOUBLE_LIMIT
    }

    #[inline]
    pub fn is_int32(&self) -> bool {
        (self.0 & TAG_MASK) == TAG_INT32
    }

    #[inline]
    pub fn is_number(&self) -> bool {
        self.is_double() || self.is_int32()
    }

    #[inline]
    pub fn is_boolean(&self) -> bool {
        (self.0 & TAG_MASK) == TAG_BOOLEAN
    }

    #[inline]
    pub fn is_null(&self) -> bool {
        self.0 == TAG_NULL
    }

    #[inline]
    pub fn is_undefined(&self) -> bool {
        self.0 == TAG_UNDEFINED
    }

    #[inline]
    pub fn is_uninitialized(&self) -> bool {
        self.0 == TAG_UNINITIALIZED
    }

    #[inline]
    pub fn is_hole(&self) -> bool {
        self.0 == TAG_HOLE
    }

    #[inline]
    pub fn is_empty_string(&self) -> bool {
        if self.is_string() {
            if let Some(ptr) = self.as_string_ptr() {
                let obj = unsafe { &*(ptr as *mut crate::object::JsObject) };
                if let crate::object::ObjectKind::String(ref s) = obj.kind {
                    return s.is_empty();
                }
            }
        }
        if self.is_object() {
            if let Some(ptr) = self.as_object_ptr() {
                let obj = unsafe { &*(ptr as *mut crate::object::JsObject) };
                if let crate::object::ObjectKind::String(ref s) = obj.kind {
                    return s.is_empty();
                }
            }
        }
        false
    }

    #[inline]
    pub fn is_nullish(&self) -> bool {
        self.is_null() || self.is_undefined()
    }

    #[inline]
    pub fn is_object(&self) -> bool {
        (self.0 & TAG_MASK) == TAG_OBJECT
    }

    #[inline]
    pub fn is_js_object(&self) -> bool {
        if !self.is_object() {
            return false;
        }
        if let Some(ptr) = self.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut crate::object::JsObject) };
            !matches!(obj.kind, crate::object::ObjectKind::String(_) | crate::object::ObjectKind::Symbol(_) | crate::object::ObjectKind::BigInt(_))
        } else {
            false
        }
    }

    #[inline]
    pub fn is_string(&self) -> bool {
        (self.0 & TAG_MASK) == TAG_STRING
    }

    #[inline]
    pub fn is_js_string(&self) -> bool {
        if self.is_string() {
            return true;
        }
        if let Some(ptr) = self.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut crate::object::JsObject) };
            matches!(obj.kind, crate::object::ObjectKind::String(_))
        } else {
            false
        }
    }

    #[inline]
    pub fn is_symbol(&self) -> bool {
        (self.0 & TAG_MASK) == TAG_SYMBOL
    }

    #[inline]
    pub fn is_js_symbol(&self) -> bool {
        if self.is_symbol() {
            return true;
        }
        if let Some(ptr) = self.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut crate::object::JsObject) };
            matches!(obj.kind, crate::object::ObjectKind::Symbol(_))
        } else {
            false
        }
    }

    #[inline]
    pub fn is_heap_ptr(&self) -> bool {
        self.is_object() || self.is_string() || self.is_symbol()
    }

    #[inline]
    pub fn is_primitive(&self) -> bool {
        !self.is_heap_ptr()
    }

    // ── Raw access ──

    #[inline]
    pub fn raw_bits(&self) -> u64 {
        self.0
    }

    /// Create a `JsValue` from raw bits.
    ///
    /// # Safety
    /// The bits must represent a valid `JsValue` encoding.
    #[inline]
    pub unsafe fn from_raw_bits(bits: u64) -> Self {
        Self(bits)
    }

    #[inline]
    pub fn tag(&self) -> u16 {
        if self.is_double() { 0 } else { (self.0 >> 48) as u16 }
    }

    // ── Truthiness ──

    pub fn is_truthy(&self) -> bool {
        if self.is_null() || self.is_undefined() {
            return false;
        }
        if self.is_boolean() {
            return self.as_boolean().unwrap();
        }
        if self.is_int32() {
            return self.as_int32().unwrap() != 0;
        }
        if self.is_double() {
            let d = self.as_double().unwrap();
            return d != 0.0 && !d.is_nan();
        }
        if self.is_string() {
            if let Some(ptr) = self.as_string_ptr() {
                let obj = unsafe { &*(ptr as *mut crate::object::JsObject) };
                if let crate::object::ObjectKind::String(ref s) = obj.kind {
                    return !s.is_empty();
                }
            }
            return false;
        }
        if self.is_object() {
            if let Some(ptr) = self.as_object_ptr() {
                let obj = unsafe { &*(ptr as *mut crate::object::JsObject) };
                if let crate::object::ObjectKind::BigInt(ref bi) = obj.kind {
                    use num_traits::Zero;
                    return !bi.is_zero();
                }
                if let crate::object::ObjectKind::String(ref s) = obj.kind {
                    return !s.is_empty();
                }
            }
            return true;
        }
        true
    }

    // ── typeof ──

    pub fn js_typeof(&self) -> &'static str {
        if self.is_undefined() {
            "undefined"
        } else if self.is_null() {
            "object"
        } else if self.is_boolean() {
            "boolean"
        } else if self.is_number() {
            "number"
        } else if self.is_string() {
            "string"
        } else if self.is_symbol() {
            "symbol"
        } else if self.is_object() {
            "object"
        } else {
            "undefined"
        }
    }
}
