//! `JsValue` constructors and tagged-value factories.

use super::*;

impl JsValue {
    // Constructors

    /// Create a `JsValue` from an `f64` double.
    #[inline]
    pub fn from_double(value: f64) -> Self {
        let bits = value.to_bits();
        if bits >= DOUBLE_LIMIT {
            Self(f64::NAN.to_bits())
        } else {
            Self(bits)
        }
    }

    /// Create a `JsValue` from an `i32` integer.
    #[inline]
    pub fn from_int32(value: i32) -> Self {
        Self(TAG_INT32 | (value as u32 as u64))
    }

    /// Create a `JsValue` from a boolean.
    #[inline]
    pub fn from_boolean(value: bool) -> Self {
        Self(TAG_BOOLEAN | value as u64)
    }

    /// Create the `null` value.
    #[inline]
    pub const fn null() -> Self {
        Self(TAG_NULL)
    }

    /// Create the `undefined` value.
    #[inline]
    pub const fn undefined() -> Self {
        Self(TAG_UNDEFINED)
    }

    /// Create the TDZ uninitialized value.
    #[inline]
    pub const fn uninitialized() -> Self {
        Self(TAG_UNINITIALIZED)
    }

    /// Create the array hole value.
    #[inline]
    pub const fn hole() -> Self {
        Self(TAG_HOLE)
    }

    /// Create a `JsValue` pointing to a heap-allocated object.
    ///
    /// # Safety
    /// The pointer must be a valid, GC-managed object pointer that fits
    /// in 48 bits.
    #[inline]
    pub unsafe fn from_object_ptr(ptr: *mut u8) -> Self {
        debug_assert!((ptr as u64) <= PAYLOAD_MASK, "Object pointer exceeds 48-bit address space");
        Self(TAG_OBJECT | (ptr as u64 & PAYLOAD_MASK))
    }

    /// Create a `JsValue` pointing to a heap-allocated string.
    ///
    /// # Safety
    /// The pointer must be a valid, GC-managed string pointer that fits
    /// in 48 bits.
    #[inline]
    pub unsafe fn from_string_ptr(ptr: *mut u8) -> Self {
        debug_assert!((ptr as u64) <= PAYLOAD_MASK, "String pointer exceeds 48-bit address space");
        Self(TAG_STRING | (ptr as u64 & PAYLOAD_MASK))
    }

    /// Create a `JsValue` pointing to a heap-allocated symbol.
    ///
    /// # Safety
    /// The pointer must be a valid, GC-managed symbol pointer that fits
    /// in 48 bits.
    #[inline]
    pub unsafe fn from_symbol_ptr(ptr: *mut u8) -> Self {
        debug_assert!((ptr as u64) <= PAYLOAD_MASK, "Symbol pointer exceeds 48-bit address space");
        Self(TAG_SYMBOL | (ptr as u64 & PAYLOAD_MASK))
    }

    /// Update the heap pointer payload in this `JsValue`, preserving the tag.
    ///
    /// # Safety
    /// `new_ptr` must be a valid, aligned GC-managed pointer fitting within 48 bits.
    #[inline]
    pub unsafe fn update_heap_ptr(&mut self, new_ptr: *mut u8) {
        let tag = self.0 & TAG_MASK;
        self.0 = tag | (new_ptr as u64 & PAYLOAD_MASK);
    }

    /// Create an SSO (Small String Optimization) inline string if len <= 5 and ASCII.
    #[inline]
    pub fn try_from_sso_string(s: &str) -> Option<Self> {
        let bytes = s.as_bytes();
        if bytes.len() <= 5 && bytes.iter().all(|&b| b.is_ascii()) {
            let mut payload = 0u64;
            for (i, &b) in bytes.iter().enumerate() {
                payload |= (b as u64) << (i * 8);
            }
            let len_bits = (bytes.len() as u64) << 40;
            Some(Self(TAG_SSO_STRING | len_bits | payload))
        } else {
            None
        }
    }
}
