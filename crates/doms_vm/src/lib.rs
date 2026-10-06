//! # DOMS VM
//!
//! Bytecode virtual machine and interpreter for the DOMS JavaScript engine.

pub mod builtins;
pub mod inline_cache;
pub mod interpreter;
pub mod object;
pub mod shape;
pub mod value;

pub use builtins::register_builtins;
pub use inline_cache::{IcState, InlineCacheSlot};
pub use interpreter::{CallFrame, Vm, VmError, VmValue};
pub use object::{JsFunction, JsObject, NativeFn, NativeFunction, ObjectKind};
pub use shape::{PropertyAttributes, PropertyInfo, Shape, ShapeId, ShapeRegistry};
pub use value::JsValue;
