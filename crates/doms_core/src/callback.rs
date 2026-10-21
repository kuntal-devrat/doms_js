//! Typed callback traits for binding Rust closures to JavaScript functions.

use crate::JsValue;
use crate::convert::{FromJs, IntoJs};
use doms_vm::Vm;
use doms_vm::interpreter::VmError;
use std::fmt::Display;

/// Trait for return types of native callbacks: supports both plain `T: IntoJs`
/// and `Result<T: IntoJs, E: Display>` for throwing JS exceptions.
pub trait IntoJsReturn {
    fn into_js_return(self, vm: &mut Vm) -> Result<JsValue, VmError>;
}

impl<T: IntoJs> IntoJsReturn for T {
    #[inline]
    fn into_js_return(self, vm: &mut Vm) -> Result<JsValue, VmError> {
        Ok(self.into_js(vm))
    }
}

impl<T: IntoJs, E: Display> IntoJsReturn for Result<T, E> {
    #[inline]
    fn into_js_return(self, vm: &mut Vm) -> Result<JsValue, VmError> {
        match self {
            Ok(val) => Ok(val.into_js(vm)),
            Err(e) => Err(VmError::TypeError(e.to_string())),
        }
    }
}

/// A Rust function or closure that can be bound to a JavaScript global or object property.
pub trait JsCallback<Args, Ret>: Send + Sync + 'static {
    fn invoke(&self, vm: &mut Vm, this: JsValue, args: &[JsValue]) -> Result<JsValue, VmError>;
    fn arity(&self) -> u8;
}

// Arity 0
impl<F, Ret> JsCallback<(), Ret> for F
where
    F: Fn() -> Ret + Send + Sync + 'static,
    Ret: IntoJsReturn,
{
    fn invoke(&self, vm: &mut Vm, _this: JsValue, _args: &[JsValue]) -> Result<JsValue, VmError> {
        (self)().into_js_return(vm)
    }
    fn arity(&self) -> u8 {
        0
    }
}

// Arity 1
impl<F, A, Ret> JsCallback<(A,), Ret> for F
where
    F: Fn(A) -> Ret + Send + Sync + 'static,
    A: FromJs,
    Ret: IntoJsReturn,
{
    fn invoke(&self, vm: &mut Vm, _this: JsValue, args: &[JsValue]) -> Result<JsValue, VmError> {
        let a = A::from_js(args.first().copied().unwrap_or_else(JsValue::undefined), vm)
            .map_err(VmError::TypeError)?;
        (self)(a).into_js_return(vm)
    }
    fn arity(&self) -> u8 {
        1
    }
}

// Arity 2
impl<F, A, B, Ret> JsCallback<(A, B), Ret> for F
where
    F: Fn(A, B) -> Ret + Send + Sync + 'static,
    A: FromJs,
    B: FromJs,
    Ret: IntoJsReturn,
{
    fn invoke(&self, vm: &mut Vm, _this: JsValue, args: &[JsValue]) -> Result<JsValue, VmError> {
        let a = A::from_js(args.first().copied().unwrap_or_else(JsValue::undefined), vm)
            .map_err(VmError::TypeError)?;
        let b = B::from_js(args.get(1).copied().unwrap_or_else(JsValue::undefined), vm)
            .map_err(VmError::TypeError)?;
        (self)(a, b).into_js_return(vm)
    }
    fn arity(&self) -> u8 {
        2
    }
}

// Arity 3
impl<F, A, B, C, Ret> JsCallback<(A, B, C), Ret> for F
where
    F: Fn(A, B, C) -> Ret + Send + Sync + 'static,
    A: FromJs,
    B: FromJs,
    C: FromJs,
    Ret: IntoJsReturn,
{
    fn invoke(&self, vm: &mut Vm, _this: JsValue, args: &[JsValue]) -> Result<JsValue, VmError> {
        let a = A::from_js(args.first().copied().unwrap_or_else(JsValue::undefined), vm)
            .map_err(VmError::TypeError)?;
        let b = B::from_js(args.get(1).copied().unwrap_or_else(JsValue::undefined), vm)
            .map_err(VmError::TypeError)?;
        let c = C::from_js(args.get(2).copied().unwrap_or_else(JsValue::undefined), vm)
            .map_err(VmError::TypeError)?;
        (self)(a, b, c).into_js_return(vm)
    }
    fn arity(&self) -> u8 {
        3
    }
}

// Arity 4
impl<F, A, B, C, D, Ret> JsCallback<(A, B, C, D), Ret> for F
where
    F: Fn(A, B, C, D) -> Ret + Send + Sync + 'static,
    A: FromJs,
    B: FromJs,
    C: FromJs,
    D: FromJs,
    Ret: IntoJsReturn,
{
    fn invoke(&self, vm: &mut Vm, _this: JsValue, args: &[JsValue]) -> Result<JsValue, VmError> {
        let a = A::from_js(args.first().copied().unwrap_or_else(JsValue::undefined), vm)
            .map_err(VmError::TypeError)?;
        let b = B::from_js(args.get(1).copied().unwrap_or_else(JsValue::undefined), vm)
            .map_err(VmError::TypeError)?;
        let c = C::from_js(args.get(2).copied().unwrap_or_else(JsValue::undefined), vm)
            .map_err(VmError::TypeError)?;
        let d = D::from_js(args.get(3).copied().unwrap_or_else(JsValue::undefined), vm)
            .map_err(VmError::TypeError)?;
        (self)(a, b, c, d).into_js_return(vm)
    }
    fn arity(&self) -> u8 {
        4
    }
}
