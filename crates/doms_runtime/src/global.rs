//! Global execution environment.

use std::collections::HashMap;

/// Runtime representation of JavaScript primitive values in doms_runtime.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum RuntimeValue {
    #[default]
    Undefined,
    Null,
    Boolean(bool),
    Number(f64),
    String(String),
}

impl RuntimeValue {
    #[inline]
    pub fn is_undefined(&self) -> bool {
        matches!(self, Self::Undefined)
    }

    #[inline]
    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }

    #[inline]
    pub fn is_number(&self) -> bool {
        matches!(self, Self::Number(_))
    }

    #[inline]
    pub fn is_string(&self) -> bool {
        matches!(self, Self::String(_))
    }

    #[inline]
    pub fn is_boolean(&self) -> bool {
        matches!(self, Self::Boolean(_))
    }
}

impl From<f64> for RuntimeValue {
    fn from(n: f64) -> Self {
        Self::Number(n)
    }
}

impl From<bool> for RuntimeValue {
    fn from(b: bool) -> Self {
        Self::Boolean(b)
    }
}

impl From<String> for RuntimeValue {
    fn from(s: String) -> Self {
        Self::String(s)
    }
}

impl From<&str> for RuntimeValue {
    fn from(s: &str) -> Self {
        Self::String(s.to_string())
    }
}

/// Global variable bindings environment.
#[derive(Debug, Clone, Default)]
pub struct GlobalEnvironment<V = RuntimeValue> {
    pub bindings: HashMap<String, V>,
}

impl GlobalEnvironment<RuntimeValue> {
    pub fn new() -> Self {
        let mut env = Self::default();
        env.initialize_defaults();
        env
    }

    fn initialize_defaults(&mut self) {
        self.bindings.insert("NaN".into(), RuntimeValue::Number(f64::NAN));
        self.bindings.insert("Infinity".into(), RuntimeValue::Number(f64::INFINITY));
        self.bindings.insert("undefined".into(), RuntimeValue::Undefined);
    }
}

impl<V> GlobalEnvironment<V> {
    pub fn set(&mut self, key: impl Into<String>, val: V) {
        self.bindings.insert(key.into(), val);
    }

    pub fn get(&self, key: &str) -> Option<&V> {
        self.bindings.get(key)
    }

    pub fn contains(&self, key: &str) -> bool {
        self.bindings.contains_key(key)
    }

    pub fn len(&self) -> usize {
        self.bindings.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_global_environment_defaults() {
        let env = GlobalEnvironment::new();
        assert_eq!(env.get("undefined"), Some(&RuntimeValue::Undefined));
        if let Some(RuntimeValue::Number(n)) = env.get("NaN") {
            assert!(n.is_nan());
        } else {
            panic!("Expected NaN to be Number");
        }
        if let Some(RuntimeValue::Number(n)) = env.get("Infinity") {
            assert!(n.is_infinite());
        } else {
            panic!("Expected Infinity to be Number");
        }
    }

    #[test]
    fn test_global_environment_set_get() {
        let mut env = GlobalEnvironment::new();
        env.set("x", RuntimeValue::from(42.0));
        assert_eq!(env.get("x"), Some(&RuntimeValue::Number(42.0)));
        assert_eq!(env.get("y"), None);
    }
}
