//! # DOMS VM
//!
//! Bytecode virtual machine and interpreter for the DOMS JavaScript engine.

pub mod builtins;
pub mod debugger;
pub mod inline_cache;
pub mod inspector_server;
pub mod interpreter;
pub mod jit;
pub mod object;
pub mod shape;
pub mod value;
pub mod web_apis;

pub use builtins::register_builtins;
pub use debugger::{Breakpoint, CallFrameInfo, CdpDispatcher, DebuggerSession, StepMode};
pub use inline_cache::{IcState, InlineCacheSlot};
pub use inspector_server::InspectorServer;
pub use interpreter::{CallFrame, Vm, VmError, VmValue};
pub use jit::{JIT_HOT_THRESHOLD, JitCompiler, JitFunction};
pub use object::{js_strict_equal, js_values_equal, JsFunction, JsObject, NativeFn, NativeFunction, ObjectKind};
pub use shape::{PropertyAttributes, PropertyInfo, Shape, ShapeId, ShapeRegistry};
pub use value::JsValue;
pub use web_apis::register_web_apis;
