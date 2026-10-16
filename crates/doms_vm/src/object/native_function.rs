//! Native (Rust-implemented) function values.

use super::*;

pub type NativeFn = fn(&mut crate::interpreter::Vm, JsValue, &[JsValue]) -> Result<JsValue, crate::interpreter::VmError>;
pub type ClosureNativeFn = Arc<dyn Fn(&mut crate::interpreter::Vm, JsValue, &[JsValue]) -> Result<JsValue, crate::interpreter::VmError> + Send + Sync>;

#[derive(Clone)]
pub enum NativeHandler {
    Fn(NativeFn),
    Closure(ClosureNativeFn),
}

#[derive(Clone)]
pub struct NativeFunction {
    pub name: String,
    pub length: u8,
    pub handler: NativeHandler,
}

impl NativeFunction {
    pub fn new_fn(name: String, length: u8, func: NativeFn) -> Self {
        Self {
            name,
            length,
            handler: NativeHandler::Fn(func),
        }
    }

    pub fn new_closure(name: String, length: u8, func: ClosureNativeFn) -> Self {
        Self {
            name,
            length,
            handler: NativeHandler::Closure(func),
        }
    }

    pub fn call(&self, vm: &mut crate::interpreter::Vm, this: JsValue, args: &[JsValue]) -> Result<JsValue, crate::interpreter::VmError> {
        match &self.handler {
            NativeHandler::Fn(f) => (f)(vm, this, args),
            NativeHandler::Closure(c) => (c)(vm, this, args),
        }
    }
}

impl std::fmt::Debug for NativeFunction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "NativeFunction({})", self.name)
    }
}
