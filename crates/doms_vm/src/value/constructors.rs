//! `JsValue` constructors and tagged-value factories.

use super::*;

impl JsValue {
    // ── Constructors ──

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
}
