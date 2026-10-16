//! `Debug` and `Display` implementations for `JsValue`.

use std::fmt;

use super::*;

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
