//! Bytecode interpreter virtual machine with NaN-boxed values, CallFrames, and Shapes.

use crate::builtins::register_builtins;
use crate::inline_cache::InlineCacheSlot;
use crate::object::{JsFunction, JsObject, NativeFn, NativeFunction, ObjectKind};
use crate::shape::ShapeRegistry;
use crate::value::JsValue;
use doms_compiler::bytecode::{BytecodeChunk, Constant, OpCode};
use doms_gc::heap::Heap;
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum VmError {
    #[error("Stack underflow")]
    StackUnderflow,
    #[error("Undefined global variable: {0}")]
    UndefinedGlobal(String),
    #[error("Type error: {0}")]
    TypeError(String),
    #[error("Runtime error: {0}")]
    RuntimeError(String),
    #[error("Invalid instruction at ip: {0}")]
    InvalidInstruction(usize),
}

/// Call frame on the execution stack.
#[derive(Debug, Clone)]
pub struct CallFrame {
    pub chunk: Arc<BytecodeChunk>,
    pub ip: usize,
    pub base_sp: usize,
    pub this_val: JsValue,
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

pub struct Vm {
    pub stack: Vec<JsValue>,
    pub frames: Vec<CallFrame>,
    pub globals: HashMap<String, JsValue>,
    pub heap: Heap,
    pub shapes: ShapeRegistry,
    pub inline_caches: Vec<InlineCacheSlot>,
    pub objects: Vec<*mut JsObject>,
    pub catch_stack: Vec<usize>,
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
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
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
                        let start = args.first().and_then(|v| v.as_int32()).unwrap_or(0);
                        let end = args.get(1).and_then(|v| v.as_int32()).unwrap_or(len);
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
                let idx = args.first().and_then(|v| v.as_int32()).unwrap_or(0) as usize;
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
                let start = args.first().and_then(|v| v.as_int32()).unwrap_or(0).clamp(0, len) as usize;
                let end = args.get(1).and_then(|v| v.as_int32()).unwrap_or(len).clamp(0, len) as usize;
                let sub = if start <= end { &s[start..end] } else { "" };
                Ok(vm.alloc_string(sub.to_string()))
            })),
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
                ObjectKind::Ordinary => "[object Object]".to_string(),
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

    pub fn run(&mut self, chunk: &BytecodeChunk) -> Result<JsValue, VmError> {
        let root_chunk = Arc::new(chunk.clone());
        let target_depth = self.frames.len();
        self.frames.push(CallFrame {
            chunk: root_chunk,
            ip: 0,
            base_sp: self.stack.len(),
            this_val: JsValue::undefined(),
        });
        self.run_until_depth(target_depth)
    }

    pub fn call(&mut self, callee: JsValue, this_val: JsValue, args: &[JsValue]) -> Result<JsValue, VmError> {
        if let Some(ptr) = callee.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            match &obj.kind {
                ObjectKind::NativeFunction(native) => {
                    return native.call(self, this_val, args);
                }
                ObjectKind::Function(f) => {
                    let chunk = Arc::clone(&f.chunk);
                    let target_depth = self.frames.len();
                    let base_sp = self.stack.len();
                    for arg in args {
                        self.push(*arg);
                    }
                    self.frames.push(CallFrame {
                        chunk,
                        ip: 0,
                        base_sp,
                        this_val,
                    });
                    return self.run_until_depth(target_depth);
                }
                _ => return Err(VmError::TypeError("Target is not a function".into())),
            }
        }
        Err(VmError::TypeError("Target is not callable".into()))
    }

    pub fn run_until_depth(&mut self, target_depth: usize) -> Result<JsValue, VmError> {
        while self.frames.len() > target_depth {
            let (op, chunk) = {
                let frame = self.frames.last_mut().unwrap();
                if frame.ip >= frame.chunk.instructions.len() {
                    let finished = self.frames.pop().unwrap();
                    let ret_val = self.stack.pop().unwrap_or_default();
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
                    let val = self.get_property(obj, &name);
                    self.push(val);
                }
                OpCode::SetProp(name_idx) => {
                    let name = match &chunk.constants[name_idx as usize] {
                        Constant::String(s) => s.clone(),
                        _ => return Err(VmError::TypeError("Property name must be string".into())),
                    };
                    let val = self.pop()?;
                    let obj = self.pop()?;
                    self.set_property(obj, &name, val);
                    self.push(val);
                }
                OpCode::GetElem => {
                    let key = self.pop()?;
                    let obj = self.pop()?;
                    if let Some(ptr) = obj.as_object_ptr() {
                        let jsobj = unsafe { &*(ptr as *mut JsObject) };
                        if let ObjectKind::Array(elements) = &jsobj.kind {
                            if let Some(i) = key.as_int32() {
                                if i >= 0 && (i as usize) < elements.len() {
                                    self.push(elements[i as usize]);
                                    continue;
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
                            if let Some(i) = key.as_int32() {
                                if i >= 0 {
                                    let idx = i as usize;
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

                    if let Some(ptr) = callee.as_object_ptr() {
                        let obj = unsafe { &*(ptr as *mut JsObject) };
                        match &obj.kind {
                            ObjectKind::NativeFunction(native) => {
                                let res = native.call(self, JsValue::undefined(), &args)?;
                                self.push(res);
                            }
                            ObjectKind::Function(f) => {
                                let new_frame = CallFrame {
                                    chunk: Arc::clone(&f.chunk),
                                    ip: 0,
                                    base_sp: self.stack.len(),
                                    this_val: JsValue::undefined(),
                                };
                                for arg in args {
                                    self.push(arg);
                                }
                                self.frames.push(new_frame);
                            }
                            _ => return Err(VmError::TypeError(format!("{} is not a function", self.to_string_value(callee)))),
                        }
                    } else {
                        return Err(VmError::TypeError(format!("{} is not a function", self.to_string_value(callee))));
                    }
                }

                OpCode::Return => {
                    let ret_val = self.pop().unwrap_or_default();
                    let finished = self.frames.pop().unwrap();
                    self.stack.truncate(finished.base_sp);
                    if self.frames.len() == target_depth {
                        return Ok(ret_val);
                    }
                    self.push(ret_val);
                }

                OpCode::Add => {
                    let b = self.pop()?;
                    let a = self.pop()?;

                    let a_is_str = a.is_object() && a.as_object_ptr().is_some_and(|p| matches!(unsafe { &(*(p as *mut JsObject)).kind }, ObjectKind::String(_)));
                    let b_is_str = b.is_object() && b.as_object_ptr().is_some_and(|p| matches!(unsafe { &(*(p as *mut JsObject)).kind }, ObjectKind::String(_)));

                    if a_is_str || b_is_str {
                        let sa = self.to_string_value(a);
                        let sb = self.to_string_value(b);
                        let combined = format!("{}{}", sa, sb);
                        let res = self.alloc_string(combined);
                        self.push(res);
                    } else {
                        let na = a.to_number().unwrap_or(f64::NAN);
                        let nb = b.to_number().unwrap_or(f64::NAN);
                        self.push(JsValue::from_double(na + nb));
                    }
                }
                OpCode::Sub => {
                    let b = self.pop()?.to_number().unwrap_or(f64::NAN);
                    let a = self.pop()?.to_number().unwrap_or(f64::NAN);
                    self.push(JsValue::from_double(a - b));
                }
                OpCode::Mul => {
                    let b = self.pop()?.to_number().unwrap_or(f64::NAN);
                    let a = self.pop()?.to_number().unwrap_or(f64::NAN);
                    self.push(JsValue::from_double(a * b));
                }
                OpCode::Div => {
                    let b = self.pop()?.to_number().unwrap_or(f64::NAN);
                    let a = self.pop()?.to_number().unwrap_or(f64::NAN);
                    self.push(JsValue::from_double(a / b));
                }
                OpCode::Mod => {
                    let b = self.pop()?.to_number().unwrap_or(f64::NAN);
                    let a = self.pop()?.to_number().unwrap_or(f64::NAN);
                    self.push(JsValue::from_double(a % b));
                }

                OpCode::Neg => {
                    let n = self.pop()?.to_number().unwrap_or(f64::NAN);
                    self.push(JsValue::from_double(-n));
                }
                OpCode::Not => {
                    let v = self.pop()?;
                    self.push(JsValue::from_boolean(!v.is_truthy()));
                }
                OpCode::BitNot => {
                    let n = self.pop()?.as_int32().unwrap_or(0);
                    self.push(JsValue::from_int32(!n));
                }
                OpCode::Typeof => {
                    let v = self.pop()?;
                    let t = v.js_typeof();
                    let s_val = self.alloc_string(t.to_string());
                    self.push(s_val);
                }

                OpCode::BitAnd => {
                    let b = self.pop()?.as_int32().unwrap_or(0);
                    let a = self.pop()?.as_int32().unwrap_or(0);
                    self.push(JsValue::from_int32(a & b));
                }
                OpCode::BitOr => {
                    let b = self.pop()?.as_int32().unwrap_or(0);
                    let a = self.pop()?.as_int32().unwrap_or(0);
                    self.push(JsValue::from_int32(a | b));
                }
                OpCode::BitXor => {
                    let b = self.pop()?.as_int32().unwrap_or(0);
                    let a = self.pop()?.as_int32().unwrap_or(0);
                    self.push(JsValue::from_int32(a ^ b));
                }

                OpCode::Eq | OpCode::StrictEq => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    if a == b {
                        self.push(JsValue::from_boolean(true));
                    } else if (a.is_number()) && (b.is_number()) {
                        self.push(JsValue::from_boolean(a.to_number() == b.to_number()));
                    } else {
                        self.push(JsValue::from_boolean(false));
                    }
                }
                OpCode::NotEq | OpCode::StrictNotEq => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    if a == b {
                        self.push(JsValue::from_boolean(false));
                    } else if (a.is_number()) && (b.is_number()) {
                        self.push(JsValue::from_boolean(a.to_number() != b.to_number()));
                    } else {
                        self.push(JsValue::from_boolean(true));
                    }
                }
                OpCode::Lt => {
                    let b = self.pop()?.to_number().unwrap_or(f64::NAN);
                    let a = self.pop()?.to_number().unwrap_or(f64::NAN);
                    self.push(JsValue::from_boolean(a < b));
                }
                OpCode::Lte => {
                    let b = self.pop()?.to_number().unwrap_or(f64::NAN);
                    let a = self.pop()?.to_number().unwrap_or(f64::NAN);
                    self.push(JsValue::from_boolean(a <= b));
                }
                OpCode::Gt => {
                    let b = self.pop()?.to_number().unwrap_or(f64::NAN);
                    let a = self.pop()?.to_number().unwrap_or(f64::NAN);
                    self.push(JsValue::from_boolean(a > b));
                }
                OpCode::Gte => {
                    let b = self.pop()?.to_number().unwrap_or(f64::NAN);
                    let a = self.pop()?.to_number().unwrap_or(f64::NAN);
                    self.push(JsValue::from_boolean(a >= b));
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
                    if let Some(target_ip) = self.catch_stack.pop() {
                        self.push(err);
                        let frame = self.frames.last_mut().unwrap();
                        frame.ip = target_ip;
                    } else {
                        return Err(VmError::RuntimeError(self.to_string_value(err)));
                    }
                }
                OpCode::PushCatch(target) => {
                    self.catch_stack.push(target as usize);
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
}
