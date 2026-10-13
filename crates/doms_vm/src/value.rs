//! NaN-boxed JavaScript value representation.
//!
//! # NaN-Boxing
//!
//! NaN-boxing exploits the IEEE 754 double-precision floating-point format
//! to store non-double values in the "NaN space." A quiet NaN has the pattern:
//!
//! ```text
//! s111 1111 1111 1xxx xxxx xxxx xxxx xxxx xxxx xxxx xxxx xxxx xxxx xxxx xxxx
//!  ^                ^                                                        ^
//!  sign             quiet NaN bit                              48-bit payload
//! ```
//!
//! ## Encoding Scheme
//!
//! ```text
//! Tag (bits 48-50)    Type             Payload (bits 0-47)
//! ──────────────────────────────────────────────────────────
//! (any non-NaN)       f64 double       IEEE 754 double
//! 0xFFF9              Int32            lower 32 bits = i32
//! 0xFFFA              Boolean          bit 0 = true/false
//! 0xFFFB              Null             (no payload)
//! 0xFFFC              Undefined        (no payload)
//! 0xFFFD              Object ptr       48-bit heap pointer
//! 0xFFFE              String ptr       48-bit heap pointer
//! 0xFFFF              Symbol ptr       48-bit heap pointer
//! ```
//!
//! This gives us an 8-byte `JsValue` that can hold any JS value without
//! heap allocation for primitives — as fast as a `u64`.

use std::fmt;

// ── Tag constants (upper 16 bits) ──

const DOUBLE_LIMIT: u64 = 0xFFF9_0000_0000_0000;

const TAG_INT32: u64 = 0xFFF9_0000_0000_0000;
const TAG_BOOLEAN: u64 = 0xFFFA_0000_0000_0000;
const TAG_NULL: u64 = 0xFFFB_0000_0000_0000;
const TAG_UNDEFINED: u64 = 0xFFFC_0000_0000_0000;
const TAG_UNINITIALIZED: u64 = 0xFFFC_0000_0000_0001;
const TAG_OBJECT: u64 = 0xFFFD_0000_0000_0000;
const TAG_STRING: u64 = 0xFFFE_0000_0000_0000;
const TAG_SYMBOL: u64 = 0xFFFF_0000_0000_0000;

const TAG_MASK: u64 = 0xFFFF_0000_0000_0000;
const PAYLOAD_MASK: u64 = 0x0000_FFFF_FFFF_FFFF;
const PAYLOAD32_MASK: u64 = 0x0000_0000_FFFF_FFFF;

/// A NaN-boxed JavaScript value.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct JsValue(pub u64);

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
    pub fn is_nullish(&self) -> bool {
        self.is_null() || self.is_undefined()
    }

    #[inline]
    pub fn is_object(&self) -> bool {
        (self.0 & TAG_MASK) == TAG_OBJECT
    }

    #[inline]
    pub fn is_string(&self) -> bool {
        (self.0 & TAG_MASK) == TAG_STRING
    }

    #[inline]
    pub fn is_symbol(&self) -> bool {
        (self.0 & TAG_MASK) == TAG_SYMBOL
    }

    #[inline]
    pub fn is_heap_ptr(&self) -> bool {
        self.is_object() || self.is_string() || self.is_symbol()
    }

    #[inline]
    pub fn is_primitive(&self) -> bool {
        !self.is_heap_ptr()
    }

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

impl Default for JsValue {
    #[inline]
    fn default() -> Self {
        Self::undefined()
    }
}

impl fmt::Debug for JsValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_undefined() {
            write!(f, "undefined")
        } else if self.is_null() {
            write!(f, "null")
        } else if self.is_boolean() {
            write!(f, "{}", self.as_boolean().unwrap())
        } else if self.is_int32() {
            write!(f, "{}i", self.as_int32().unwrap())
        } else if self.is_double() {
            write!(f, "{}f", self.as_double().unwrap())
        } else if self.is_object() {
            write!(f, "Object({:p})", self.as_object_ptr().unwrap())
        } else if self.is_string() {
            write!(f, "String({:p})", self.as_string_ptr().unwrap())
        } else if self.is_symbol() {
            write!(f, "Symbol({:p})", self.as_heap_ptr().unwrap())
        } else {
            write!(f, "JsValue(0x{:016X})", self.0)
        }
    }
}

impl fmt::Display for JsValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_undefined() {
            write!(f, "undefined")
        } else if self.is_null() {
            write!(f, "null")
        } else if self.is_boolean() {
            write!(f, "{}", self.as_boolean().unwrap())
        } else if self.is_int32() {
            write!(f, "{}", self.as_int32().unwrap())
        } else if self.is_double() {
            let d = self.as_double().unwrap();
            if d.fract() == 0.0 && d.is_finite() && d.abs() < (i64::MAX as f64) {
                write!(f, "{}", d as i64)
            } else {
                write!(f, "{}", d)
            }
        } else if self.is_object() {
            write!(f, "[object Object]")
        } else if self.is_string() {
            write!(f, "<string@{:p}>", self.as_string_ptr().unwrap())
        } else {
            write!(f, "<unknown>")
        }
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
