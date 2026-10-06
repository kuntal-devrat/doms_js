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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromiseState {
    Pending,
    Fulfilled,
    Rejected,
}

#[derive(Debug, Clone)]
pub struct JsPromise {
    pub state: PromiseState,
    pub result: JsValue,
    pub fulfill_reactions: Vec<JsValue>,
    pub reject_reactions: Vec<JsValue>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeneratorState {
    SuspendedStart,
    SuspendedYield,
    Executing,
    Completed,
}

#[derive(Debug, Clone)]
pub struct JsGenerator {
    pub function: JsFunction,
    pub ip: usize,
    pub stack: Vec<JsValue>,
    pub state: GeneratorState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypedArrayKind {
    Int32,
    Uint8,
}

#[derive(Debug, Clone)]
pub struct JsTypedArray {
    pub buffer: Arc<doms_parallel::SharedArrayBuffer>,
    pub kind: TypedArrayKind,
    pub length: usize,
}

#[derive(Debug, Clone)]
pub struct JsFunction {
    pub name: Option<String>,
    pub params: Vec<String>,
    pub chunk: Arc<BytecodeChunk>,
    pub upvalues: Vec<JsValue>,
    pub is_async: bool,
    pub is_generator: bool,
    pub call_count: u32,
    pub jit_entry: Option<crate::jit::JitFunction>,
}

#[derive(Debug, Clone)]
pub enum ObjectKind {
    Ordinary,
    Array(Vec<JsValue>),
    Function(JsFunction),
    NativeFunction(NativeFunction),
    String(String),
    Error { name: String, message: String },
    Promise(JsPromise),
    Generator(JsGenerator),
    SharedArrayBuffer(Arc<doms_parallel::SharedArrayBuffer>),
    TypedArray(JsTypedArray),
    Worker(Arc<std::sync::Mutex<doms_parallel::MessagePort>>),
    Map(Vec<(JsValue, JsValue)>),
    Set(Vec<JsValue>),
    WeakRef(JsValue),
    FinalizationRegistry {
        callback: JsValue,
        entries: Vec<(JsValue, JsValue, Option<JsValue>)>,
    },
    RegExp {
        pattern: String,
        flags: String,
    },
    Date(f64),
    Symbol(Option<String>),
    BigInt(i64),
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

    pub fn new_promise(
        shape_id: ShapeId,
        prototype: Option<JsValue>,
        state: PromiseState,
        result: JsValue,
    ) -> Self {
        Self {
            shape_id,
            prototype,
            slots: Vec::new(),
            kind: ObjectKind::Promise(JsPromise {
                state,
                result,
                fulfill_reactions: Vec::new(),
                reject_reactions: Vec::new(),
            }),
        }
    }

    pub fn new_shared_array_buffer(
        shape_id: ShapeId,
        prototype: Option<JsValue>,
        sab: Arc<doms_parallel::SharedArrayBuffer>,
    ) -> Self {
        Self {
            shape_id,
            prototype,
            slots: Vec::new(),
            kind: ObjectKind::SharedArrayBuffer(sab),
        }
    }

    pub fn new_typed_array(
        shape_id: ShapeId,
        prototype: Option<JsValue>,
        typed_array: JsTypedArray,
    ) -> Self {
        Self {
            shape_id,
            prototype,
            slots: Vec::new(),
            kind: ObjectKind::TypedArray(typed_array),
        }
    }

    pub fn new_generator(
        shape_id: ShapeId,
        prototype: Option<JsValue>,
        generator: JsGenerator,
    ) -> Self {
        Self {
            shape_id,
            prototype,
            slots: Vec::new(),
            kind: ObjectKind::Generator(generator),
        }
    }

    pub fn new_worker(
        shape_id: ShapeId,
        prototype: Option<JsValue>,
        port: Arc<std::sync::Mutex<doms_parallel::MessagePort>>,
    ) -> Self {
        Self {
            shape_id,
            prototype,
            slots: Vec::new(),
            kind: ObjectKind::Worker(port),
        }
    }

    pub fn new_map(shape_id: ShapeId, prototype: Option<JsValue>) -> Self {
        Self {
            shape_id,
            prototype,
            slots: Vec::new(),
            kind: ObjectKind::Map(Vec::new()),
        }
    }

