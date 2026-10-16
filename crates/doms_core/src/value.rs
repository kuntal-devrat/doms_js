//! NaN-boxed JavaScript value representation.
//!
//! Re-exports the unified [`JsValue`] type from `doms_vm`.

pub use doms_vm::value::JsValue;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jsvalue_is_8_bytes() {
        assert_eq!(std::mem::size_of::<JsValue>(), 8);
    }

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
    fn test_int32_positive() {
        let val = JsValue::from_int32(42);
        assert!(val.is_int32());
        assert!(val.is_number());
        assert!(!val.is_double());
        assert_eq!(val.as_int32(), Some(42));
        assert_eq!(val.to_number(), Some(42.0));
    }

    #[test]
    fn test_boolean_true() {
        let val = JsValue::from_boolean(true);
        assert!(val.is_boolean());
        assert!(!val.is_number());
        assert_eq!(val.as_boolean(), Some(true));
    }

    #[test]
    fn test_null_undefined() {
        assert!(JsValue::null().is_null());
        assert!(JsValue::undefined().is_undefined());
    }

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
