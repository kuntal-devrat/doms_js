//! SameValueZero and Strict Equality comparisons for `JsValue`.

use super::*;

fn get_string_content_if_string(val: JsValue) -> Option<String> {
    if val.is_sso_string() {
        return val.as_sso_string();
    }
    if let Some(ptr) = val.as_heap_ptr() {
        let obj = unsafe { &*(ptr as *const JsObject) };
        match &obj.kind {
            ObjectKind::String(s) => return Some(s.clone()),
            ObjectKind::ConsString { left, right, length } => {
                let mut result = String::with_capacity(*length);
                let mut stack = vec![*right, *left];
                while let Some(node) = stack.pop() {
                    if node.is_sso_string() {
                        if let Some(s) = node.as_sso_string() {
                            result.push_str(&s);
                        }
                    } else if let Some(nptr) = node.as_heap_ptr() {
                        let nobj = unsafe { &*(nptr as *const JsObject) };
                        match &nobj.kind {
                            ObjectKind::String(s) => result.push_str(s),
                            ObjectKind::ConsString { left: l, right: r, .. } => {
                                stack.push(*r);
                                stack.push(*l);
                            }
                            _ => {}
                        }
                    }
                }
                return Some(result);
            }
            _ => {}
        }
    }
    None
}

fn js_strings_equal(a: JsValue, b: JsValue) -> Option<bool> {
    if a.is_sso_string() && b.is_sso_string() {
        return Some(false);
    }
    if a.is_sso_string() {
        if let Some(ptr_b) = b.as_heap_ptr() {
            let obj_b = unsafe { &*(ptr_b as *const JsObject) };
            match &obj_b.kind {
                ObjectKind::String(sb) => return Some(a.sso_equals_str(sb)),
                ObjectKind::ConsString { length, .. } => {
                    let a_len = ((a.0 >> 40) & 0x7) as usize;
                    if *length != a_len {
                        return Some(false);
                    }
                    if let Some(sb) = get_string_content_if_string(b) {
                        return Some(a.sso_equals_str(&sb));
                    }
                }
                _ => {}
            }
        }
        return None;
    }
    if b.is_sso_string() {
        if let Some(ptr_a) = a.as_heap_ptr() {
            let obj_a = unsafe { &*(ptr_a as *const JsObject) };
            match &obj_a.kind {
                ObjectKind::String(sa) => return Some(b.sso_equals_str(sa)),
                ObjectKind::ConsString { length, .. } => {
                    let b_len = ((b.0 >> 40) & 0x7) as usize;
                    if *length != b_len {
                        return Some(false);
                    }
                    if let Some(sa) = get_string_content_if_string(a) {
                        return Some(b.sso_equals_str(&sa));
                    }
                }
                _ => {}
            }
        }
        return None;
    }
    if let (Some(ptr_a), Some(ptr_b)) = (a.as_heap_ptr(), b.as_heap_ptr()) {
        let obj_a = unsafe { &*(ptr_a as *const JsObject) };
        let obj_b = unsafe { &*(ptr_b as *const JsObject) };
        match (&obj_a.kind, &obj_b.kind) {
            (ObjectKind::String(sa), ObjectKind::String(sb)) => return Some(sa == sb),
            (ObjectKind::ConsString { length: la, .. }, ObjectKind::ConsString { length: lb, .. }) => {
                if la != lb {
                    return Some(false);
                }
                let sa = get_string_content_if_string(a)?;
                let sb = get_string_content_if_string(b)?;
                return Some(sa == sb);
            }
            (ObjectKind::String(sa), ObjectKind::ConsString { length, .. }) |
            (ObjectKind::ConsString { length, .. }, ObjectKind::String(sa)) => {
                if sa.encode_utf16().count() != *length {
                    return Some(false);
                }
                let s_other = if matches!(&obj_a.kind, ObjectKind::ConsString { .. }) {
                    get_string_content_if_string(a)?
                } else {
                    get_string_content_if_string(b)?
                };
                return Some(sa == &s_other);
            }
            _ => {}
        }
    }
    None
}

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
    if let Some(eq) = js_strings_equal(a, b) {
        return eq;
    }
    if let (Some(ptr_a), Some(ptr_b)) = (a.as_heap_ptr(), b.as_heap_ptr()) {
        let obj_a = unsafe { &*(ptr_a as *const JsObject) };
        let obj_b = unsafe { &*(ptr_b as *const JsObject) };
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
    if let Some(eq) = js_strings_equal(a, b) {
        return eq;
    }
    if let (Some(ptr_a), Some(ptr_b)) = (a.as_heap_ptr(), b.as_heap_ptr()) {
        let obj_a = unsafe { &*(ptr_a as *const JsObject) };
        let obj_b = unsafe { &*(ptr_b as *const JsObject) };
        if let (ObjectKind::BigInt(ba), ObjectKind::BigInt(bb)) = (&obj_a.kind, &obj_b.kind) {
            return ba == bb;
        }
    }
    false
}

