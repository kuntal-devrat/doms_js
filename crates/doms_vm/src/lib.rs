//! # DOMS VM
//!
//! Bytecode virtual machine and interpreter for the DOMS JavaScript engine.

pub mod builtins;
pub mod inline_cache;
pub mod interpreter;
pub mod jit;
pub mod object;
pub mod shape;
pub mod value;

pub use builtins::register_builtins;
pub use inline_cache::{IcState, InlineCacheSlot};
pub use interpreter::{CallFrame, Vm, VmError, VmValue};
pub use jit::{JIT_HOT_THRESHOLD, JitCompiler, JitFunction};
pub use object::{js_values_equal, JsFunction, JsObject, NativeFn, NativeFunction, ObjectKind};
pub use shape::{PropertyAttributes, PropertyInfo, Shape, ShapeId, ShapeRegistry};
pub use value::JsValue;
