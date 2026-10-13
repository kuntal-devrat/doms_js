//! Object model, prototypes, functions, and arrays.

use crate::shape::ShapeId;
use crate::value::JsValue;
use doms_compiler::bytecode::BytecodeChunk;
use doms_gc::trace::{Trace, Tracer};
use num_bigint::BigInt;
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
    pub this_val: JsValue,
    pub arguments: Option<JsValue>,
    pub mapped_arguments: Option<Vec<bool>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypedArrayKind {
    Int8,
    Uint8,
    Uint8Clamped,
    Int16,
    Uint16,
    Int32,
    Uint32,
    Float32,
    Float64,
    BigInt64,
    BigUint64,
}

impl TypedArrayKind {
    pub fn bytes_per_element(&self) -> usize {
        match self {
            Self::Int8 | Self::Uint8 | Self::Uint8Clamped => 1,
            Self::Int16 | Self::Uint16 => 2,
            Self::Int32 | Self::Uint32 | Self::Float32 => 4,
            Self::Float64 | Self::BigInt64 | Self::BigUint64 => 8,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Int8 => "Int8Array",
            Self::Uint8 => "Uint8Array",
            Self::Uint8Clamped => "Uint8ClampedArray",
            Self::Int16 => "Int16Array",
            Self::Uint16 => "Uint16Array",
            Self::Int32 => "Int32Array",
            Self::Uint32 => "Uint32Array",
            Self::Float32 => "Float32Array",
            Self::Float64 => "Float64Array",
            Self::BigInt64 => "BigInt64Array",
            Self::BigUint64 => "BigUint64Array",
        }
    }
}

#[derive(Debug, Clone)]
pub struct JsTypedArray {
    pub buffer: Arc<doms_parallel::SharedArrayBuffer>,
    pub kind: TypedArrayKind,
    pub length: usize,
    pub byte_offset: usize,
}

#[derive(Debug, Clone)]
pub struct JsFunction {
    pub name: Option<String>,
    pub params: Vec<String>,
    pub local_names: Vec<String>,
    pub lexical_local_names: Vec<String>,
    pub chunk: Arc<BytecodeChunk>,
    pub upvalues: Vec<JsValue>,
    pub is_async: bool,
    pub is_generator: bool,
    pub call_count: u32,
    pub jit_entry: Option<crate::jit::JitFunction>,
    pub is_strict: bool,
    pub has_param_exprs: bool,
    pub is_method: bool,
    pub is_arrow: bool,
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
    ArrayBuffer(Arc<doms_parallel::SharedArrayBuffer>),
    SharedArrayBuffer(Arc<doms_parallel::SharedArrayBuffer>),
    DataView {
        buffer: Arc<doms_parallel::SharedArrayBuffer>,
        byte_offset: usize,
        byte_length: usize,
    },
    TypedArray(JsTypedArray),
    Worker(Arc<std::sync::Mutex<doms_parallel::MessagePort>>),
    Map(Vec<(JsValue, JsValue)>),
    Set(Vec<JsValue>),
    WeakMap(Vec<(JsValue, JsValue)>),
    WeakSet(Vec<JsValue>),
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
    BigInt(BigInt),
    Proxy {
        target: JsValue,
        handler: JsValue,
    },
    StringObject(String),
    NumberObject(f64),
    BooleanObject(bool),
    BigIntObject(BigInt),
}

#[derive(Debug, Clone, Default)]
pub struct Accessor {
    pub getter: Option<JsValue>,
    pub setter: Option<JsValue>,
}

#[derive(Debug, Clone)]
pub struct JsObject {
    pub shape_id: ShapeId,
    pub prototype: Option<JsValue>,
    pub slots: Vec<JsValue>,
    pub kind: ObjectKind,
    pub accessors: Option<Box<std::collections::HashMap<String, Accessor>>>,
    pub symbol_properties: Option<Box<std::collections::HashMap<JsValue, JsValue>>>,
    pub symbol_accessors: Option<Box<std::collections::HashMap<JsValue, Accessor>>>,
    pub symbol_order: Option<Vec<JsValue>>,
}

