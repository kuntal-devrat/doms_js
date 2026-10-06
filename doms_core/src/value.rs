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
//! Any double where the exponent is all 1s and the mantissa is non-zero is NaN.
//! Since there are ~2^52 NaN bit patterns but only one canonical NaN, we can
//! use the rest to encode other JavaScript types.
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

/// Any value whose bits, reinterpreted as u64, are below this is a double.
const DOUBLE_LIMIT: u64 = 0xFFF9_0000_0000_0000;

const TAG_INT32: u64 = 0xFFF9_0000_0000_0000;
const TAG_BOOLEAN: u64 = 0xFFFA_0000_0000_0000;
const TAG_NULL: u64 = 0xFFFB_0000_0000_0000;
const TAG_UNDEFINED: u64 = 0xFFFC_0000_0000_0000;
const TAG_OBJECT: u64 = 0xFFFD_0000_0000_0000;
const TAG_STRING: u64 = 0xFFFE_0000_0000_0000;
const TAG_SYMBOL: u64 = 0xFFFF_0000_0000_0000;

/// Mask to extract the tag (upper 16 bits).
const TAG_MASK: u64 = 0xFFFF_0000_0000_0000;
/// Mask to extract the payload (lower 48 bits).
const PAYLOAD_MASK: u64 = 0x0000_FFFF_FFFF_FFFF;
/// Mask to extract a 32-bit payload.
const PAYLOAD32_MASK: u64 = 0x0000_0000_FFFF_FFFF;

/// A NaN-boxed JavaScript value.
///
/// This is the fundamental value type in the DOMS engine. It is 8 bytes
/// (the same size as a `f64`) and can represent any JavaScript value:
/// doubles, integers, booleans, null, undefined, and heap pointers.
///
/// # Examples
///
/// ```rust
/// use doms_core::JsValue;
///
/// let num = JsValue::from_double(3.25);
/// assert!(num.is_double());
/// assert_eq!(num.as_double(), Some(3.25));
///
/// let int = JsValue::from_int32(42);
/// assert!(int.is_int32());
/// assert_eq!(int.as_int32(), Some(42));
///
/// let b = JsValue::from_boolean(true);
/// assert!(b.is_boolean());
/// assert_eq!(b.as_boolean(), Some(true));
///
/// let null = JsValue::null();
/// assert!(null.is_null());
///
/// let undef = JsValue::undefined();
/// assert!(undef.is_undefined());
/// ```
#[derive(Clone, Copy, PartialEq)]
#[repr(transparent)]
pub struct JsValue(u64);

impl JsValue {
    // ── Constructors ──

    /// Create a `JsValue` from an `f64` double.
    ///
    /// All IEEE 754 doubles are valid, including NaN, Infinity, and -0.
    /// The engine's canonical NaN is preserved.
    #[inline]
    pub fn from_double(value: f64) -> Self {
        let bits = value.to_bits();
        // If this double happens to collide with our tag space, use canonical NaN.
        // In practice, this only happens for signaling NaNs with specific patterns.
        if bits >= DOUBLE_LIMIT {
            // Return the canonical quiet NaN.
            Self(f64::NAN.to_bits())
        } else {
            Self(bits)
        }
    }

    /// Create a `JsValue` from an `i32` integer.
    ///
    /// Integers are stored directly in the payload — no heap allocation.
    /// This is used for Smi (small integer) optimization.
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

    /// Create a `JsValue` pointing to a heap-allocated object.
    ///
    /// # Safety
    /// The pointer must be a valid, GC-managed object pointer that fits
    /// in 48 bits (which is true on all current 64-bit platforms).
    #[inline]
    pub unsafe fn from_object_ptr(ptr: *mut u8) -> Self {
        debug_assert!((ptr as u64) <= PAYLOAD_MASK, "Object pointer exceeds 48-bit address space");
        Self(TAG_OBJECT | (ptr as u64 & PAYLOAD_MASK))
    }

    /// Create a `JsValue` pointing to a heap-allocated string.
    ///
    /// # Safety
    /// Same requirements as [`Self::from_object_ptr`].
    #[inline]
    pub unsafe fn from_string_ptr(ptr: *mut u8) -> Self {
        debug_assert!((ptr as u64) <= PAYLOAD_MASK, "String pointer exceeds 48-bit address space");
        Self(TAG_STRING | (ptr as u64 & PAYLOAD_MASK))
    }

    /// Create a `JsValue` pointing to a heap-allocated symbol.
    ///
    /// # Safety
    /// Same requirements as [`Self::from_object_ptr`].
    #[inline]
    pub unsafe fn from_symbol_ptr(ptr: *mut u8) -> Self {
        debug_assert!((ptr as u64) <= PAYLOAD_MASK, "Symbol pointer exceeds 48-bit address space");
        Self(TAG_SYMBOL | (ptr as u64 & PAYLOAD_MASK))
    }

