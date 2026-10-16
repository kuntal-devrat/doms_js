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

mod checks;
mod constructors;
mod conversions;
mod extractors;
mod formatting;

// ── Tag constants (upper 16 bits) ──

const DOUBLE_LIMIT: u64 = 0xFFF9_0000_0000_0000;

const TAG_INT32: u64 = 0xFFF9_0000_0000_0000;
const TAG_BOOLEAN: u64 = 0xFFFA_0000_0000_0000;
const TAG_NULL: u64 = 0xFFFB_0000_0000_0000;
const TAG_UNDEFINED: u64 = 0xFFFC_0000_0000_0000;
const TAG_UNINITIALIZED: u64 = 0xFFFC_0000_0000_0001;
const TAG_HOLE: u64 = 0xFFFC_0000_0000_0002;
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