impl JsObject {
    pub fn new_with_kind(shape_id: ShapeId, prototype: Option<JsValue>, kind: ObjectKind) -> Self {
        Self {
            shape_id,
            prototype,
            slots: Vec::new(),
            kind,
            accessors: None,
            symbol_properties: None,
            symbol_accessors: None,
            symbol_order: None,
        }
    }

    pub fn insert_symbol_property(&mut self, sym: JsValue, val: JsValue) {
        let syms = self.symbol_properties.get_or_insert_with(|| Box::new(std::collections::HashMap::new()));
        syms.insert(sym, val);
        let order = self.symbol_order.get_or_insert_with(Vec::new);
        if !order.contains(&sym) {
            order.push(sym);
        }
    }

    pub fn insert_symbol_accessor_getter(&mut self, sym: JsValue, getter: JsValue) {
        let accs = self.symbol_accessors.get_or_insert_with(|| Box::new(std::collections::HashMap::new()));
        accs.entry(sym).or_insert(Accessor { getter: None, setter: None }).getter = Some(getter);
        let order = self.symbol_order.get_or_insert_with(Vec::new);
        if !order.contains(&sym) {
            order.push(sym);
        }
    }

    pub fn insert_symbol_accessor_setter(&mut self, sym: JsValue, setter: JsValue) {
        let accs = self.symbol_accessors.get_or_insert_with(|| Box::new(std::collections::HashMap::new()));
        accs.entry(sym).or_insert(Accessor { getter: None, setter: None }).setter = Some(setter);
        let order = self.symbol_order.get_or_insert_with(Vec::new);
        if !order.contains(&sym) {
            order.push(sym);
        }
    }

    pub fn new_ordinary(shape_id: ShapeId, prototype: Option<JsValue>) -> Self {
        Self::new_with_kind(shape_id, prototype, ObjectKind::Ordinary)
    }

    pub fn new_array(shape_id: ShapeId, prototype: Option<JsValue>, elements: Vec<JsValue>) -> Self {
        Self::new_with_kind(shape_id, prototype, ObjectKind::Array(elements))
    }

    pub fn new_function(
        shape_id: ShapeId,
        prototype: Option<JsValue>,
        func: JsFunction,
    ) -> Self {
        Self::new_with_kind(shape_id, prototype, ObjectKind::Function(func))
    }

    pub fn new_native_function(
        shape_id: ShapeId,
        prototype: Option<JsValue>,
        native: NativeFunction,
    ) -> Self {
        Self::new_with_kind(shape_id, prototype, ObjectKind::NativeFunction(native))
    }

    pub fn new_string(
        shape_id: ShapeId,
        prototype: Option<JsValue>,
        content: String,
    ) -> Self {
        Self::new_with_kind(shape_id, prototype, ObjectKind::String(content))
    }

    pub fn new_proxy(
        shape_id: ShapeId,
        prototype: Option<JsValue>,
        target: JsValue,
        handler: JsValue,
    ) -> Self {
        Self::new_with_kind(shape_id, prototype, ObjectKind::Proxy { target, handler })
    }

    pub fn new_string_object(
        shape_id: ShapeId,
        prototype: Option<JsValue>,
        content: String,
    ) -> Self {
        Self::new_with_kind(shape_id, prototype, ObjectKind::StringObject(content))
    }

    pub fn new_number_object(
        shape_id: ShapeId,
        prototype: Option<JsValue>,
        value: f64,
    ) -> Self {
        Self::new_with_kind(shape_id, prototype, ObjectKind::NumberObject(value))
    }

    pub fn new_boolean_object(
        shape_id: ShapeId,
        prototype: Option<JsValue>,
        value: bool,
    ) -> Self {
        Self::new_with_kind(shape_id, prototype, ObjectKind::BooleanObject(value))
    }

