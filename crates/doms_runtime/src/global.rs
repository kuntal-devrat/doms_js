//! Global execution environment.

use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct GlobalEnvironment {
    pub bindings: HashMap<String, String>,
}

impl GlobalEnvironment {
    pub fn new() -> Self {
        let mut env = Self::default();
        env.initialize_defaults();
        env
    }

    fn initialize_defaults(&mut self) {
        self.bindings.insert("NaN".into(), "NaN".into());
        self.bindings.insert("Infinity".into(), "Infinity".into());
        self.bindings.insert("undefined".into(), "undefined".into());
    }

    pub fn set(&mut self, key: impl Into<String>, val: impl Into<String>) {
        self.bindings.insert(key.into(), val.into());
    }

    pub fn get(&self, key: &str) -> Option<&String> {
        self.bindings.get(key)
    }
}
