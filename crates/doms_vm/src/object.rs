//! Object model, prototypes, functions, and arrays.

use crate::shape::ShapeId;
use crate::value::JsValue;
use doms_compiler::bytecode::BytecodeChunk;
use doms_gc::trace::{Trace, Tracer};
use std::sync::Arc;

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

#[derive(Debug, Clone)]
pub struct JsFunction {
    pub name: Option<String>,
    pub params: Vec<String>,
    pub chunk: Arc<BytecodeChunk>,
    pub upvalues: Vec<JsValue>,
}

#[derive(Debug, Clone)]
pub enum ObjectKind {
    Ordinary,
    Array(Vec<JsValue>),
    Function(JsFunction),
    NativeFunction(NativeFunction),
    String(String),
    Error { name: String, message: String },
}

#[derive(Debug, Clone)]
pub struct JsObject {
    pub shape_id: ShapeId,
    pub prototype: Option<JsValue>,
    pub slots: Vec<JsValue>,
    pub kind: ObjectKind,
}

impl JsObject {
    pub fn new_ordinary(shape_id: ShapeId, prototype: Option<JsValue>) -> Self {
        Self {
            shape_id,
            prototype,
            slots: Vec::new(),
            kind: ObjectKind::Ordinary,
        }
    }

    pub fn new_array(shape_id: ShapeId, prototype: Option<JsValue>, elements: Vec<JsValue>) -> Self {
        Self {
            shape_id,
            prototype,
            slots: Vec::new(),
            kind: ObjectKind::Array(elements),
        }
    }

    pub fn new_function(
        shape_id: ShapeId,
        prototype: Option<JsValue>,
        func: JsFunction,
    ) -> Self {
        Self {
            shape_id,
            prototype,
            slots: Vec::new(),
            kind: ObjectKind::Function(func),
        }
    }

    pub fn new_native_function(
        shape_id: ShapeId,
        prototype: Option<JsValue>,
        native: NativeFunction,
    ) -> Self {
        Self {
            shape_id,
            prototype,
            slots: Vec::new(),
            kind: ObjectKind::NativeFunction(native),
        }
    }

    pub fn new_string(
        shape_id: ShapeId,
        prototype: Option<JsValue>,
        content: String,
    ) -> Self {
        Self {
            shape_id,
            prototype,
            slots: Vec::new(),
            kind: ObjectKind::String(content),
        }
    }

    pub fn new_error(
        shape_id: ShapeId,
        prototype: Option<JsValue>,
        name: String,
        message: String,
    ) -> Self {
        Self {
            shape_id,
            prototype,
            slots: Vec::new(),
            kind: ObjectKind::Error { name, message },
        }
    }

    #[inline]
    pub fn get_slot(&self, offset: u32) -> Option<JsValue> {
        self.slots.get(offset as usize).copied()
    }

    #[inline]
    pub fn set_slot(&mut self, offset: u32, val: JsValue) {
        let idx = offset as usize;
        if idx >= self.slots.len() {
            self.slots.resize(idx + 1, JsValue::undefined());
        }
        self.slots[idx] = val;
    }
}

impl Trace for JsObject {
    fn trace(&self, tracer: &mut Tracer) {
        if let Some(proto) = self.prototype {
            if let Some(p) = proto.as_heap_ptr() {
                tracer.trace_ref(p);
            }
        }
        for slot in &self.slots {
            if let Some(p) = slot.as_heap_ptr() {
                tracer.trace_ref(p);
            }
        }
        match &self.kind {
            ObjectKind::Array(elements) => {
                for el in elements {
                    if let Some(p) = el.as_heap_ptr() {
                        tracer.trace_ref(p);
                    }
                }
            }
            ObjectKind::Function(f) => {
                for upval in &f.upvalues {
                    if let Some(p) = upval.as_heap_ptr() {
                        tracer.trace_ref(p);
                    }
                }
            }
            _ => {}
        }
    }
}