    pub fn new_error(
        shape_id: ShapeId,
        prototype: Option<JsValue>,
        name: String,
        message: String,
    ) -> Self {
        Self::new_with_kind(shape_id, prototype, ObjectKind::Error { name, message })
    }

    pub fn new_promise(
        shape_id: ShapeId,
        prototype: Option<JsValue>,
        state: PromiseState,
        result: JsValue,
    ) -> Self {
        Self::new_with_kind(shape_id, prototype, ObjectKind::Promise(JsPromise {
            state,
            result,
            fulfill_reactions: Vec::new(),
            reject_reactions: Vec::new(),
        }))
    }

    pub fn new_array_buffer(
        shape_id: ShapeId,
        prototype: Option<JsValue>,
        ab: Arc<doms_parallel::SharedArrayBuffer>,
    ) -> Self {
        Self::new_with_kind(shape_id, prototype, ObjectKind::ArrayBuffer(ab))
    }

    pub fn new_data_view(
        shape_id: ShapeId,
        prototype: Option<JsValue>,
        buffer: Arc<doms_parallel::SharedArrayBuffer>,
        byte_offset: usize,
        byte_length: usize,
    ) -> Self {
        Self::new_with_kind(shape_id, prototype, ObjectKind::DataView { buffer, byte_offset, byte_length })
    }

    pub fn new_shared_array_buffer(
        shape_id: ShapeId,
        prototype: Option<JsValue>,
        sab: Arc<doms_parallel::SharedArrayBuffer>,
    ) -> Self {
        Self::new_with_kind(shape_id, prototype, ObjectKind::SharedArrayBuffer(sab))
    }

    pub fn new_typed_array(
        shape_id: ShapeId,
        prototype: Option<JsValue>,
        typed_array: JsTypedArray,
    ) -> Self {
        Self::new_with_kind(shape_id, prototype, ObjectKind::TypedArray(typed_array))
    }

    pub fn new_generator(
        shape_id: ShapeId,
        prototype: Option<JsValue>,
        generator: JsGenerator,
    ) -> Self {
        Self::new_with_kind(shape_id, prototype, ObjectKind::Generator(generator))
    }

    pub fn new_worker(
        shape_id: ShapeId,
        prototype: Option<JsValue>,
        port: Arc<std::sync::Mutex<doms_parallel::MessagePort>>,
    ) -> Self {
        Self::new_with_kind(shape_id, prototype, ObjectKind::Worker(port))
    }

    pub fn new_map(shape_id: ShapeId, prototype: Option<JsValue>) -> Self {
        Self::new_with_kind(shape_id, prototype, ObjectKind::Map(Vec::new()))
    }

    pub fn new_set(shape_id: ShapeId, prototype: Option<JsValue>) -> Self {
        Self::new_with_kind(shape_id, prototype, ObjectKind::Set(Vec::new()))
    }

    pub fn new_weak_map(shape_id: ShapeId, prototype: Option<JsValue>) -> Self {
        Self::new_with_kind(shape_id, prototype, ObjectKind::WeakMap(Vec::new()))
    }

    pub fn new_weak_set(shape_id: ShapeId, prototype: Option<JsValue>) -> Self {
        Self::new_with_kind(shape_id, prototype, ObjectKind::WeakSet(Vec::new()))
    }

    pub fn new_weak_ref(shape_id: ShapeId, prototype: Option<JsValue>, target: JsValue) -> Self {
        Self::new_with_kind(shape_id, prototype, ObjectKind::WeakRef(target))
    }

    pub fn new_finalization_registry(shape_id: ShapeId, prototype: Option<JsValue>, callback: JsValue) -> Self {
        Self::new_with_kind(shape_id, prototype, ObjectKind::FinalizationRegistry { callback, entries: Vec::new() })
    }