    // ── Type checks ──

    /// Check if this value is a double-precision float.
    #[inline]
    pub fn is_double(&self) -> bool {
        self.0 < DOUBLE_LIMIT
    }

    /// Check if this value is a 32-bit integer.
    #[inline]
    pub fn is_int32(&self) -> bool {
        (self.0 & TAG_MASK) == TAG_INT32
    }

    /// Check if this value is a number (either double or int32).
    #[inline]
    pub fn is_number(&self) -> bool {
        self.is_double() || self.is_int32()
    }

    /// Check if this value is a boolean.
    #[inline]
    pub fn is_boolean(&self) -> bool {
        (self.0 & TAG_MASK) == TAG_BOOLEAN
    }

    /// Check if this value is `null`.
    #[inline]
    pub fn is_null(&self) -> bool {
        self.0 == TAG_NULL
    }

    /// Check if this value is `undefined`.
    #[inline]
    pub fn is_undefined(&self) -> bool {
        self.0 == TAG_UNDEFINED
    }

    /// Check if this value is `null` or `undefined` (nullish).
    #[inline]
    pub fn is_nullish(&self) -> bool {
        self.is_null() || self.is_undefined()
    }

    /// Check if this value is a heap object pointer.
    #[inline]
    pub fn is_object(&self) -> bool {
        (self.0 & TAG_MASK) == TAG_OBJECT
    }

    /// Check if this value is a heap string pointer.
    #[inline]
    pub fn is_string(&self) -> bool {
        (self.0 & TAG_MASK) == TAG_STRING
    }

    /// Check if this value is a symbol pointer.
    #[inline]
    pub fn is_symbol(&self) -> bool {
        (self.0 & TAG_MASK) == TAG_SYMBOL
    }

    /// Check if this value is a heap pointer of any kind.
    #[inline]
    pub fn is_heap_ptr(&self) -> bool {
        self.is_object() || self.is_string() || self.is_symbol()
    }

    /// Check if this value is a primitive (not a heap pointer).
    #[inline]
    pub fn is_primitive(&self) -> bool {
        !self.is_heap_ptr()
    }

    // ── Extractors ──

    /// Extract the `f64` value, or `None` if not a double.
    #[inline]
    pub fn as_double(&self) -> Option<f64> {
        if self.is_double() { Some(f64::from_bits(self.0)) } else { None }
    }

    /// Extract the `i32` value, or `None` if not an int32.
    #[inline]
    pub fn as_int32(&self) -> Option<i32> {
        if self.is_int32() { Some((self.0 & PAYLOAD32_MASK) as u32 as i32) } else { None }
    }

    /// Convert to an `f64`, coercing int32 to double.
    ///
    /// Returns `None` for non-numeric values.
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

    /// Alias for `to_number()`.
    #[inline]
    pub fn as_number(&self) -> Option<f64> {
        self.to_number()
    }

    /// Extract the boolean value, or `None` if not a boolean.
    #[inline]
    pub fn as_boolean(&self) -> Option<bool> {
        if self.is_boolean() { Some((self.0 & 1) != 0) } else { None }
    }

    /// Extract the heap pointer, or `None` if not a heap pointer.
    ///
    /// Works for objects, strings, and symbols.
    #[inline]
    pub fn as_heap_ptr(&self) -> Option<*mut u8> {
        if self.is_heap_ptr() { Some((self.0 & PAYLOAD_MASK) as *mut u8) } else { None }
    }

    /// Extract the object pointer, or `None` if not an object.
    #[inline]
    pub fn as_object_ptr(&self) -> Option<*mut u8> {
        if self.is_object() { Some((self.0 & PAYLOAD_MASK) as *mut u8) } else { None }
    }

    /// Extract the string pointer, or `None` if not a string.
    #[inline]
    pub fn as_string_ptr(&self) -> Option<*mut u8> {
        if self.is_string() { Some((self.0 & PAYLOAD_MASK) as *mut u8) } else { None }
    }

    // ── Raw access (for GC and VM internals) ──

    /// Get the raw `u64` bits. Used by the GC and VM internals.
    #[inline]
    pub fn raw_bits(&self) -> u64 {
        self.0
    }

    /// Create a `JsValue` from raw `u64` bits.
    ///
    /// # Safety
    /// The bits must represent a valid `JsValue` encoding.
    #[inline]
    pub unsafe fn from_raw_bits(bits: u64) -> Self {
        Self(bits)
    }

    /// Get the tag (upper 16 bits). Returns 0 for doubles.
    #[inline]
    pub fn tag(&self) -> u16 {
        if self.is_double() { 0 } else { (self.0 >> 48) as u16 }
    }

    // ── JavaScript truthiness ──