    pub fn new_set(shape_id: ShapeId, prototype: Option<JsValue>) -> Self {
        Self {
            shape_id,
            prototype,
            slots: Vec::new(),
            kind: ObjectKind::Set(Vec::new()),
        }
    }

    pub fn new_weak_ref(shape_id: ShapeId, prototype: Option<JsValue>, target: JsValue) -> Self {
        Self {
            shape_id,
            prototype,
            slots: Vec::new(),
            kind: ObjectKind::WeakRef(target),
        }
    }

    pub fn new_finalization_registry(shape_id: ShapeId, prototype: Option<JsValue>, callback: JsValue) -> Self {
        Self {
            shape_id,
            prototype,
            slots: Vec::new(),
            kind: ObjectKind::FinalizationRegistry { callback, entries: Vec::new() },
        }
    }

    pub fn new_regexp(shape_id: ShapeId, prototype: Option<JsValue>, pattern: String, flags: String) -> Self {
        Self {
            shape_id,
            prototype,
            slots: Vec::new(),
            kind: ObjectKind::RegExp { pattern, flags },
        }
    }

    pub fn new_date(shape_id: ShapeId, prototype: Option<JsValue>, time_ms: f64) -> Self {
        Self {
            shape_id,
            prototype,
            slots: Vec::new(),
            kind: ObjectKind::Date(time_ms),
        }
    }

    pub fn new_symbol(shape_id: ShapeId, prototype: Option<JsValue>, desc: Option<String>) -> Self {
        Self {
            shape_id,
            prototype,
            slots: Vec::new(),
            kind: ObjectKind::Symbol(desc),
        }
    }

    pub fn new_bigint(shape_id: ShapeId, prototype: Option<JsValue>, val: i64) -> Self {
        Self {
            shape_id,
            prototype,
            slots: Vec::new(),
            kind: ObjectKind::BigInt(val),
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
            ObjectKind::Promise(p) => {
                if let Some(r) = p.result.as_heap_ptr() {
                    tracer.trace_ref(r);
                }
                for cb in &p.fulfill_reactions {
                    if let Some(r) = cb.as_heap_ptr() {
                        tracer.trace_ref(r);
                    }
                }
                for cb in &p.reject_reactions {
                    if let Some(r) = cb.as_heap_ptr() {
                        tracer.trace_ref(r);
                    }
                }
            }
            ObjectKind::Generator(g) => {
                for v in &g.stack {
                    if let Some(r) = v.as_heap_ptr() {
                        tracer.trace_ref(r);
                    }
                }
            }
            ObjectKind::Map(entries) => {
                for (k, v) in entries {
                    if let Some(p) = k.as_heap_ptr() {
                        tracer.trace_ref(p);
                    }
                    if let Some(p) = v.as_heap_ptr() {
                        tracer.trace_ref(p);
                    }
                }
            }
            ObjectKind::Set(elements) => {
                for el in elements {
                    if let Some(p) = el.as_heap_ptr() {
                        tracer.trace_ref(p);
                    }
                }
            }
            ObjectKind::FinalizationRegistry { callback, entries } => {
                if let Some(p) = callback.as_heap_ptr() {
                    tracer.trace_ref(p);
                }
                for (_, held, tok) in entries {
                    if let Some(p) = held.as_heap_ptr() {
                        tracer.trace_ref(p);
                    }
                    if let Some(t) = tok {
                        if let Some(p) = t.as_heap_ptr() {
                            tracer.trace_ref(p);
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

/// Compare two JavaScript values according to SameValueZero semantics.
pub fn js_values_equal(a: JsValue, b: JsValue) -> bool {
    if a == b {
        return true;
    }
    if a.is_number() && b.is_number() {
        let na = a.to_number().unwrap();
        let nb = b.to_number().unwrap();
        if na.is_nan() && nb.is_nan() {
            return true;
        }
        return na == nb;
    }
    if let (Some(ptr_a), Some(ptr_b)) = (a.as_heap_ptr(), b.as_heap_ptr()) {
        let obj_a = unsafe { &*(ptr_a as *const JsObject) };
        let obj_b = unsafe { &*(ptr_b as *const JsObject) };
        if let (ObjectKind::String(sa), ObjectKind::String(sb)) = (&obj_a.kind, &obj_b.kind) {
            return sa == sb;
        }
    }
    false
}
