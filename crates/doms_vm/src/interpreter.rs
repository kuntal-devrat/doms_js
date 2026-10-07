//! Bytecode interpreter virtual machine with NaN-boxed values, CallFrames, and Shapes.

use crate::builtins::register_builtins;
use crate::inline_cache::InlineCacheSlot;
use crate::object::{
    JsFunction, JsGenerator, JsObject, JsTypedArray, NativeFn, NativeFunction, ObjectKind,
    PromiseState, TypedArrayKind,
};
use crate::shape::ShapeRegistry;
use crate::value::JsValue;
use doms_compiler::bytecode::{BytecodeChunk, Constant, OpCode};
use doms_gc::heap::Heap;
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::fmt;
use std::sync::Arc;
use std::time::{Duration, Instant};
use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum VmError {
    #[error("Stack underflow")]
    StackUnderflow,
    #[error("Undefined global variable: {0}")]
    UndefinedGlobal(String),
    #[error("Syntax error: {0}")]
    SyntaxError(String),
    #[error("Reference error: {0}")]
    ReferenceError(String),
    #[error("Type error: {0}")]
    TypeError(String),
    #[error("Runtime error: {0}")]
    RuntimeError(String),
    #[error("Invalid instruction at ip: {0}")]
    InvalidInstruction(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreferredType {
    Default,
    Number,
    String,
}

/// Call frame on the execution stack.
#[derive(Debug, Clone)]
pub struct CallFrame {
    pub chunk: Arc<BytecodeChunk>,
    pub ip: usize,
    pub base_sp: usize,
    pub this_val: JsValue,
    pub gen_ptr: Option<*mut JsObject>,
}

/// Compatibility enum for earlier Phase 0 API callers.
#[derive(Debug, Clone, PartialEq)]
pub enum VmValue {
    Undefined,
    Null,
    Boolean(bool),
    Number(f64),
    String(String),
}

impl fmt::Display for VmValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VmValue::Undefined => write!(f, "undefined"),
            VmValue::Null => write!(f, "null"),
            VmValue::Boolean(b) => write!(f, "{}", b),
            VmValue::Number(n) => {
                if n.fract() == 0.0 && n.abs() < 1e16 {
                    write!(f, "{}", *n as i64)
                } else {
                    write!(f, "{}", n)
                }
            }
            VmValue::String(s) => write!(f, "{}", s),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CatchHandler {
    pub frame_idx: usize,
    pub target_ip: usize,
    pub stack_len: usize,
}

#[derive(Clone)]
pub struct TimerEntry {
    pub id: u32,
    pub callback: JsValue,
    pub args: Vec<JsValue>,
    pub due_time: Instant,
    pub interval: Option<Duration>,
}

pub struct Vm {
    pub stack: Vec<JsValue>,
    pub frames: Vec<CallFrame>,
    pub globals: HashMap<String, JsValue>,
    pub heap: Heap,
    pub shapes: ShapeRegistry,
    pub inline_caches: Vec<InlineCacheSlot>,
    pub objects: Vec<*mut JsObject>,
    pub catch_stack: Vec<CatchHandler>,
    pub microtasks: VecDeque<(JsValue, Vec<JsValue>)>,
    pub modules: HashMap<String, JsValue>,
    pub current_exports: HashMap<String, JsValue>,
    pub timers: BTreeMap<u32, TimerEntry>,
    pub next_timer_id: u32,
    pub instruction_count: usize,
    pub max_instructions: Option<usize>,
}

impl Vm {
    pub fn new() -> Self {
        let mut vm = Self {
            stack: Vec::with_capacity(256),
            frames: Vec::with_capacity(32),
            globals: HashMap::new(),
            heap: Heap::with_defaults(),
            shapes: ShapeRegistry::new(),
            inline_caches: Vec::new(),
            objects: Vec::with_capacity(128),
            catch_stack: Vec::new(),
            microtasks: VecDeque::new(),
            modules: HashMap::new(),
            current_exports: HashMap::new(),
            timers: BTreeMap::new(),
            next_timer_id: 1,
            instruction_count: 0,
            max_instructions: None,
        };
        register_builtins(&mut vm);
        vm
    }

    pub fn push(&mut self, val: JsValue) {
        self.stack.push(val);
    }

    pub fn pop(&mut self) -> Result<JsValue, VmError> {
        self.stack.pop().ok_or(VmError::StackUnderflow)
    }

    pub fn peek(&self) -> Result<&JsValue, VmError> {
        self.stack.last().ok_or(VmError::StackUnderflow)
    }

    // ── Allocation Helpers ──

    pub fn alloc_object(&mut self) -> JsValue {
        let obj = Box::new(JsObject::new_ordinary(ShapeRegistry::ROOT_SHAPE_ID, None));
        let ptr = Box::into_raw(obj);
        self.objects.push(ptr);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }

    pub fn alloc_array(&mut self, elements: Vec<JsValue>) -> JsValue {
        let obj = Box::new(JsObject::new_array(ShapeRegistry::ROOT_SHAPE_ID, None, elements));
        let ptr = Box::into_raw(obj);
        self.objects.push(ptr);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }

    pub fn alloc_string(&mut self, s: String) -> JsValue {
        let obj = Box::new(JsObject::new_string(ShapeRegistry::ROOT_SHAPE_ID, None, s));
        let ptr = Box::into_raw(obj);
        self.objects.push(ptr);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }

    pub fn alloc_native_function(&mut self, name: &str, length: u8, func: NativeFn) -> JsValue {
        let native = NativeFunction::new_fn(name.to_string(), length, func);
        let obj = Box::new(JsObject::new_native_function(ShapeRegistry::ROOT_SHAPE_ID, None, native));
        let ptr = Box::into_raw(obj);
        self.objects.push(ptr);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }

    pub fn alloc_closure_native_function(&mut self, name: &str, length: u8, func: crate::object::ClosureNativeFn) -> JsValue {
        let native = NativeFunction::new_closure(name.to_string(), length, func);
        let obj = Box::new(JsObject::new_native_function(ShapeRegistry::ROOT_SHAPE_ID, None, native));
        let ptr = Box::into_raw(obj);
        self.objects.push(ptr);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }

    pub fn alloc_function(&mut self, func: JsFunction) -> JsValue {
        let obj = Box::new(JsObject::new_function(ShapeRegistry::ROOT_SHAPE_ID, None, func));
        let ptr = Box::into_raw(obj);
        self.objects.push(ptr);
        let fn_val = unsafe { JsValue::from_object_ptr(ptr as *mut u8) };
        let proto = self.alloc_object();
        self.set_property(proto, "constructor", fn_val);
        self.set_property(fn_val, "prototype", proto);
        fn_val
    }

    pub fn alloc_promise(&mut self, state: PromiseState, result: JsValue) -> JsValue {
        let obj = Box::new(JsObject::new_promise(ShapeRegistry::ROOT_SHAPE_ID, None, state, result));
        let ptr = Box::into_raw(obj);
        self.objects.push(ptr);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }

    pub fn alloc_generator(&mut self, generator: JsGenerator) -> JsValue {
        let obj = Box::new(JsObject::new_generator(ShapeRegistry::ROOT_SHAPE_ID, None, generator));
        let ptr = Box::into_raw(obj);
        self.objects.push(ptr);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }

    pub fn alloc_shared_array_buffer(&mut self, sab: Arc<doms_parallel::SharedArrayBuffer>) -> JsValue {
        let obj = Box::new(JsObject::new_shared_array_buffer(ShapeRegistry::ROOT_SHAPE_ID, None, sab));
        let ptr = Box::into_raw(obj);
        self.objects.push(ptr);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }

    pub fn alloc_typed_array(&mut self, typed_array: JsTypedArray) -> JsValue {
        let obj = Box::new(JsObject::new_typed_array(ShapeRegistry::ROOT_SHAPE_ID, None, typed_array));
        let ptr = Box::into_raw(obj);
        self.objects.push(ptr);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }

    pub fn alloc_worker(&mut self, port: Arc<std::sync::Mutex<doms_parallel::MessagePort>>) -> JsValue {
        let obj = Box::new(JsObject::new_worker(ShapeRegistry::ROOT_SHAPE_ID, None, port));
        let ptr = Box::into_raw(obj);
        self.objects.push(ptr);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }

    pub fn alloc_map(&mut self) -> JsValue {
        let obj = Box::new(JsObject::new_map(ShapeRegistry::ROOT_SHAPE_ID, None));
        let ptr = Box::into_raw(obj);
        self.objects.push(ptr);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }

    pub fn alloc_set(&mut self) -> JsValue {
        let obj = Box::new(JsObject::new_set(ShapeRegistry::ROOT_SHAPE_ID, None));
        let ptr = Box::into_raw(obj);
        self.objects.push(ptr);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }

    pub fn alloc_weak_ref(&mut self, target: JsValue) -> JsValue {
        let obj = Box::new(JsObject::new_weak_ref(ShapeRegistry::ROOT_SHAPE_ID, None, target));
        let ptr = Box::into_raw(obj);
        self.objects.push(ptr);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }

    pub fn alloc_finalization_registry(&mut self, callback: JsValue) -> JsValue {
        let obj = Box::new(JsObject::new_finalization_registry(ShapeRegistry::ROOT_SHAPE_ID, None, callback));
        let ptr = Box::into_raw(obj);
        self.objects.push(ptr);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }

    pub fn alloc_regexp(&mut self, pattern: String, flags: String) -> JsValue {
        let obj = Box::new(JsObject::new_regexp(ShapeRegistry::ROOT_SHAPE_ID, None, pattern, flags));
        let ptr = Box::into_raw(obj);
        self.objects.push(ptr);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }

    pub fn alloc_date(&mut self, proto: Option<JsValue>, time_ms: f64) -> JsValue {
        let obj = Box::new(JsObject::new_date(ShapeRegistry::ROOT_SHAPE_ID, proto, time_ms));
        let ptr = Box::into_raw(obj);
        self.objects.push(ptr);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }

    pub fn alloc_symbol(&mut self, desc: Option<String>) -> JsValue {
        let obj = Box::new(JsObject::new_symbol(ShapeRegistry::ROOT_SHAPE_ID, None, desc));
        let ptr = Box::into_raw(obj);
        self.objects.push(ptr);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }

    pub fn alloc_bigint(&mut self, val: i64) -> JsValue {
        let obj = Box::new(JsObject::new_bigint(ShapeRegistry::ROOT_SHAPE_ID, None, val));
        let ptr = Box::into_raw(obj);
        self.objects.push(ptr);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }

    pub fn alloc_error(&mut self, name: String, message: String) -> JsValue {
        let ctor = self.globals.get(&name).copied().unwrap_or(JsValue::undefined());
        let proto = if ctor.is_object() {
            let p = self.get_property(ctor, "prototype");
            if p.is_object() { Some(p) } else { None }
        } else {
            None
        };
        let obj = Box::new(JsObject::new_error(ShapeRegistry::ROOT_SHAPE_ID, proto, name.clone(), message.clone()));
        let ptr = Box::into_raw(obj);
        self.objects.push(ptr);
        let val = unsafe { JsValue::from_object_ptr(ptr as *mut u8) };
        let name_val = self.alloc_string(name);
        let msg_val = self.alloc_string(message);
        self.set_property(val, "name", name_val);
        self.set_property(val, "message", msg_val);
        if ctor.is_object() {
            self.set_property(val, "constructor", ctor);
        }
        val
    }

    pub fn get_element(&mut self, obj: JsValue, index: usize) -> JsValue {
        if let Some(ptr) = obj.as_object_ptr() {
            let jsobj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::Array(elements) = &jsobj.kind {
                if index < elements.len() {
                    return elements[index];
                }
            }
        }
        let key = index.to_string();
        self.get_property(obj, &key)
    }

    pub fn enqueue_microtask(&mut self, callee: JsValue, args: Vec<JsValue>) {
        self.microtasks.push_back((callee, args));
    }

    pub fn drain_microtasks(&mut self) -> Result<(), VmError> {
        let mut count = 0;
        while let Some((callee, args)) = self.microtasks.pop_front() {
            count += 1;
            if count > 1000 {
                return Err(VmError::RuntimeError("Microtask queue limit exceeded".into()));
            }
            let _ = self.call(callee, JsValue::undefined(), &args);
        }
        Ok(())
    }

    pub fn step_event_loop(&mut self) -> Result<bool, VmError> {
        self.drain_microtasks()?;
        let now = Instant::now();
        let mut due_id = None;
        for (&id, entry) in &self.timers {
            if entry.due_time <= now {
                due_id = Some(id);
                break;
            }
        }
        if let Some(id) = due_id {
            if let Some(mut entry) = self.timers.remove(&id) {
                let _ = self.call(entry.callback, JsValue::undefined(), &entry.args);
                if let Some(interval) = entry.interval {
                    entry.due_time = Instant::now() + interval;
                    self.timers.insert(id, entry);
                }
                self.drain_microtasks()?;
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub fn run_event_loop(&mut self) -> Result<(), VmError> {
        let max_ticks = 100_000;
        let mut ticks = 0;
        while !self.timers.is_empty() || !self.microtasks.is_empty() {
            ticks += 1;
            if ticks > max_ticks {
                return Err(VmError::RuntimeError("Event loop max ticks exceeded".into()));
            }
            let worked = self.step_event_loop()?;
            if !worked && !self.timers.is_empty() {
                let next_due = self.timers.values().map(|t| t.due_time).min();
                if let Some(due) = next_due {
                    let now = Instant::now();
                    if due > now {
                        let wait = (due - now).min(Duration::from_millis(5));
                        std::thread::sleep(wait);
                    }
                }
            }
        }
        Ok(())
    }

    pub fn resolve_promise(&mut self, promise_val: JsValue, resolution: JsValue) {
        if let Some(ptr) = resolution.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::Promise(_) = &obj.kind {
                let then_fn = self.get_property(resolution, "then");
                if then_fn.is_object() {
                    let p_resolve = promise_val;
                    let fulfill_cb = self.alloc_closure_native_function("chain_resolve", 1, Arc::new(move |vm, _, args| {
                        let inner_val = args.first().copied().unwrap_or(JsValue::undefined());
                        vm.resolve_promise(p_resolve, inner_val);
                        Ok(JsValue::undefined())
                    }));
                    let p_reject = promise_val;
                    let reject_cb = self.alloc_closure_native_function("chain_reject", 1, Arc::new(move |vm, _, args| {
                        let inner_err = args.first().copied().unwrap_or(JsValue::undefined());
                        vm.reject_promise(p_reject, inner_err);
                        Ok(JsValue::undefined())
                    }));
                    let _ = self.call(then_fn, resolution, &[fulfill_cb, reject_cb]);
                    return;
                }
            }
        }

        if let Some(ptr) = promise_val.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::Promise(ref mut p) = obj.kind {
                if p.state == PromiseState::Pending {
                    p.state = PromiseState::Fulfilled;
                    p.result = resolution;
                    let reactions = std::mem::take(&mut p.fulfill_reactions);
                    p.reject_reactions.clear();
                    for cb in reactions {
                        self.enqueue_microtask(cb, vec![resolution]);
                    }
                }
            }
        }
    }

    pub fn reject_promise(&mut self, promise_val: JsValue, reason: JsValue) {
        if let Some(ptr) = promise_val.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::Promise(ref mut p) = obj.kind {
                if p.state == PromiseState::Pending {
                    p.state = PromiseState::Rejected;
                    p.result = reason;
                    let reactions = std::mem::take(&mut p.reject_reactions);
                    p.fulfill_reactions.clear();
                    for cb in reactions {
                        self.enqueue_microtask(cb, vec![reason]);
                    }
                }
            }
        }
    }

    // ── Property Access Helpers ──

    pub fn set_property(&mut self, target: JsValue, key: &str, value: JsValue) {
        if let Some(ptr) = target.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let Some(offset) = self.shapes.lookup_offset(obj.shape_id, key) {
                obj.set_slot(offset, value);
            } else {
                let (new_shape_id, offset) = self.shapes.transition(obj.shape_id, key);
                obj.shape_id = new_shape_id;
                obj.set_slot(offset, value);
            }
        }
    }

    pub fn get_property(&mut self, target: JsValue, key: &str) -> JsValue {
        if let Some(ptr) = target.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };

            // Special properties on Array
            if let ObjectKind::Array(elements) = &obj.kind {
                if key == "length" {
                    return JsValue::from_int32(elements.len() as i32);
                }
                if let Some(method) = self.get_array_method(key) {
                    return method;
                }
            }

            // Special properties on String
            if let ObjectKind::String(s) = &obj.kind {
                if key == "length" {
                    return JsValue::from_int32(s.len() as i32);
                }
                if let Some(method) = self.get_string_method(key) {
                    return method;
                }
            }

            // Special properties on Promise
            if let ObjectKind::Promise(_) = &obj.kind {
                match key {
                    "then" => return self.get_promise_then(target).unwrap_or_default(),
                    "catch" => return self.get_promise_catch(target).unwrap_or_default(),
                    "finally" => return self.get_promise_finally(target).unwrap_or_default(),
                    _ => {}
                }
            }

            // Special properties on Generator
            if let ObjectKind::Generator(_) = &obj.kind {
                if key == "next" {
                    return self.get_generator_next(target).unwrap_or_default();
                }
            }

            // Special properties on SharedArrayBuffer
            if let ObjectKind::SharedArrayBuffer(sab) = &obj.kind {
                if key == "byteLength" {
                    return JsValue::from_int32(sab.byte_length() as i32);
                }
            }

            // Special properties on TypedArray
            if let ObjectKind::TypedArray(ta) = &obj.kind {
                match key {
                    "length" => return JsValue::from_int32(ta.length as i32),
                    "byteLength" => {
                        let bytes = match ta.kind {
                            TypedArrayKind::Int32 => ta.length * 4,
                            TypedArrayKind::Uint8 => ta.length,
                        };
                        return JsValue::from_int32(bytes as i32);
                    }
                    "buffer" => return self.alloc_shared_array_buffer(Arc::clone(&ta.buffer)),
                    _ => {}
                }
            }

            // Special properties on Worker
            if let ObjectKind::Worker(port) = &obj.kind {
                let port_clone = Arc::clone(port);
                match key {
                    "postMessage" => return self.get_worker_post_message(port_clone).unwrap_or_default(),
                    "receiveMessage" => return self.get_worker_receive_message(port_clone).unwrap_or_default(),
                    "terminate" => return self.alloc_native_function("terminate", 0, |_vm, _this, _args| Ok(JsValue::undefined())),
                    _ => {}
                }
            }

            // Special properties on Map
            if let ObjectKind::Map(entries) = &obj.kind {
                if key == "size" {
                    return JsValue::from_int32(entries.len() as i32);
                }
                if let Some(m) = self.get_map_method(target, key) {
                    return m;
                }
            }

            // Special properties on Set
            if let ObjectKind::Set(elements) = &obj.kind {
                if key == "size" {
                    return JsValue::from_int32(elements.len() as i32);
                }
                if let Some(m) = self.get_set_method(target, key) {
                    return m;
                }
            }

            // Special properties on WeakRef
            if let ObjectKind::WeakRef(target_ref) = &obj.kind {
                let tr = *target_ref;
                if key == "deref" {
                    return self.alloc_closure_native_function("deref", 0, Arc::new(move |_vm, _this, _args| {
                        Ok(tr)
                    }));
                }
            }

            // Special properties on FinalizationRegistry
            if let ObjectKind::FinalizationRegistry { .. } = &obj.kind {
                if let Some(m) = self.get_finalization_registry_method(target, key) {
                    return m;
                }
            }

            // Special properties on RegExp
            if let ObjectKind::RegExp { pattern, flags } = &obj.kind {
                match key {
                    "source" => return self.alloc_string(pattern.clone()),
                    "flags" => return self.alloc_string(flags.clone()),
                    _ => {
                        if let Some(m) = self.get_regexp_method(target, key) {
                            return m;
                        }
                    }
                }
            }

            // Special properties on Error
            if let ObjectKind::Error { name, message } = &obj.kind {
                match key {
                    "name" => return self.alloc_string(name.clone()),
                    "message" => return self.alloc_string(message.clone()),
                    _ => {}
                }
            }

            // Special properties on Function / NativeFunction
            if matches!(obj.kind, ObjectKind::Function(_) | ObjectKind::NativeFunction(_)) {
                match key {
                    "call" => {
                        let fn_target = target;
                        return self.alloc_closure_native_function("call", 1, Arc::new(move |vm, _this, args| {
                            let this_arg = args.first().copied().unwrap_or(JsValue::undefined());
                            let rest_args = if args.len() > 1 { &args[1..] } else { &[] };
                            vm.call(fn_target, this_arg, rest_args)
                        }));
                    }
                    "apply" => {
                        let fn_target = target;
                        return self.alloc_closure_native_function("apply", 2, Arc::new(move |vm, _this, args| {
                            let this_arg = args.first().copied().unwrap_or(JsValue::undefined());
                            let mut call_args = Vec::new();
                            if let Some(arr_val) = args.get(1) {
                                if let Some(arr_ptr) = arr_val.as_object_ptr() {
                                    let arr_obj = unsafe { &*(arr_ptr as *mut JsObject) };
                                    if let ObjectKind::Array(elems) = &arr_obj.kind {
                                        call_args.extend_from_slice(elems);
                                    }
                                }
                            }
                            vm.call(fn_target, this_arg, &call_args)
                        }));
                    }
                    "name" => {
                        let name_str = match &obj.kind {
                            ObjectKind::Function(f) => f.name.as_deref().unwrap_or(""),
                            ObjectKind::NativeFunction(n) => &n.name,
                            _ => "",
                        };
                        return self.alloc_string(name_str.to_string());
                    }
                    "length" => {
                        let len = match &obj.kind {
                            ObjectKind::Function(f) => f.params.len(),
                            ObjectKind::NativeFunction(n) => n.length as usize,
                            _ => 0,
                        };
                        return JsValue::from_int32(len as i32);
                    }
                    _ => {}
                }
            }

            // Shape offset lookup
            if let Some(offset) = self.shapes.lookup_offset(obj.shape_id, key) {
                if let Some(val) = obj.get_slot(offset) {
                    return val;
                }
            }

            // Prototype chain lookup
            let mut curr_proto = obj.prototype;
            while let Some(proto_val) = curr_proto {
                if let Some(proto_ptr) = proto_val.as_object_ptr() {
                    let proto_obj = unsafe { &*(proto_ptr as *mut JsObject) };
                    if let Some(offset) = self.shapes.lookup_offset(proto_obj.shape_id, key) {
                        if let Some(val) = proto_obj.get_slot(offset) {
                            return val;
                        }
                    }
                    curr_proto = proto_obj.prototype;
                } else {
                    break;
                }
            }
        }

        JsValue::undefined()
    }

    fn get_array_method(&mut self, name: &str) -> Option<JsValue> {
        match name {
            "push" => Some(self.alloc_native_function("push", 1, |_vm, this, args| {
                if let Some(ptr) = this.as_object_ptr() {
                    let obj = unsafe { &mut *(ptr as *mut JsObject) };
                    if let ObjectKind::Array(elements) = &mut obj.kind {
                        for arg in args {
                            elements.push(*arg);
                        }
                        return Ok(JsValue::from_int32(elements.len() as i32));
                    }
                }
                Ok(JsValue::undefined())
            })),
            "pop" => Some(self.alloc_native_function("pop", 0, |_vm, this, _args| {
                if let Some(ptr) = this.as_object_ptr() {
                    let obj = unsafe { &mut *(ptr as *mut JsObject) };
                    if let ObjectKind::Array(elements) = &mut obj.kind {
                        return Ok(elements.pop().unwrap_or(JsValue::undefined()));
                    }
                }
                Ok(JsValue::undefined())
            })),
            "shift" => Some(self.alloc_native_function("shift", 0, |_vm, this, _args| {
                if let Some(ptr) = this.as_object_ptr() {
                    let obj = unsafe { &mut *(ptr as *mut JsObject) };
                    if let ObjectKind::Array(elements) = &mut obj.kind {
                        if !elements.is_empty() {
                            return Ok(elements.remove(0));
                        }
                    }
                }
                Ok(JsValue::undefined())
            })),
            "unshift" => Some(self.alloc_native_function("unshift", 1, |_vm, this, args| {
                if let Some(ptr) = this.as_object_ptr() {
                    let obj = unsafe { &mut *(ptr as *mut JsObject) };
                    if let ObjectKind::Array(elements) = &mut obj.kind {
                        for (i, arg) in args.iter().enumerate() {
                            elements.insert(i, *arg);
                        }
                        return Ok(JsValue::from_int32(elements.len() as i32));
                    }
                }
                Ok(JsValue::undefined())
            })),
            "join" => Some(self.alloc_native_function("join", 1, |vm, this, args| {
                let sep = args.first().map(|v| vm.to_string_value(*v)).unwrap_or_else(|| ",".to_string());
                if let Some(ptr) = this.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    if let ObjectKind::Array(elements) = &obj.kind {
                        let strs: Vec<String> = elements.iter().map(|e| vm.to_string_value(*e)).collect();
                        let joined = strs.join(&sep);
                        return Ok(vm.alloc_string(joined));
                    }
                }
                Ok(vm.alloc_string("".to_string()))
            })),
            "slice" => Some(self.alloc_native_function("slice", 2, |vm, this, args| {
                if let Some(ptr) = this.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    if let ObjectKind::Array(elements) = &obj.kind {
                        let len = elements.len() as i32;
                        let start = args.first().and_then(|v| v.to_int32()).unwrap_or(0);
                        let end = args.get(1).and_then(|v| v.to_int32()).unwrap_or(len);
                        let s = start.clamp(0, len) as usize;
                        let e = end.clamp(0, len) as usize;
                        let sliced = if s <= e { elements[s..e].to_vec() } else { Vec::new() };
                        return Ok(vm.alloc_array(sliced));
                    }
                }
                Ok(vm.alloc_array(Vec::new()))
            })),
            "indexOf" => Some(self.alloc_native_function("indexOf", 1, |_vm, this, args| {
                let target = args.first().copied().unwrap_or(JsValue::undefined());
                if let Some(ptr) = this.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    if let ObjectKind::Array(elements) = &obj.kind {
                        for (i, el) in elements.iter().enumerate() {
                            if *el == target {
                                return Ok(JsValue::from_int32(i as i32));
                            }
                        }
                    }
                }
                Ok(JsValue::from_int32(-1))
            })),
            "includes" => Some(self.alloc_native_function("includes", 1, |_vm, this, args| {
                let target = args.first().copied().unwrap_or(JsValue::undefined());
                if let Some(ptr) = this.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    if let ObjectKind::Array(elements) = &obj.kind {
                        return Ok(JsValue::from_boolean(elements.contains(&target)));
                    }
                }
                Ok(JsValue::from_boolean(false))
            })),
            "map" => Some(self.alloc_native_function("map", 1, |vm, this, args| {
                let cb = args.first().copied().unwrap_or(JsValue::undefined());
                let len_val = vm.get_property(this, "length");
                let len = len_val.to_int32().unwrap_or(0).max(0) as usize;
                let mut mapped = Vec::with_capacity(len);
                for i in 0..len {
                    let el = vm.get_element(this, i);
                    let res = vm.call(cb, JsValue::undefined(), &[el, JsValue::from_int32(i as i32), this])?;
                    mapped.push(res);
                }
                Ok(vm.alloc_array(mapped))
            })),
            "forEach" => Some(self.alloc_native_function("forEach", 1, |vm, this, args| {
                let cb = args.first().copied().unwrap_or(JsValue::undefined());
                let len_val = vm.get_property(this, "length");
                let len = len_val.to_int32().unwrap_or(0).max(0) as usize;
                for i in 0..len {
                    let el = vm.get_element(this, i);
                    let _ = vm.call(cb, JsValue::undefined(), &[el, JsValue::from_int32(i as i32), this])?;
                }
                Ok(JsValue::undefined())
            })),
            "filter" => Some(self.alloc_native_function("filter", 1, |vm, this, args| {
                let cb = args.first().copied().unwrap_or(JsValue::undefined());
                let len_val = vm.get_property(this, "length");
                let len = len_val.to_int32().unwrap_or(0).max(0) as usize;
                let mut filtered = Vec::new();
                for i in 0..len {
                    let el = vm.get_element(this, i);
                    let res = vm.call(cb, JsValue::undefined(), &[el, JsValue::from_int32(i as i32), this])?;
                    if res.is_truthy() {
                        filtered.push(el);
                    }
                }
                Ok(vm.alloc_array(filtered))
            })),
            _ => None,
        }
    }

    fn get_string_method(&mut self, name: &str) -> Option<JsValue> {
        match name {
            "indexOf" => Some(self.alloc_native_function("indexOf", 1, |vm, this, args| {
                let target = args.first().map(|v| vm.to_string_value(*v)).unwrap_or_default();
                let this_str = vm.to_string_value(this);
                if let Some(idx) = this_str.find(&target) {
                    Ok(JsValue::from_int32(idx as i32))
                } else {
                    Ok(JsValue::from_int32(-1))
                }
            })),
            "includes" => Some(self.alloc_native_function("includes", 1, |vm, this, args| {
                let target = args.first().map(|v| vm.to_string_value(*v)).unwrap_or_default();
                let this_str = vm.to_string_value(this);
                Ok(JsValue::from_boolean(this_str.contains(&target)))
            })),
            "toLowerCase" => Some(self.alloc_native_function("toLowerCase", 0, |vm, this, _args| {
                let s = vm.to_string_value(this).to_lowercase();
                Ok(vm.alloc_string(s))
            })),
            "toUpperCase" => Some(self.alloc_native_function("toUpperCase", 0, |vm, this, _args| {
                let s = vm.to_string_value(this).to_uppercase();
                Ok(vm.alloc_string(s))
            })),
            "trim" => Some(self.alloc_native_function("trim", 0, |vm, this, _args| {
                let s = vm.to_string_value(this).trim().to_string();
                Ok(vm.alloc_string(s))
            })),
            "charAt" => Some(self.alloc_native_function("charAt", 1, |vm, this, args| {
                let idx = args.first().and_then(|v| v.to_int32()).unwrap_or(0) as usize;
                let s = vm.to_string_value(this);
                if let Some(ch) = s.chars().nth(idx) {
                    Ok(vm.alloc_string(ch.to_string()))
                } else {
                    Ok(vm.alloc_string("".to_string()))
                }
            })),
            "slice" => Some(self.alloc_native_function("slice", 2, |vm, this, args| {
                let s = vm.to_string_value(this);
                let len = s.len() as i32;
                let start = args.first().and_then(|v| v.to_int32()).unwrap_or(0).clamp(0, len) as usize;
                let end = args.get(1).and_then(|v| v.to_int32()).unwrap_or(len).clamp(0, len) as usize;
                let sub = if start <= end { &s[start..end] } else { "" };
                Ok(vm.alloc_string(sub.to_string()))
            })),
            "isWellFormed" => Some(self.alloc_native_function("isWellFormed", 0, |vm, this, _args| {
                let s = vm.to_string_value(this);
                let mut ok = true;
                for c in s.chars() {
                    let u = c as u32;
                    if (0xD800..=0xDFFF).contains(&u) {
                        ok = false;
                        break;
                    }
                }
                Ok(JsValue::from_boolean(ok))
            })),
            "toWellFormed" => Some(self.alloc_native_function("toWellFormed", 0, |vm, this, _args| {
                let s = vm.to_string_value(this);
                let sanitized: String = s.chars().map(|c| {
                    let u = c as u32;
                    if (0xD800..=0xDFFF).contains(&u) {
                        '\u{FFFD}'
                    } else {
                        c
                    }
                }).collect();
                Ok(vm.alloc_string(sanitized))
            })),
            _ => None,
        }
    }

    fn get_promise_then(&mut self, promise_val: JsValue) -> Option<JsValue> {
        Some(self.alloc_closure_native_function("then", 2, Arc::new(move |vm: &mut Vm, _this: JsValue, args: &[JsValue]| -> Result<JsValue, VmError> {
            let on_fulfilled = args.first().copied().unwrap_or_default();
            let on_rejected = args.get(1).copied().unwrap_or_default();
            let next_promise = vm.alloc_promise(PromiseState::Pending, JsValue::undefined());

            if let Some(ptr) = promise_val.as_object_ptr() {
                let obj = unsafe { &mut *(ptr as *mut JsObject) };
                if let ObjectKind::Promise(ref mut p) = obj.kind {
                    match p.state {
                        PromiseState::Fulfilled => {
                            let val = p.result;
                            if on_fulfilled.is_object() {
                                let next_p = next_promise;
                                let cb_task = vm.alloc_closure_native_function("then_cb", 0, Arc::new(move |vm: &mut Vm, _, _| -> Result<JsValue, VmError> {
                                    match vm.call(on_fulfilled, JsValue::undefined(), &[val]) {
                                        Ok(res) => vm.resolve_promise(next_p, res),
                                        Err(e) => {
                                            let err_val = vm.alloc_string(e.to_string());
                                            vm.reject_promise(next_p, err_val);
                                        }
                                    }
                                    Ok(JsValue::undefined())
                                }));
                                vm.enqueue_microtask(cb_task, Vec::new());
                            } else {
                                vm.resolve_promise(next_promise, val);
                            }
                        }
                        PromiseState::Rejected => {
                            let err = p.result;
                            if on_rejected.is_object() {
                                let next_p = next_promise;
                                let cb_task = vm.alloc_closure_native_function("catch_cb", 0, Arc::new(move |vm: &mut Vm, _, _| -> Result<JsValue, VmError> {
                                    match vm.call(on_rejected, JsValue::undefined(), &[err]) {
                                        Ok(res) => vm.resolve_promise(next_p, res),
                                        Err(e) => {
                                            let err_val = vm.alloc_string(e.to_string());
                                            vm.reject_promise(next_p, err_val);
                                        }
                                    }
                                    Ok(JsValue::undefined())
                                }));
                                vm.enqueue_microtask(cb_task, Vec::new());
                            } else {
                                vm.reject_promise(next_promise, err);
                            }
                        }
                        PromiseState::Pending => {
                            let next_p = next_promise;
                            if on_fulfilled.is_object() {
                                let cb_task = vm.alloc_closure_native_function("pending_then", 1, Arc::new(move |vm: &mut Vm, _, args: &[JsValue]| -> Result<JsValue, VmError> {
                                    let val = args.first().copied().unwrap_or_default();
                                    match vm.call(on_fulfilled, JsValue::undefined(), &[val]) {
                                        Ok(res) => vm.resolve_promise(next_p, res),
                                        Err(e) => {
                                            let err_val = vm.alloc_string(e.to_string());
                                            vm.reject_promise(next_p, err_val);
                                        }
                                    }
                                    Ok(JsValue::undefined())
                                }));
                                p.fulfill_reactions.push(cb_task);
                            } else {
                                let cb_task = vm.alloc_closure_native_function("pending_fwd", 1, Arc::new(move |vm: &mut Vm, _, args: &[JsValue]| -> Result<JsValue, VmError> {
                                    let val = args.first().copied().unwrap_or_default();
                                    vm.resolve_promise(next_p, val);
                                    Ok(JsValue::undefined())
                                }));
                                p.fulfill_reactions.push(cb_task);
                            }

                            if on_rejected.is_object() {
                                let cb_task = vm.alloc_closure_native_function("pending_catch", 1, Arc::new(move |vm: &mut Vm, _, args: &[JsValue]| -> Result<JsValue, VmError> {
                                    let err = args.first().copied().unwrap_or_default();
                                    match vm.call(on_rejected, JsValue::undefined(), &[err]) {
                                        Ok(res) => vm.resolve_promise(next_p, res),
                                        Err(e) => {
                                            let err_val = vm.alloc_string(e.to_string());
                                            vm.reject_promise(next_p, err_val);
                                        }
                                    }
                                    Ok(JsValue::undefined())
                                }));
                                p.reject_reactions.push(cb_task);
                            } else {
                                let cb_task = vm.alloc_closure_native_function("pending_rfwd", 1, Arc::new(move |vm: &mut Vm, _, args: &[JsValue]| -> Result<JsValue, VmError> {
                                    let err = args.first().copied().unwrap_or_default();
                                    vm.reject_promise(next_p, err);
                                    Ok(JsValue::undefined())
                                }));
                                p.reject_reactions.push(cb_task);
                            }
                        }
                    }
                }
            }

            Ok(next_promise)
        })))
    }

    fn get_promise_catch(&mut self, promise_val: JsValue) -> Option<JsValue> {
        Some(self.alloc_closure_native_function("catch", 1, Arc::new(move |vm: &mut Vm, _this: JsValue, args: &[JsValue]| -> Result<JsValue, VmError> {
            let on_rejected = args.first().copied().unwrap_or_default();
            let then_fn = vm.get_property(promise_val, "then");
            vm.call(then_fn, promise_val, &[JsValue::undefined(), on_rejected])
        })))
    }

    fn get_promise_finally(&mut self, promise_val: JsValue) -> Option<JsValue> {
        Some(self.alloc_closure_native_function("finally", 1, Arc::new(move |vm: &mut Vm, _this: JsValue, args: &[JsValue]| -> Result<JsValue, VmError> {
            let on_finally = args.first().copied().unwrap_or_default();
            let then_fn = vm.get_property(promise_val, "then");
            vm.call(then_fn, promise_val, &[on_finally, on_finally])
        })))
    }

    fn get_generator_next(&mut self, gen_val: JsValue) -> Option<JsValue> {
        Some(self.alloc_closure_native_function("next", 1, Arc::new(move |vm: &mut Vm, _this: JsValue, args: &[JsValue]| -> Result<JsValue, VmError> {
            let send_val = args.first().copied().unwrap_or_default();
            vm.generator_next(gen_val, send_val)
        })))
    }

    pub fn make_iter_result(&mut self, value: JsValue, done: bool) -> JsValue {
        let obj = self.alloc_object();
        self.set_property(obj, "value", value);
        self.set_property(obj, "done", JsValue::from_boolean(done));
        obj
    }

    pub fn generator_next(&mut self, gen_val: JsValue, send_val: JsValue) -> Result<JsValue, VmError> {
        let ptr = gen_val.as_object_ptr().ok_or(VmError::TypeError("Not a generator".into()))?;
        let (gen_fn, ip, stack, state) = {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            match &obj.kind {
                ObjectKind::Generator(g) => (g.function.clone(), g.ip, g.stack.clone(), g.state),
                _ => return Err(VmError::TypeError("Not a generator".into())),
            }
        };

        if state == crate::object::GeneratorState::Completed {
            return Ok(self.make_iter_result(JsValue::undefined(), true));
        }

        let mut current_stack = stack;
        if state == crate::object::GeneratorState::SuspendedYield {
            current_stack.push(send_val);
        }

        {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::Generator(ref mut g) = obj.kind {
                g.state = crate::object::GeneratorState::Executing;
            }
        }

        let chunk = Arc::clone(&gen_fn.chunk);
        let target_depth = self.frames.len();
        let base_sp = self.stack.len();
        for val in &current_stack {
            self.push(*val);
        }
        self.frames.push(CallFrame {
            chunk,
            ip,
            base_sp,
            this_val: JsValue::undefined(),
            gen_ptr: Some(ptr as *mut JsObject),
        });

        self.run_until_depth(target_depth)
    }

    fn get_worker_post_message(&mut self, port: Arc<std::sync::Mutex<doms_parallel::MessagePort>>) -> Option<JsValue> {
        Some(self.alloc_closure_native_function("postMessage", 1, Arc::new(move |vm: &mut Vm, _this: JsValue, args: &[JsValue]| -> Result<JsValue, VmError> {
            let msg = args.first().copied().unwrap_or_default();
            let json = vm.to_json_string(msg);
            let guard = port.lock().unwrap();
            guard.post_message(json.into_bytes()).map_err(|e| VmError::RuntimeError(e.to_string()))?;
            Ok(JsValue::undefined())
        })))
    }

    fn get_worker_receive_message(&mut self, port: Arc<std::sync::Mutex<doms_parallel::MessagePort>>) -> Option<JsValue> {
        Some(self.alloc_closure_native_function("receiveMessage", 0, Arc::new(move |vm: &mut Vm, _this: JsValue, _args: &[JsValue]| -> Result<JsValue, VmError> {
            let mut data_opt = {
                let guard = port.lock().unwrap();
                guard.try_recv().ok()
            };

            if data_opt.is_none() {
                for _ in 0..50 {
                    std::thread::sleep(std::time::Duration::from_millis(2));
                    let guard = port.lock().unwrap();
                    if let Ok(data) = guard.try_recv() {
                        data_opt = Some(data);
                        break;
                    }
                }
            }

            match data_opt {
                Some(data) => {
                    let s = String::from_utf8_lossy(&data.payload).to_string();
                    if let Ok(n) = s.parse::<f64>() {
                        Ok(JsValue::from_double(n))
                    } else if s == "true" {
                        Ok(JsValue::from_boolean(true))
                    } else if s == "false" {
                        Ok(JsValue::from_boolean(false))
                    } else {
                        let mut clean = s;
                        if clean.starts_with('"') && clean.ends_with('"') && clean.len() >= 2 {
                            clean = clean[1..clean.len()-1].to_string();
                        }
                        Ok(vm.alloc_string(clean))
                    }
                }
                None => Ok(JsValue::null()),
            }
        })))
    }

    fn get_map_method(&mut self, target: JsValue, name: &str) -> Option<JsValue> {
        match name {
            "get" => Some(self.alloc_closure_native_function("get", 1, Arc::new(move |_vm, _this, args| {
                let key = args.first().copied().unwrap_or(JsValue::undefined());
                if let Some(ptr) = target.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    if let ObjectKind::Map(entries) = &obj.kind {
                        for (k, v) in entries {
                            if crate::object::js_values_equal(*k, key) {
                                return Ok(*v);
                            }
                        }
                    }
                }
                Ok(JsValue::undefined())
            }))),
            "set" => Some(self.alloc_closure_native_function("set", 2, Arc::new(move |_vm, _this, args| {
                let key = args.first().copied().unwrap_or(JsValue::undefined());
                let val = args.get(1).copied().unwrap_or(JsValue::undefined());
                if let Some(ptr) = target.as_object_ptr() {
                    let obj = unsafe { &mut *(ptr as *mut JsObject) };
                    if let ObjectKind::Map(entries) = &mut obj.kind {
                        for entry in entries.iter_mut() {
                            if crate::object::js_values_equal(entry.0, key) {
                                entry.1 = val;
                                return Ok(target);
                            }
                        }
                        entries.push((key, val));
                    }
                }
                Ok(target)
            }))),
            "has" => Some(self.alloc_closure_native_function("has", 1, Arc::new(move |_vm, _this, args| {
                let key = args.first().copied().unwrap_or(JsValue::undefined());
                if let Some(ptr) = target.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    if let ObjectKind::Map(entries) = &obj.kind {
                        for (k, _) in entries {
                            if crate::object::js_values_equal(*k, key) {
                                return Ok(JsValue::from_boolean(true));
                            }
                        }
                    }
                }
                Ok(JsValue::from_boolean(false))
            }))),
            "delete" => Some(self.alloc_closure_native_function("delete", 1, Arc::new(move |_vm, _this, args| {
                let key = args.first().copied().unwrap_or(JsValue::undefined());
                if let Some(ptr) = target.as_object_ptr() {
                    let obj = unsafe { &mut *(ptr as *mut JsObject) };
                    if let ObjectKind::Map(entries) = &mut obj.kind {
                        if let Some(idx) = entries.iter().position(|(k, _)| crate::object::js_values_equal(*k, key)) {
                            entries.remove(idx);
                            return Ok(JsValue::from_boolean(true));
                        }
                    }
                }
                Ok(JsValue::from_boolean(false))
            }))),
            "clear" => Some(self.alloc_closure_native_function("clear", 0, Arc::new(move |_vm, _this, _args| {
                if let Some(ptr) = target.as_object_ptr() {
                    let obj = unsafe { &mut *(ptr as *mut JsObject) };
                    if let ObjectKind::Map(entries) = &mut obj.kind {
                        entries.clear();
                    }
                }
                Ok(JsValue::undefined())
            }))),
            "forEach" => Some(self.alloc_closure_native_function("forEach", 1, Arc::new(move |vm, _this, args| {
                let cb = args.first().copied().unwrap_or(JsValue::undefined());
                let entries = if let Some(ptr) = target.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    if let ObjectKind::Map(entries) = &obj.kind {
                        entries.clone()
                    } else {
                        Vec::new()
                    }
                } else {
                    Vec::new()
                };
                for (k, v) in entries {
                    let _ = vm.call(cb, target, &[v, k, target]);
                }
                Ok(JsValue::undefined())
            }))),
            _ => None,
        }
    }

    fn get_set_method(&mut self, target: JsValue, name: &str) -> Option<JsValue> {
        match name {
            "add" => Some(self.alloc_closure_native_function("add", 1, Arc::new(move |_vm, _this, args| {
                let val = args.first().copied().unwrap_or(JsValue::undefined());
                if let Some(ptr) = target.as_object_ptr() {
                    let obj = unsafe { &mut *(ptr as *mut JsObject) };
                    if let ObjectKind::Set(elements) = &mut obj.kind {
                        if !elements.iter().any(|el| crate::object::js_values_equal(*el, val)) {
                            elements.push(val);
                        }
                    }
                }
                Ok(target)
            }))),
            "has" => Some(self.alloc_closure_native_function("has", 1, Arc::new(move |_vm, _this, args| {
                let val = args.first().copied().unwrap_or(JsValue::undefined());
                if let Some(ptr) = target.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    if let ObjectKind::Set(elements) = &obj.kind {
                        return Ok(JsValue::from_boolean(elements.iter().any(|el| crate::object::js_values_equal(*el, val))));
                    }
                }
                Ok(JsValue::from_boolean(false))
            }))),
            "delete" => Some(self.alloc_closure_native_function("delete", 1, Arc::new(move |_vm, _this, args| {
                let val = args.first().copied().unwrap_or(JsValue::undefined());
                if let Some(ptr) = target.as_object_ptr() {
                    let obj = unsafe { &mut *(ptr as *mut JsObject) };
                    if let ObjectKind::Set(elements) = &mut obj.kind {
                        if let Some(idx) = elements.iter().position(|el| crate::object::js_values_equal(*el, val)) {
                            elements.remove(idx);
                            return Ok(JsValue::from_boolean(true));
                        }
                    }
                }
                Ok(JsValue::from_boolean(false))
            }))),
            "clear" => Some(self.alloc_closure_native_function("clear", 0, Arc::new(move |_vm, _this, _args| {
                if let Some(ptr) = target.as_object_ptr() {
                    let obj = unsafe { &mut *(ptr as *mut JsObject) };
                    if let ObjectKind::Set(elements) = &mut obj.kind {
                        elements.clear();
                    }
                }
                Ok(JsValue::undefined())
            }))),
            "forEach" => Some(self.alloc_closure_native_function("forEach", 1, Arc::new(move |vm, _this, args| {
                let cb = args.first().copied().unwrap_or(JsValue::undefined());
                let elements = if let Some(ptr) = target.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    if let ObjectKind::Set(elements) = &obj.kind {
                        elements.clone()
                    } else {
                        Vec::new()
                    }
                } else {
                    Vec::new()
                };
                for el in elements {
                    let _ = vm.call(cb, target, &[el, el, target]);
                }
                Ok(JsValue::undefined())
            }))),
            "union" => Some(self.alloc_closure_native_function("union", 1, Arc::new(move |vm, _this, args| {
                let other = args.first().copied().unwrap_or(JsValue::undefined());
                let new_set = vm.alloc_set();
                if let Some(ptr) = target.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    if let ObjectKind::Set(elements) = &obj.kind {
                        for el in elements {
                            let n_obj = unsafe { &mut *(new_set.as_object_ptr().unwrap() as *mut JsObject) };
                            if let ObjectKind::Set(ref mut set_elements) = n_obj.kind {
                                if !set_elements.iter().any(|existing| crate::object::js_values_equal(*existing, *el)) {
                                    set_elements.push(*el);
                                }
                            }
                        }
                    }
                }
                if let Some(ptr) = other.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    if let ObjectKind::Set(elements) = &obj.kind {
                        for el in elements {
                            let n_obj = unsafe { &mut *(new_set.as_object_ptr().unwrap() as *mut JsObject) };
                            if let ObjectKind::Set(ref mut set_elements) = n_obj.kind {
                                if !set_elements.iter().any(|existing| crate::object::js_values_equal(*existing, *el)) {
                                    set_elements.push(*el);
                                }
                            }
                        }
                    }
                }
                Ok(new_set)
            }))),
            "intersection" => Some(self.alloc_closure_native_function("intersection", 1, Arc::new(move |vm, _this, args| {
                let other = args.first().copied().unwrap_or(JsValue::undefined());
                let new_set = vm.alloc_set();
                if let Some(ptr) = target.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    if let ObjectKind::Set(elements) = &obj.kind {
                        for el in elements {
                            let has_fn = vm.get_property(other, "has");
                            let res = vm.call(has_fn, other, &[*el]).unwrap_or_else(|_| JsValue::from_boolean(false));
                            if res.is_truthy() {
                                let n_obj = unsafe { &mut *(new_set.as_object_ptr().unwrap() as *mut JsObject) };
                                if let ObjectKind::Set(ref mut set_elements) = n_obj.kind {
                                    if !set_elements.iter().any(|existing| crate::object::js_values_equal(*existing, *el)) {
                                        set_elements.push(*el);
                                    }
                                }
                            }
                        }
                    }
                }
                Ok(new_set)
            }))),
            "difference" => Some(self.alloc_closure_native_function("difference", 1, Arc::new(move |vm, _this, args| {
                let other = args.first().copied().unwrap_or(JsValue::undefined());
                let new_set = vm.alloc_set();
                if let Some(ptr) = target.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    if let ObjectKind::Set(elements) = &obj.kind {
                        for el in elements {
                            let has_fn = vm.get_property(other, "has");
                            let res = vm.call(has_fn, other, &[*el]).unwrap_or_else(|_| JsValue::from_boolean(false));
                            if !res.is_truthy() {
                                let n_obj = unsafe { &mut *(new_set.as_object_ptr().unwrap() as *mut JsObject) };
                                if let ObjectKind::Set(ref mut set_elements) = n_obj.kind {
                                    if !set_elements.iter().any(|existing| crate::object::js_values_equal(*existing, *el)) {
                                        set_elements.push(*el);
                                    }
                                }
                            }
                        }
                    }
                }
                Ok(new_set)
            }))),
            "symmetricDifference" => Some(self.alloc_closure_native_function("symmetricDifference", 1, Arc::new(move |vm, _this, args| {
                let other = args.first().copied().unwrap_or(JsValue::undefined());
                let new_set = vm.alloc_set();
                if let Some(ptr) = target.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    if let ObjectKind::Set(elements) = &obj.kind {
                        for el in elements {
                            let has_fn = vm.get_property(other, "has");
                            let res = vm.call(has_fn, other, &[*el]).unwrap_or_else(|_| JsValue::from_boolean(false));
                            if !res.is_truthy() {
                                let n_obj = unsafe { &mut *(new_set.as_object_ptr().unwrap() as *mut JsObject) };
                                if let ObjectKind::Set(ref mut set_elements) = n_obj.kind {
                                    if !set_elements.iter().any(|existing| crate::object::js_values_equal(*existing, *el)) {
                                        set_elements.push(*el);
                                    }
                                }
                            }
                        }
                    }
                }
                if let Some(ptr) = other.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    if let ObjectKind::Set(elements) = &obj.kind {
                        for el in elements {
                            let has_fn = vm.get_property(target, "has");
                            let res = vm.call(has_fn, target, &[*el]).unwrap_or_else(|_| JsValue::from_boolean(false));
                            if !res.is_truthy() {
                                let n_obj = unsafe { &mut *(new_set.as_object_ptr().unwrap() as *mut JsObject) };
                                if let ObjectKind::Set(ref mut set_elements) = n_obj.kind {
                                    if !set_elements.iter().any(|existing| crate::object::js_values_equal(*existing, *el)) {
                                        set_elements.push(*el);
                                    }
                                }
                            }
                        }
                    }
                }
                Ok(new_set)
            }))),
            "isSubsetOf" => Some(self.alloc_closure_native_function("isSubsetOf", 1, Arc::new(move |vm, _this, args| {
                let other = args.first().copied().unwrap_or(JsValue::undefined());
                if let Some(ptr) = target.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    if let ObjectKind::Set(elements) = &obj.kind {
                        for el in elements {
                            let has_fn = vm.get_property(other, "has");
                            let res = vm.call(has_fn, other, &[*el]).unwrap_or_else(|_| JsValue::from_boolean(false));
                            if !res.is_truthy() {
                                return Ok(JsValue::from_boolean(false));
                            }
                        }
                    }
                }
                Ok(JsValue::from_boolean(true))
            }))),
            "isSupersetOf" => Some(self.alloc_closure_native_function("isSupersetOf", 1, Arc::new(move |vm, _this, args| {
                let other = args.first().copied().unwrap_or(JsValue::undefined());
                if let Some(ptr) = other.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    if let ObjectKind::Set(elements) = &obj.kind {
                        for el in elements {
                            let has_fn = vm.get_property(target, "has");
                            let res = vm.call(has_fn, target, &[*el]).unwrap_or_else(|_| JsValue::from_boolean(false));
                            if !res.is_truthy() {
                                return Ok(JsValue::from_boolean(false));
                            }
                        }
                    }
                }
                Ok(JsValue::from_boolean(true))
            }))),
            "isDisjointFrom" => Some(self.alloc_closure_native_function("isDisjointFrom", 1, Arc::new(move |vm, _this, args| {
                let other = args.first().copied().unwrap_or(JsValue::undefined());
                if let Some(ptr) = target.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    if let ObjectKind::Set(elements) = &obj.kind {
                        for el in elements {
                            let has_fn = vm.get_property(other, "has");
                            let res = vm.call(has_fn, other, &[*el]).unwrap_or_else(|_| JsValue::from_boolean(false));
                            if res.is_truthy() {
                                return Ok(JsValue::from_boolean(false));
                            }
                        }
                    }
                }
                Ok(JsValue::from_boolean(true))
            }))),
            _ => None,
        }
    }

    fn get_finalization_registry_method(&mut self, target: JsValue, name: &str) -> Option<JsValue> {
        match name {
            "register" => Some(self.alloc_closure_native_function("register", 3, Arc::new(move |_vm, _this, args| {
                let target_obj = args.first().copied().unwrap_or(JsValue::undefined());
                let held_value = args.get(1).copied().unwrap_or(JsValue::undefined());
                let unreg_token = args.get(2).copied();
                if let Some(ptr) = target.as_object_ptr() {
                    let obj = unsafe { &mut *(ptr as *mut JsObject) };
                    if let ObjectKind::FinalizationRegistry { entries, .. } = &mut obj.kind {
                        entries.push((target_obj, held_value, unreg_token));
                    }
                }
                Ok(JsValue::undefined())
            }))),
            "unregister" => Some(self.alloc_closure_native_function("unregister", 1, Arc::new(move |_vm, _this, args| {
                let token = args.first().copied().unwrap_or(JsValue::undefined());
                let mut removed = false;
                if let Some(ptr) = target.as_object_ptr() {
                    let obj = unsafe { &mut *(ptr as *mut JsObject) };
                    if let ObjectKind::FinalizationRegistry { entries, .. } = &mut obj.kind {
                        let len_before = entries.len();
                        entries.retain(|(_, _, tok)| {
                            if let Some(t) = tok {
                                !crate::object::js_values_equal(*t, token)
                            } else {
                                true
                            }
                        });
                        removed = entries.len() < len_before;
                    }
                }
                Ok(JsValue::from_boolean(removed))
            }))),
            _ => None,
        }
    }

    fn get_regexp_method(&mut self, target: JsValue, name: &str) -> Option<JsValue> {
        match name {
            "test" => Some(self.alloc_closure_native_function("test", 1, Arc::new(move |vm, _this, args| {
                let text = args.first().map(|v| vm.to_string_value(*v)).unwrap_or_default();
                if let Some(ptr) = target.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    if let ObjectKind::RegExp { pattern, .. } = &obj.kind {
                        let matches = text.contains(pattern);
                        return Ok(JsValue::from_boolean(matches));
                    }
                }
                Ok(JsValue::from_boolean(false))
            }))),
            "exec" => Some(self.alloc_closure_native_function("exec", 1, Arc::new(move |vm, _this, args| {
                let text = args.first().map(|v| vm.to_string_value(*v)).unwrap_or_default();
                if let Some(ptr) = target.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    if let ObjectKind::RegExp { pattern, .. } = &obj.kind {
                        if let Some(idx) = text.find(pattern) {
                            let match_str = vm.alloc_string(pattern.clone());
                            let arr_val = vm.alloc_array(vec![match_str]);
                            vm.set_property(arr_val, "index", JsValue::from_int32(idx as i32));
                            let text_val = vm.alloc_string(text);
                            vm.set_property(arr_val, "input", text_val);
                            return Ok(arr_val);
                        }
                    }
                }
                Ok(JsValue::null())
            }))),
            _ => None,
        }
    }

    // ── String Coercion and JSON ──

    pub fn to_string_value(&self, val: JsValue) -> String {
        if val.is_undefined() {
            "undefined".to_string()
        } else if val.is_null() {
            "null".to_string()
        } else if val.is_boolean() {
            if val.as_boolean().unwrap() { "true".to_string() } else { "false".to_string() }
        } else if val.is_int32() {
            format!("{}", val.as_int32().unwrap())
        } else if val.is_double() {
            let d = val.as_double().unwrap();
            if d.is_nan() {
                "NaN".to_string()
            } else if d.is_infinite() {
                if d > 0.0 { "Infinity".to_string() } else { "-Infinity".to_string() }
            } else if d.fract() == 0.0 && d.abs() < 1e16 {
                format!("{}", d as i64)
            } else {
                format!("{}", d)
            }
        } else if let Some(ptr) = val.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            match &obj.kind {
                ObjectKind::String(s) => s.clone(),
                ObjectKind::Array(elements) => {
                    let parts: Vec<String> = elements.iter().map(|e| self.to_string_value(*e)).collect();
                    parts.join(",")
                }
                ObjectKind::Function(f) => {
                    format!("function {}() {{ [native code] }}", f.name.as_deref().unwrap_or("anonymous"))
                }
                ObjectKind::NativeFunction(n) => {
                    format!("function {}() {{ [native code] }}", n.name)
                }
                ObjectKind::Error { name, message } => {
                    format!("{}: {}", name, message)
                }
                ObjectKind::Date(ms) => format!("Date({})", ms),
                ObjectKind::Symbol(desc) => format!("Symbol({})", desc.as_deref().unwrap_or("")),
                ObjectKind::BigInt(b) => format!("{}n", b),
                ObjectKind::RegExp { pattern, flags } => format!("/{}/{}", pattern, flags),
                _ => {
                    let has_name_or_msg = if let Some(shape) = self.shapes.get(obj.shape_id) {
                        let name = shape.property_offsets.get("name").and_then(|info| obj.get_slot(info.offset));
                        let msg = shape.property_offsets.get("message").and_then(|info| obj.get_slot(info.offset));
                        if let Some(msg_val) = msg {
                            let name_str = if let Some(n_val) = name { self.to_string_value(n_val) } else { "Error".to_string() };
                            let msg_str = self.to_string_value(msg_val);
                            Some(format!("{}: {}", name_str, msg_str))
                        } else {
                            None
                        }
                    } else {
                        None
                    };
                    has_name_or_msg.unwrap_or_else(|| "[object Object]".to_string())
                }
            }
        } else {
            "undefined".to_string()
        }
    }

    pub fn to_json_string(&self, val: JsValue) -> String {
        if val.is_null() {
            "null".to_string()
        } else if val.is_boolean() {
            if val.as_boolean().unwrap() { "true".to_string() } else { "false".to_string() }
        } else if val.is_number() {
            self.to_string_value(val)
        } else if let Some(ptr) = val.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            match &obj.kind {
                ObjectKind::String(s) => format!("\"{}\"", s),
                ObjectKind::Array(elements) => {
                    let items: Vec<String> = elements.iter().map(|e| self.to_json_string(*e)).collect();
                    format!("[{}]", items.join(","))
                }
                ObjectKind::Ordinary => {
                    let mut pairs = Vec::new();
                    if let Some(shape) = self.shapes.get(obj.shape_id) {
                        for (k, info) in &shape.property_offsets {
                            if let Some(v) = obj.get_slot(info.offset) {
                                pairs.push(format!("\"{}\":{}", k, self.to_json_string(v)));
                            }
                        }
                    }
                    format!("{{{}}}", pairs.join(","))
                }
                _ => "null".to_string(),
            }
        } else {
            "null".to_string()
        }
    }

    // ── Main Execution Engine ──

    pub fn eval(&mut self, source: &str) -> Result<JsValue, VmError> {
        let program = doms_parser::parse(source)
            .map_err(|e| VmError::SyntaxError(e.to_string()))?;
        let chunk = doms_compiler::compile(&program)
            .map_err(|e| VmError::TypeError(e.to_string()))?;
        self.run(&chunk)
    }

    pub fn run(&mut self, chunk: &BytecodeChunk) -> Result<JsValue, VmError> {
        let root_chunk = Arc::new(chunk.clone());
        let target_depth = self.frames.len();
        self.frames.push(CallFrame {
            chunk: root_chunk,
            ip: 0,
            base_sp: self.stack.len(),
            this_val: JsValue::undefined(),
            gen_ptr: None,
        });
        let result = self.run_until_depth(target_depth)?;
        self.drain_microtasks()?;
        Ok(result)
    }

    pub fn is_callable(&self, val: JsValue) -> bool {
        if let Some(ptr) = val.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            matches!(obj.kind, ObjectKind::Function(_) | ObjectKind::NativeFunction(_))
        } else {
            false
        }
    }

    pub fn to_primitive(&mut self, val: JsValue, hint: PreferredType) -> Result<JsValue, VmError> {
        if !val.is_object() {
            return Ok(val);
        }
        let ptr = match val.as_object_ptr() {
            Some(p) => p,
            None => return Ok(val),
        };
        let obj = unsafe { &*(ptr as *mut JsObject) };

        match &obj.kind {
            ObjectKind::String(_) => Ok(val),
            ObjectKind::Date(_) => {
                let actual_hint = if hint == PreferredType::Default {
                    PreferredType::String
                } else {
                    hint
                };
                let methods = if actual_hint == PreferredType::String {
                    ["toString", "valueOf"]
                } else {
                    ["valueOf", "toString"]
                };
                for m in methods {
                    let fn_val = self.get_property(val, m);
                    if self.is_callable(fn_val) {
                        let res = self.call(fn_val, val, &[])?;
                        if !res.is_object() || res.as_object_ptr().is_some_and(|p| matches!(unsafe { &(*(p as *mut JsObject)).kind }, ObjectKind::String(_))) {
                            return Ok(res);
                        }
                    }
                }
                Ok(val)
            }
            _ => {
                let methods = if hint == PreferredType::String {
                    ["toString", "valueOf"]
                } else {
                    ["valueOf", "toString"]
                };
                for m in methods {
                    let fn_val = self.get_property(val, m);
                    if self.is_callable(fn_val) {
                        let res = self.call(fn_val, val, &[])?;
                        if !res.is_object() || res.as_object_ptr().is_some_and(|p| matches!(unsafe { &(*(p as *mut JsObject)).kind }, ObjectKind::String(_))) {
                            return Ok(res);
                        }
                    }
                }
                Ok(val)
            }
        }
    }

    pub fn to_number_value(&mut self, val: JsValue) -> Result<f64, VmError> {
        let prim = self.to_primitive(val, PreferredType::Number)?;
        if prim.is_int32() {
            Ok(prim.as_int32().unwrap() as f64)
        } else if prim.is_double() {
            Ok(prim.as_double().unwrap())
        } else if prim.is_boolean() {
            Ok(if prim.as_boolean() == Some(true) { 1.0 } else { 0.0 })
        } else if prim.is_null() {
            Ok(0.0)
        } else if prim.is_undefined() {
            Ok(f64::NAN)
        } else if let Some(ptr) = prim.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            match &obj.kind {
                ObjectKind::String(s) => {
                    let trimmed = s.trim();
                    if trimmed.is_empty() {
                        Ok(0.0)
                    } else if trimmed == "Infinity" || trimmed == "+Infinity" {
                        Ok(f64::INFINITY)
                    } else if trimmed == "-Infinity" {
                        Ok(f64::NEG_INFINITY)
                    } else if trimmed == "NaN" {
                        Ok(f64::NAN)
                    } else if trimmed.starts_with("0x") || trimmed.starts_with("0X") {
                        Ok(u64::from_str_radix(&trimmed[2..], 16).map(|v| v as f64).unwrap_or(f64::NAN))
                    } else if trimmed.starts_with("0o") || trimmed.starts_with("0O") {
                        Ok(u64::from_str_radix(&trimmed[2..], 8).map(|v| v as f64).unwrap_or(f64::NAN))
                    } else if trimmed.starts_with("0b") || trimmed.starts_with("0B") {
                        Ok(u64::from_str_radix(&trimmed[2..], 2).map(|v| v as f64).unwrap_or(f64::NAN))
                    } else {
                        Ok(trimmed.parse::<f64>().unwrap_or(f64::NAN))
                    }
                }
                ObjectKind::BigInt(b) => Ok(*b as f64),
                _ => Ok(f64::NAN),
            }
        } else {
            Ok(f64::NAN)
        }
    }

    pub fn execute_call_internal(
        &mut self,
        callee: JsValue,
        this_val: JsValue,
        args: Vec<JsValue>,
    ) -> Result<Option<JsValue>, VmError> {
        if let Some(ptr) = callee.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            match &obj.kind {
                ObjectKind::NativeFunction(native) => {
                    let res = native.call(self, this_val, &args)?;
                    Ok(Some(res))
                }
                ObjectKind::Function(f) => {
                    let obj_mut = unsafe { &mut *(ptr as *mut JsObject) };
                    if let ObjectKind::Function(ref mut fn_info) = obj_mut.kind {
                        if self.frames.len() < 32 {
                            if let Some(jit_fn) = &fn_info.jit_entry {
                                let callable = Arc::clone(&jit_fn.executable);
                                let res = (callable)(self, this_val, &args)?;
                                return Ok(Some(res));
                            } else {
                                fn_info.call_count = fn_info.call_count.saturating_add(1);
                                if fn_info.call_count >= crate::jit::JIT_HOT_THRESHOLD && !fn_info.is_generator && !fn_info.is_async {
                                    if let Some(jit_fn) = crate::jit::JitCompiler::compile(&fn_info.chunk) {
                                        let callable = Arc::clone(&jit_fn.executable);
                                        fn_info.jit_entry = Some(jit_fn);
                                        let res = (callable)(self, this_val, &args)?;
                                        return Ok(Some(res));
                                    }
                                }
                            }
                        }
                    }
                    if f.is_generator {
                        let generator_instance = JsGenerator {
                            function: f.clone(),
                            ip: 0,
                            stack: args,
                            state: crate::object::GeneratorState::SuspendedStart,
                        };
                        let gen_val = self.alloc_generator(generator_instance);
                        Ok(Some(gen_val))
                    } else if f.is_async {
                        let promise_val = self.alloc_promise(PromiseState::Pending, JsValue::undefined());
                        let chunk = Arc::clone(&f.chunk);
                        let target_depth = self.frames.len();
                        let base_sp = self.stack.len();
                        for arg in &args {
                            self.push(*arg);
                        }
                        self.frames.push(CallFrame {
                            chunk,
                            ip: 0,
                            base_sp,
                            this_val,
                            gen_ptr: None,
                        });
                        match self.run_until_depth(target_depth) {
                            Ok(res) => self.resolve_promise(promise_val, res),
                            Err(e) => {
                                let err_val = self.alloc_string(e.to_string());
                                self.reject_promise(promise_val, err_val);
                            }
                        }
                        let _ = self.drain_microtasks();
                        Ok(Some(promise_val))
                    } else {
                        let new_frame = CallFrame {
                            chunk: Arc::clone(&f.chunk),
                            ip: 0,
                            base_sp: self.stack.len(),
                            this_val,
                            gen_ptr: None,
                        };
                        for arg in args {
                            self.push(arg);
                        }
                        self.frames.push(new_frame);
                        Ok(None)
                    }
                }
                _ => Err(VmError::TypeError(format!("{} is not a function", self.to_string_value(callee)))),
            }
        } else {
            Err(VmError::TypeError(format!("{} is not a function", self.to_string_value(callee))))
        }
    }

    pub fn call(&mut self, callee: JsValue, this_val: JsValue, args: &[JsValue]) -> Result<JsValue, VmError> {
        let res_opt = self.execute_call_internal(callee, this_val, args.to_vec())?;
        if let Some(res) = res_opt {
            Ok(res)
        } else {
            let target_depth = self.frames.len() - 1;
            self.run_until_depth(target_depth)
        }
    }

    pub fn run_until_depth(&mut self, target_depth: usize) -> Result<JsValue, VmError> {
        while self.frames.len() > target_depth {
            if let Some(max) = self.max_instructions {
                self.instruction_count += 1;
                if self.instruction_count > max {
                    return Err(VmError::RuntimeError(
                        "Execution step limit exceeded (timeout)".to_string(),
                    ));
                }
            }
            let (op, chunk) = {
                let frame = self.frames.last_mut().unwrap();
                if frame.ip >= frame.chunk.instructions.len() {
                    let finished = self.frames.pop().unwrap();
                    let ret_val = self.stack.pop().unwrap_or_default();
                    if let Some(g_ptr) = finished.gen_ptr {
                        let g_obj = unsafe { &mut *(g_ptr) };
                        if let ObjectKind::Generator(ref mut g) = g_obj.kind {
                            g.state = crate::object::GeneratorState::Completed;
                        }
                        let iter_res = self.make_iter_result(ret_val, true);
                        self.stack.truncate(finished.base_sp);
                        if self.frames.len() == target_depth {
                            return Ok(iter_res);
                        }
                        self.push(iter_res);
                        continue;
                    }
                    self.stack.truncate(finished.base_sp);
                    if self.frames.len() == target_depth {
                        return Ok(ret_val);
                    }
                    self.push(ret_val);
                    continue;
                }
                let op = frame.chunk.instructions[frame.ip];
                frame.ip += 1;
                (op, Arc::clone(&frame.chunk))
            };

            match op {
                OpCode::LoadConst(idx) => {
                    let c = &chunk.constants[idx as usize];
                    let val = match c {
                        Constant::Number(n) => JsValue::from_double(*n),
                        Constant::String(s) => self.alloc_string(s.clone()),
                        Constant::Boolean(b) => JsValue::from_boolean(*b),
                        Constant::Function(f) => {
                            let js_fn = JsFunction {
                                name: f.name.clone(),
                                params: f.params.clone(),
                                chunk: Arc::new(f.chunk.clone()),
                                upvalues: Vec::new(),
                                is_async: false,
                                is_generator: false,
                                call_count: 0,
                                jit_entry: None,
                            };
                            self.alloc_function(js_fn)
                        }
                    };
                    self.push(val);
                }
                OpCode::LoadNull => self.push(JsValue::null()),
                OpCode::LoadUndefined => self.push(JsValue::undefined()),
                OpCode::LoadTrue => self.push(JsValue::from_boolean(true)),
                OpCode::LoadFalse => self.push(JsValue::from_boolean(false)),
                OpCode::This => {
                    let this_val = self.frames.last().unwrap().this_val;
                    self.push(this_val);
                }

                OpCode::CreateObject => {
                    let obj = self.alloc_object();
                    self.push(obj);
                }
                OpCode::CreateArray(count) => {
                    let mut elements = Vec::with_capacity(count as usize);
                    for _ in 0..count {
                        elements.push(self.pop()?);
                    }
                    elements.reverse();
                    let arr = self.alloc_array(elements);
                    self.push(arr);
                }
                OpCode::GetProp(name_idx) => {
                    let name = match &chunk.constants[name_idx as usize] {
                        Constant::String(s) => s.clone(),
                        _ => return Err(VmError::TypeError("Property name must be string".into())),
                    };
                    let obj = self.pop()?;

                    let ic_idx = name_idx as usize;
                    if ic_idx >= self.inline_caches.len() {
                        self.inline_caches.resize(ic_idx + 1, InlineCacheSlot::new());
                    }

                    if let Some(ptr) = obj.as_object_ptr() {
                        let jsobj = unsafe { &*(ptr as *mut JsObject) };
                        let ic = &self.inline_caches[ic_idx];
                        if ic.state == crate::inline_cache::IcState::Monomorphic && ic.cached_shape_id == Some(jsobj.shape_id) {
                            if let Some(offset) = ic.cached_offset {
                                if let Some(val) = jsobj.get_slot(offset) {
                                    self.push(val);
                                    continue;
                                }
                            }
                        }
                    }

                    let val = self.get_property(obj, &name);

                    if let Some(ptr) = obj.as_object_ptr() {
                        let jsobj = unsafe { &*(ptr as *mut JsObject) };
                        if let Some(offset) = self.shapes.lookup_offset(jsobj.shape_id, &name) {
                            self.inline_caches[ic_idx].record_hit(jsobj.shape_id, offset);
                        }
                    }

                    self.push(val);
                }
                OpCode::SetProp(name_idx) => {
                    let name = match &chunk.constants[name_idx as usize] {
                        Constant::String(s) => s.clone(),
                        _ => return Err(VmError::TypeError("Property name must be string".into())),
                    };
                    let val = self.pop()?;
                    let obj = self.pop()?;

                    let ic_idx = name_idx as usize;
                    if ic_idx >= self.inline_caches.len() {
                        self.inline_caches.resize(ic_idx + 1, InlineCacheSlot::new());
                    }

                    if let Some(ptr) = obj.as_object_ptr() {
                        let jsobj = unsafe { &mut *(ptr as *mut JsObject) };
                        let ic = &self.inline_caches[ic_idx];
                        if ic.state == crate::inline_cache::IcState::Monomorphic && ic.cached_shape_id == Some(jsobj.shape_id) {
                            if let Some(offset) = ic.cached_offset {
                                jsobj.set_slot(offset, val);
                                self.push(val);
                                continue;
                            }
                        }
                    }

                    self.set_property(obj, &name, val);

                    if let Some(ptr) = obj.as_object_ptr() {
                        let jsobj = unsafe { &*(ptr as *mut JsObject) };
                        if let Some(offset) = self.shapes.lookup_offset(jsobj.shape_id, &name) {
                            self.inline_caches[ic_idx].record_hit(jsobj.shape_id, offset);
                        }
                    }

                    self.push(val);
                }
                OpCode::GetElem => {
                    let key = self.pop()?;
                    let obj = self.pop()?;
                    if let Some(ptr) = obj.as_object_ptr() {
                        let jsobj = unsafe { &*(ptr as *mut JsObject) };
                        if let ObjectKind::Array(elements) = &jsobj.kind {
                            if let Some(i) = key.to_int32() {
                                if i >= 0 && (i as usize) < elements.len() {
                                    self.push(elements[i as usize]);
                                    continue;
                                }
                            }
                        }
                        if let ObjectKind::TypedArray(ta) = &jsobj.kind {
                            if let Some(i) = key.to_int32() {
                                if i >= 0 && (i as usize) < ta.length {
                                    let idx = i as usize;
                                    match ta.kind {
                                        TypedArrayKind::Int32 => {
                                            let off = idx * 4;
                                            let b0 = ta.buffer.get(off).unwrap_or(0);
                                            let b1 = ta.buffer.get(off + 1).unwrap_or(0);
                                            let b2 = ta.buffer.get(off + 2).unwrap_or(0);
                                            let b3 = ta.buffer.get(off + 3).unwrap_or(0);
                                            let n = i32::from_le_bytes([b0, b1, b2, b3]);
                                            self.push(JsValue::from_int32(n));
                                            continue;
                                        }
                                        TypedArrayKind::Uint8 => {
                                            let n = ta.buffer.get(idx).unwrap_or(0);
                                            self.push(JsValue::from_int32(n as i32));
                                            continue;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    let key_str = self.to_string_value(key);
                    let val = self.get_property(obj, &key_str);
                    self.push(val);
                }
                OpCode::SetElem => {
                    let val = self.pop()?;
                    let key = self.pop()?;
                    let obj = self.pop()?;
                    if let Some(ptr) = obj.as_object_ptr() {
                        let jsobj = unsafe { &mut *(ptr as *mut JsObject) };
                        if let ObjectKind::Array(elements) = &mut jsobj.kind {
                            if let Some(i) = key.to_int32() {
                                if i >= 0 {
                                    let idx = i as usize;
                                    if idx < 65536 && idx <= elements.len() + 1024 {
                                        if idx >= elements.len() {
                                            elements.resize(idx + 1, JsValue::undefined());
                                        }
                                        elements[idx] = val;
                                        self.push(val);
                                        continue;
                                    }
                                }
                            }
                        }
                        if let ObjectKind::TypedArray(ta) = &mut jsobj.kind {
                            if let Some(i) = key.to_int32() {
                                if i >= 0 && (i as usize) < ta.length {
                                    let idx = i as usize;
                                    let num = val.to_number().unwrap_or(0.0);
                                    match ta.kind {
                                        TypedArrayKind::Int32 => {
                                            let off = idx * 4;
                                            let bytes = (num as i32).to_le_bytes();
                                            for (k, &b) in bytes.iter().enumerate() {
                                                ta.buffer.set(off + k, b);
                                            }
                                            self.push(val);
                                            continue;
                                        }
                                        TypedArrayKind::Uint8 => {
                                            ta.buffer.set(idx, num as u8);
                                            self.push(val);
                                            continue;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    let key_str = self.to_string_value(key);
                    self.set_property(obj, &key_str, val);
                    self.push(val);
                }

                OpCode::CreateFunction(const_idx) => {
                    let def = match &chunk.constants[const_idx as usize] {
                        Constant::Function(f) => Arc::clone(f),
                        _ => return Err(VmError::TypeError("Expected function def".into())),
                    };
                    let js_fn = JsFunction {
                        name: def.name.clone(),
                        params: def.params.clone(),
                        chunk: Arc::new(def.chunk.clone()),
                        upvalues: Vec::new(),
                        is_async: false,
                        is_generator: false,
                        call_count: 0,
                        jit_entry: None,
                    };
                    let func_val = self.alloc_function(js_fn);
                    self.push(func_val);
                }

                OpCode::CreateAsyncFunction(const_idx) => {
                    let def = match &chunk.constants[const_idx as usize] {
                        Constant::Function(f) => Arc::clone(f),
                        _ => return Err(VmError::TypeError("Expected function def".into())),
                    };
                    let js_fn = JsFunction {
                        name: def.name.clone(),
                        params: def.params.clone(),
                        chunk: Arc::new(def.chunk.clone()),
                        upvalues: Vec::new(),
                        is_async: true,
                        is_generator: false,
                        call_count: 0,
                        jit_entry: None,
                    };
                    let func_val = self.alloc_function(js_fn);
                    self.push(func_val);
                }

                OpCode::CreateGenerator(const_idx) => {
                    let def = match &chunk.constants[const_idx as usize] {
                        Constant::Function(f) => Arc::clone(f),
                        _ => return Err(VmError::TypeError("Expected function def".into())),
                    };
                    let js_fn = JsFunction {
                        name: def.name.clone(),
                        params: def.params.clone(),
                        chunk: Arc::new(def.chunk.clone()),
                        upvalues: Vec::new(),
                        is_async: false,
                        is_generator: true,
                        call_count: 0,
                        jit_entry: None,
                    };
                    let func_val = self.alloc_function(js_fn);
                    self.push(func_val);
                }

                OpCode::Call(argc) => {
                    let mut args = Vec::with_capacity(argc as usize);
                    for _ in 0..argc {
                        args.push(self.pop()?);
                    }
                    args.reverse();
                    let callee = self.pop()?;
                    if let Some(res) = self.execute_call_internal(callee, JsValue::undefined(), args)? {
                        self.push(res);
                    }
                }

                OpCode::CallWithThis(argc) => {
                    let mut args = Vec::with_capacity(argc as usize);
                    for _ in 0..argc {
                        args.push(self.pop()?);
                    }
                    args.reverse();
                    let callee = self.pop()?;
                    let this_val = self.pop()?;
                    if let Some(res) = self.execute_call_internal(callee, this_val, args)? {
                        self.push(res);
                    }
                }

                OpCode::Construct(argc) => {
                    let mut args = Vec::with_capacity(argc as usize);
                    for _ in 0..argc {
                        args.push(self.pop()?);
                    }
                    args.reverse();
                    let callee = self.pop()?;

                    if let Some(ptr) = callee.as_object_ptr() {
                        let obj = unsafe { &*(ptr as *mut JsObject) };
                        match &obj.kind {
                            ObjectKind::NativeFunction(native) => {
                                let res = native.call(self, JsValue::undefined(), &args)?;
                                self.push(res);
                            }
                            ObjectKind::Function(_) => {
                                let this_obj = self.alloc_object();
                                let proto = self.get_property(callee, "prototype");
                                if proto.is_object() {
                                    let this_ptr = this_obj.as_object_ptr().unwrap();
                                    let jsobj = unsafe { &mut *(this_ptr as *mut JsObject) };
                                    jsobj.prototype = Some(proto);
                                }
                                let res = self.call(callee, this_obj, &args)?;
                                if res.is_object() {
                                    self.push(res);
                                } else {
                                    self.push(this_obj);
                                }
                            }
                            _ => return Err(VmError::TypeError(format!("{} is not a constructor", self.to_string_value(callee)))),
                        }
                    } else {
                        return Err(VmError::TypeError("Target is not a constructor".into()));
                    }
                }

                OpCode::Return => {
                    let ret_val = self.pop().unwrap_or_default();
                    let finished = self.frames.pop().unwrap();
                    while let Some(top) = self.catch_stack.last() {
                        if top.frame_idx >= self.frames.len() {
                            self.catch_stack.pop();
                        } else {
                            break;
                        }
                    }
                    if let Some(g_ptr) = finished.gen_ptr {
                        let g_obj = unsafe { &mut *(g_ptr) };
                        if let ObjectKind::Generator(ref mut g) = g_obj.kind {
                            g.state = crate::object::GeneratorState::Completed;
                        }
                        let iter_res = self.make_iter_result(ret_val, true);
                        self.stack.truncate(finished.base_sp);
                        if self.frames.len() == target_depth {
                            return Ok(iter_res);
                        }
                        self.push(iter_res);
                    } else {
                        self.stack.truncate(finished.base_sp);
                        if self.frames.len() == target_depth {
                            return Ok(ret_val);
                        }
                        self.push(ret_val);
                    }
                }

                OpCode::Await => {
                    let val = self.pop()?;
                    let resolved = if let Some(ptr) = val.as_object_ptr() {
                        let obj = unsafe { &*(ptr as *mut JsObject) };
                        if let ObjectKind::Promise(_) = &obj.kind {
                            loop {
                                let current_state = {
                                    let o = unsafe { &*(ptr as *mut JsObject) };
                                    if let ObjectKind::Promise(p) = &o.kind {
                                        p.state
                                    } else {
                                        break;
                                    }
                                };
                                if current_state != PromiseState::Pending {
                                    break;
                                }
                                if self.microtasks.is_empty() {
                                    break;
                                }
                                self.drain_microtasks()?;
                            }
                            self.drain_microtasks()?;
                            let o_final = unsafe { &*(ptr as *mut JsObject) };
                            if let ObjectKind::Promise(p) = &o_final.kind {
                                match p.state {
                                    PromiseState::Fulfilled => p.result,
                                    PromiseState::Rejected => {
                                        if let Some(handler) = self.catch_stack.pop() {
                                            self.frames.truncate(handler.frame_idx + 1);
                                            self.stack.truncate(handler.stack_len);
                                            self.push(p.result);
                                            if let Some(frame) = self.frames.last_mut() {
                                                frame.ip = handler.target_ip;
                                            }
                                            continue;
                                        } else {
                                            let err_str = self.to_string_value(p.result);
                                            return Err(VmError::RuntimeError(err_str));
                                        }
                                    }
                                    PromiseState::Pending => val,
                                }
                            } else {
                                val
                            }
                        } else {
                            val
                        }
                    } else {
                        val
                    };
                    self.push(resolved);
                }

                OpCode::Yield => {
                    let yielded_val = self.pop().unwrap_or_default();
                    let frame = self.frames.last_mut().unwrap();
                    if let Some(g_ptr) = frame.gen_ptr {
                        let g_obj = unsafe { &mut *(g_ptr) };
                        if let ObjectKind::Generator(ref mut g) = g_obj.kind {
                            g.ip = frame.ip;
                            g.stack = self.stack[frame.base_sp..].to_vec();
                            g.state = crate::object::GeneratorState::SuspendedYield;
                        }
                    }
                    let finished = self.frames.pop().unwrap();
                    let iter_res = self.make_iter_result(yielded_val, false);
                    self.stack.truncate(finished.base_sp);
                    if self.frames.len() == target_depth {
                        return Ok(iter_res);
                    }
                    self.push(iter_res);
                }

                OpCode::Export(name_idx) => {
                    let name = match &chunk.constants[name_idx as usize] {
                        Constant::String(s) => s.clone(),
                        _ => return Err(VmError::TypeError("Export name must be string".into())),
                    };
                    let val = *self.peek()?;
                    self.current_exports.insert(name, val);
                }

                OpCode::ImportModule(src_idx) => {
                    let src = match &chunk.constants[src_idx as usize] {
                        Constant::String(s) => s.clone(),
                        _ => return Err(VmError::TypeError("Import source must be string".into())),
                    };
                    let mod_obj = if let Some(m) = self.modules.get(&src).copied() {
                        m
                    } else {
                        let m = self.alloc_object();
                        if let Ok(content) = std::fs::read_to_string(&src) {
                            if let Ok(prog) = doms_parser::parse(&content) {
                                if let Ok(c) = doms_compiler::compile(&prog) {
                                    let old_exports = std::mem::take(&mut self.current_exports);
                                    let _ = self.run(&c);
                                    let exports = std::mem::replace(&mut self.current_exports, old_exports);
                                    for (k, v) in exports {
                                        self.set_property(m, &k, v);
                                    }
                                }
                            }
                        }
                        self.modules.insert(src, m);
                        m
                    };
                    self.push(mod_obj);
                }

                OpCode::DynamicImport => {
                    let specifier_val = self.pop()?;
                    let src = self.to_string_value(specifier_val);
                    let mod_obj = if let Some(m) = self.modules.get(&src).copied() {
                        m
                    } else {
                        let m = self.alloc_object();
                        if let Ok(content) = std::fs::read_to_string(&src) {
                            if let Ok(prog) = doms_parser::parse(&content) {
                                if let Ok(c) = doms_compiler::compile(&prog) {
                                    let old_exports = std::mem::take(&mut self.current_exports);
                                    let _ = self.run(&c);
                                    let exports = std::mem::replace(&mut self.current_exports, old_exports);
                                    for (k, v) in exports {
                                        self.set_property(m, &k, v);
                                    }
                                }
                            }
                        }
                        self.modules.insert(src, m);
                        m
                    };
                    let promise_val = self.alloc_promise(PromiseState::Fulfilled, mod_obj);
                    self.push(promise_val);
                }

                OpCode::Add => {
                    let b = self.pop()?;
                    let a = self.pop()?;

                    let a_prim = self.to_primitive(a, PreferredType::Default)?;
                    let b_prim = self.to_primitive(b, PreferredType::Default)?;

                    let a_is_str = a_prim.is_object() && a_prim.as_object_ptr().is_some_and(|p| matches!(unsafe { &(*(p as *mut JsObject)).kind }, ObjectKind::String(_)));
                    let b_is_str = b_prim.is_object() && b_prim.as_object_ptr().is_some_and(|p| matches!(unsafe { &(*(p as *mut JsObject)).kind }, ObjectKind::String(_)));

                    if a_is_str || b_is_str {
                        let sa = self.to_string_value(a_prim);
                        let sb = self.to_string_value(b_prim);
                        let combined = format!("{}{}", sa, sb);
                        let res = self.alloc_string(combined);
                        self.push(res);
                    } else {
                        let na = self.to_number_value(a_prim)?;
                        let nb = self.to_number_value(b_prim)?;
                        self.push(JsValue::from_double(na + nb));
                    }
                }
                OpCode::Sub => {
                    let b_val = self.pop()?;
                    let a_val = self.pop()?;
                    let b = self.to_number_value(b_val)?;
                    let a = self.to_number_value(a_val)?;
                    self.push(JsValue::from_double(a - b));
                }
                OpCode::Mul => {
                    let b_val = self.pop()?;
                    let a_val = self.pop()?;
                    let b = self.to_number_value(b_val)?;
                    let a = self.to_number_value(a_val)?;
                    self.push(JsValue::from_double(a * b));
                }
                OpCode::Div => {
                    let b_val = self.pop()?;
                    let a_val = self.pop()?;
                    let b = self.to_number_value(b_val)?;
                    let a = self.to_number_value(a_val)?;
                    self.push(JsValue::from_double(a / b));
                }
                OpCode::Mod => {
                    let b_val = self.pop()?;
                    let a_val = self.pop()?;
                    let b = self.to_number_value(b_val)?;
                    let a = self.to_number_value(a_val)?;
                    self.push(JsValue::from_double(a % b));
                }

                OpCode::Pos => {
                    let v = self.pop()?;
                    let n = self.to_number_value(v)?;
                    self.push(JsValue::from_double(n));
                }
                OpCode::Neg => {
                    let v = self.pop()?;
                    let n = self.to_number_value(v)?;
                    self.push(JsValue::from_double(-n));
                }
                OpCode::Void => {
                    let _ = self.pop()?;
                    self.push(JsValue::undefined());
                }
                OpCode::Delete => {
                    let _ = self.pop()?;
                    self.push(JsValue::from_boolean(true));
                }
                OpCode::Not => {
                    let v = self.pop()?;
                    self.push(JsValue::from_boolean(!v.is_truthy()));
                }
                OpCode::BitNot => {
                    let n = self.pop()?.to_int32().unwrap_or(0);
                    self.push(JsValue::from_int32(!n));
                }
                OpCode::Typeof => {
                    let v = self.pop()?;
                    let t = if let Some(ptr) = v.as_heap_ptr() {
                        let obj = unsafe { &*(ptr as *mut JsObject) };
                        match &obj.kind {
                            ObjectKind::String(_) => "string",
                            ObjectKind::Function(_) | ObjectKind::NativeFunction(_) => "function",
                            _ => "object",
                        }
                    } else {
                        v.js_typeof()
                    };
                    let s_val = self.alloc_string(t.to_string());
                    self.push(s_val);
                }

                OpCode::BitAnd => {
                    let b = self.pop()?.to_int32().unwrap_or(0);
                    let a = self.pop()?.to_int32().unwrap_or(0);
                    self.push(JsValue::from_int32(a & b));
                }
                OpCode::BitOr => {
                    let b = self.pop()?.to_int32().unwrap_or(0);
                    let a = self.pop()?.to_int32().unwrap_or(0);
                    self.push(JsValue::from_int32(a | b));
                }
                OpCode::BitXor => {
                    let b = self.pop()?.to_int32().unwrap_or(0);
                    let a = self.pop()?.to_int32().unwrap_or(0);
                    self.push(JsValue::from_int32(a ^ b));
                }

                OpCode::Eq | OpCode::StrictEq => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    self.push(JsValue::from_boolean(crate::object::js_values_equal(a, b)));
                }
                OpCode::NotEq | OpCode::StrictNotEq => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    self.push(JsValue::from_boolean(!crate::object::js_values_equal(a, b)));
                }
                OpCode::Lt => {
                    let b_val = self.pop()?;
                    let a_val = self.pop()?;
                    let a_prim = self.to_primitive(a_val, PreferredType::Number)?;
                    let b_prim = self.to_primitive(b_val, PreferredType::Number)?;
                    let a_is_str = a_prim.is_object() && a_prim.as_object_ptr().is_some_and(|p| matches!(unsafe { &(*(p as *mut JsObject)).kind }, ObjectKind::String(_)));
                    let b_is_str = b_prim.is_object() && b_prim.as_object_ptr().is_some_and(|p| matches!(unsafe { &(*(p as *mut JsObject)).kind }, ObjectKind::String(_)));
                    if a_is_str && b_is_str {
                        let sa = self.to_string_value(a_prim);
                        let sb = self.to_string_value(b_prim);
                        self.push(JsValue::from_boolean(sa < sb));
                    } else {
                        let a = self.to_number_value(a_prim)?;
                        let b = self.to_number_value(b_prim)?;
                        self.push(JsValue::from_boolean(a < b));
                    }
                }
                OpCode::Lte => {
                    let b_val = self.pop()?;
                    let a_val = self.pop()?;
                    let a_prim = self.to_primitive(a_val, PreferredType::Number)?;
                    let b_prim = self.to_primitive(b_val, PreferredType::Number)?;
                    let a_is_str = a_prim.is_object() && a_prim.as_object_ptr().is_some_and(|p| matches!(unsafe { &(*(p as *mut JsObject)).kind }, ObjectKind::String(_)));
                    let b_is_str = b_prim.is_object() && b_prim.as_object_ptr().is_some_and(|p| matches!(unsafe { &(*(p as *mut JsObject)).kind }, ObjectKind::String(_)));
                    if a_is_str && b_is_str {
                        let sa = self.to_string_value(a_prim);
                        let sb = self.to_string_value(b_prim);
                        self.push(JsValue::from_boolean(sa <= sb));
                    } else {
                        let a = self.to_number_value(a_prim)?;
                        let b = self.to_number_value(b_prim)?;
                        self.push(JsValue::from_boolean(a <= b));
                    }
                }
                OpCode::Gt => {
                    let b_val = self.pop()?;
                    let a_val = self.pop()?;
                    let a_prim = self.to_primitive(a_val, PreferredType::Number)?;
                    let b_prim = self.to_primitive(b_val, PreferredType::Number)?;
                    let a_is_str = a_prim.is_object() && a_prim.as_object_ptr().is_some_and(|p| matches!(unsafe { &(*(p as *mut JsObject)).kind }, ObjectKind::String(_)));
                    let b_is_str = b_prim.is_object() && b_prim.as_object_ptr().is_some_and(|p| matches!(unsafe { &(*(p as *mut JsObject)).kind }, ObjectKind::String(_)));
                    if a_is_str && b_is_str {
                        let sa = self.to_string_value(a_prim);
                        let sb = self.to_string_value(b_prim);
                        self.push(JsValue::from_boolean(sa > sb));
                    } else {
                        let a = self.to_number_value(a_prim)?;
                        let b = self.to_number_value(b_prim)?;
                        self.push(JsValue::from_boolean(a > b));
                    }
                }
                OpCode::Gte => {
                    let b_val = self.pop()?;
                    let a_val = self.pop()?;
                    let a_prim = self.to_primitive(a_val, PreferredType::Number)?;
                    let b_prim = self.to_primitive(b_val, PreferredType::Number)?;
                    let a_is_str = a_prim.is_object() && a_prim.as_object_ptr().is_some_and(|p| matches!(unsafe { &(*(p as *mut JsObject)).kind }, ObjectKind::String(_)));
                    let b_is_str = b_prim.is_object() && b_prim.as_object_ptr().is_some_and(|p| matches!(unsafe { &(*(p as *mut JsObject)).kind }, ObjectKind::String(_)));
                    if a_is_str && b_is_str {
                        let sa = self.to_string_value(a_prim);
                        let sb = self.to_string_value(b_prim);
                        self.push(JsValue::from_boolean(sa >= sb));
                    } else {
                        let a = self.to_number_value(a_prim)?;
                        let b = self.to_number_value(b_prim)?;
                        self.push(JsValue::from_boolean(a >= b));
                    }
                }
                OpCode::Instanceof => {
                    let rhs = self.pop()?;
                    let lhs = self.pop()?;
                    if !lhs.is_object() {
                        self.push(JsValue::from_boolean(false));
                    } else if let Some(rhs_ptr) = rhs.as_object_ptr() {
                        let _rhs_obj = unsafe { &*(rhs_ptr as *mut JsObject) };
                        let proto = self.get_property(rhs, "prototype");
                        let mut curr_obj = Some(lhs);
                        let mut is_instance = false;
                        if proto.is_object() {
                            while let Some(curr) = curr_obj {
                                if let Some(ptr) = curr.as_object_ptr() {
                                    let obj = unsafe { &*(ptr as *mut JsObject) };
                                    if let Some(p) = obj.prototype {
                                        if p == proto {
                                            is_instance = true;
                                            break;
                                        }
                                        curr_obj = Some(p);
                                    } else {
                                        break;
                                    }
                                } else {
                                    break;
                                }
                            }
                        }
                        self.push(JsValue::from_boolean(is_instance));
                    } else {
                        return Err(VmError::TypeError("Right-hand side of 'instanceof' is not an object".into()));
                    }
                }
                OpCode::In => {
                    let rhs = self.pop()?;
                    let lhs = self.pop()?;
                    let key = self.to_string_value(lhs);
                    if let Some(ptr) = rhs.as_object_ptr() {
                        let obj = unsafe { &*(ptr as *mut JsObject) };
                        let has = self.shapes.has_property(obj.shape_id, &key);
                        self.push(JsValue::from_boolean(has));
                    } else {
                        return Err(VmError::TypeError("Cannot use 'in' operator to search for property in non-object".into()));
                    }
                }

                OpCode::Jump(target) => {
                    let frame = self.frames.last_mut().unwrap();
                    frame.ip = target as usize;
                }
                OpCode::JumpIfFalse(target) => {
                    let cond = self.peek()?;
                    if !cond.is_truthy() {
                        let frame = self.frames.last_mut().unwrap();
                        frame.ip = target as usize;
                    }
                }
                OpCode::JumpIfTrue(target) => {
                    let cond = self.peek()?;
                    if cond.is_truthy() {
                        let frame = self.frames.last_mut().unwrap();
                        frame.ip = target as usize;
                    }
                }

                OpCode::Throw => {
                    let err = self.pop()?;
                    if let Some(handler) = self.catch_stack.pop() {
                        self.frames.truncate(handler.frame_idx + 1);
                        self.stack.truncate(handler.stack_len);
                        self.push(err);
                        if let Some(frame) = self.frames.last_mut() {
                            frame.ip = handler.target_ip;
                        }
                    } else {
                        return Err(VmError::RuntimeError(self.to_string_value(err)));
                    }
                }
                OpCode::PushCatch(target) => {
                    self.catch_stack.push(CatchHandler {
                        frame_idx: self.frames.len().saturating_sub(1),
                        target_ip: target as usize,
                        stack_len: self.stack.len(),
                    });
                }
                OpCode::PopCatch => {
                    self.catch_stack.pop();
                }

                OpCode::GetLocal(slot) => {
                    let frame = self.frames.last().unwrap();
                    let idx = frame.base_sp + slot as usize;
                    let val = self.stack.get(idx).copied().unwrap_or(JsValue::undefined());
                    self.push(val);
                }
                OpCode::SetLocal(slot) => {
                    let val = *self.peek()?;
                    let frame = self.frames.last().unwrap();
                    let idx = frame.base_sp + slot as usize;
                    if idx < self.stack.len() {
                        self.stack[idx] = val;
                    } else {
                        self.stack.push(val);
                    }
                }
                OpCode::GetGlobal(name_idx) => {
                    let name = match &chunk.constants[name_idx as usize] {
                        Constant::String(s) => s.as_str(),
                        _ => return Err(VmError::TypeError("Global name must be string".into())),
                    };
                    let val = self.globals.get(name).copied().unwrap_or(JsValue::undefined());
                    self.push(val);
                }
                OpCode::SetGlobal(name_idx) => {
                    let name = match &chunk.constants[name_idx as usize] {
                        Constant::String(s) => s.clone(),
                        _ => return Err(VmError::TypeError("Global name must be string".into())),
                    };
                    let val = *self.peek()?;
                    self.globals.insert(name, val);
                }

                OpCode::Pop => {
                    self.pop()?;
                }
                OpCode::Dup => {
                    let val = *self.peek()?;
                    self.push(val);
                }
                OpCode::Halt => {
                    let finished = self.frames.pop().unwrap();
                    let ret_val = self.stack.pop().unwrap_or_default();
                    self.stack.truncate(finished.base_sp);
                    return Ok(ret_val);
                }
            }
        }

        Ok(self.pop().unwrap_or_default())
    }
}

impl Drop for Vm {
    fn drop(&mut self) {
        for &ptr in &self.objects {
            if !ptr.is_null() {
                unsafe {
                    drop(Box::from_raw(ptr));
                }
            }
        }
    }
}

impl Default for Vm {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_eval_simple_bytecode() {
        let mut chunk = BytecodeChunk::new();
        let c1 = chunk.add_constant(Constant::Number(10.0));
        let c2 = chunk.add_constant(Constant::Number(32.0));
        chunk.emit(OpCode::LoadConst(c1), 1);
        chunk.emit(OpCode::LoadConst(c2), 1);
        chunk.emit(OpCode::Add, 1);
        chunk.emit(OpCode::Halt, 1);

        let mut vm = Vm::new();
        let res = vm.run(&chunk).unwrap();
        assert_eq!(res.as_number(), Some(42.0));
    }

    #[test]
    fn test_eval_object_and_properties() {
        let mut chunk = BytecodeChunk::new();
        let name_prop = chunk.add_constant(Constant::String("x".into()));
        let val_const = chunk.add_constant(Constant::Number(99.0));

        chunk.emit(OpCode::CreateObject, 1);
        chunk.emit(OpCode::Dup, 1);
        chunk.emit(OpCode::LoadConst(val_const), 1);
        chunk.emit(OpCode::SetProp(name_prop), 1);
        chunk.emit(OpCode::Pop, 1);
        chunk.emit(OpCode::GetProp(name_prop), 1);
        chunk.emit(OpCode::Halt, 1);

        let mut vm = Vm::new();
        let res = vm.run(&chunk).unwrap();
        assert_eq!(res.as_number(), Some(99.0));
    }

    #[test]
    fn test_phase3_builtins_and_collections() {
        let mut vm = Vm::new();

        // Map test
        let map_val = vm.alloc_map();
        let set_method = vm.get_property(map_val, "set");
        let get_method = vm.get_property(map_val, "get");
        let k = vm.alloc_string("hello".into());
        let v = JsValue::from_int32(123);
        vm.call(set_method, map_val, &[k, v]).unwrap();
        let k_lookup = vm.alloc_string("hello".into());
        let got = vm.call(get_method, map_val, &[k_lookup]).unwrap();
        assert_eq!(got.as_number(), Some(123.0));

        // Set test
        let set_val = vm.alloc_set();
        let add_method = vm.get_property(set_val, "add");
        let has_method = vm.get_property(set_val, "has");
        let num = JsValue::from_int32(77);
        vm.call(add_method, set_val, &[num]).unwrap();
        let has_res = vm.call(has_method, set_val, &[num]).unwrap();
        assert_eq!(has_res.as_boolean(), Some(true));

        // WeakRef test
        let obj = vm.alloc_object();
        let wr = vm.alloc_weak_ref(obj);
        let deref_fn = vm.get_property(wr, "deref");
        let derefed = vm.call(deref_fn, wr, &[]).unwrap();
        assert_eq!(derefed, obj);

        // RegExp test
        let re = vm.alloc_regexp("doms".into(), "".into());
        let test_fn = vm.get_property(re, "test");
        let target_str = vm.alloc_string("doms_js engine".into());
        let matched = vm.call(test_fn, re, &[target_str]).unwrap();
        assert_eq!(matched.as_boolean(), Some(true));
    }

    #[test]
    fn test_phase3_jit_hot_execution() {
        let mut vm = Vm::new();
        // Function that adds two numbers: arg0 + arg1
        let mut chunk = BytecodeChunk::new();
        chunk.emit(OpCode::GetLocal(0), 1);
        chunk.emit(OpCode::GetLocal(1), 1);
        chunk.emit(OpCode::Add, 1);
        chunk.emit(OpCode::Return, 1);

        let js_fn = JsFunction {
            name: Some("add".into()),
            params: vec!["a".into(), "b".into()],
            chunk: Arc::new(chunk),
            upvalues: Vec::new(),
            is_async: false,
            is_generator: false,
            call_count: 0,
            jit_entry: None,
        };
        let fn_val = vm.alloc_function(js_fn);

        // Call 15 times to exceed JIT_HOT_THRESHOLD (10)
        let mut total = 0.0;
        for i in 0..15 {
            let res = vm.call(fn_val, JsValue::undefined(), &[JsValue::from_int32(i), JsValue::from_int32(1)]).unwrap();
            total += res.as_number().unwrap();
        }
        assert_eq!(total, 120.0);
    }
}
