//! Callability and constructor checks.

use super::*;

impl Vm {
    pub fn is_callable(&self, val: JsValue) -> bool {
        if let Some(ptr) = val.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            match &obj.kind {
                ObjectKind::Function(_) | ObjectKind::NativeFunction(_) => true,
                ObjectKind::Proxy { target, .. } => self.is_callable(*target),
                _ => false,
            }
        } else {
            false
        }
    }

    pub fn is_constructor(&mut self, val: JsValue) -> bool {
        if let Some(ptr) = val.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let Some(sym_ctor) = self.globals.get("Symbol") {
                if val == *sym_ctor {
                    return false;
                }
            }
            match &obj.kind {
                ObjectKind::Function(f) => {
                    !f.is_generator && !f.is_async && self.shapes.has_property(obj.shape_id, "prototype")
                }
                ObjectKind::NativeFunction(n) => {
                    const NON_CONSTRUCTORS: &[&str] = &[
                        "encodeURI", "decodeURI", "encodeURIComponent", "decodeURIComponent",
                        "parseInt", "parseFloat", "isNaN", "isFinite", "eval", "Symbol",
                        "escape", "unescape",
                    ];
                    if NON_CONSTRUCTORS.contains(&n.name.as_str()) || n.name.contains('.') {
                        return false;
                    }
                    if self.shapes.has_property(obj.shape_id, "prototype") {
                        return true;
                    }
                    if n.name.starts_with("bound ") {
                        return true;
                    }
                    n.name.chars().next().map_or(false, |c| c.is_ascii_uppercase())
                }
                ObjectKind::Proxy { target, .. } => {
                    let t = *target;
                    self.is_constructor(t)
                }
                _ => false,
            }
        } else {
            false
        }
    }

    pub fn is_primitive_value(&self, val: JsValue) -> bool {
        if !val.is_object() {
            return true;
        }
        if let Some(ptr) = val.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            matches!(obj.kind, ObjectKind::String(_) | ObjectKind::BigInt(_))
        } else {
            true
        }
    }
}