    /// Evaluate JavaScript "truthiness".
    ///
    /// Returns `false` for: `false`, `0`, `-0`, `NaN`, `""`, `null`, `undefined`.
    /// Returns `true` for everything else (including all objects).
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
        // Objects, strings (non-empty), symbols are truthy.
        // For strings, we'd need to check length — for now, all heap ptrs are truthy.
        true
    }

    // ── JavaScript typeof ──

    /// Returns the JavaScript `typeof` string for this value.
    pub fn js_typeof(&self) -> &'static str {
        if self.is_undefined() {
            "undefined"
        } else if self.is_null() {
            // Yes, typeof null === "object". It's a famous JS quirk.
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
            // TODO: distinguish "function" when we have callable objects
            "object"
        } else {
            "undefined"
        }
    }
}

impl Default for JsValue {
    /// The default `JsValue` is `undefined`.
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

#[cfg(test)]
mod tests {
    use super::*;

    // ── Size and alignment ──

    #[test]
    fn test_jsvalue_is_8_bytes() {
        assert_eq!(std::mem::size_of::<JsValue>(), 8);
    }

    // ── Double encoding ──

    #[test]
    fn test_double_positive() {
        let val = JsValue::from_double(3.25);
        assert!(val.is_double());
        assert!(val.is_number());
        assert!(!val.is_int32());
        assert_eq!(val.as_double(), Some(3.25));
        assert_eq!(val.to_number(), Some(3.25));
    }

    #[test]
    fn test_double_negative() {
        let val = JsValue::from_double(-273.15);
        assert!(val.is_double());
        assert_eq!(val.as_double(), Some(-273.15));
    }

    #[test]
    fn test_double_zero() {
        let val = JsValue::from_double(0.0);
        assert!(val.is_double());
        assert_eq!(val.as_double(), Some(0.0));
    }

    #[test]
    fn test_double_negative_zero() {
        let val = JsValue::from_double(-0.0);
        assert!(val.is_double());
        let d = val.as_double().unwrap();
        assert!(d.is_sign_negative());
        assert_eq!(d, 0.0);
    }

    #[test]
    fn test_double_infinity() {
        let val = JsValue::from_double(f64::INFINITY);
        assert!(val.is_double());
        assert_eq!(val.as_double(), Some(f64::INFINITY));

        let val = JsValue::from_double(f64::NEG_INFINITY);
        assert!(val.is_double());
        assert_eq!(val.as_double(), Some(f64::NEG_INFINITY));
    }

    #[test]
    fn test_double_nan() {
        let val = JsValue::from_double(f64::NAN);
        assert!(val.is_double());
        assert!(val.as_double().unwrap().is_nan());
    }

    #[test]
    fn test_double_max() {
        let val = JsValue::from_double(f64::MAX);
        assert!(val.is_double());
        assert_eq!(val.as_double(), Some(f64::MAX));
    }

    #[test]
    fn test_double_min_positive() {
        let val = JsValue::from_double(f64::MIN_POSITIVE);
        assert!(val.is_double());
        assert_eq!(val.as_double(), Some(f64::MIN_POSITIVE));
    }

    // ── Int32 encoding ──

    #[test]
    fn test_int32_positive() {
        let val = JsValue::from_int32(42);
        assert!(val.is_int32());
        assert!(val.is_number());
        assert!(!val.is_double());
        assert_eq!(val.as_int32(), Some(42));
        assert_eq!(val.to_number(), Some(42.0));
    }

    #[test]
    fn test_int32_zero() {
        let val = JsValue::from_int32(0);
        assert!(val.is_int32());
        assert_eq!(val.as_int32(), Some(0));
    }

    #[test]
    fn test_int32_negative() {
        let val = JsValue::from_int32(-1);
        assert!(val.is_int32());
        assert_eq!(val.as_int32(), Some(-1));
    }

    #[test]
    fn test_int32_max() {
        let val = JsValue::from_int32(i32::MAX);
        assert!(val.is_int32());
        assert_eq!(val.as_int32(), Some(i32::MAX));
    }

    #[test]
    fn test_int32_min() {
        let val = JsValue::from_int32(i32::MIN);
        assert!(val.is_int32());
        assert_eq!(val.as_int32(), Some(i32::MIN));
    }

    // ── Boolean encoding ──

    #[test]
    fn test_boolean_true() {
        let val = JsValue::from_boolean(true);
        assert!(val.is_boolean());
        assert!(!val.is_number());
        assert_eq!(val.as_boolean(), Some(true));
    }

    #[test]
    fn test_boolean_false() {
        let val = JsValue::from_boolean(false);
        assert!(val.is_boolean());
        assert_eq!(val.as_boolean(), Some(false));
    }

    // ── Null and Undefined ──

