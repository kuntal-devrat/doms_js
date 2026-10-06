//! # DOMS Runtime
//!
//! JavaScript standard library, global environment, and built-ins for DOMS.

pub mod builtins;
pub mod global;

pub use builtins::*;
pub use global::GlobalEnvironment;
