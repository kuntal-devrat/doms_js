//! Standard built-in objects for DOMS runtime.

#[derive(Debug, Clone)]
pub struct BuiltinFunction {
    pub name: &'static str,
    pub length: u8,
}

pub const MATH_BUILTINS: &[&str] =
    &["abs", "ceil", "floor", "round", "max", "min", "sqrt", "pow", "random"];

pub const CONSOLE_BUILTINS: &[&str] = &["log", "info", "warn", "error", "debug"];