    pub fn new_regexp(shape_id: ShapeId, prototype: Option<JsValue>, pattern: String, flags: String) -> Self {
        Self::new_with_kind(shape_id, prototype, ObjectKind::RegExp { pattern, flags })
    }

    pub fn new_date(shape_id: ShapeId, prototype: Option<JsValue>, time_ms: f64) -> Self {
        Self::new_with_kind(shape_id, prototype, ObjectKind::Date(time_ms))
    }

    pub fn new_symbol(shape_id: ShapeId, prototype: Option<JsValue>, desc: Option<String>) -> Self {
        Self::new_with_kind(shape_id, prototype, ObjectKind::Symbol(desc))
    }

    pub fn new_bigint(shape_id: ShapeId, prototype: Option<JsValue>, val: BigInt) -> Self {
        Self::new_with_kind(shape_id, prototype, ObjectKind::BigInt(val))
    }

    pub fn define_getter(&mut self, key: String, getter: JsValue) {
        let accs = self.accessors.get_or_insert_with(|| Box::new(std::collections::HashMap::new()));
        let acc = accs.entry(key).or_default();
        acc.getter = Some(getter);
    }

    pub fn define_setter(&mut self, key: String, setter: JsValue) {
        let accs = self.accessors.get_or_insert_with(|| Box::new(std::collections::HashMap::new()));
        let acc = accs.entry(key).or_default();
        acc.setter = Some(setter);
    }

    pub fn define_accessor(&mut self, key: String, getter: Option<JsValue>, setter: Option<JsValue>) {
        let accs = self.accessors.get_or_insert_with(|| Box::new(std::collections::HashMap::new()));
        let acc = accs.entry(key).or_default();
        if getter.is_some() {
            acc.getter = getter;
        }
        if setter.is_some() {
            acc.setter = setter;
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
        if let Some(accs) = &self.accessors {
            for acc in accs.values() {
                if let Some(g) = acc.getter {
                    if let Some(p) = g.as_heap_ptr() {
                        tracer.trace_ref(p);
                    }
                }
                if let Some(s) = acc.setter {
                    if let Some(p) = s.as_heap_ptr() {
                        tracer.trace_ref(p);
                    }
                }
            }
        }
        if let Some(syms) = &self.symbol_properties {
            for (k, v) in syms.iter() {
                if let Some(p) = k.as_heap_ptr() {
                    tracer.trace_ref(p);
                }
                if let Some(p) = v.as_heap_ptr() {
                    tracer.trace_ref(p);
                }
            }
        }
        if let Some(sym_accs) = &self.symbol_accessors {
            for (k, acc) in sym_accs.iter() {
                if let Some(p) = k.as_heap_ptr() {
                    tracer.trace_ref(p);
                }
                if let Some(g) = acc.getter {
                    if let Some(p) = g.as_heap_ptr() {
                        tracer.trace_ref(p);
                    }
                }
                if let Some(s) = acc.setter {
                    if let Some(p) = s.as_heap_ptr() {
                        tracer.trace_ref(p);
                    }
                }
            }
        }
        if let Some(order) = &self.symbol_order {
            for k in order {
                if let Some(p) = k.as_heap_ptr() {
                    tracer.trace_ref(p);
                }
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
            ObjectKind::WeakMap(entries) => {
                for (k, v) in entries {
                    if let Some(p) = k.as_heap_ptr() {
                        tracer.trace_ref(p);
                    }
                    if let Some(p) = v.as_heap_ptr() {
                        tracer.trace_ref(p);
                    }
                }
            }
            ObjectKind::WeakSet(elements) => {
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
            ObjectKind::Proxy { target, handler } => {
                if let Some(p) = target.as_heap_ptr() {
                    tracer.trace_ref(p);
                }
                if let Some(p) = handler.as_heap_ptr() {
                    tracer.trace_ref(p);
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
        if let (ObjectKind::BigInt(ba), ObjectKind::BigInt(bb)) = (&obj_a.kind, &obj_b.kind) {
            return ba == bb;
        }
    }
    false
}
