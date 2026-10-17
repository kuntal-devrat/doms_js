//! SameValueZero comparison for `JsValue`.

use super::*;

/// Compare two JavaScript values according to SameValueZero semantics.
pub fn js_values_equal(a: JsValue, b: JsValue) -> bool {
    if a == b {
        return true;
    }
    if a.is_number() && b.is_number() {
        let na = a.to_number().unwrap();
        let nb = b.to_number().unwrap();
        if na.is_nan() && nb.is_nan() {
            return true;
        }
        return na == nb;
    }
    if let (Some(ptr_a), Some(ptr_b)) = (a.as_heap_ptr(), b.as_heap_ptr()) {
        let obj_a = unsafe { &*(ptr_a as *const JsObject) };
        let obj_b = unsafe { &*(ptr_b as *const JsObject) };
        if let (ObjectKind::String(sa), ObjectKind::String(sb)) = (&obj_a.kind, &obj_b.kind) {
            return sa == sb;
        }
        if let (ObjectKind::BigInt(ba), ObjectKind::BigInt(bb)) = (&obj_a.kind, &obj_b.kind) {
            return ba == bb;
        }
    }
    false
}

/// Compare two JavaScript values according to Strict Equality Comparison (`===`) semantics.
pub fn js_strict_equal(a: JsValue, b: JsValue) -> bool {
    if a == b {
        if a.is_double() && a.as_double().unwrap().is_nan() {
            return false;
        }
        return true;
    }
    if a.is_number() && b.is_number() {
        let na = a.to_number().unwrap();
        let nb = b.to_number().unwrap();
        if na.is_nan() || nb.is_nan() {
            return false;
        }
        return na == nb;
    }
    if let (Some(ptr_a), Some(ptr_b)) = (a.as_heap_ptr(), b.as_heap_ptr()) {
        let obj_a = unsafe { &*(ptr_a as *const JsObject) };
        let obj_b = unsafe { &*(ptr_b as *const JsObject) };
        if let (ObjectKind::String(sa), ObjectKind::String(sb)) = (&obj_a.kind, &obj_b.kind) {
            return sa == sb;
        }
        if let (ObjectKind::BigInt(ba), ObjectKind::BigInt(bb)) = (&obj_a.kind, &obj_b.kind) {
            return ba == bb;
        }
    }
    false
}