    #[test]
    fn test_null() {
        let val = JsValue::null();
        assert!(val.is_null());
        assert!(val.is_nullish());
        assert!(!val.is_undefined());
        assert!(!val.is_number());
        assert!(!val.is_boolean());
    }

    #[test]
    fn test_undefined() {
        let val = JsValue::undefined();
        assert!(val.is_undefined());
        assert!(val.is_nullish());
        assert!(!val.is_null());
        assert!(!val.is_number());
    }

    #[test]
    fn test_default_is_undefined() {
        let val = JsValue::default();
        assert!(val.is_undefined());
    }

    // ── Pointer encoding ──

    #[test]
    fn test_object_ptr() {
        let mut data: u64 = 0xBEEF;
        let ptr = &mut data as *mut u64 as *mut u8;
        let val = unsafe { JsValue::from_object_ptr(ptr) };

        assert!(val.is_object());
        assert!(val.is_heap_ptr());
        assert!(!val.is_primitive());
        assert!(!val.is_string());
        assert_eq!(val.as_object_ptr(), Some(ptr));
        assert_eq!(val.as_heap_ptr(), Some(ptr));
    }

    #[test]
    fn test_string_ptr() {
        let mut data: u64 = 0xCAFE;
        let ptr = &mut data as *mut u64 as *mut u8;
        let val = unsafe { JsValue::from_string_ptr(ptr) };

        assert!(val.is_string());
        assert!(val.is_heap_ptr());
        assert!(!val.is_object());
        assert_eq!(val.as_string_ptr(), Some(ptr));
    }

    // ── Truthiness ──

    #[test]
    fn test_truthiness() {
        assert!(!JsValue::null().is_truthy());
        assert!(!JsValue::undefined().is_truthy());
        assert!(!JsValue::from_boolean(false).is_truthy());
        assert!(!JsValue::from_int32(0).is_truthy());
        assert!(!JsValue::from_double(0.0).is_truthy());
        assert!(!JsValue::from_double(-0.0).is_truthy());
        assert!(!JsValue::from_double(f64::NAN).is_truthy());

        assert!(JsValue::from_boolean(true).is_truthy());
        assert!(JsValue::from_int32(1).is_truthy());
        assert!(JsValue::from_int32(-1).is_truthy());
        assert!(JsValue::from_double(3.25).is_truthy());
        assert!(JsValue::from_double(f64::INFINITY).is_truthy());
    }

    // ── typeof ──

    #[test]
    fn test_typeof() {
        assert_eq!(JsValue::undefined().js_typeof(), "undefined");
        assert_eq!(JsValue::null().js_typeof(), "object"); // Famous JS quirk
        assert_eq!(JsValue::from_boolean(true).js_typeof(), "boolean");
        assert_eq!(JsValue::from_int32(42).js_typeof(), "number");
        assert_eq!(JsValue::from_double(3.25).js_typeof(), "number");
    }

    // ── Cross-type checks ──

    #[test]
    fn test_types_are_exclusive() {
        let values = vec![
            JsValue::from_double(1.0),
            JsValue::from_int32(1),
            JsValue::from_boolean(true),
            JsValue::null(),
            JsValue::undefined(),
        ];

        for val in &values {
            let type_count = [
                val.is_double(),
                val.is_int32(),
                val.is_boolean(),
                val.is_null(),
                val.is_undefined(),
            ]
            .iter()
            .filter(|&&b| b)
            .count();

            assert_eq!(type_count, 1, "Value {:?} matched multiple types", val);
        }
    }

    // ── From trait ──

    #[test]
    fn test_from_trait() {
        let v: JsValue = 42i32.into();
        assert_eq!(v.as_int32(), Some(42));

        let v: JsValue = 3.25f64.into();
        assert_eq!(v.as_double(), Some(3.25));

        let v: JsValue = true.into();
        assert_eq!(v.as_boolean(), Some(true));

        let v: JsValue = ().into();
        assert!(v.is_undefined());
    }

    // ── Display ──

    #[test]
    fn test_display() {
        assert_eq!(format!("{}", JsValue::undefined()), "undefined");
        assert_eq!(format!("{}", JsValue::null()), "null");
        assert_eq!(format!("{}", JsValue::from_boolean(true)), "true");
        assert_eq!(format!("{}", JsValue::from_int32(42)), "42");
        assert_eq!(format!("{}", JsValue::from_double(3.25)), "3.25");
    }

    // ── Raw bits roundtrip ──

    #[test]
    fn test_raw_bits_roundtrip() {
        let vals = vec![
            JsValue::from_double(1.23456789),
            JsValue::from_int32(-999),
            JsValue::from_boolean(false),
            JsValue::null(),
            JsValue::undefined(),
        ];

        for val in vals {
            let bits = val.raw_bits();
            let restored = unsafe { JsValue::from_raw_bits(bits) };
            assert_eq!(val, restored);
        }
    }
}
