//! Bytecode interpreter virtual machine with NaN-boxed values, CallFrames, and Shapes.

use crate::builtins::register_builtins;
use crate::inline_cache::InlineCacheSlot;
use crate::object::{
    Accessor, JsFunction, JsGenerator, JsObject, JsTypedArray, NativeFn, NativeFunction, ObjectKind,
    PromiseState, TypedArrayKind,
};
use crate::shape::ShapeRegistry;
use crate::value::JsValue;
use doms_compiler::bytecode::{BytecodeChunk, Constant, OpCode};
use doms_gc::heap::Heap;
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::fmt;
use std::sync::Arc;
use std::time::{Duration, Instant};
use thiserror::Error;
use num_bigint::BigInt;
use num_traits::{Zero, One, Signed, ToPrimitive, Num};
use std::str::FromStr;

pub fn double_to_int32(d: f64) -> i32 {
    if d.is_nan() || d.is_infinite() || d == 0.0 {
        return 0;
    }
    let d = d % 4294967296.0;
    let i = d as i64;
    (i as u32) as i32
}

pub fn f64_to_bigint_exact(f: f64) -> Option<(BigInt, bool)> {
    if f.is_nan() || f.is_infinite() {
        return None;
    }
    let bits = f.to_bits();
    let is_neg = (bits >> 63) != 0;
    let raw_exp = ((bits >> 52) & 0x7FF) as i32;
    let mantissa = bits & 0x000F_FFFF_FFFF_FFFF;

    if raw_exp == 0 {
        if mantissa == 0 {
            return Some((BigInt::zero(), false));
        } else {
            return Some((BigInt::zero(), true));
        }
    }

    let significand = (1u128 << 52) | (mantissa as u128);
    let exp = raw_exp - 1023 - 52;

    if exp >= 0 {
        let bi = BigInt::from(significand) << (exp as usize);
        Some((if is_neg { -bi } else { bi }, false))
    } else {
        let shift = (-exp) as u32;
        if shift >= 128 {
            Some((BigInt::zero(), true))
        } else {
            let int_part = significand >> shift;
            let fract_mask = (1u128 << shift) - 1;
            let has_fract = (significand & fract_mask) != 0;
            let bi = BigInt::from(int_part);
            Some((if is_neg { -bi } else { bi }, has_fract))
        }
    }
}

pub fn bigint_cmp_f64(bi: &BigInt, f: f64) -> Option<std::cmp::Ordering> {
    if f.is_nan() {
        return None;
    }
    if f.is_infinite() {
        return Some(if f > 0.0 { std::cmp::Ordering::Less } else { std::cmp::Ordering::Greater });
    }
    if let Some((f_bi, has_fract)) = f64_to_bigint_exact(f) {
        match bi.cmp(&f_bi) {
            std::cmp::Ordering::Equal => {
                if has_fract {
                    if f > 0.0 {
                        Some(std::cmp::Ordering::Less)
                    } else {
                        Some(std::cmp::Ordering::Greater)
                    }
                } else {
                    Some(std::cmp::Ordering::Equal)
                }
            }
            other => Some(other),
        }
    } else {
        None
    }
}

pub fn string_to_bigint(s: &str) -> Option<BigInt> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Some(BigInt::zero());
    }
    if let Some(rest) = trimmed.strip_prefix("0x").or_else(|| trimmed.strip_prefix("0X")) {
        if rest.is_empty() || !rest.chars().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        BigInt::from_str_radix(rest, 16).ok()
    } else if let Some(rest) = trimmed.strip_prefix("0o").or_else(|| trimmed.strip_prefix("0O")) {
        if rest.is_empty() || !rest.chars().all(|c| matches!(c, '0'..='7')) {
            return None;
        }
        BigInt::from_str_radix(rest, 8).ok()
    } else if let Some(rest) = trimmed.strip_prefix("0b").or_else(|| trimmed.strip_prefix("0B")) {
        if rest.is_empty() || !rest.chars().all(|c| matches!(c, '0' | '1')) {
            return None;
        }
        BigInt::from_str_radix(rest, 2).ok()
    } else if let Some(rest) = trimmed.strip_prefix('+') {
        if rest.is_empty() || !rest.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        BigInt::from_str(rest).ok()
    } else if let Some(rest) = trimmed.strip_prefix('-') {
        if rest.is_empty() || !rest.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        BigInt::from_str(rest).ok().map(|b| -b)
    } else {
        if !trimmed.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        BigInt::from_str(trimmed).ok()
    }
}

pub fn js_exponentiate(base: f64, exp: f64) -> f64 {
    if exp.is_nan() {
        return f64::NAN;
    }
    if exp == 0.0 {
        return 1.0;
    }
    if base.abs() == 1.0 && exp.is_infinite() {
        return f64::NAN;
    }
    base.powf(exp)
}

pub fn js_string_code_units(s: &str) -> Vec<u16> {
    let bytes = s.as_bytes();
    let mut units = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b < 0x80 {
            units.push(b as u16);
            i += 1;
        } else if b & 0xE0 == 0xC0 {
            if i + 1 < bytes.len() {
                let cp = (((b & 0x1F) as u16) << 6) | ((bytes[i + 1] & 0x3F) as u16);
                units.push(cp);
                i += 2;
            } else {
                units.push(b as u16);
                i += 1;
            }
        } else if b & 0xF0 == 0xE0 {
            if i + 2 < bytes.len() {
                let cp = (((b & 0x0F) as u16) << 12)
                    | (((bytes[i + 1] & 0x3F) as u16) << 6)
                    | ((bytes[i + 2] & 0x3F) as u16);
                units.push(cp);
                i += 3;
            } else {
                units.push(b as u16);
                i += 1;
            }
        } else if b & 0xF8 == 0xF0 {
            if i + 3 < bytes.len() {
                let cp = (((b & 0x07) as u32) << 18)
                    | (((bytes[i + 1] & 0x3F) as u32) << 12)
                    | (((bytes[i + 2] & 0x3F) as u32) << 6)
                    | ((bytes[i + 3] & 0x3F) as u32);
                let u = cp.saturating_sub(0x10000);
                let hi = ((u >> 10) as u16) + 0xD800;
                let lo = ((u & 0x3FF) as u16) + 0xDC00;
                units.push(hi);
                units.push(lo);
                i += 4;
            } else {
                units.push(b as u16);
                i += 1;
            }
        } else {
            units.push(b as u16);
            i += 1;
        }
    }
    units
}

pub fn js_string_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let ua = js_string_code_units(a);
    let ub = js_string_code_units(b);
    ua.cmp(&ub)
}

#[derive(Debug, Clone, Error, PartialEq)]
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
    #[error("Range error: {0}")]
    RangeError(String),
    #[error("Runtime error: {0}")]
    RuntimeError(String),
    #[error("Invalid instruction at ip: {0}")]
    InvalidInstruction(usize),
    #[error("URI error: {0}")]
    UriError(String),
    #[error("Thrown JavaScript exception")]
    Thrown(JsValue),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreferredType {
    Default,
    Number,
    String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PropertyKeyVal {
    String(String),
    Symbol(JsValue),
}

/// Call frame on the execution stack.
#[derive(Debug, Clone)]
pub struct CallFrame {
    pub chunk: Arc<BytecodeChunk>,
    pub ip: usize,
    pub base_sp: usize,
    pub this_val: JsValue,
    pub gen_ptr: Option<*mut JsObject>,
    pub locals: Vec<JsValue>,
    pub local_names: Vec<String>,
    pub lexical_local_names: Vec<String>,
    pub scopes: Vec<JsValue>,
    pub upvalues_count: usize,
    pub env_obj: Option<JsValue>,
    pub arguments: Option<JsValue>,
    pub is_strict: bool,
    pub new_target: Option<JsValue>,
    pub is_function: bool,
    pub is_method: bool,
    pub is_arrow: bool,
    pub has_param_exprs: bool,
    pub in_param_init: bool,
    pub is_indirect_eval: bool,
    pub callee_func: Option<JsFunction>,
    pub mapped_arguments: Option<Vec<bool>>,
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
    pub scopes_len: usize,
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
    pub deadline: Option<Instant>,
    pub max_objects: Option<usize>,
    pub eval_count: usize,
    pub max_evals: Option<usize>,
    pub oom: bool,
    pub gc_threshold: usize,
    pub temp_roots: Vec<JsValue>,
    pub pending_alloc: Option<*mut JsObject>,
    pub object_proto: Option<JsValue>,
    pub array_proto: Option<JsValue>,
    pub function_proto: Option<JsValue>,
    pub regexp_proto: Option<JsValue>,
    pub string_proto: Option<JsValue>,
    pub number_proto: Option<JsValue>,
    pub boolean_proto: Option<JsValue>,
    pub symbol_proto: Option<JsValue>,
    pub promise_proto: Option<JsValue>,
    pub global_object: JsValue,
    pub global_lexicals: HashMap<String, JsValue>,
    pub const_bindings: HashSet<String>,
    pub non_configurable_props: HashSet<(JsValue, String)>,
    pub non_writable_props: HashSet<(JsValue, String)>,
    pub non_enumerable_props: HashSet<(JsValue, String)>,
    pub non_extensible_objects: HashSet<JsValue>,
    pub var_names: HashSet<String>,
    pub script_var_names: HashSet<String>,
    pub in_eval: bool,
    pub in_direct_eval: bool,
    pub is_strict: bool,
    pub is_constructing: bool,
    pub symbol_registry: HashMap<String, JsValue>,
    pub symbol_iterator: Option<JsValue>,
    pub symbol_to_string_tag: Option<JsValue>,
    pub symbol_is_concat_spreadable: Option<JsValue>,
    pub symbol_species: Option<JsValue>,
    pub symbol_to_primitive: Option<JsValue>,
    pub deleted_props: HashSet<(JsValue, String)>,
    pub is_module: bool,
    pub base_dir: Option<std::path::PathBuf>,
    pub module_namespace_objects: HashSet<JsValue>,
    pub current_module_obj: Option<JsValue>,
    pub imported_bindings: HashMap<String, (JsValue, String)>,
    pub star_exports: HashMap<String, JsValue>,
    pub module_reexports: HashMap<(JsValue, String), (JsValue, String)>,
}

impl Vm {
    pub fn new() -> Self {
        let mut vm = Self {
            stack: Vec::with_capacity(4096),
            frames: Vec::with_capacity(256),
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
            deadline: None,
            max_objects: None,
            eval_count: 0,
            max_evals: None,
            oom: false,
            gc_threshold: 4096,
            temp_roots: Vec::new(),
            pending_alloc: None,
            object_proto: None,
            array_proto: None,
            function_proto: None,
            regexp_proto: None,
            string_proto: None,
            number_proto: None,
            boolean_proto: None,
            symbol_proto: None,
            promise_proto: None,
            global_object: JsValue::undefined(),
            global_lexicals: HashMap::new(),
            const_bindings: HashSet::new(),
            non_configurable_props: HashSet::new(),
            non_writable_props: HashSet::new(),
            non_enumerable_props: HashSet::new(),
            non_extensible_objects: HashSet::new(),
            var_names: HashSet::new(),
            script_var_names: HashSet::new(),
            in_eval: false,
            in_direct_eval: false,
            is_strict: false,
            is_constructing: false,
            symbol_registry: HashMap::new(),
            symbol_iterator: None,
            symbol_to_string_tag: None,
            symbol_is_concat_spreadable: None,
            symbol_species: None,
            symbol_to_primitive: None,
            deleted_props: HashSet::new(),
            is_module: false,
            base_dir: None,
            module_namespace_objects: HashSet::new(),
            current_module_obj: None,
            imported_bindings: HashMap::new(),
            star_exports: HashMap::new(),
            module_reexports: HashMap::new(),
        };
        let global_obj = vm.alloc_object();
        vm.global_object = global_obj;
        vm.globals.insert("globalThis".to_string(), global_obj);
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

    #[inline]
    pub fn track_ptr(&mut self, ptr: *mut JsObject) {
        self.objects.push(ptr);
        if let Some(max) = self.max_objects {
            if self.objects.len() >= max {
                self.oom = true;
            }
        }
    }

    /// Perform a full tracing garbage collection cycle across all live roots.
    pub fn gc_collect(&mut self) -> usize {
        use std::collections::HashSet;

        let mut live = HashSet::new();
        let mut worklist = Vec::new();

        let mark_val = |val: JsValue, worklist: &mut Vec<*mut JsObject>, live: &mut HashSet<*mut JsObject>| {
            if let Some(ptr) = val.as_heap_ptr() {
                let obj_ptr = ptr as *mut JsObject;
                if live.insert(obj_ptr) {
                    worklist.push(obj_ptr);
                }
            }
        };

        // 1. Mark Roots
        if let Some(pending) = self.pending_alloc {
            if live.insert(pending) {
                worklist.push(pending);
            }
        }
        if let Some(proto) = self.object_proto {
            mark_val(proto, &mut worklist, &mut live);
        }
        if let Some(proto) = self.array_proto {
            mark_val(proto, &mut worklist, &mut live);
        }
        if let Some(proto) = self.function_proto {
            mark_val(proto, &mut worklist, &mut live);
        }
        if let Some(proto) = self.regexp_proto {
            mark_val(proto, &mut worklist, &mut live);
        }
        if let Some(proto) = self.string_proto {
            mark_val(proto, &mut worklist, &mut live);
        }
        if let Some(proto) = self.number_proto {
            mark_val(proto, &mut worklist, &mut live);
        }
        if let Some(proto) = self.boolean_proto {
            mark_val(proto, &mut worklist, &mut live);
        }
        if let Some(proto) = self.symbol_proto {
            mark_val(proto, &mut worklist, &mut live);
        }
        if let Some(proto) = self.promise_proto {
            mark_val(proto, &mut worklist, &mut live);
        }
        if let Some(s) = self.symbol_species {
            mark_val(s, &mut worklist, &mut live);
        }
        for val in self.global_lexicals.values() {
            mark_val(*val, &mut worklist, &mut live);
        }
        for val in &self.temp_roots {
            mark_val(*val, &mut worklist, &mut live);
        }
        for val in &self.stack {
            mark_val(*val, &mut worklist, &mut live);
        }
        mark_val(self.global_object, &mut worklist, &mut live);
        for (target, _) in &self.non_configurable_props {
            mark_val(*target, &mut worklist, &mut live);
        }
        for (target, _) in &self.non_writable_props {
            mark_val(*target, &mut worklist, &mut live);
        }
        for (target, _) in &self.non_enumerable_props {
            mark_val(*target, &mut worklist, &mut live);
        }
        for frame in &self.frames {
            mark_val(frame.this_val, &mut worklist, &mut live);
            for local in &frame.locals {
                mark_val(*local, &mut worklist, &mut live);
            }
            for scope in &frame.scopes {
                mark_val(*scope, &mut worklist, &mut live);
            }
            if let Some(env) = frame.env_obj {
                mark_val(env, &mut worklist, &mut live);
            }
            if let Some(args_val) = frame.arguments {
                mark_val(args_val, &mut worklist, &mut live);
            }
        }
        for val in self.globals.values() {
            mark_val(*val, &mut worklist, &mut live);
        }
        for val in self.modules.values() {
            mark_val(*val, &mut worklist, &mut live);
        }
        for val in self.current_exports.values() {
            mark_val(*val, &mut worklist, &mut live);
        }
        for timer in self.timers.values() {
            mark_val(timer.callback, &mut worklist, &mut live);
            for arg in &timer.args {
                mark_val(*arg, &mut worklist, &mut live);
            }
        }
        for (callee, args) in &self.microtasks {
            mark_val(*callee, &mut worklist, &mut live);
            for arg in args {
                mark_val(*arg, &mut worklist, &mut live);
            }
        }
        if let Some(s) = self.symbol_iterator {
            mark_val(s, &mut worklist, &mut live);
        }
        if let Some(s) = self.symbol_to_string_tag {
            mark_val(s, &mut worklist, &mut live);
        }
        if let Some(s) = self.symbol_is_concat_spreadable {
            mark_val(s, &mut worklist, &mut live);
        }
        if let Some(s) = self.symbol_to_primitive {
            mark_val(s, &mut worklist, &mut live);
        }
        for s in self.symbol_registry.values() {
            mark_val(*s, &mut worklist, &mut live);
        }

        // 2. Trace transitively
        while let Some(ptr) = worklist.pop() {
            let obj = unsafe { &*ptr };
            if let Some(proto) = obj.prototype {
                mark_val(proto, &mut worklist, &mut live);
            }
            for slot in &obj.slots {
                mark_val(*slot, &mut worklist, &mut live);
            }
            if let Some(accs) = &obj.accessors {
                for acc in accs.values() {
                    if let Some(g) = acc.getter {
                        mark_val(g, &mut worklist, &mut live);
                    }
                    if let Some(s) = acc.setter {
                        mark_val(s, &mut worklist, &mut live);
                    }
                }
            }
            if let Some(syms) = &obj.symbol_properties {
                for (sym_key, val) in syms.iter() {
                    mark_val(*sym_key, &mut worklist, &mut live);
                    mark_val(*val, &mut worklist, &mut live);
                }
            }
            if let Some(sym_accs) = &obj.symbol_accessors {
                for (sym_key, acc) in sym_accs.iter() {
                    mark_val(*sym_key, &mut worklist, &mut live);
                    if let Some(g) = acc.getter {
                        mark_val(g, &mut worklist, &mut live);
                    }
                    if let Some(s) = acc.setter {
                        mark_val(s, &mut worklist, &mut live);
                    }
                }
            }
            match &obj.kind {
                ObjectKind::Array(elements) => {
                    for el in elements {
                        mark_val(*el, &mut worklist, &mut live);
                    }
                }
                ObjectKind::Proxy { target, handler } => {
                    mark_val(*target, &mut worklist, &mut live);
                    mark_val(*handler, &mut worklist, &mut live);
                }
                ObjectKind::WeakRef(target) => {
                    mark_val(*target, &mut worklist, &mut live);
                }
                ObjectKind::Function(f) => {
                    for uv in &f.upvalues {
                        mark_val(*uv, &mut worklist, &mut live);
                    }
                }
                ObjectKind::Promise(p) => {
                    mark_val(p.result, &mut worklist, &mut live);
                    for r in &p.fulfill_reactions {
                        mark_val(*r, &mut worklist, &mut live);
                    }
                    for r in &p.reject_reactions {
                        mark_val(*r, &mut worklist, &mut live);
                    }
                }
                ObjectKind::Generator(g) => {
                    for v in &g.stack {
                        mark_val(*v, &mut worklist, &mut live);
                    }
                    for uv in &g.function.upvalues {
                        mark_val(*uv, &mut worklist, &mut live);
                    }
                    mark_val(g.this_val, &mut worklist, &mut live);
                    if let Some(args) = g.arguments {
                        mark_val(args, &mut worklist, &mut live);
                    }
                }
                ObjectKind::Map(entries) => {
                    for (k, v) in entries {
                        mark_val(*k, &mut worklist, &mut live);
                        mark_val(*v, &mut worklist, &mut live);
                    }
                }
                ObjectKind::Set(entries) => {
                    for v in entries {
                        mark_val(*v, &mut worklist, &mut live);
                    }
                }
                ObjectKind::WeakMap(entries) => {
                    for (k, v) in entries {
                        mark_val(*k, &mut worklist, &mut live);
                        mark_val(*v, &mut worklist, &mut live);
                    }
                }
                ObjectKind::WeakSet(entries) => {
                    for v in entries {
                        mark_val(*v, &mut worklist, &mut live);
                    }
                }
                ObjectKind::FinalizationRegistry { callback, entries } => {
                    mark_val(*callback, &mut worklist, &mut live);
                    for (t, h, unreg) in entries {
                        mark_val(*t, &mut worklist, &mut live);
                        mark_val(*h, &mut worklist, &mut live);
                        if let Some(u) = unreg {
                            mark_val(*u, &mut worklist, &mut live);
                        }
                    }
                }
                _ => {}
            }
        }

        // 3. Sweep dead objects
        let before_count = self.objects.len();
        let mut i = 0;
        while i < self.objects.len() {
            let ptr = self.objects[i];
            if !live.contains(&ptr) {
                unsafe {
                    drop(Box::from_raw(ptr));
                }
                self.objects.swap_remove(i);
            } else {
                i += 1;
            }
        }

        before_count - self.objects.len()
    }

    pub fn alloc_object(&mut self) -> JsValue {
        let obj = Box::new(JsObject::new_ordinary(ShapeRegistry::ROOT_SHAPE_ID, self.object_proto));
        let ptr = Box::into_raw(obj);
        self.track_ptr(ptr);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }

    pub fn alloc_array(&mut self, elements: Vec<JsValue>) -> JsValue {
        let obj = Box::new(JsObject::new_array(ShapeRegistry::ROOT_SHAPE_ID, self.array_proto.or(self.object_proto), elements));
        let ptr = Box::into_raw(obj);
        self.track_ptr(ptr);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }

    pub fn alloc_string(&mut self, s: String) -> JsValue {
        let obj = Box::new(JsObject::new_string(ShapeRegistry::ROOT_SHAPE_ID, self.string_proto, s));
        let ptr = Box::into_raw(obj);
        self.track_ptr(ptr);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }

    pub fn alloc_proxy(&mut self, target: JsValue, handler: JsValue) -> JsValue {
        let proto = if let Some(ptr) = target.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            obj.prototype
        } else {
            None
        };
        let obj = Box::new(JsObject::new_proxy(ShapeRegistry::ROOT_SHAPE_ID, proto, target, handler));
        let ptr = Box::into_raw(obj);
        self.track_ptr(ptr);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }

    pub fn alloc_string_object(&mut self, s: String, proto: Option<JsValue>) -> JsValue {
        let obj = Box::new(JsObject::new_string_object(ShapeRegistry::ROOT_SHAPE_ID, proto, s));
        let ptr = Box::into_raw(obj);
        self.track_ptr(ptr);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }

    pub fn alloc_number_object(&mut self, n: f64, proto: Option<JsValue>) -> JsValue {
        let obj = Box::new(JsObject::new_number_object(ShapeRegistry::ROOT_SHAPE_ID, proto, n));
        let ptr = Box::into_raw(obj);
        self.track_ptr(ptr);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }

    pub fn alloc_boolean_object(&mut self, b: bool, proto: Option<JsValue>) -> JsValue {
        let obj = Box::new(JsObject::new_boolean_object(ShapeRegistry::ROOT_SHAPE_ID, proto, b));
        let ptr = Box::into_raw(obj);
        self.track_ptr(ptr);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }

    pub fn alloc_native_function(&mut self, name: &str, length: u8, func: NativeFn) -> JsValue {
        let native = NativeFunction::new_fn(name.to_string(), length, func);
        let proto = self.function_proto.or(self.object_proto);
        let obj = Box::new(JsObject::new_native_function(ShapeRegistry::ROOT_SHAPE_ID, proto, native));
        let ptr = Box::into_raw(obj);
        self.track_ptr(ptr);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }

    pub fn alloc_closure_native_function(&mut self, name: &str, length: u8, func: crate::object::ClosureNativeFn) -> JsValue {
        let native = NativeFunction::new_closure(name.to_string(), length, func);
        let proto = self.function_proto.or(self.object_proto);
        let obj = Box::new(JsObject::new_native_function(ShapeRegistry::ROOT_SHAPE_ID, proto, native));
        let ptr = Box::into_raw(obj);
        self.track_ptr(ptr);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }

    pub fn alloc_function(&mut self, func: JsFunction) -> JsValue {
        let fn_proto = self.function_proto.or(self.object_proto);
        let len = func.params.iter().take_while(|p| !p.starts_with("...")).count();
        let name_str = func.name.as_deref().unwrap_or("").to_string();
        let obj = Box::new(JsObject::new_function(ShapeRegistry::ROOT_SHAPE_ID, fn_proto, func));
        let ptr = Box::into_raw(obj);
        self.track_ptr(ptr);
        let fn_val = unsafe { JsValue::from_object_ptr(ptr as *mut u8) };

        let len_val = JsValue::from_int32(len as i32);
        self.set_own_property(fn_val, "length", len_val);
        self.non_enumerable_props.insert((fn_val, "length".to_string()));

        let name_val = self.alloc_string(name_str);
        self.set_own_property(fn_val, "name", name_val);
        self.non_enumerable_props.insert((fn_val, "name".to_string()));

        let proto = self.alloc_object();
        self.set_own_property(proto, "constructor", fn_val);
        self.non_enumerable_props.insert((proto, "constructor".to_string()));
        self.set_own_property(fn_val, "prototype", proto);
        self.non_enumerable_props.insert((fn_val, "prototype".to_string()));

        fn_val
    }

    pub fn alloc_promise(&mut self, state: PromiseState, result: JsValue) -> JsValue {
        let proto = self.promise_proto.or(self.object_proto);
        let obj = Box::new(JsObject::new_promise(ShapeRegistry::ROOT_SHAPE_ID, proto, state, result));
        let ptr = Box::into_raw(obj);
        self.track_ptr(ptr);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }

    pub fn alloc_arguments(&mut self, callee: JsValue, args: Vec<JsValue>, is_strict: bool) -> JsValue {
        let args_obj = self.alloc_object();
        let len = args.len();
        for (i, val) in args.into_iter().enumerate() {
            self.set_own_property(args_obj, &i.to_string(), val);
        }
        self.set_own_property(args_obj, "length", JsValue::from_int32(len as i32));
        self.non_enumerable_props.insert((args_obj, "length".to_string()));

        if let Some(sym_tag) = self.symbol_to_string_tag {
            let tag_val = self.alloc_string("Arguments".to_string());
            self.set_symbol_property(args_obj, sym_tag, tag_val);
            self.non_enumerable_props.insert((args_obj, "[Symbol.toStringTag]".to_string()));
        }
        if let Some(sym_iter) = self.symbol_iterator {
            if let Some(arr_proto) = self.array_proto {
                let iter_fn = self.get_property(arr_proto, "values");
                if iter_fn.is_object() {
                    self.set_symbol_property(args_obj, sym_iter, iter_fn);
                }
            }
        }

        if !is_strict {
            self.set_own_property(args_obj, "callee", callee);
            self.non_enumerable_props.insert((args_obj, "callee".to_string()));
        } else {
            let thrower = self.alloc_native_function("thrower", 0, |_vm, _this, _args| {
                Err(VmError::TypeError("'arguments.callee' cannot be accessed in strict mode".into()))
            });
            if let Some(ptr) = args_obj.as_object_ptr() {
                let obj = unsafe { &mut *(ptr as *mut JsObject) };
                obj.define_getter("callee".into(), thrower);
                obj.define_setter("callee".into(), thrower);
            }
            self.non_configurable_props.insert((args_obj, "callee".to_string()));
            self.non_enumerable_props.insert((args_obj, "callee".to_string()));
        }
        args_obj
    }

    pub fn alloc_generator(&mut self, generator: JsGenerator) -> JsValue {
        let obj = Box::new(JsObject::new_generator(ShapeRegistry::ROOT_SHAPE_ID, None, generator));
        let ptr = Box::into_raw(obj);
        self.track_ptr(ptr);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }

    pub fn alloc_shared_array_buffer(&mut self, sab: Arc<doms_parallel::SharedArrayBuffer>) -> JsValue {
        let obj = Box::new(JsObject::new_shared_array_buffer(ShapeRegistry::ROOT_SHAPE_ID, None, sab));
        let ptr = Box::into_raw(obj);
        self.track_ptr(ptr);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }

    pub fn alloc_typed_array(&mut self, typed_array: JsTypedArray) -> JsValue {
        let proto = self.globals.get(typed_array.kind.name()).copied().map(|ctor| self.get_property(ctor, "prototype"));
        let obj = Box::new(JsObject::new_typed_array(ShapeRegistry::ROOT_SHAPE_ID, proto, typed_array));
        let ptr = Box::into_raw(obj);
        self.track_ptr(ptr);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }

    pub fn alloc_array_buffer(&mut self, buffer: Arc<doms_parallel::SharedArrayBuffer>) -> JsValue {
        let proto = self.globals.get("ArrayBuffer").copied().map(|ctor| self.get_property(ctor, "prototype"));
        let obj = Box::new(JsObject::new_array_buffer(ShapeRegistry::ROOT_SHAPE_ID, proto, buffer));
        let ptr = Box::into_raw(obj);
        self.track_ptr(ptr);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }

    pub fn alloc_data_view(&mut self, buffer: Arc<doms_parallel::SharedArrayBuffer>, byte_offset: usize, byte_length: usize) -> JsValue {
        let proto = self.globals.get("DataView").copied().map(|ctor| self.get_property(ctor, "prototype"));
        let obj = Box::new(JsObject::new_data_view(ShapeRegistry::ROOT_SHAPE_ID, proto, buffer, byte_offset, byte_length));
        let ptr = Box::into_raw(obj);
        self.track_ptr(ptr);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }

    pub fn alloc_iterator(&mut self, items: Vec<JsValue>) -> JsValue {
        let iter_obj = self.alloc_object();
        let idx = Arc::new(std::sync::Mutex::new(0usize));
        let items_arc = Arc::new(items);
        let next_fn = self.alloc_closure_native_function("next", 0, Arc::new({
            let idx = Arc::clone(&idx);
            let items = Arc::clone(&items_arc);
            move |vm: &mut Vm, _this: JsValue, _args: &[JsValue]| -> Result<JsValue, VmError> {
                let mut i = idx.lock().unwrap();
                let res = vm.alloc_object();
                if *i < items.len() {
                    let val = items[*i];
                    *i += 1;
                    vm.set_property(res, "value", val);
                    vm.set_property(res, "done", JsValue::from_boolean(false));
                } else {
                    vm.set_property(res, "value", JsValue::undefined());
                    vm.set_property(res, "done", JsValue::from_boolean(true));
                }
                Ok(res)
            }
        }));
        self.set_property(iter_obj, "next", next_fn);
        if let Some(sym_iter) = self.symbol_iterator {
            let self_val = iter_obj;
            let self_fn = self.alloc_closure_native_function("[Symbol.iterator]", 0, Arc::new(move |_vm, _this, _args| Ok(self_val)));
            self.set_symbol_property(iter_obj, sym_iter, self_fn);
        }
        iter_obj
    }

    pub fn read_typed_array_element(ta: &JsTypedArray, index: usize) -> Option<JsValue> {
        if index >= ta.length {
            return None;
        }
        let bpe = ta.kind.bytes_per_element();
        let off = ta.byte_offset + index * bpe;
        if off + bpe > ta.buffer.byte_length() {
            return None;
        }
        match ta.kind {
            TypedArrayKind::Int8 => {
                let b = ta.buffer.get(off)? as i8;
                Some(JsValue::from_int32(b as i32))
            }
            TypedArrayKind::Uint8 | TypedArrayKind::Uint8Clamped => {
                let b = ta.buffer.get(off)?;
                Some(JsValue::from_int32(b as i32))
            }
            TypedArrayKind::Int16 => {
                let b0 = ta.buffer.get(off)?;
                let b1 = ta.buffer.get(off + 1)?;
                let val = i16::from_le_bytes([b0, b1]);
                Some(JsValue::from_int32(val as i32))
            }
            TypedArrayKind::Uint16 => {
                let b0 = ta.buffer.get(off)?;
                let b1 = ta.buffer.get(off + 1)?;
                let val = u16::from_le_bytes([b0, b1]);
                Some(JsValue::from_int32(val as i32))
            }
            TypedArrayKind::Int32 => {
                let b0 = ta.buffer.get(off)?;
                let b1 = ta.buffer.get(off + 1)?;
                let b2 = ta.buffer.get(off + 2)?;
                let b3 = ta.buffer.get(off + 3)?;
                let val = i32::from_le_bytes([b0, b1, b2, b3]);
                Some(JsValue::from_int32(val))
            }
            TypedArrayKind::Uint32 => {
                let b0 = ta.buffer.get(off)?;
                let b1 = ta.buffer.get(off + 1)?;
                let b2 = ta.buffer.get(off + 2)?;
                let b3 = ta.buffer.get(off + 3)?;
                let val = u32::from_le_bytes([b0, b1, b2, b3]);
                Some(JsValue::from_double(val as f64))
            }
            TypedArrayKind::Float32 => {
                let b0 = ta.buffer.get(off)?;
                let b1 = ta.buffer.get(off + 1)?;
                let b2 = ta.buffer.get(off + 2)?;
                let b3 = ta.buffer.get(off + 3)?;
                let val = f32::from_le_bytes([b0, b1, b2, b3]);
                Some(JsValue::from_double(val as f64))
            }
            TypedArrayKind::Float64 => {
                let mut bytes = [0u8; 8];
                for i in 0..8 {
                    bytes[i] = ta.buffer.get(off + i)?;
                }
                let val = f64::from_le_bytes(bytes);
                Some(JsValue::from_double(val))
            }
            TypedArrayKind::BigInt64 => {
                let mut bytes = [0u8; 8];
                for i in 0..8 {
                    bytes[i] = ta.buffer.get(off + i)?;
                }
                let val = i64::from_le_bytes(bytes);
                Some(JsValue::from_double(val as f64))
            }
            TypedArrayKind::BigUint64 => {
                let mut bytes = [0u8; 8];
                for i in 0..8 {
                    bytes[i] = ta.buffer.get(off + i)?;
                }
                let val = u64::from_le_bytes(bytes);
                Some(JsValue::from_double(val as f64))
            }
        }
    }

    pub fn write_typed_array_element(ta: &mut JsTypedArray, index: usize, value: JsValue) -> bool {
        if index >= ta.length {
            return false;
        }
        let bpe = ta.kind.bytes_per_element();
        let off = ta.byte_offset + index * bpe;
        if off + bpe > ta.buffer.byte_length() {
            return false;
        }
        let num = value.to_number().unwrap_or(0.0);
        match ta.kind {
            TypedArrayKind::Int8 => {
                let b = (num as i32 as i8) as u8;
                ta.buffer.set(off, b);
            }
            TypedArrayKind::Uint8 => {
                let b = num as i32 as u8;
                ta.buffer.set(off, b);
            }
            TypedArrayKind::Uint8Clamped => {
                let clamped = if num.is_nan() || num <= 0.0 {
                    0u8
                } else if num >= 255.0 {
                    255u8
                } else {
                    let floor = num.floor();
                    let diff = num - floor;
                    if diff < 0.5 {
                        floor as u8
                    } else if diff > 0.5 {
                        (floor + 1.0) as u8
                    } else if (floor as u64) % 2 == 0 {
                        floor as u8
                    } else {
                        (floor + 1.0) as u8
                    }
                };
                ta.buffer.set(off, clamped);
            }
            TypedArrayKind::Int16 => {
                let bytes = (num as i32 as i16).to_le_bytes();
                ta.buffer.set(off, bytes[0]);
                ta.buffer.set(off + 1, bytes[1]);
            }
            TypedArrayKind::Uint16 => {
                let bytes = (num as u32 as u16).to_le_bytes();
                ta.buffer.set(off, bytes[0]);
                ta.buffer.set(off + 1, bytes[1]);
            }
            TypedArrayKind::Int32 => {
                let bytes = (num as i32).to_le_bytes();
                for (i, &b) in bytes.iter().enumerate() {
                    ta.buffer.set(off + i, b);
                }
            }
            TypedArrayKind::Uint32 => {
                let bytes = (num as u32).to_le_bytes();
                for (i, &b) in bytes.iter().enumerate() {
                    ta.buffer.set(off + i, b);
                }
            }
            TypedArrayKind::Float32 => {
                let bytes = (num as f32).to_le_bytes();
                for (i, &b) in bytes.iter().enumerate() {
                    ta.buffer.set(off + i, b);
                }
            }
            TypedArrayKind::Float64 => {
                let bytes = num.to_le_bytes();
                for (i, &b) in bytes.iter().enumerate() {
                    ta.buffer.set(off + i, b);
                }
            }
            TypedArrayKind::BigInt64 => {
                let bytes = (num as i64).to_le_bytes();
                for (i, &b) in bytes.iter().enumerate() {
                    ta.buffer.set(off + i, b);
                }
            }
            TypedArrayKind::BigUint64 => {
                let bytes = (num as u64).to_le_bytes();
                for (i, &b) in bytes.iter().enumerate() {
                    ta.buffer.set(off + i, b);
                }
            }
        }
        true
    }

    pub fn alloc_worker(&mut self, port: Arc<std::sync::Mutex<doms_parallel::MessagePort>>) -> JsValue {
        let obj = Box::new(JsObject::new_worker(ShapeRegistry::ROOT_SHAPE_ID, None, port));
        let ptr = Box::into_raw(obj);
        self.track_ptr(ptr);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }

    pub fn alloc_map(&mut self) -> JsValue {
        let proto = self.globals.get("Map").copied().and_then(|c| {
            let p = self.get_property(c, "prototype");
            if p.is_object() { Some(p) } else { None }
        }).or(self.object_proto);
        let obj = Box::new(JsObject::new_map(ShapeRegistry::ROOT_SHAPE_ID, proto));
        let ptr = Box::into_raw(obj);
        self.track_ptr(ptr);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }

    pub fn alloc_set(&mut self) -> JsValue {
        let proto = self.globals.get("Set").copied().and_then(|c| {
            let p = self.get_property(c, "prototype");
            if p.is_object() { Some(p) } else { None }
        }).or(self.object_proto);
        let obj = Box::new(JsObject::new_set(ShapeRegistry::ROOT_SHAPE_ID, proto));
        let ptr = Box::into_raw(obj);
        self.track_ptr(ptr);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }

    pub fn alloc_weak_map(&mut self) -> JsValue {
        let proto = self.globals.get("WeakMap").copied().and_then(|c| {
            let p = self.get_property(c, "prototype");
            if p.is_object() { Some(p) } else { None }
        }).or(self.object_proto);
        let obj = Box::new(JsObject::new_weak_map(ShapeRegistry::ROOT_SHAPE_ID, proto));
        let ptr = Box::into_raw(obj);
        self.track_ptr(ptr);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }

    pub fn alloc_weak_set(&mut self) -> JsValue {
        let proto = self.globals.get("WeakSet").copied().and_then(|c| {
            let p = self.get_property(c, "prototype");
            if p.is_object() { Some(p) } else { None }
        }).or(self.object_proto);
        let obj = Box::new(JsObject::new_weak_set(ShapeRegistry::ROOT_SHAPE_ID, proto));
        let ptr = Box::into_raw(obj);
        self.track_ptr(ptr);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }

    pub fn alloc_bigint(&mut self, val: BigInt) -> JsValue {
        let proto = self.globals.get("BigInt").copied().and_then(|c| {
            let p = self.get_property(c, "prototype");
            if p.is_object() { Some(p) } else { None }
        }).or(self.object_proto);
        let obj = Box::new(JsObject::new_bigint(ShapeRegistry::ROOT_SHAPE_ID, proto, val));
        let ptr = Box::into_raw(obj);
        self.track_ptr(ptr);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }

    pub fn alloc_bigint_object(&mut self, val: BigInt) -> JsValue {
        let proto = self.globals.get("BigInt").copied().and_then(|c| {
            let p = self.get_property(c, "prototype");
            if p.is_object() { Some(p) } else { None }
        }).or(self.object_proto);
        let obj = Box::new(JsObject::new_with_kind(ShapeRegistry::ROOT_SHAPE_ID, proto, ObjectKind::BigIntObject(val)));
        let ptr = Box::into_raw(obj);
        self.track_ptr(ptr);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }

    pub fn alloc_weak_ref(&mut self, target: JsValue) -> JsValue {
        let obj = Box::new(JsObject::new_weak_ref(ShapeRegistry::ROOT_SHAPE_ID, None, target));
        let ptr = Box::into_raw(obj);
        self.track_ptr(ptr);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }

    pub fn alloc_finalization_registry(&mut self, callback: JsValue) -> JsValue {
        let obj = Box::new(JsObject::new_finalization_registry(ShapeRegistry::ROOT_SHAPE_ID, None, callback));
        let ptr = Box::into_raw(obj);
        self.track_ptr(ptr);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }

    pub fn alloc_regexp(&mut self, pattern: String, flags: String) -> JsValue {
        let proto = self.regexp_proto.or(self.object_proto);
        let obj = Box::new(JsObject::new_regexp(ShapeRegistry::ROOT_SHAPE_ID, proto, pattern, flags));
        let ptr = Box::into_raw(obj);
        self.track_ptr(ptr);
        let val = unsafe { JsValue::from_object_ptr(ptr as *mut u8) };
        self.set_property(val, "lastIndex", JsValue::from_int32(0));
        self.non_enumerable_props.insert((val, "lastIndex".to_string()));
        self.non_configurable_props.insert((val, "lastIndex".to_string()));
        val
    }

    pub fn alloc_date(&mut self, proto: Option<JsValue>, time_ms: f64) -> JsValue {
        let obj = Box::new(JsObject::new_date(ShapeRegistry::ROOT_SHAPE_ID, proto, time_ms));
        let ptr = Box::into_raw(obj);
        self.track_ptr(ptr);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }

    pub fn alloc_symbol(&mut self, desc: Option<String>) -> JsValue {
        let obj = Box::new(JsObject::new_symbol(ShapeRegistry::ROOT_SHAPE_ID, self.symbol_proto, desc));
        let ptr = Box::into_raw(obj);
        self.track_ptr(ptr);
        unsafe { JsValue::from_symbol_ptr(ptr as *mut u8) }
    }

    pub fn to_object_value(&mut self, val: JsValue) -> Result<JsValue, VmError> {
        if val.is_nullish() {
            return Err(VmError::TypeError("Cannot convert undefined or null to object".into()));
        }
        if let Some(bi) = self.as_bigint_val(val) {
            return Ok(self.alloc_bigint_object(bi));
        }
        if let Some(ptr) = val.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            match &obj.kind {
                ObjectKind::String(s) => {
                    let s_clone = s.clone();
                    return Ok(self.alloc_string_object(s_clone, self.string_proto));
                }
                _ => return Ok(val),
            }
        }
        if val.is_string() {
            let s = self.to_string_value(val);
            return Ok(self.alloc_string_object(s, self.string_proto));
        }
        if val.is_number() {
            let n = val.to_number().unwrap_or(0.0);
            return Ok(self.alloc_number_object(n, self.number_proto));
        }
        if val.is_boolean() {
            let b = val.as_boolean().unwrap_or(false);
            return Ok(self.alloc_boolean_object(b, self.boolean_proto));
        }
        if val.is_symbol() {
            let desc = if let Some(ptr) = val.as_symbol_ptr() {
                let obj = unsafe { &*(ptr as *mut JsObject) };
                if let ObjectKind::Symbol(d) = &obj.kind {
                    d.clone()
                } else {
                    None
                }
            } else {
                None
            };
            let proto = self.symbol_proto.or(self.object_proto);
            let obj = Box::new(JsObject::new_symbol(ShapeRegistry::ROOT_SHAPE_ID, proto, desc));
            let ptr = Box::into_raw(obj);
            self.track_ptr(ptr);
            return Ok(unsafe { JsValue::from_object_ptr(ptr as *mut u8) });
        }
        Ok(val)
    }

    pub fn as_bigint_val(&self, val: JsValue) -> Option<BigInt> {
        if let Some(ptr) = val.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::BigInt(bi) = &obj.kind {
                return Some(bi.clone());
            }
        }
        None
    }

    pub fn alloc_error_opt(&mut self, name: String, message: Option<String>) -> JsValue {
        let ctor = self.globals.get(&name).copied().unwrap_or(JsValue::undefined());
        let proto = if ctor.is_object() {
            let p = self.get_property(ctor, "prototype");
            if p.is_object() { Some(p) } else { None }
        } else {
            None
        };
        let msg_str = message.clone().unwrap_or_default();
        let obj = Box::new(JsObject::new_error(ShapeRegistry::ROOT_SHAPE_ID, proto, name.clone(), msg_str));
        let ptr = Box::into_raw(obj);
        self.track_ptr(ptr);
        let val = unsafe { JsValue::from_object_ptr(ptr as *mut u8) };
        if let Some(msg) = message {
            let msg_val = self.alloc_string(msg);
            self.set_property(val, "message", msg_val);
            self.non_enumerable_props.insert((val, "message".to_string()));
        }
        val
    }

    pub fn alloc_error(&mut self, name: String, message: String) -> JsValue {
        self.alloc_error_opt(name, Some(message))
    }

    pub fn vm_error_to_js_value(&mut self, err: &VmError) -> JsValue {
        match err {
            VmError::Thrown(val) => *val,
            VmError::TypeError(msg) => self.alloc_error("TypeError".into(), msg.clone()),
            VmError::ReferenceError(msg) => self.alloc_error("ReferenceError".into(), msg.clone()),
            VmError::RangeError(msg) => self.alloc_error("RangeError".into(), msg.clone()),
            VmError::SyntaxError(msg) => self.alloc_error("SyntaxError".into(), msg.clone()),
            VmError::UriError(msg) => self.alloc_error("URIError".into(), msg.clone()),
            VmError::RuntimeError(msg) | VmError::UndefinedGlobal(msg) => {
                self.alloc_error("Error".into(), msg.clone())
            }
            VmError::StackUnderflow | VmError::InvalidInstruction(_) => {
                self.alloc_error("Error".into(), err.to_string())
            }
        }
    }

    pub fn handle_exception_at_depth(&mut self, err: VmError, min_frame_idx: usize) -> Result<(), VmError> {
        if let VmError::RuntimeError(ref msg) = err {
            if msg.contains("Execution step limit exceeded") {
                return Err(err);
            }
        }
        if let Some(handler) = self.catch_stack.last() {
            if handler.frame_idx >= min_frame_idx {
                let handler = self.catch_stack.pop().unwrap();
                let err_val = self.vm_error_to_js_value(&err);
                self.frames.truncate(handler.frame_idx + 1);
                self.stack.truncate(handler.stack_len);
                self.push(err_val);
                if let Some(frame) = self.frames.last_mut() {
                    frame.scopes.truncate(handler.scopes_len);
                    frame.ip = handler.target_ip;
                }
                return Ok(());
            }
        }
        Err(err)
    }

    pub fn handle_exception(&mut self, err: VmError) -> Result<(), VmError> {
        self.handle_exception_at_depth(err, 0)
    }

    pub fn get_element(&mut self, obj: JsValue, index: usize) -> JsValue {
        if let Some(ptr) = obj.as_object_ptr() {
            let jsobj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::Array(elements) = &jsobj.kind {
                if index < elements.len() {
                    return elements[index];
                }
            }
            if let ObjectKind::TypedArray(ta) = &jsobj.kind {
                if let Some(v) = Self::read_typed_array_element(ta, index) {
                    return v;
                }
            }
        }
        let key = index.to_string();
        self.get_property(obj, &key)
    }

    pub fn set_element(&mut self, obj: JsValue, index: usize, val: JsValue) {
        if let Some(ptr) = obj.as_object_ptr() {
            let jsobj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::Array(elements) = &mut jsobj.kind {
                if index < elements.len() {
                    elements[index] = val;
                    return;
                }
            }
            if let ObjectKind::TypedArray(ta) = &mut jsobj.kind {
                Self::write_typed_array_element(ta, index, val);
                return;
            }
        }
        let key = index.to_string();
        self.set_property(obj, &key, val);
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

    pub fn get_prototype_of(&mut self, target: JsValue) -> Option<JsValue> {
        if let Some(ptr) = target.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            obj.prototype
        } else if target.is_string() {
            self.string_proto
        } else if target.is_number() {
            self.number_proto
        } else if target.is_boolean() {
            self.boolean_proto
        } else if target.is_symbol() {
            self.symbol_proto
        } else {
            None
        }
    }

    pub fn set_prototype(&mut self, target: JsValue, proto: JsValue) {
        if let Some(ptr) = target.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            obj.prototype = if proto.is_null() { None } else { Some(proto) };
        }
    }

    pub fn set_property_direct(&mut self, target: JsValue, key: &str, value: JsValue) {
        self.deleted_props.remove(&(target, key.to_string()));
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

    pub fn set_property(&mut self, target: JsValue, key: &str, value: JsValue) {
        self.deleted_props.remove(&(target, key.to_string()));
        if self.non_writable_props.contains(&(target, key.to_string())) {
            return;
        }
        if self.non_extensible_objects.contains(&target) {
            if let Some(ptr) = target.as_object_ptr() {
                let obj = unsafe { &*(ptr as *mut JsObject) };
                if !self.shapes.has_property(obj.shape_id, key) {
                    return;
                }
            }
        }
        if self.non_writable_props.contains(&(target, key.to_string())) {
            return;
        }
        if let Some(ptr) = target.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };

            if let ObjectKind::Array(elements) = &mut obj.kind {
                if key == "length" {
                    if let Some(new_len) = value.to_number().map(|n| n as usize) {
                        elements.truncate(new_len);
                        return;
                    }
                }
                if let Ok(idx) = key.parse::<usize>() {
                    if idx < elements.len() {
                        elements[idx] = value;
                    } else if idx < 100_000 {
                        elements.resize(idx + 1, JsValue::undefined());
                        elements[idx] = value;
                    }
                    return;
                }
            }

            if let ObjectKind::TypedArray(ta) = &mut obj.kind {
                if let Ok(idx) = key.parse::<usize>() {
                    Self::write_typed_array_element(ta, idx, value);
                    return;
                }
            }

            // 1. Check own accessor
            if let Some(accs) = &obj.accessors {
                if let Some(acc) = accs.get(key) {
                    if let Some(setter) = acc.setter {
                        let _ = self.call(setter, target, &[value]);
                    }
                    return;
                }
            }

            // 2. Check prototype chain accessor
            let has_own_data = self.shapes.has_property(obj.shape_id, key)
                || matches!(&obj.kind, ObjectKind::Array(elements) if key == "length" || key.parse::<usize>().map(|idx| idx < elements.len()).unwrap_or(false))
                || matches!(&obj.kind, ObjectKind::TypedArray(ta) if matches!(key, "length" | "byteLength" | "byteOffset" | "buffer") || key.parse::<usize>().map(|idx| idx < ta.length).unwrap_or(false))
                || matches!(&obj.kind, ObjectKind::String(_) | ObjectKind::StringObject(_) if key == "length" || key.parse::<usize>().is_ok());
            if !has_own_data {
                let mut curr_proto = obj.prototype;
                while let Some(proto_val) = curr_proto {
                    if let Some(proto_ptr) = proto_val.as_object_ptr() {
                        let proto_obj = unsafe { &*(proto_ptr as *mut JsObject) };
                        if let Some(accs) = &proto_obj.accessors {
                            if let Some(acc) = accs.get(key) {
                                if let Some(setter) = acc.setter {
                                    let _ = self.call(setter, target, &[value]);
                                }
                                return;
                            }
                        }
                        curr_proto = proto_obj.prototype;
                    } else {
                        break;
                    }
                }
            }

            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let Some(offset) = self.shapes.lookup_offset(obj.shape_id, key) {
                obj.set_slot(offset, value);
            } else {
                let (new_shape_id, offset) = self.shapes.transition(obj.shape_id, key);
                obj.shape_id = new_shape_id;
                obj.set_slot(offset, value);
            }
        }
        self.sync_mapped_arguments_write(target, key, value);
    }

    pub fn set_own_property(&mut self, target: JsValue, key: &str, value: JsValue) {
        self.deleted_props.remove(&(target, key.to_string()));
        if let Some(ptr) = target.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };

            if let ObjectKind::Array(elements) = &mut obj.kind {
                if key == "length" {
                    if let Some(new_len) = value.to_number().map(|n| n as usize) {
                        elements.truncate(new_len);
                        return;
                    }
                }
                if let Ok(idx) = key.parse::<usize>() {
                    if idx < elements.len() {
                        elements[idx] = value;
                    } else if idx < 100_000 {
                        elements.resize(idx + 1, JsValue::undefined());
                        elements[idx] = value;
                    }
                    return;
                }
            }

            if let ObjectKind::TypedArray(ta) = &mut obj.kind {
                if let Ok(idx) = key.parse::<usize>() {
                    Self::write_typed_array_element(ta, idx, value);
                    return;
                }
            }

            if let Some(accs) = &mut obj.accessors {
                accs.remove(key);
            }

            if let Some(offset) = self.shapes.lookup_offset(obj.shape_id, key) {
                obj.set_slot(offset, value);
            } else {
                let (new_shape_id, offset) = self.shapes.transition(obj.shape_id, key);
                obj.shape_id = new_shape_id;
                obj.set_slot(offset, value);
            }
        }
        self.sync_mapped_arguments_write(target, key, value);
    }

    pub fn sync_mapped_arguments_write(&mut self, target: JsValue, key: &str, val: JsValue) {
        if let Ok(idx) = key.parse::<usize>() {
            let mut env_to_sync = None;
            for frame in self.frames.iter_mut().rev() {
                if frame.arguments == Some(target) {
                    if let Some(mapped) = &frame.mapped_arguments {
                        if mapped.get(idx).copied().unwrap_or(false) {
                            if idx < frame.locals.len() {
                                frame.locals[idx] = val;
                            }
                            if let Some(env) = frame.env_obj {
                                if let Some(name) = frame.local_names.get(idx).cloned() {
                                    env_to_sync = Some((env, name));
                                }
                            }
                        }
                    }
                    break;
                }
            }
            if let Some((env, name)) = env_to_sync {
                self.set_property(env, &name, val);
            }
        }
    }

    pub fn sync_mapped_arguments_delete(&mut self, target: JsValue, key: &str) {
        if let Ok(idx) = key.parse::<usize>() {
            for frame in self.frames.iter_mut().rev() {
                if frame.arguments == Some(target) {
                    if let Some(mapped) = &mut frame.mapped_arguments {
                        if let Some(m) = mapped.get_mut(idx) {
                            *m = false;
                        }
                    }
                    break;
                }
            }
        }
    }

    pub fn has_own_property(&self, target: JsValue, key: &str) -> bool {
        self.has_own_property_key(target, &PropertyKeyVal::String(key.to_string()))
    }

    pub fn get_property_result(&mut self, target: JsValue, key: &str) -> Result<JsValue, VmError> {
        if target.is_null() || target.is_undefined() {
            return Err(VmError::TypeError(format!(
                "Cannot read properties of {} (reading '{}')",
                if target.is_null() { "null" } else { "undefined" },
                key
            )));
        }
        if let Some(ptr) = target.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };

            // 1. Check own accessor
            if let Some(accs) = &obj.accessors {
                if let Some(acc) = accs.get(key) {
                    if let Some(getter) = acc.getter {
                        return self.call(getter, target, &[]);
                    } else {
                        return Ok(JsValue::undefined());
                    }
                }
            }

            // 2. Check prototype chain accessor only if object does NOT have own data property
            let has_own_data = self.shapes.has_property(obj.shape_id, key)
                || matches!(&obj.kind, ObjectKind::Array(elements) if key == "length" || key.parse::<usize>().map(|idx| idx < elements.len()).unwrap_or(false))
                || matches!(&obj.kind, ObjectKind::TypedArray(ta) if matches!(key, "length" | "byteLength" | "byteOffset" | "buffer") || key.parse::<usize>().map(|idx| idx < ta.length).unwrap_or(false))
                || matches!(&obj.kind, ObjectKind::String(_) | ObjectKind::StringObject(_) if key == "length" || key.parse::<usize>().is_ok());
            if !has_own_data {
                let mut curr_proto = obj.prototype;
                while let Some(proto_val) = curr_proto {
                    if let Some(proto_ptr) = proto_val.as_object_ptr() {
                        let proto_obj = unsafe { &*(proto_ptr as *mut JsObject) };
                        if let Some(accs) = &proto_obj.accessors {
                            if let Some(acc) = accs.get(key) {
                                if let Some(getter) = acc.getter {
                                    return self.call(getter, target, &[]);
                                } else {
                                    return Ok(JsValue::undefined());
                                }
                            }
                        }
                        curr_proto = proto_obj.prototype;
                    } else {
                        break;
                    }
                }
            }
        }
        if self.module_namespace_objects.contains(&target) {
            let val = self.get_property(target, key);
            if val == JsValue::uninitialized() {
                return Err(VmError::ReferenceError(format!("Cannot access '{}' before initialization", key)));
            }
            return Ok(val);
        }
        Ok(self.get_property(target, key))
    }

    pub fn set_property_result(&mut self, target: JsValue, key: &str, value: JsValue) -> Result<(), VmError> {
        if target.is_null() || target.is_undefined() {
            return Err(VmError::TypeError(format!(
                "Cannot set properties of {} (setting '{}')",
                if target.is_null() { "null" } else { "undefined" },
                key
            )));
        }
        if self.module_namespace_objects.contains(&target) {
            let is_strict = self.frames.last().map(|f| f.is_strict).unwrap_or(self.is_strict);
            if is_strict {
                return Err(VmError::TypeError(format!("Cannot assign to read only property '{}'", key)));
            }
            return Ok(());
        }
        let is_func = target.as_object_ptr().is_some_and(|p| matches!(unsafe { &(*(p as *mut JsObject)).kind }, ObjectKind::Function(_) | ObjectKind::NativeFunction(_)));
        if is_func && (key == "length" || key == "name") {
            let is_strict = self.frames.last().map(|f| f.is_strict).unwrap_or(self.is_strict);
            if is_strict {
                return Err(VmError::TypeError(format!("Cannot assign to read only property '{}'", key)));
            }
            return Ok(());
        }
        if self.non_writable_props.contains(&(target, key.to_string())) {
            let is_strict = self.frames.last().map(|f| f.is_strict).unwrap_or(self.is_strict);
            if is_strict {
                return Err(VmError::TypeError(format!("Cannot assign to read only property '{}'", key)));
            }
            return Ok(());
        }
        let is_prim_str = target.as_object_ptr().is_some_and(|p| matches!(unsafe { &(*(p as *mut JsObject)).kind }, ObjectKind::String(_)));
        let is_prim_sym = target.as_object_ptr().is_some_and(|p| matches!(unsafe { &(*(p as *mut JsObject)).kind }, ObjectKind::Symbol(_)));
        if !target.is_object() || is_prim_str || is_prim_sym {
            let proto = if target.is_number() {
                self.number_proto
            } else if target.is_string() || is_prim_str {
                self.string_proto
            } else if target.is_boolean() {
                self.boolean_proto
            } else if target.is_symbol() || is_prim_sym {
                self.symbol_proto
            } else {
                None
            };
            if let Some(proto_val) = proto {
                let mut curr_proto = Some(proto_val);
                while let Some(pv) = curr_proto {
                    if let Some(ptr) = pv.as_object_ptr() {
                        let pobj = unsafe { &*(ptr as *mut JsObject) };
                        if let ObjectKind::Proxy { target: tgt, handler: hdl } = &pobj.kind {
                            let tgt = *tgt;
                            let hdl = *hdl;
                            let trap = self.get_property(hdl, "set");
                            if self.is_callable(trap) {
                                let key_val = self.alloc_string(key.to_string());
                                self.call(trap, hdl, &[tgt, key_val, value, target])?;
                                return Ok(());
                            }
                        }
                        if let Some(accs) = &pobj.accessors {
                            if let Some(acc) = accs.get(key) {
                                if let Some(setter) = acc.setter {
                                    self.call(setter, target, &[value])?;
                                } else {
                                    let is_strict = self.frames.last().map(|f| f.is_strict).unwrap_or(self.is_strict);
                                    if is_strict {
                                        return Err(VmError::TypeError(format!("Cannot set property '{}' of object which has only a getter", key)));
                                    }
                                }
                                return Ok(());
                            }
                        }
                        curr_proto = pobj.prototype;
                    } else {
                        break;
                    }
                }
            }
            let is_strict = self.frames.last().map(|f| f.is_strict).unwrap_or(self.is_strict);
            if is_strict {
                return Err(VmError::TypeError(format!("Cannot create property '{}' on primitive", key)));
            }
            return Ok(());
        }
        if let Some(ptr) = target.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };

            if let ObjectKind::Proxy { target: tgt, handler: hdl } = &obj.kind {
                let tgt = *tgt;
                let hdl = *hdl;
                let trap = self.get_property(hdl, "set");
                if self.is_callable(trap) {
                    let key_val = self.alloc_string(key.to_string());
                    self.call(trap, hdl, &[tgt, key_val, value, target])?;
                    return Ok(());
                } else {
                    return self.set_property_result(tgt, key, value);
                }
            }

            // 1. Check own accessor
            if let Some(accs) = &obj.accessors {
                if let Some(acc) = accs.get(key) {
                    if let Some(setter) = acc.setter {
                        self.call(setter, target, &[value])?;
                    } else {
                        let is_strict = self.frames.last().map(|f| f.is_strict).unwrap_or(self.is_strict);
                        if is_strict {
                            return Err(VmError::TypeError(format!("Cannot set property '{}' of object which has only a getter", key)));
                        }
                    }
                    return Ok(());
                }
            }

            if self.non_writable_props.contains(&(target, key.to_string())) {
                let is_strict = self.frames.last().map(|f| f.is_strict).unwrap_or(self.is_strict);
                if is_strict {
                    return Err(VmError::TypeError(format!("Cannot assign to read only property '{}'", key)));
                }
                return Ok(());
            }

            // 2. Check prototype chain accessor
            let has_own_data = self.shapes.has_property(obj.shape_id, key)
                || matches!(&obj.kind, ObjectKind::Array(elements) if key == "length" || key.parse::<usize>().map(|idx| idx < elements.len()).unwrap_or(false))
                || matches!(&obj.kind, ObjectKind::TypedArray(ta) if matches!(key, "length" | "byteLength" | "byteOffset" | "buffer") || key.parse::<usize>().map(|idx| idx < ta.length).unwrap_or(false))
                || matches!(&obj.kind, ObjectKind::String(_) | ObjectKind::StringObject(_) if key == "length" || key.parse::<usize>().is_ok());
            if !has_own_data {
                let mut curr_proto = obj.prototype;
                while let Some(proto_val) = curr_proto {
                    if let Some(proto_ptr) = proto_val.as_object_ptr() {
                        let proto_obj = unsafe { &*(proto_ptr as *mut JsObject) };
                        if let ObjectKind::Proxy { target: tgt, handler: hdl } = &proto_obj.kind {
                            let tgt = *tgt;
                            let hdl = *hdl;
                            let trap = self.get_property(hdl, "set");
                            if self.is_callable(trap) {
                                let key_val = self.alloc_string(key.to_string());
                                self.call(trap, hdl, &[tgt, key_val, value, target])?;
                                return Ok(());
                            }
                        }
                        if let Some(accs) = &proto_obj.accessors {
                            if let Some(acc) = accs.get(key) {
                                if let Some(setter) = acc.setter {
                                    self.call(setter, target, &[value])?;
                                } else {
                                    let is_strict = self.frames.last().map(|f| f.is_strict).unwrap_or(self.is_strict);
                                    if is_strict {
                                        return Err(VmError::TypeError(format!("Cannot set property '{}' of object which has only a getter", key)));
                                    }
                                }
                                return Ok(());
                            }
                        }
                        curr_proto = proto_obj.prototype;
                    } else {
                        break;
                    }
                }
            }

            if self.non_extensible_objects.contains(&target) && !self.shapes.has_property(obj.shape_id, key) {
                let is_strict = self.frames.last().map(|f| f.is_strict).unwrap_or(self.is_strict);
                if is_strict {
                    return Err(VmError::TypeError(format!("Cannot add property '{}', object is not extensible", key)));
                }
                return Ok(());
            }
        }
        self.set_property(target, key, value);
        Ok(())
    }

    pub fn delete_property(&mut self, target: JsValue, key: &str) -> Result<bool, VmError> {
        if let Some(ptr) = target.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if self.non_configurable_props.contains(&(target, key.to_string())) {
                return Ok(false);
            }
            if let Some(accs) = &mut obj.accessors {
                accs.remove(key);
            }
            let new_shape_id = self.shapes.remove_property(obj.shape_id, key);
            obj.shape_id = new_shape_id;
            if target == self.global_object {
                self.globals.remove(key);
            }
            self.sync_mapped_arguments_delete(target, key);
            Ok(true)
        } else {
            Ok(true)
        }
    }

    pub fn to_property_key(&mut self, val: JsValue) -> Result<PropertyKeyVal, VmError> {
        if val.is_symbol() {
            return Ok(PropertyKeyVal::Symbol(val));
        }
        if let Some(ptr) = val.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if matches!(obj.kind, ObjectKind::Symbol(_)) {
                return Ok(PropertyKeyVal::Symbol(val));
            }
        }
        let prim = self.to_primitive(val, PreferredType::String)?;
        if prim.is_symbol() {
            Ok(PropertyKeyVal::Symbol(prim))
        } else if prim.as_object_ptr().is_some_and(|p| matches!(unsafe { &(*(p as *mut JsObject)).kind }, ObjectKind::Symbol(_))) {
            Ok(PropertyKeyVal::Symbol(prim))
        } else {
            Ok(PropertyKeyVal::String(self.to_string_value(prim)))
        }
    }

    pub fn get_symbol_property_result(&mut self, target: JsValue, key: JsValue) -> Result<JsValue, VmError> {
        if target.is_null() || target.is_undefined() {
            return Err(VmError::TypeError("Cannot read property of null or undefined".into()));
        }
        if let Some(sym_iter) = self.symbol_iterator {
            if key == sym_iter {
                if let Some(ptr) = target.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    match &obj.kind {
                        ObjectKind::Map(_) => {
                            if let Some(m) = self.get_map_method(target, "entries") {
                                return Ok(m);
                            }
                        }
                        ObjectKind::Set(_) => {
                            if let Some(m) = self.get_set_method(target, "values") {
                                return Ok(m);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        let mut curr_proto = if let Some(ptr) = target.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };

            if let Some(accs) = &obj.symbol_accessors {
                if let Some(acc) = accs.get(&key) {
                    if let Some(getter) = acc.getter {
                        return self.call(getter, target, &[]);
                    } else {
                        return Ok(JsValue::undefined());
                    }
                }
            }

            if let Some(syms) = &obj.symbol_properties {
                if let Some(val) = syms.get(&key) {
                    return Ok(*val);
                }
            }

            if obj.prototype.is_some() {
                obj.prototype
            } else if matches!(obj.kind, ObjectKind::String(_)) {
                self.string_proto
            } else {
                None
            }
        } else if target.is_string() {
            self.string_proto
        } else if target.is_symbol() {
            self.symbol_proto
        } else if target.is_number() {
            self.number_proto
        } else if target.is_boolean() {
            self.boolean_proto
        } else {
            None
        };

        while let Some(proto_val) = curr_proto {
            if let Some(proto_ptr) = proto_val.as_object_ptr() {
                let proto_obj = unsafe { &*(proto_ptr as *mut JsObject) };
                if let Some(accs) = &proto_obj.symbol_accessors {
                    if let Some(acc) = accs.get(&key) {
                        if let Some(getter) = acc.getter {
                            return self.call(getter, target, &[]);
                        } else {
                            return Ok(JsValue::undefined());
                        }
                    }
                }
                if let Some(syms) = &proto_obj.symbol_properties {
                    if let Some(val) = syms.get(&key) {
                        return Ok(*val);
                    }
                }
                curr_proto = proto_obj.prototype;
            } else {
                break;
            }
        }
        Ok(JsValue::undefined())
    }

    pub fn get_symbol_property(&mut self, target: JsValue, key: JsValue) -> JsValue {
        self.get_symbol_property_result(target, key).unwrap_or(JsValue::undefined())
    }

    pub fn set_symbol_property_result(&mut self, target: JsValue, key: JsValue, value: JsValue) -> Result<(), VmError> {
        if self.module_namespace_objects.contains(&target) {
            let is_strict = self.frames.last().map(|f| f.is_strict).unwrap_or(self.is_strict);
            if is_strict {
                return Err(VmError::TypeError("Cannot assign to read only property on module namespace object".into()));
            }
            return Ok(());
        }
        if let Some(ptr) = target.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };

            if let Some(accs) = &obj.symbol_accessors {
                if let Some(acc) = accs.get(&key) {
                    if let Some(setter) = acc.setter {
                        let _ = self.call(setter, target, &[value])?;
                    }
                    return Ok(());
                }
            }

            let mut curr_proto = obj.prototype;
            while let Some(proto_val) = curr_proto {
                if let Some(proto_ptr) = proto_val.as_object_ptr() {
                    let proto_obj = unsafe { &*(proto_ptr as *mut JsObject) };
                    if let Some(accs) = &proto_obj.symbol_accessors {
                        if let Some(acc) = accs.get(&key) {
                            if let Some(setter) = acc.setter {
                                let _ = self.call(setter, target, &[value])?;
                            }
                            return Ok(());
                        }
                    }
                    curr_proto = proto_obj.prototype;
                } else {
                    break;
                }
            }

            obj.insert_symbol_property(key, value);
        }
        Ok(())
    }

    pub fn set_symbol_property(&mut self, target: JsValue, key: JsValue, value: JsValue) {
        let _ = self.set_symbol_property_result(target, key, value);
    }

    pub fn has_property(&mut self, target: JsValue, key: &str) -> Result<bool, VmError> {
        if !target.is_object() {
            return Ok(false);
        }
        if self.deleted_props.contains(&(target, key.to_string())) {
            return Ok(false);
        }
        if self.module_reexports.contains_key(&(target, key.to_string())) {
            return Ok(true);
        }
        let ptr = target.as_object_ptr().unwrap();
        let obj = unsafe { &*(ptr as *mut JsObject) };
        if let ObjectKind::Proxy { target: tgt, handler: hdl } = &obj.kind {
            let tgt = *tgt;
            let hdl = *hdl;
            let trap = self.get_property(hdl, "has");
            if self.is_callable(trap) {
                let key_val = self.alloc_string(key.to_string());
                let res = self.call(trap, hdl, &[tgt, key_val])?;
                return Ok(res.is_truthy());
            }
            return self.has_property(tgt, key);
        }
        let mut curr = Some(target);
        while let Some(c_val) = curr {
            if let Some(c_ptr) = c_val.as_object_ptr() {
                let c_obj = unsafe { &*(c_ptr as *mut JsObject) };
                if self.shapes.has_property(c_obj.shape_id, key) {
                    return Ok(true);
                }
                if let Some(accs) = &c_obj.accessors {
                    if accs.contains_key(key) {
                        return Ok(true);
                    }
                }
                curr = c_obj.prototype;
            } else {
                break;
            }
        }
        Ok(false)
    }

    pub fn has_own_property_key(&self, target: JsValue, prop_key: &PropertyKeyVal) -> bool {
        if !target.is_object() {
            return false;
        }
        let ptr = target.as_object_ptr().unwrap();
        let obj = unsafe { &*(ptr as *mut JsObject) };

        match prop_key {
            PropertyKeyVal::Symbol(sym) => {
                if let Some(syms) = &obj.symbol_properties {
                    if syms.contains_key(sym) {
                        return true;
                    }
                }
                if let Some(accs) = &obj.symbol_accessors {
                    if accs.contains_key(sym) {
                        return true;
                    }
                }
                false
            }
            PropertyKeyVal::String(key) => {
                if self.deleted_props.contains(&(target, key.clone())) {
                    return false;
                }
                match &obj.kind {
                    ObjectKind::Array(elements) => {
                        if key == "length" || key.parse::<usize>().map(|idx| idx < elements.len()).unwrap_or(false) {
                            return true;
                        }
                    }
                    ObjectKind::String(s) | ObjectKind::StringObject(s) => {
                        if key == "length" || key.parse::<usize>().map(|idx| idx < s.chars().count()).unwrap_or(false) {
                            return true;
                        }
                    }
                    ObjectKind::TypedArray(ta) => {
                        if matches!(key.as_str(), "length" | "byteLength" | "byteOffset" | "buffer")
                            || key.parse::<usize>().map(|idx| idx < ta.length).unwrap_or(false) {
                            return true;
                        }
                    }
                    ObjectKind::Function(_) | ObjectKind::NativeFunction(_) => {
                        if key == "length" || key == "name" || (matches!(&obj.kind, ObjectKind::Function(_)) && key == "prototype") {
                            return true;
                        }
                    }
                    _ => {}
                }
                self.shapes.has_property(obj.shape_id, key)
                    || obj.accessors.as_ref().map(|a| a.contains_key(key)).unwrap_or(false)
                    || self.module_reexports.contains_key(&(target, key.clone()))
            }
        }
    }

    pub fn get_for_in_keys(&self, curr: JsValue) -> Vec<String> {
        if !curr.is_object() {
            if let Some(ptr) = curr.as_object_ptr() {
                let obj = unsafe { &*(ptr as *mut JsObject) };
                if let ObjectKind::String(ref s) | ObjectKind::StringObject(ref s) = obj.kind {
                    return (0..s.chars().count()).map(|i| i.to_string()).collect();
                }
            }
            return Vec::new();
        }
        let mut visited: Vec<String> = Vec::new();
        let mut current_val = Some(curr);

        while let Some(obj_val) = current_val {
            if let Some(ptr) = obj_val.as_object_ptr() {
                let obj = unsafe { &*(ptr as *mut JsObject) };
                let mut int_keys: Vec<(usize, String)> = Vec::new();
                let mut str_keys: Vec<String> = Vec::new();

                // 1. Array elements or string characters
                if let ObjectKind::Array(ref elements) = obj.kind {
                    for i in 0..elements.len() {
                        let k = i.to_string();
                        if !self.deleted_props.contains(&(obj_val, k.clone()))
                            && !self.non_enumerable_props.contains(&(obj_val, k.clone()))
                            && !visited.contains(&k)
                        {
                            int_keys.push((i, k));
                        }
                    }
                } else if let ObjectKind::String(ref s) | ObjectKind::StringObject(ref s) = obj.kind {
                    for (i, _) in s.chars().enumerate() {
                        let k = i.to_string();
                        if !self.non_enumerable_props.contains(&(obj_val, k.clone()))
                            && !visited.contains(&k)
                        {
                            int_keys.push((i, k));
                        }
                    }
                }

                // 2. Accessors
                if let Some(accs) = &obj.accessors {
                    for k in accs.keys() {
                        if !self.non_enumerable_props.contains(&(obj_val, k.clone()))
                            && !visited.contains(k)
                        {
                            if let Ok(idx) = k.parse::<usize>() {
                                if idx.to_string() == *k {
                                    if !int_keys.iter().any(|(_, ik)| ik == k) {
                                        int_keys.push((idx, k.clone()));
                                    }
                                    continue;
                                }
                            }
                            if !str_keys.contains(k) {
                                str_keys.push(k.clone());
                            }
                        }
                    }
                }

                // 3. Properties from shape in insertion order
                for prop in self.shapes.all_properties(obj.shape_id) {
                    if !self.non_enumerable_props.contains(&(obj_val, prop.clone()))
                        && !self.deleted_props.contains(&(obj_val, prop.clone()))
                        && !visited.contains(&prop)
                    {
                        if let Ok(idx) = prop.parse::<usize>() {
                            if idx.to_string() == prop {
                                if !int_keys.iter().any(|(_, ik)| ik == &prop) {
                                    int_keys.push((idx, prop));
                                }
                                continue;
                            }
                        }
                        if !str_keys.contains(&prop) {
                            str_keys.push(prop);
                        }
                    }
                }

                int_keys.sort_by_key(|(idx, _)| *idx);
                for (_, k) in int_keys {
                    visited.push(k);
                }
                for k in str_keys {
                    visited.push(k);
                }

                current_val = obj.prototype;
            } else {
                break;
            }
        }

        visited
    }

    pub fn array_to_vec_spread(&self, val: JsValue) -> Vec<JsValue> {
        if let Some(ptr) = val.as_object_ptr() {
            let jsobj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::Array(elements) = &jsobj.kind {
                return elements.clone();
            }
        }
        Vec::new()
    }

    pub fn to_spread_args(&mut self, val: JsValue) -> Result<Vec<JsValue>, VmError> {
        if let Some(sym_iter) = self.symbol_iterator {
            if let Ok(method) = self.get_symbol_property_result(val, sym_iter) {
                if !method.is_undefined() && !method.is_null() {
                    if !self.is_callable(method) {
                        return Err(VmError::TypeError("Symbol.iterator is not callable".into()));
                    }
                    let it = self.call(method, val, &[])?;
                    let next_fn = self.get_property(it, "next");
                    if !self.is_callable(next_fn) {
                        return Err(VmError::TypeError("iterator.next is not callable".into()));
                    }
                    let mut res = Vec::new();
                    loop {
                        let step = self.call(next_fn, it, &[])?;
                        let done = self.get_property(step, "done");
                        if done.is_truthy() {
                            break;
                        }
                        let v = self.get_property(step, "value");
                        res.push(v);
                    }
                    return Ok(res);
                }
            }
        }

        if val.is_string() {
            let s = self.to_string_value(val);
            return Ok(s.chars().map(|c| self.alloc_string(c.to_string())).collect());
        }

        if let Some(ptr) = val.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::Array(arr) = &obj.kind {
                return Ok(arr.clone());
            }
        }

        Err(VmError::TypeError(format!("{} is not iterable", self.to_string_value(val))))
    }

    pub fn execute_construct(
        &mut self,
        callee: JsValue,
        args: Vec<JsValue>,
        target_depth: usize,
    ) -> Result<(), VmError> {
        if !self.is_constructor(callee) {
            let msg = format!("{} is not a constructor", self.to_string_value(callee));
            self.handle_exception_at_depth(VmError::TypeError(msg), target_depth)?;
            return Ok(());
        }

        if let Some(ptr) = callee.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            match &obj.kind {
                ObjectKind::NativeFunction(native) => {
                    let proto = self.get_property(callee, "prototype");
                    let this_obj = self.alloc_object();
                    if proto.is_object() {
                        let this_ptr = this_obj.as_object_ptr().unwrap();
                        let jsobj = unsafe { &mut *(this_ptr as *mut JsObject) };
                        jsobj.prototype = Some(proto);
                    }
                    let prev_temp = self.temp_roots.len();
                    self.temp_roots.extend_from_slice(&args);
                    let prev_constructing = self.is_constructing;
                    self.is_constructing = true;
                    let res = native.call(self, this_obj, &args);
                    self.is_constructing = prev_constructing;
                    self.temp_roots.truncate(prev_temp);
                    match res {
                        Ok(res) => {
                            if res.is_object() {
                                self.push(res);
                            } else {
                                self.push(this_obj);
                            }
                        }
                        Err(err) => {
                            self.handle_exception_at_depth(err, target_depth)?;
                        }
                    }
                }
                ObjectKind::Function(_) => {
                    let this_obj = self.alloc_object();
                    let proto = self.get_property(callee, "prototype");
                    if proto.is_object() {
                        let this_ptr = this_obj.as_object_ptr().unwrap();
                        let jsobj = unsafe { &mut *(this_ptr as *mut JsObject) };
                        jsobj.prototype = Some(proto);
                    }
                    let prev_constructing = self.is_constructing;
                    self.is_constructing = true;
                    let res = self.call(callee, this_obj, &args);
                    self.is_constructing = prev_constructing;
                    match res {
                        Ok(res) => {
                            if res.is_object() {
                                self.push(res);
                            } else {
                                self.push(this_obj);
                            }
                        }
                        Err(err) => {
                            self.handle_exception_at_depth(err, target_depth)?;
                        }
                    }
                }
                _ => {
                    let err = VmError::TypeError(format!("{} is not a constructor", self.to_string_value(callee)));
                    self.handle_exception_at_depth(err, target_depth)?;
                }
            }
        } else {
            let err = VmError::TypeError("Target is not a constructor".into());
            self.handle_exception_at_depth(err, target_depth)?;
        }
        Ok(())
    }

    pub fn capture_upvalues(&mut self) -> Vec<JsValue> {
        let mut upvalues = Vec::new();
        if let Some(current_frame) = self.frames.last() {
            if current_frame.in_param_init {
                return current_frame.scopes.clone();
            }
            upvalues.extend_from_slice(&current_frame.scopes);
            if !current_frame.local_names.is_empty() || current_frame.arguments.is_some() {
                if let Some(env) = current_frame.env_obj {
                    upvalues.push(env);
                    return upvalues;
                }
            } else {
                return upvalues;
            }
        } else {
            return upvalues;
        }

        let (local_names, locals) = {
            let frame = self.frames.last().unwrap();
            (frame.local_names.clone(), frame.locals.clone())
        };
        let env = self.alloc_object();
        for (i, name) in local_names.iter().enumerate() {
            let val = locals.get(i).copied().unwrap_or(JsValue::undefined());
            self.set_property(env, name, val);
        }
        if let Some(frame) = self.frames.last() {
            if let Some(args_val) = frame.arguments {
                if !local_names.iter().any(|n| n == "arguments") {
                    self.set_property(env, "arguments", args_val);
                }
            }
        }
        if let Some(frame) = self.frames.last_mut() {
            frame.env_obj = Some(env);
        }
        upvalues.push(env);
        upvalues
    }

    pub fn resolve_identifier(&mut self, name: &str) -> Option<JsValue> {
        if let Some(frame) = self.frames.last() {
            if name == "arguments" && !frame.is_arrow {
                if let Some(args_val) = frame.arguments {
                    let has_local_arg = frame.local_names.iter().any(|n| n == "arguments");
                    if !has_local_arg {
                        return Some(args_val);
                    }
                }
            }

            if !name.starts_with('@') {
                let scopes = frame.scopes.clone();
                for scope_val in scopes.into_iter().rev() {
                    if scope_val.is_object() {
                        let has = self.has_property(scope_val, name).unwrap_or(false);
                        if has {
                            return Some(self.get_property(scope_val, name));
                        }
                    }
                }
            }
            if let Some(frame) = self.frames.last() {
                if !frame.scopes.is_empty() {
                    for (i, local_name) in frame.local_names.iter().enumerate().rev() {
                        if local_name == name {
                            return Some(frame.locals.get(i).copied().unwrap_or(JsValue::undefined()));
                        }
                    }
                }
                if name == "arguments" && !frame.is_arrow {
                    if let Some(args_val) = frame.arguments {
                        return Some(args_val);
                    }
                }
            }
        }
        if let Some((mod_obj, export_name)) = self.imported_bindings.get(name).cloned() {
            if export_name == "*" {
                return Some(mod_obj);
            }
            if let Some(ptr) = mod_obj.as_object_ptr() {
                let obj = unsafe { &*(ptr as *mut JsObject) };
                if !self.shapes.has_property(obj.shape_id, &export_name)
                    && !self.module_reexports.contains_key(&(mod_obj, export_name.clone()))
                {
                    return Some(JsValue::uninitialized());
                }
            }
            let val = self.get_property(mod_obj, &export_name);
            return Some(val);
        }
        if let Some(val) = self.global_lexicals.get(name) {
            return Some(*val);
        }
        if let Some(val) = self.globals.get(name) {
            return Some(*val);
        }
        if self.global_object.is_object() {
            let val = self.get_property(self.global_object, name);
            if !val.is_undefined() {
                return Some(val);
            }
            if let Some(ptr) = self.global_object.as_object_ptr() {
                let obj = unsafe { &*(ptr as *mut JsObject) };
                if self.shapes.has_property(obj.shape_id, name) {
                    return Some(val);
                }
            }
        }
        None
    }

    pub fn get_property(&mut self, target: JsValue, key: &str) -> JsValue {
        self.get_property_with_receiver(target, key, target)
    }

    pub fn get_property_with_receiver(&mut self, target: JsValue, key: &str, receiver: JsValue) -> JsValue {
        if let Some((tgt_mod, tgt_key)) = self.module_reexports.get(&(target, key.to_string())).cloned() {
            if let Some(ptr) = tgt_mod.as_object_ptr() {
                let obj = unsafe { &*(ptr as *mut JsObject) };
                if !self.shapes.has_property(obj.shape_id, &tgt_key)
                    && !self.module_reexports.contains_key(&(tgt_mod, tgt_key.clone()))
                {
                    return JsValue::uninitialized();
                }
            }
            return self.get_property(tgt_mod, &tgt_key);
        }
        if target.is_string() {
            let s = self.to_string_value(target);
            if key == "length" {
                return JsValue::from_int32(s.encode_utf16().count() as i32);
            }
            if let Ok(idx) = key.parse::<usize>() {
                if let Some(ch) = s.chars().nth(idx) {
                    return self.alloc_string(ch.to_string());
                }
            }
            if let Some(method) = self.get_string_method(key) {
                return method;
            }
            if let Some(proto) = self.string_proto {
                return self.get_property_with_receiver(proto, key, receiver);
            }
            return JsValue::undefined();
        }
        if target.is_number() {
            if let Some(proto) = self.number_proto {
                return self.get_property_with_receiver(proto, key, receiver);
            }
            return JsValue::undefined();
        }
        if target.is_boolean() {
            if let Some(proto) = self.boolean_proto {
                return self.get_property_with_receiver(proto, key, receiver);
            }
            return JsValue::undefined();
        }
        if target.is_symbol() {
            if let Some(proto) = self.symbol_proto {
                return self.get_property_with_receiver(proto, key, receiver);
            }
            return JsValue::undefined();
        }

        if let Some(ptr) = target.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };

            if let ObjectKind::Proxy { target: tgt, handler: hdl } = &obj.kind {
                let tgt = *tgt;
                let hdl = *hdl;
                let trap = self.get_property(hdl, "get");
                if self.is_callable(trap) {
                    let key_val = self.alloc_string(key.to_string());
                    return self.call(trap, hdl, &[tgt, key_val, receiver]).unwrap_or(JsValue::undefined());
                } else {
                    return self.get_property_with_receiver(tgt, key, receiver);
                }
            }

            // Check own accessor
            if let Some(accs) = &obj.accessors {
                if let Some(acc) = accs.get(key) {
                    if let Some(getter) = acc.getter {
                        return self.call(getter, receiver, &[]).unwrap_or(JsValue::undefined());
                    } else {
                        return JsValue::undefined();
                    }
                }
            }

            // Special properties on Array
            if let ObjectKind::Array(elements) = &obj.kind {
                if key == "length" {
                    return JsValue::from_int32(elements.len() as i32);
                }
                if let Ok(idx) = key.parse::<usize>() {
                    if let Some(elem) = elements.get(idx) {
                        return *elem;
                    }
                }
                if let Some(method) = self.get_array_method(key) {
                    return method;
                }
            }

            // Special properties on String or StringObject
            if let ObjectKind::String(s) | ObjectKind::StringObject(s) = &obj.kind {
                if key == "length" {
                    return JsValue::from_int32(s.encode_utf16().count() as i32);
                }
                if let Ok(idx) = key.parse::<usize>() {
                    if let Some(ch) = s.chars().nth(idx) {
                        return self.alloc_string(ch.to_string());
                    }
                }
                if let Some(method) = self.get_string_method(key) {
                    return method;
                }
            }

            // Special properties on Promise (fallback only if no prototype)
            if let ObjectKind::Promise(_) = &obj.kind {
                if obj.prototype.is_none() {
                    match key {
                        "then" => return self.get_promise_then(target).unwrap_or_default(),
                        "catch" => return self.get_promise_catch(target).unwrap_or_default(),
                        "finally" => return self.get_promise_finally(target).unwrap_or_default(),
                        _ => {}
                    }
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

            // Special properties on ArrayBuffer
            if let ObjectKind::ArrayBuffer(ab) = &obj.kind {
                if key == "byteLength" {
                    return JsValue::from_int32(ab.byte_length() as i32);
                }
            }

            // Special properties on DataView
            if let ObjectKind::DataView { buffer, byte_offset, byte_length } = &obj.kind {
                match key {
                    "byteLength" => return JsValue::from_int32(*byte_length as i32),
                    "byteOffset" => return JsValue::from_int32(*byte_offset as i32),
                    "buffer" => return self.alloc_array_buffer(Arc::clone(buffer)),
                    _ => {}
                }
            }

            // Special properties on TypedArray
            if let ObjectKind::TypedArray(ta) = &obj.kind {
                match key {
                    "length" => return JsValue::from_int32(ta.length as i32),
                    "byteLength" => return JsValue::from_int32((ta.length * ta.kind.bytes_per_element()) as i32),
                    "byteOffset" => return JsValue::from_int32(ta.byte_offset as i32),
                    "buffer" => return self.alloc_array_buffer(Arc::clone(&ta.buffer)),
                    "BYTES_PER_ELEMENT" => return JsValue::from_int32(ta.kind.bytes_per_element() as i32),
                    _ => {
                        if let Ok(idx) = key.parse::<usize>() {
                            if let Some(val) = Self::read_typed_array_element(ta, idx) {
                                return val;
                            }
                        }
                    }
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


            // Shape offset lookup (own properties take precedence)
            if let Some(offset) = self.shapes.lookup_offset(obj.shape_id, key) {
                if let Some(val) = obj.get_slot(offset) {
                    return val;
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
            if matches!(obj.kind, ObjectKind::Function(_) | ObjectKind::NativeFunction(_)) && !self.deleted_props.contains(&(target, key.to_string())) {
                match key {
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
                            ObjectKind::Function(f) => f.params.iter().take_while(|p| !p.starts_with("...")).count(),
                            ObjectKind::NativeFunction(n) => n.length as usize,
                            _ => 0,
                        };
                        return JsValue::from_int32(len as i32);
                    }
                    _ => {}
                }
            }

            // Prototype chain lookup
            let mut curr_proto = obj.prototype;
            while let Some(proto_val) = curr_proto {
                if let Some(proto_ptr) = proto_val.as_object_ptr() {
                    let proto_obj = unsafe { &*(proto_ptr as *mut JsObject) };
                    if let ObjectKind::Proxy { target: tgt, handler: hdl } = &proto_obj.kind {
                        let tgt = *tgt;
                        let hdl = *hdl;
                        let trap = self.get_property(hdl, "get");
                        if self.is_callable(trap) {
                            let key_val = self.alloc_string(key.to_string());
                            return self.call(trap, hdl, &[tgt, key_val, receiver]).unwrap_or(JsValue::undefined());
                        } else {
                            return self.get_property_with_receiver(tgt, key, receiver);
                        }
                    }
                    if let Some(accs) = &proto_obj.accessors {
                        if let Some(acc) = accs.get(key) {
                            if let Some(getter) = acc.getter {
                                return self.call(getter, receiver, &[]).unwrap_or(JsValue::undefined());
                            } else {
                                return JsValue::undefined();
                            }
                        }
                    }
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

        if let Some(ptr) = target.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if matches!(&obj.kind, ObjectKind::String(_)) {
                if let Some(proto) = self.string_proto {
                    return self.get_property(proto, key);
                }
            }
            if matches!(&obj.kind, ObjectKind::Symbol(_)) {
                if let Some(proto) = self.symbol_proto {
                    return self.get_property(proto, key);
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
                let mut mapped = Vec::with_capacity(len.min(65536));
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

    pub fn get_string_method(&mut self, name: &str) -> Option<JsValue> {
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
                let s = vm.to_string_value(this);
                let idx = match args.first() {
                    Some(val) => {
                        let num = vm.to_number_value(*val)?;
                        if num.is_nan() || num.is_infinite() {
                            return Ok(vm.alloc_string("".to_string()));
                        }
                        let int = num.trunc();
                        if int < 0.0 || int >= (usize::MAX as f64) {
                            return Ok(vm.alloc_string("".to_string()));
                        }
                        int as usize
                    }
                    None => 0,
                };
                if let Some(code) = s.encode_utf16().nth(idx) {
                    let s_char = String::from_utf16_lossy(&[code]);
                    Ok(vm.alloc_string(s_char))
                } else {
                    Ok(vm.alloc_string("".to_string()))
                }
            })),
            "charCodeAt" => Some(self.alloc_native_function("charCodeAt", 1, |vm, this, args| {
                let s = vm.to_string_value(this);
                let idx = match args.first() {
                    Some(val) => {
                        let num = vm.to_number_value(*val)?;
                        if num.is_nan() || num.is_infinite() {
                            return Ok(JsValue::from_double(f64::NAN));
                        }
                        let int = num.trunc();
                        if int < 0.0 || int >= (usize::MAX as f64) {
                            return Ok(JsValue::from_double(f64::NAN));
                        }
                        int as usize
                    }
                    None => 0,
                };
                if let Some(code) = s.encode_utf16().nth(idx) {
                    Ok(JsValue::from_int32(code as i32))
                } else {
                    Ok(JsValue::from_double(f64::NAN))
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
            "replace" => Some(self.alloc_native_function("replace", 2, |vm, this, args| {
                if this.is_nullish() {
                    return Err(VmError::TypeError("String.prototype.replace called on null or undefined".into()));
                }
                let this_str = vm.to_string_value(this);
                let search_arg = args.first().copied().unwrap_or(JsValue::undefined());
                let replace_arg = args.get(1).copied().unwrap_or(JsValue::undefined());

                let is_regexp = if let Some(ptr) = search_arg.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    matches!(obj.kind, ObjectKind::RegExp { .. })
                } else {
                    false
                };

                let is_replace_fn = vm.is_callable(replace_arg);

                if is_regexp {
                    let (pattern, flags) = if let Some(ptr) = search_arg.as_object_ptr() {
                        let obj = unsafe { &*(ptr as *mut JsObject) };
                        if let ObjectKind::RegExp { ref pattern, ref flags } = obj.kind {
                            (pattern.clone(), flags.clone())
                        } else {
                            (String::new(), String::new())
                        }
                    } else {
                        (String::new(), String::new())
                    };

                    let is_global = flags.contains('g');
                    let mut result = String::new();
                    let mut search_from = 0;

                    while search_from <= this_str.len() {
                        if pattern.is_empty() {
                            let matched_str = "";
                            let replacement = if is_replace_fn {
                                let arg0 = vm.alloc_string(matched_str.to_string());
                                let arg1 = JsValue::from_int32(search_from as i32);
                                let arg2 = vm.alloc_string(this_str.clone());
                                let call_res = vm.call(replace_arg, JsValue::undefined(), &[arg0, arg1, arg2])?;
                                vm.to_string_value(call_res)
                            } else {
                                vm.to_string_value(replace_arg)
                            };
                            result.push_str(&replacement);
                            if search_from < this_str.len() {
                                if let Some(ch) = this_str[search_from..].chars().next() {
                                    result.push(ch);
                                    search_from += ch.len_utf8();
                                } else {
                                    break;
                                }
                            } else {
                                break;
                            }
                            if !is_global {
                                result.push_str(&this_str[search_from..]);
                                break;
                            }
                        } else if let Some(found_idx) = this_str[search_from..].find(&pattern) {
                            let match_start = search_from + found_idx;
                            let match_end = match_start + pattern.len();
                            result.push_str(&this_str[search_from..match_start]);

                            let replacement = if is_replace_fn {
                                let arg0 = vm.alloc_string(pattern.clone());
                                let arg1 = JsValue::from_int32(match_start as i32);
                                let arg2 = vm.alloc_string(this_str.clone());
                                let call_res = vm.call(replace_arg, JsValue::undefined(), &[arg0, arg1, arg2])?;
                                vm.to_string_value(call_res)
                            } else {
                                vm.to_string_value(replace_arg)
                            };
                            result.push_str(&replacement);
                            search_from = match_end;

                            if !is_global {
                                result.push_str(&this_str[search_from..]);
                                break;
                            }
                        } else {
                            result.push_str(&this_str[search_from..]);
                            break;
                        }
                    }
                    Ok(vm.alloc_string(result))
                } else {
                    let search_str = vm.to_string_value(search_arg);
                    if let Some(pos) = this_str.find(&search_str) {
                        let before = &this_str[..pos];
                        let after = &this_str[pos + search_str.len()..];
                        let replacement = if is_replace_fn {
                            let arg0 = vm.alloc_string(search_str.clone());
                            let arg1 = JsValue::from_int32(pos as i32);
                            let arg2 = vm.alloc_string(this_str.clone());
                            let call_res = vm.call(replace_arg, JsValue::undefined(), &[arg0, arg1, arg2])?;
                            vm.to_string_value(call_res)
                        } else {
                            vm.to_string_value(replace_arg)
                        };
                        let mut result = String::with_capacity(before.len() + replacement.len() + after.len());
                        result.push_str(before);
                        result.push_str(&replacement);
                        result.push_str(after);
                        Ok(vm.alloc_string(result))
                    } else {
                        Ok(vm.alloc_string(this_str))
                    }
                }
            })),
            "match" => Some(self.alloc_native_function("match", 1, |vm, this, args| {
                let this_str = vm.to_string_value(this);
                let re_arg = args.first().copied().unwrap_or(JsValue::undefined());
                let re = if let Some(ptr) = re_arg.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    if matches!(obj.kind, ObjectKind::RegExp { .. }) {
                        re_arg
                    } else {
                        let s = if re_arg.is_undefined() { "".to_string() } else { vm.to_string_value(re_arg) };
                        vm.alloc_regexp(s, "".to_string())
                    }
                } else {
                    let s = if re_arg.is_undefined() { "".to_string() } else { vm.to_string_value(re_arg) };
                    vm.alloc_regexp(s, "".to_string())
                };
                let exec_fn = vm.get_property(re, "exec");
                let this_str_val = vm.alloc_string(this_str);
                vm.call(exec_fn, re, &[this_str_val])
            })),
            "search" => Some(self.alloc_native_function("search", 1, |vm, this, args| {
                let this_str = vm.to_string_value(this);
                let re_arg = args.first().copied().unwrap_or(JsValue::undefined());
                let re = if let Some(ptr) = re_arg.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    if matches!(obj.kind, ObjectKind::RegExp { .. }) {
                        re_arg
                    } else {
                        let s = if re_arg.is_undefined() { "".to_string() } else { vm.to_string_value(re_arg) };
                        vm.alloc_regexp(s, "".to_string())
                    }
                } else {
                    let s = if re_arg.is_undefined() { "".to_string() } else { vm.to_string_value(re_arg) };
                    vm.alloc_regexp(s, "".to_string())
                };
                let exec_fn = vm.get_property(re, "exec");
                let this_str_val = vm.alloc_string(this_str);
                let res = vm.call(exec_fn, re, &[this_str_val])?;
                if res.is_nullish() || res.is_null() {
                    Ok(JsValue::from_int32(-1))
                } else {
                    let idx = vm.get_property(res, "index");
                    Ok(idx)
                }
            })),
            _ => None,
        }
    }

    pub fn promise_then(&mut self, promise_val: JsValue, on_fulfilled: JsValue, on_rejected: JsValue) -> Result<JsValue, VmError> {
        let ptr = promise_val.as_object_ptr().ok_or_else(|| VmError::TypeError("Promise.prototype.then called on non-object".into()))?;
        let is_promise = {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            matches!(obj.kind, ObjectKind::Promise(_))
        };
        if !is_promise {
            return Err(VmError::TypeError("Promise.prototype.then called on non-promise".into()));
        }

        let next_promise = self.alloc_promise(PromiseState::Pending, JsValue::undefined());
        let obj = unsafe { &mut *(ptr as *mut JsObject) };
        if let ObjectKind::Promise(ref mut p) = obj.kind {
            match p.state {
                PromiseState::Fulfilled => {
                    let val = p.result;
                    let next_p = next_promise;
                    if on_fulfilled.is_object() {
                        let cb_task = self.alloc_closure_native_function("then_cb", 0, Arc::new(move |vm: &mut Vm, _, _| -> Result<JsValue, VmError> {
                            match vm.call(on_fulfilled, JsValue::undefined(), &[val]) {
                                Ok(res) => vm.resolve_promise(next_p, res),
                                Err(e) => {
                                    let err_val = vm.alloc_string(e.to_string());
                                    vm.reject_promise(next_p, err_val);
                                }
                            }
                            Ok(JsValue::undefined())
                        }));
                        self.enqueue_microtask(cb_task, Vec::new());
                    } else {
                        let cb_task = self.alloc_closure_native_function("fwd_cb", 0, Arc::new(move |vm: &mut Vm, _, _| -> Result<JsValue, VmError> {
                            vm.resolve_promise(next_p, val);
                            Ok(JsValue::undefined())
                        }));
                        self.enqueue_microtask(cb_task, Vec::new());
                    }
                }
                PromiseState::Rejected => {
                    let err = p.result;
                    let next_p = next_promise;
                    if on_rejected.is_object() {
                        let cb_task = self.alloc_closure_native_function("catch_cb", 0, Arc::new(move |vm: &mut Vm, _, _| -> Result<JsValue, VmError> {
                            match vm.call(on_rejected, JsValue::undefined(), &[err]) {
                                Ok(res) => vm.resolve_promise(next_p, res),
                                Err(e) => {
                                    let err_val = vm.alloc_string(e.to_string());
                                    vm.reject_promise(next_p, err_val);
                                }
                            }
                            Ok(JsValue::undefined())
                        }));
                        self.enqueue_microtask(cb_task, Vec::new());
                    } else {
                        let cb_task = self.alloc_closure_native_function("rfwd_cb", 0, Arc::new(move |vm: &mut Vm, _, _| -> Result<JsValue, VmError> {
                            vm.reject_promise(next_p, err);
                            Ok(JsValue::undefined())
                        }));
                        self.enqueue_microtask(cb_task, Vec::new());
                    }
                }
                PromiseState::Pending => {
                    let next_p = next_promise;
                    if on_fulfilled.is_object() {
                        let cb_task = self.alloc_closure_native_function("pending_then", 1, Arc::new(move |vm: &mut Vm, _, args: &[JsValue]| -> Result<JsValue, VmError> {
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
                        let cb_task = self.alloc_closure_native_function("pending_fwd", 1, Arc::new(move |vm: &mut Vm, _, args: &[JsValue]| -> Result<JsValue, VmError> {
                            let val = args.first().copied().unwrap_or_default();
                            vm.resolve_promise(next_p, val);
                            Ok(JsValue::undefined())
                        }));
                        p.fulfill_reactions.push(cb_task);
                    }

                    if on_rejected.is_object() {
                        let cb_task = self.alloc_closure_native_function("pending_catch", 1, Arc::new(move |vm: &mut Vm, _, args: &[JsValue]| -> Result<JsValue, VmError> {
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
                        let cb_task = self.alloc_closure_native_function("pending_rfwd", 1, Arc::new(move |vm: &mut Vm, _, args: &[JsValue]| -> Result<JsValue, VmError> {
                            let err = args.first().copied().unwrap_or_default();
                            vm.reject_promise(next_p, err);
                            Ok(JsValue::undefined())
                        }));
                        p.reject_reactions.push(cb_task);
                    }
                }
            }
        }

        Ok(next_promise)
    }

    fn get_promise_then(&mut self, promise_val: JsValue) -> Option<JsValue> {
        Some(self.alloc_closure_native_function("then", 2, Arc::new(move |vm: &mut Vm, _this: JsValue, args: &[JsValue]| -> Result<JsValue, VmError> {
            let on_fulfilled = args.first().copied().unwrap_or_default();
            let on_rejected = args.get(1).copied().unwrap_or_default();
            vm.promise_then(promise_val, on_fulfilled, on_rejected)
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
        let (gen_fn, ip, stack, state, this_val, args_obj, mapped_args) = {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            match &obj.kind {
                ObjectKind::Generator(g) => (g.function.clone(), g.ip, g.stack.clone(), g.state, g.this_val, g.arguments, g.mapped_arguments.clone()),
                _ => return Err(VmError::TypeError("Not a generator".into())),
            }
        };

        if state == crate::object::GeneratorState::Completed {
            let res = self.make_iter_result(JsValue::undefined(), true);
            if gen_fn.is_async {
                let promise = self.alloc_promise(crate::object::PromiseState::Pending, JsValue::undefined());
                self.resolve_promise(promise, res);
                let _ = self.drain_microtasks();
                return Ok(promise);
            }
            return Ok(res);
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
            this_val,
            gen_ptr: Some(ptr as *mut JsObject),
            locals: current_stack.clone(),
            local_names: gen_fn.local_names.clone(),
            lexical_local_names: gen_fn.lexical_local_names.clone(),
            scopes: gen_fn.upvalues.clone(),
            upvalues_count: gen_fn.upvalues.len(),
            env_obj: None,
            arguments: args_obj,
            is_strict: gen_fn.is_strict,
            new_target: None,
            is_function: true,
            is_method: gen_fn.is_method,
            is_arrow: gen_fn.is_arrow,
            has_param_exprs: gen_fn.has_param_exprs,
            in_param_init: false,
            is_indirect_eval: false,
            callee_func: Some(gen_fn.clone()),
            mapped_arguments: mapped_args,
        });

        let res = self.run_until_depth(target_depth);
        if gen_fn.is_async {
            match res {
                Ok(val) => {
                    let promise = self.alloc_promise(crate::object::PromiseState::Pending, JsValue::undefined());
                    self.resolve_promise(promise, val);
                    let _ = self.drain_microtasks();
                    Ok(promise)
                }
                Err(e) => {
                    let promise = self.alloc_promise(crate::object::PromiseState::Pending, JsValue::undefined());
                    let err_val = self.alloc_string(e.to_string());
                    self.reject_promise(promise, err_val);
                    let _ = self.drain_microtasks();
                    Ok(promise)
                }
            }
        } else {
            res
        }
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
            "entries" => Some(self.alloc_closure_native_function("entries", 0, Arc::new(move |vm, _this, _args| {
                let entries = if let Some(ptr) = target.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    if let ObjectKind::Map(entries) = &obj.kind {
                        entries.iter().map(|(k, v)| vm.alloc_array(vec![*k, *v])).collect()
                    } else {
                        Vec::new()
                    }
                } else {
                    Vec::new()
                };
                Ok(vm.alloc_iterator(entries))
            }))),
            "keys" => Some(self.alloc_closure_native_function("keys", 0, Arc::new(move |vm, _this, _args| {
                let keys = if let Some(ptr) = target.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    if let ObjectKind::Map(entries) = &obj.kind {
                        entries.iter().map(|(k, _)| *k).collect()
                    } else {
                        Vec::new()
                    }
                } else {
                    Vec::new()
                };
                Ok(vm.alloc_iterator(keys))
            }))),
            "values" => Some(self.alloc_closure_native_function("values", 0, Arc::new(move |vm, _this, _args| {
                let values = if let Some(ptr) = target.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    if let ObjectKind::Map(entries) = &obj.kind {
                        entries.iter().map(|(_, v)| *v).collect()
                    } else {
                        Vec::new()
                    }
                } else {
                    Vec::new()
                };
                Ok(vm.alloc_iterator(values))
            }))),
            _ => None,
        }
    }

    fn get_set_method(&mut self, target: JsValue, name: &str) -> Option<JsValue> {
        match name {
            "values" | "keys" => Some(self.alloc_closure_native_function("values", 0, Arc::new(move |vm, _this, _args| {
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
                Ok(vm.alloc_iterator(elements))
            }))),
            "entries" => Some(self.alloc_closure_native_function("entries", 0, Arc::new(move |vm, _this, _args| {
                let entries = if let Some(ptr) = target.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    if let ObjectKind::Set(elements) = &obj.kind {
                        elements.iter().map(|el| vm.alloc_array(vec![*el, *el])).collect()
                    } else {
                        Vec::new()
                    }
                } else {
                    Vec::new()
                };
                Ok(vm.alloc_iterator(entries))
            }))),
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
            } else if d == 0.0 {
                "0".to_string()
            } else if d.is_infinite() {
                if d > 0.0 { "Infinity".to_string() } else { "-Infinity".to_string() }
            } else {
                let abs_d = d.abs();
                if abs_d >= 1e21 || abs_d < 1e-6 {
                    let s = format!("{:e}", d);
                    if let Some(pos) = s.find('e') {
                        let after = &s[pos + 1..];
                        if !after.starts_with('-') && !after.starts_with('+') {
                            format!("{}e+{}", &s[..pos], after)
                        } else {
                            s
                        }
                    } else {
                        s
                    }
                } else if d.fract() == 0.0 && abs_d < 1e16 {
                    format!("{}", d as i64)
                } else {
                    format!("{}", d)
                }
            }
        } else if val.is_symbol() {
            if let Some(ptr) = val.as_heap_ptr() {
                let obj = unsafe { &*(ptr as *mut JsObject) };
                if let ObjectKind::Symbol(desc) = &obj.kind {
                    return format!("Symbol({})", desc.as_deref().unwrap_or(""));
                }
            }
            "Symbol()".to_string()
        } else if let Some(ptr) = val.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            match &obj.kind {
                ObjectKind::String(s) | ObjectKind::StringObject(s) => s.clone(),
                ObjectKind::NumberObject(n) => self.to_string_value(JsValue::from_double(*n)),
                ObjectKind::BooleanObject(b) => self.to_string_value(JsValue::from_boolean(*b)),
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
                ObjectKind::BigInt(b) => b.to_string(),
                ObjectKind::RegExp { pattern, flags } => format!("/{}/{}", pattern, flags),
                _ => {
                    let has_name_or_msg = {
                        let name = self.shapes.lookup_offset(obj.shape_id, "name").and_then(|offset| obj.get_slot(offset));
                        let msg = self.shapes.lookup_offset(obj.shape_id, "message").and_then(|offset| obj.get_slot(offset));
                        if let Some(msg_val) = msg {
                            let name_str = if let Some(n_val) = name { self.to_string_value(n_val) } else { "Error".to_string() };
                            let msg_str = self.to_string_value(msg_val);
                            Some(format!("{}: {}", name_str, msg_str))
                        } else {
                            None
                        }
                    };
                    has_name_or_msg.unwrap_or_else(|| "[object Object]".to_string())
                }
            }
        } else {
            "undefined".to_string()
        }
    }

    pub fn to_json_string(&self, val: JsValue) -> String {
        let mut visited = std::collections::HashSet::new();
        self.to_json_string_safe(val, &mut visited)
    }

    fn to_json_string_safe(&self, val: JsValue, visited: &mut std::collections::HashSet<*const u8>) -> String {
        if val.is_null() {
            "null".to_string()
        } else if val.is_boolean() {
            if val.as_boolean().unwrap() { "true".to_string() } else { "false".to_string() }
        } else if val.is_number() {
            let n = self.to_string_value(val);
            if n == "NaN" || n == "Infinity" || n == "-Infinity" {
                "null".to_string()
            } else {
                n
            }
        } else if let Some(ptr) = val.as_object_ptr() {
            if !visited.insert(ptr as *const u8) {
                return "null".to_string();
            }
            let obj = unsafe { &*(ptr as *mut JsObject) };
            let res = match &obj.kind {
                ObjectKind::String(s) | ObjectKind::StringObject(s) => {
                    let mut escaped = String::with_capacity(s.len() + 2);
                    escaped.push('"');
                    for c in s.chars() {
                        match c {
                            '"' => escaped.push_str("\\\""),
                            '\\' => escaped.push_str("\\\\"),
                            '\u{08}' => escaped.push_str("\\b"),
                            '\u{0C}' => escaped.push_str("\\f"),
                            '\n' => escaped.push_str("\\n"),
                            '\r' => escaped.push_str("\\r"),
                            '\t' => escaped.push_str("\\t"),
                            _ => escaped.push(c),
                        }
                    }
                    escaped.push('"');
                    escaped
                }
                ObjectKind::Array(elements) => {
                    let items: Vec<String> = elements.iter().map(|e| self.to_json_string_safe(*e, visited)).collect();
                    format!("[{}]", items.join(","))
                }
                ObjectKind::Ordinary => {
                    let mut pairs = Vec::new();
                    for (k, offset) in self.shapes.all_property_entries(obj.shape_id) {
                        if let Some(v) = obj.get_slot(offset) {
                            if !v.is_undefined() && !v.is_symbol() {
                                pairs.push(format!("\"{}\":{}", k, self.to_json_string_safe(v, visited)));
                            }
                        }
                    }
                    format!("{{{}}}", pairs.join(","))
                }
                _ => "null".to_string(),
            };
            visited.remove(&(ptr as *const u8));
            res
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

    pub fn instantiate_global_declarations(&mut self, chunk: &BytecodeChunk) -> Result<(), VmError> {
        if !self.is_module {
            // Step 1: Validate top_lex_names
            for name in &chunk.top_lex_names {
                if self.global_lexicals.contains_key(name) {
                    return Err(VmError::SyntaxError(format!("Identifier '{}' has already been declared", name)));
                }
                if self.script_var_names.contains(name) {
                    return Err(VmError::SyntaxError(format!("Identifier '{}' has already been declared", name)));
                }
                if self.non_configurable_props.contains(&(self.global_object, name.clone()))
                    || name == "undefined" || name == "NaN" || name == "Infinity"
                {
                    return Err(VmError::SyntaxError(format!("Identifier '{}' has already been declared", name)));
                }
            }

            // Step 2: Validate top_var_names
            for name in &chunk.top_var_names {
                if self.global_lexicals.contains_key(name) {
                    return Err(VmError::SyntaxError(format!("Identifier '{}' has already been declared", name)));
                }
                if self.non_extensible_objects.contains(&self.global_object) {
                    let has = if let Some(ptr) = self.global_object.as_object_ptr() {
                        let obj = unsafe { &*(ptr as *mut JsObject) };
                        self.shapes.has_property(obj.shape_id, name)
                    } else {
                        false
                    };
                    if !has {
                        return Err(VmError::TypeError(format!("Cannot define property '{}' on non-extensible global object", name)));
                    }
                }
            }

            // Step 3: Validate top_func_names
            for name in &chunk.top_func_names {
                if self.global_lexicals.contains_key(name) {
                    return Err(VmError::SyntaxError(format!("Identifier '{}' has already been declared", name)));
                }
                if self.non_configurable_props.contains(&(self.global_object, name.clone())) {
                    let is_writable = !self.non_writable_props.contains(&(self.global_object, name.clone()));
                    let is_enum = !self.non_enumerable_props.contains(&(self.global_object, name.clone()));
                    let has_accessor = if let Some(ptr) = self.global_object.as_object_ptr() {
                        let obj = unsafe { &*(ptr as *mut JsObject) };
                        obj.accessors.as_ref().map(|a| a.contains_key(name)).unwrap_or(false)
                    } else {
                        false
                    };
                    if has_accessor || !is_writable || !is_enum {
                        return Err(VmError::TypeError(format!("Cannot declare global function '{}': existing property is non-configurable", name)));
                    }
                }
                if self.non_extensible_objects.contains(&self.global_object) {
                    let has = if let Some(ptr) = self.global_object.as_object_ptr() {
                        let obj = unsafe { &*(ptr as *mut JsObject) };
                        self.shapes.has_property(obj.shape_id, name)
                    } else {
                        false
                    };
                    if !has {
                        return Err(VmError::TypeError(format!("Cannot define function '{}' on non-extensible global object", name)));
                    }
                }
            }
        }

        // All validations passed! Instantiate bindings:
        for name in &chunk.top_lex_names {
            if self.is_module {
                self.global_lexicals.insert(name.clone(), JsValue::uninitialized());
            } else {
                self.global_lexicals.insert(name.clone(), JsValue::undefined());
            }
        }

        let in_eval = self.in_eval;
        for name in &chunk.top_var_names {
            self.var_names.insert(name.clone());
            if !in_eval {
                self.script_var_names.insert(name.clone());
            }
            let has = if let Some(ptr) = self.global_object.as_object_ptr() {
                let obj = unsafe { &*(ptr as *mut JsObject) };
                self.shapes.has_property(obj.shape_id, name)
            } else {
                false
            };
            if !has {
                self.globals.insert(name.clone(), JsValue::undefined());
                if !self.is_module && self.global_object.is_object() {
                    self.set_property(self.global_object, name, JsValue::undefined());
                    if !in_eval {
                        self.non_configurable_props.insert((self.global_object, name.clone()));
                    }
                }
            } else if !self.globals.contains_key(name) {
                let existing_val = self.get_property(self.global_object, name);
                self.globals.insert(name.clone(), existing_val);
            }
        }

        for name in &chunk.top_func_names {
            self.var_names.insert(name.clone());
            if !in_eval {
                self.script_var_names.insert(name.clone());
                if !self.is_module && self.global_object.is_object() {
                    self.non_configurable_props.insert((self.global_object, name.clone()));
                }
            }
            if !self.globals.contains_key(name) {
                self.globals.insert(name.clone(), JsValue::undefined());
                if !self.is_module && self.global_object.is_object() {
                    self.set_property(self.global_object, name, JsValue::undefined());
                }
            }
        }

        if self.is_module {
            if let Some(mod_obj) = self.current_module_obj {
                for name in &chunk.export_names {
                    let val = if chunk.top_func_names.contains(name) {
                        self.globals.get(name).copied().unwrap_or_else(JsValue::undefined)
                    } else if chunk.top_var_names.contains(name) {
                        JsValue::undefined()
                    } else {
                        JsValue::uninitialized()
                    };
                    self.set_property_direct(mod_obj, name, val);
                    self.non_configurable_props.insert((mod_obj, name.clone()));
                }
            }
        }

        Ok(())
    }

    pub fn run(&mut self, chunk: &BytecodeChunk) -> Result<JsValue, VmError> {
        self.instantiate_global_declarations(chunk)?;
        let root_chunk = Arc::new(chunk.clone());
        let target_depth = self.frames.len();
        self.frames.push(CallFrame {
            chunk: root_chunk,
            ip: 0,
            base_sp: self.stack.len(),
            this_val: if self.is_module { JsValue::undefined() } else { self.global_object },
            gen_ptr: None,
            locals: Vec::new(),
            local_names: Vec::new(),
            lexical_local_names: Vec::new(),
            scopes: Vec::new(),
            upvalues_count: 0,
            env_obj: None,
            arguments: None,
            is_strict: self.is_strict,
            new_target: None,
            is_function: false,
            is_method: false,
            is_arrow: false,
            has_param_exprs: false,
            in_param_init: false,
            is_indirect_eval: false,
            callee_func: None,
            mapped_arguments: None,
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

    pub fn is_constructor(&mut self, val: JsValue) -> bool {
        if let Some(ptr) = val.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let Some(sym_ctor) = self.globals.get("Symbol") {
                if val == *sym_ctor {
                    return false;
                }
            }
            match &obj.kind {
                ObjectKind::Function(f) => {
                    !f.is_generator && !f.is_async && self.shapes.has_property(obj.shape_id, "prototype")
                }
                ObjectKind::NativeFunction(n) => {
                    const NON_CONSTRUCTORS: &[&str] = &[
                        "encodeURI", "decodeURI", "encodeURIComponent", "decodeURIComponent",
                        "parseInt", "parseFloat", "isNaN", "isFinite", "eval", "Symbol",
                        "escape", "unescape",
                    ];
                    if NON_CONSTRUCTORS.contains(&n.name.as_str()) || n.name.contains('.') {
                        return false;
                    }
                    if self.shapes.has_property(obj.shape_id, "prototype") {
                        return true;
                    }
                    n.name.chars().next().map_or(false, |c| c.is_ascii_uppercase())
                }
                ObjectKind::Proxy { target, .. } => {
                    let t = *target;
                    self.is_constructor(t)
                }
                _ => false,
            }
        } else {
            false
        }
    }

    pub fn is_primitive_value(&self, val: JsValue) -> bool {
        if !val.is_object() {
            return true;
        }
        if let Some(ptr) = val.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            matches!(obj.kind, ObjectKind::String(_) | ObjectKind::BigInt(_))
        } else {
            true
        }
    }

    pub fn to_primitive(&mut self, val: JsValue, hint: PreferredType) -> Result<JsValue, VmError> {
        if self.is_primitive_value(val) {
            return Ok(val);
        }

        // 1. Check Symbol.toPrimitive
        if let Some(sym_prim) = self.symbol_to_primitive {
            let exotic = self.get_symbol_property_result(val, sym_prim)?;
            if !exotic.is_undefined() && !exotic.is_null() {
                if !self.is_callable(exotic) {
                    return Err(VmError::TypeError("Symbol.toPrimitive must be callable".into()));
                }
                let hint_str = match hint {
                    PreferredType::String => "string",
                    PreferredType::Number => "number",
                    PreferredType::Default => "default",
                };
                let hint_val = self.alloc_string(hint_str.to_string());
                let res = self.call(exotic, val, &[hint_val])?;
                if !self.is_primitive_value(res) {
                    return Err(VmError::TypeError("Cannot convert object to primitive value".into()));
                }
                return Ok(res);
            }
        }

        if let Some(ptr) = val.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            match &obj.kind {
                ObjectKind::StringObject(s) => Ok(self.alloc_string(s.clone())),
                ObjectKind::NumberObject(n) => Ok(JsValue::from_double(*n)),
                ObjectKind::BooleanObject(b) => Ok(JsValue::from_boolean(*b)),
                ObjectKind::BigIntObject(b) => Ok(self.alloc_bigint(b.clone())),
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
                            if self.is_primitive_value(res) {
                                return Ok(res);
                            }
                        }
                    }
                    Err(VmError::TypeError("Cannot convert Date to primitive value".into()))
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
                            if self.is_primitive_value(res) {
                                return Ok(res);
                            }
                        }
                    }
                    Err(VmError::TypeError("Cannot convert object to primitive value".into()))
                }
            }
        } else {
            Ok(val)
        }
    }

    pub fn to_numeric(&mut self, val: JsValue) -> Result<JsValue, VmError> {
        let prim = self.to_primitive(val, PreferredType::Number)?;
        if self.as_bigint_val(prim).is_some() {
            Ok(prim)
        } else {
            let num = self.to_number_value(prim)?;
            Ok(JsValue::from_double(num))
        }
    }

    pub fn to_number_value(&mut self, val: JsValue) -> Result<f64, VmError> {
        let prim = self.to_primitive(val, PreferredType::Number)?;
        if prim.is_symbol() {
            return Err(VmError::TypeError("Cannot convert a Symbol value to a number".into()));
        }
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
                ObjectKind::String(s) | ObjectKind::StringObject(s) => {
                    let trimmed = s.trim();
                    if trimmed.is_empty() {
                        Ok(0.0)
                    } else if trimmed == "Infinity" || trimmed == "+Infinity" {
                        Ok(f64::INFINITY)
                    } else if trimmed == "-Infinity" {
                        Ok(f64::NEG_INFINITY)
                    } else if trimmed.to_ascii_lowercase().contains("inf") || trimmed.to_ascii_lowercase().contains("nan") {
                        Ok(f64::NAN)
                    } else if let Some(hex_part) = trimmed.strip_prefix("0x").or_else(|| trimmed.strip_prefix("0X")) {
                        Ok(u64::from_str_radix(hex_part, 16).map(|v| v as f64).unwrap_or(f64::NAN))
                    } else if let Some(oct_part) = trimmed.strip_prefix("0o").or_else(|| trimmed.strip_prefix("0O")) {
                        Ok(u64::from_str_radix(oct_part, 8).map(|v| v as f64).unwrap_or(f64::NAN))
                    } else if let Some(bin_part) = trimmed.strip_prefix("0b").or_else(|| trimmed.strip_prefix("0B")) {
                        Ok(u64::from_str_radix(bin_part, 2).map(|v| v as f64).unwrap_or(f64::NAN))
                    } else {
                        Ok(trimmed.parse::<f64>().unwrap_or(f64::NAN))
                    }
                }
                ObjectKind::NumberObject(n) => Ok(*n),
                ObjectKind::BooleanObject(b) => Ok(if *b { 1.0 } else { 0.0 }),
                ObjectKind::BigInt(_) => Err(VmError::TypeError("Cannot convert a BigInt value to a number".into())),
                _ => Ok(f64::NAN),
            }
        } else {
            Ok(f64::NAN)
        }
    }

    pub fn strict_equal(&self, a: JsValue, b: JsValue) -> bool {
        if a == b {
            if a.is_double() && a.as_double().unwrap().is_nan() {
                return false;
            }
            return true;
        }
        if a.is_number() && b.is_number() {
            let na = a.to_number().unwrap();
            let nb = b.to_number().unwrap();
            if na.is_nan() || nb.is_nan() {
                return false;
            }
            return na == nb;
        }
        let a_str = if a.is_string() {
            a.as_string_ptr().map(|p| unsafe { &*(p as *mut JsObject) })
        } else if a.is_object() {
            a.as_object_ptr().map(|p| unsafe { &*(p as *mut JsObject) })
        } else {
            None
        };
        let b_str = if b.is_string() {
            b.as_string_ptr().map(|p| unsafe { &*(p as *mut JsObject) })
        } else if b.is_object() {
            b.as_object_ptr().map(|p| unsafe { &*(p as *mut JsObject) })
        } else {
            None
        };
        if let (Some(oa), Some(ob)) = (a_str, b_str) {
            if let (ObjectKind::String(sa), ObjectKind::String(sb)) = (&oa.kind, &ob.kind) {
                return sa == sb;
            }
        }
        let a_ptr = a.as_object_ptr();
        let b_ptr = b.as_object_ptr();
        if let (Some(pa), Some(pb)) = (a_ptr, b_ptr) {
            let oa = unsafe { &*(pa as *mut JsObject) };
            let ob = unsafe { &*(pb as *mut JsObject) };
            if let (ObjectKind::BigInt(ba), ObjectKind::BigInt(bb)) = (&oa.kind, &ob.kind) {
                return ba == bb;
            }
            return pa == pb;
        }
        false
    }

    pub fn same_value(&self, a: JsValue, b: JsValue) -> bool {
        if a.is_number() && b.is_number() {
            let na = a.to_number().unwrap();
            let nb = b.to_number().unwrap();
            if na.is_nan() && nb.is_nan() {
                return true;
            }
            if na == 0.0 && nb == 0.0 {
                return na.to_bits() == nb.to_bits();
            }
            return na == nb;
        }
        self.strict_equal(a, b)
    }

    pub fn abstract_equal(&mut self, a: JsValue, b: JsValue) -> Result<bool, VmError> {
        if self.strict_equal(a, b) {
            return Ok(true);
        }
        if (a.is_null() && b.is_undefined()) || (a.is_undefined() && b.is_null()) {
            return Ok(true);
        }
        if a.is_null() || a.is_undefined() || b.is_null() || b.is_undefined() {
            return Ok(false);
        }

        let a_bi = self.as_bigint_val(a);
        let b_bi = self.as_bigint_val(b);
        let a_is_str = a.is_string() || (a.is_object() && a.as_object_ptr().is_some_and(|p| matches!(unsafe { &(*(p as *mut JsObject)).kind }, ObjectKind::String(_))));
        let b_is_str = b.is_string() || (b.is_object() && b.as_object_ptr().is_some_and(|p| matches!(unsafe { &(*(p as *mut JsObject)).kind }, ObjectKind::String(_))));

        if a.is_number() && b_is_str {
            let nb = self.to_number_value(b)?;
            let na = a.to_number().unwrap();
            return Ok(!na.is_nan() && !nb.is_nan() && na == nb);
        }
        if a_is_str && b.is_number() {
            let na = self.to_number_value(a)?;
            let nb = b.to_number().unwrap();
            return Ok(!na.is_nan() && !nb.is_nan() && na == nb);
        }

        if let Some(ref bi) = a_bi {
            if b_is_str {
                let sb = self.to_string_value(b);
                return Ok(string_to_bigint(&sb).map_or(false, |b_val| *bi == b_val));
            }
        }
        if let Some(ref bi) = b_bi {
            if a_is_str {
                let sa = self.to_string_value(a);
                return Ok(string_to_bigint(&sa).map_or(false, |a_val| a_val == *bi));
            }
        }

        if a.is_boolean() {
            let na = if a.as_boolean() == Some(true) { 1.0 } else { 0.0 };
            return self.abstract_equal(JsValue::from_double(na), b);
        }
        if b.is_boolean() {
            let nb = if b.as_boolean() == Some(true) { 1.0 } else { 0.0 };
            return self.abstract_equal(a, JsValue::from_double(nb));
        }

        if let Some(ref bi) = a_bi {
            if b.is_number() {
                let nb = b.to_number().unwrap();
                return Ok(bigint_cmp_f64(bi, nb) == Some(std::cmp::Ordering::Equal));
            }
        }
        if let Some(ref bi) = b_bi {
            if a.is_number() {
                let na = a.to_number().unwrap();
                return Ok(bigint_cmp_f64(bi, na) == Some(std::cmp::Ordering::Equal));
            }
        }

        let a_is_actual_obj = a.is_object() && a_bi.is_none() && !a_is_str;
        let b_is_actual_obj = b.is_object() && b_bi.is_none() && !b_is_str;

        if !a_is_actual_obj && b_is_actual_obj {
            let b_prim = self.to_primitive(b, PreferredType::Default)?;
            return self.abstract_equal(a, b_prim);
        }
        if a_is_actual_obj && !b_is_actual_obj {
            let a_prim = self.to_primitive(a, PreferredType::Default)?;
            return self.abstract_equal(a_prim, b);
        }

        Ok(false)
    }

    pub fn abstract_relational_comparison(&mut self, x: JsValue, y: JsValue) -> Result<Option<std::cmp::Ordering>, VmError> {
        let px = self.to_primitive(x, PreferredType::Number)?;
        let py = self.to_primitive(y, PreferredType::Number)?;

        let px_is_sym = px.is_symbol() || (px.is_object() && px.as_object_ptr().is_some_and(|p| matches!(unsafe { &(*(p as *mut JsObject)).kind }, ObjectKind::Symbol(_))));
        let py_is_sym = py.is_symbol() || (py.is_object() && py.as_object_ptr().is_some_and(|p| matches!(unsafe { &(*(p as *mut JsObject)).kind }, ObjectKind::Symbol(_))));
        if px_is_sym || py_is_sym {
            return Err(VmError::TypeError("Cannot convert a Symbol value to a number".into()));
        }

        let px_is_str = px.is_string() || (px.is_object() && px.as_object_ptr().is_some_and(|p| matches!(unsafe { &(*(p as *mut JsObject)).kind }, ObjectKind::String(_))));
        let py_is_str = py.is_string() || (py.is_object() && py.as_object_ptr().is_some_and(|p| matches!(unsafe { &(*(p as *mut JsObject)).kind }, ObjectKind::String(_))));

        if px_is_str && py_is_str {
            let sx = self.to_string_value(px);
            let sy = self.to_string_value(py);
            return Ok(Some(js_string_cmp(&sx, &sy)));
        }

        let px_bi = self.as_bigint_val(px);
        let py_bi = self.as_bigint_val(py);

        if let (Some(bx), Some(by)) = (&px_bi, &py_bi) {
            return Ok(Some(bx.cmp(by)));
        }

        if let Some(ref bx) = px_bi {
            if py_is_str {
                let sy = self.to_string_value(py);
                return Ok(string_to_bigint(&sy).map(|by| bx.cmp(&by)));
            }
            let ny = self.to_number_value(py)?;
            return Ok(bigint_cmp_f64(bx, ny));
        }

        if let Some(ref by) = py_bi {
            if px_is_str {
                let sx = self.to_string_value(px);
                return Ok(string_to_bigint(&sx).map(|bx| bx.cmp(by)));
            }
            let nx = self.to_number_value(px)?;
            return Ok(bigint_cmp_f64(by, nx).map(|ord| ord.reverse()));
        }

        let nx = self.to_number_value(px)?;
        let ny = self.to_number_value(py)?;
        if nx.is_nan() || ny.is_nan() {
            return Ok(None);
        }
        if nx < ny {
            Ok(Some(std::cmp::Ordering::Less))
        } else if nx > ny {
            Ok(Some(std::cmp::Ordering::Greater))
        } else {
            Ok(Some(std::cmp::Ordering::Equal))
        }
    }

    pub fn execute_call_internal(
        &mut self,
        callee: JsValue,
        this_val: JsValue,
        args: Vec<JsValue>,
    ) -> Result<Option<JsValue>, VmError> {
        if self.frames.len() >= 1024 {
            return Err(VmError::RangeError("Maximum call stack size exceeded".to_string()));
        }
        if let Some(ptr) = callee.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            match &obj.kind {
                ObjectKind::NativeFunction(native) => {
                    let prev_temp = self.temp_roots.len();
                    self.temp_roots.push(this_val);
                    self.temp_roots.extend_from_slice(&args);
                    let res = native.call(self, this_val, &args);
                    self.temp_roots.truncate(prev_temp);
                    Ok(Some(res?))
                }
                ObjectKind::Function(f) => {
                    let is_strict_func = f.is_strict;
                    let effective_this = if !is_strict_func {
                        if this_val.is_undefined() || this_val.is_null() {
                            self.global_object
                        } else {
                            let is_primitive = this_val.is_number()
                                || this_val.is_boolean()
                                || this_val.is_symbol()
                                || this_val.is_string()
                                || this_val.as_object_ptr().is_some_and(|ptr| {
                                    let obj = unsafe { &*(ptr as *mut JsObject) };
                                    matches!(obj.kind, ObjectKind::String(_) | ObjectKind::Symbol(_) | ObjectKind::BigInt(_))
                                });
                            if is_primitive {
                                self.to_object_value(this_val).unwrap_or(this_val)
                            } else {
                                this_val
                            }
                        }
                    } else {
                        this_val
                    };

                    let has_rest = f.params.last().map(|p| p.starts_with("...")).unwrap_or(false);
                    let rest_idx = if has_rest { f.params.len() - 1 } else { usize::MAX };

                    let mut locals = Vec::with_capacity(f.local_names.len().max(args.len()));
                    for i in 0..f.local_names.len() {
                        if i < f.params.len() {
                            if i == rest_idx {
                                let rest_args = if args.len() > rest_idx {
                                    args[rest_idx..].to_vec()
                                } else {
                                    Vec::new()
                                };
                                let rest_arr = self.alloc_array(rest_args);
                                locals.push(rest_arr);
                            } else if i < args.len() {
                                locals.push(args[i]);
                            } else {
                                locals.push(JsValue::undefined());
                            }
                        } else {
                            locals.push(JsValue::undefined());
                        }
                    }
                    if !has_rest && args.len() > f.local_names.len() {
                        for arg in &args[f.local_names.len()..] {
                            locals.push(*arg);
                        }
                    }

                    let new_target = if self.is_constructing { Some(callee) } else { None };

                    let (args_obj, mapped_args) = if f.is_arrow {
                        (None, None)
                    } else {
                        let mapped = if !is_strict_func && !f.has_param_exprs && !has_rest {
                            Some(vec![true; f.params.len().min(args.len())])
                        } else {
                            None
                        };
                        (Some(self.alloc_arguments(callee, args.clone(), is_strict_func)), mapped)
                    };

                    if f.is_generator {
                        let chunk = Arc::clone(&f.chunk);
                        let target_depth = self.frames.len();
                        let base_sp = self.stack.len();
                        self.frames.push(CallFrame {
                            chunk,
                            ip: 0,
                            base_sp,
                            this_val: effective_this,
                            gen_ptr: None,
                            locals: locals.clone(),
                            local_names: f.local_names.clone(),
                            lexical_local_names: f.lexical_local_names.clone(),
                            scopes: f.upvalues.clone(),
                            upvalues_count: f.upvalues.len(),
                            env_obj: None,
                            arguments: args_obj,
                            is_strict: is_strict_func,
                            new_target,
                            is_function: true,
                            is_method: f.is_method,
                            is_arrow: f.is_arrow,
                            has_param_exprs: f.has_param_exprs,
                            in_param_init: f.has_param_exprs,
                            is_indirect_eval: false,
                            callee_func: Some(f.clone()),
                            mapped_arguments: mapped_args.clone(),
                        });
                        let gen_val = self.run_until_depth(target_depth)?;
                        Ok(Some(gen_val))
                    } else if f.is_async {
                        let promise_val = self.alloc_promise(PromiseState::Pending, JsValue::undefined());
                        let chunk = Arc::clone(&f.chunk);
                        let target_depth = self.frames.len();
                        let base_sp = self.stack.len();
                        self.frames.push(CallFrame {
                            chunk,
                            ip: 0,
                            base_sp,
                            this_val: effective_this,
                            gen_ptr: None,
                            locals: locals.clone(),
                            local_names: f.local_names.clone(),
                            lexical_local_names: f.lexical_local_names.clone(),
                            scopes: f.upvalues.clone(),
                            upvalues_count: f.upvalues.len(),
                            env_obj: None,
                            arguments: args_obj,
                            is_strict: is_strict_func,
                            new_target,
                            is_function: true,
                            is_method: f.is_method,
                            is_arrow: f.is_arrow,
                            has_param_exprs: f.has_param_exprs,
                            in_param_init: f.has_param_exprs,
                            is_indirect_eval: false,
                            callee_func: Some(f.clone()),
                            mapped_arguments: mapped_args.clone(),
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
                            this_val: effective_this,
                            gen_ptr: None,
                            locals,
                            local_names: f.local_names.clone(),
                            lexical_local_names: f.lexical_local_names.clone(),
                            scopes: f.upvalues.clone(),
                            upvalues_count: f.upvalues.len(),
                            env_obj: None,
                            arguments: args_obj,
                            is_strict: is_strict_func,
                            new_target,
                            is_function: true,
                            is_method: f.is_method,
                            is_arrow: f.is_arrow,
                            has_param_exprs: f.has_param_exprs,
                            in_param_init: f.has_param_exprs,
                            is_indirect_eval: false,
                            callee_func: Some(f.clone()),
                            mapped_arguments: mapped_args,
                        };
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
        loop {
            match self.run_until_depth_internal(target_depth) {
                Ok(val) => return Ok(val),
                Err(err) => {
                    if self.handle_exception_at_depth(err.clone(), target_depth).is_ok() {
                        continue;
                    }
                    while let Some(top) = self.catch_stack.last() {
                        if top.frame_idx >= target_depth {
                            self.catch_stack.pop();
                        } else {
                            break;
                        }
                    }
                    if let Some(target_frame) = self.frames.get(target_depth) {
                        let base_sp = target_frame.base_sp;
                        self.stack.truncate(base_sp);
                    }
                    self.frames.truncate(target_depth);
                    return Err(err);
                }
            }
        }
    }

    fn run_until_depth_internal(&mut self, target_depth: usize) -> Result<JsValue, VmError> {
        while self.frames.len() > target_depth {
            if self.objects.len() >= self.gc_threshold {
                self.gc_collect();
                if self.objects.len() * 2 > self.gc_threshold {
                    self.gc_threshold = (self.objects.len() * 2).max(4096);
                }
            }
            if self.oom {
                return Err(VmError::RangeError("Out of memory: allocation limit reached".to_string()));
            }
            if let Some(max) = self.max_instructions {
                self.instruction_count += 1;
                if self.instruction_count > max {
                    return Err(VmError::RuntimeError(
                        "Execution step limit exceeded (timeout)".to_string(),
                    ));
                }
            }
            if self.instruction_count % 1024 == 0 {
                if let Some(deadline) = self.deadline {
                    if Instant::now() > deadline {
                        return Err(VmError::RuntimeError(
                            "Execution timeout exceeded".to_string(),
                        ));
                    }
                }
            }
            let (op, chunk) = {
                let frame = self.frames.last_mut().unwrap();
                if frame.ip >= frame.chunk.instructions.len() {
                    let finished = self.frames.pop().unwrap();
                    while let Some(top) = self.catch_stack.last() {
                        if top.frame_idx >= self.frames.len() {
                            self.catch_stack.pop();
                        } else {
                            break;
                        }
                    }
                    let ret_val = if self.stack.len() > finished.base_sp {
                        self.stack.pop().unwrap_or_default()
                    } else {
                        JsValue::undefined()
                    };
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
                        Constant::BigInt(bi) => self.alloc_bigint(bi.clone()),
                        Constant::String(s) => self.alloc_string(s.clone()),
                        Constant::Boolean(b) => JsValue::from_boolean(*b),
                        Constant::Function(f) => {
                            let upvalues = self.capture_upvalues();
                            let js_fn = JsFunction {
                                name: f.name.clone(),
                                params: f.params.clone(),
                                local_names: f.local_names.clone(),
                                lexical_local_names: f.lexical_local_names.clone(),
                                chunk: Arc::new(f.chunk.clone()),
                                upvalues,
                                is_async: false,
                                is_generator: f.is_generator,
                                call_count: 0,
                                jit_entry: None,
                                is_strict: f.is_strict,
                                has_param_exprs: f.has_param_exprs,
                                is_method: f.is_method,
                                is_arrow: f.is_arrow,
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
                OpCode::LoadSuper => {
                    let this_val = self.frames.last().unwrap().this_val;
                    if let Some(ptr) = this_val.as_object_ptr() {
                        let obj = unsafe { &*(ptr as *mut JsObject) };
                        self.push(obj.prototype.unwrap_or(JsValue::null()));
                    } else {
                        self.push(JsValue::null());
                    }
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

                OpCode::ArrayPush => {
                    let val = self.pop()?;
                    let arr_val = *self.peek()?;
                    if let Some(ptr) = arr_val.as_object_ptr() {
                        let jsobj = unsafe { &mut *(ptr as *mut JsObject) };
                        if let ObjectKind::Array(elements) = &mut jsobj.kind {
                            elements.push(val);
                        } else {
                            let len = self.get_property(arr_val, "length").to_number().unwrap_or(0.0) as usize;
                            self.set_property(arr_val, &len.to_string(), val);
                            self.set_property(arr_val, "length", JsValue::from_int32((len + 1) as i32));
                        }
                    }
                }

                OpCode::ArraySpreadPush => {
                    let spread_target = self.pop()?;
                    let items = match self.to_spread_args(spread_target) {
                        Ok(items) => items,
                        Err(err) => {
                            self.handle_exception_at_depth(err, target_depth)?;
                            continue;
                        }
                    };
                    let arr_val = *self.peek()?;
                    if let Some(ptr) = arr_val.as_object_ptr() {
                        let jsobj = unsafe { &mut *(ptr as *mut JsObject) };
                        if let ObjectKind::Array(elements) = &mut jsobj.kind {
                            elements.extend(items);
                        } else {
                            let mut len = self.get_property(arr_val, "length").to_number().unwrap_or(0.0) as usize;
                            for item in items {
                                self.set_property(arr_val, &len.to_string(), item);
                                len += 1;
                            }
                            self.set_property(arr_val, "length", JsValue::from_int32(len as i32));
                        }
                    }
                }
                OpCode::RequireObjectCoercible => {
                    let val = *self.peek()?;
                    if val.is_null() || val.is_undefined() {
                        self.handle_exception_at_depth(
                            VmError::TypeError("Cannot destructure 'undefined' or 'null'".into()),
                            target_depth,
                        )?;
                        continue;
                    }
                }
                OpCode::CreateRegExp(p_idx, f_idx) => {
                    let pattern = match &chunk.constants[p_idx as usize] {
                        Constant::String(s) => s.clone(),
                        _ => String::new(),
                    };
                    let flags = match &chunk.constants[f_idx as usize] {
                        Constant::String(s) => s.clone(),
                        _ => String::new(),
                    };
                    let re = self.alloc_regexp(pattern, flags);
                    self.push(re);
                }
                OpCode::GetKeys => {
                    let obj_val = self.pop()?;
                    if self.module_namespace_objects.contains(&obj_val) {
                        if let Some(ptr) = obj_val.as_object_ptr() {
                            let obj = unsafe { &*(ptr as *mut JsObject) };
                            let names = self.shapes.all_properties(obj.shape_id);
                            for name in names {
                                if !self.deleted_props.contains(&(obj_val, name.clone())) {
                                    let val = self.get_property(obj_val, &name);
                                    if val == JsValue::uninitialized() {
                                        self.handle_exception_at_depth(
                                            VmError::ReferenceError(format!("Cannot access '{}' before initialization", name)),
                                            target_depth,
                                        )?;
                                        continue;
                                    }
                                }
                            }
                        }
                    }
                    let keys_to_alloc = self.get_for_in_keys(obj_val);
                    let keys_vec: Vec<JsValue> = keys_to_alloc.into_iter().map(|k| self.alloc_string(k)).collect();
                    let arr = self.alloc_array(keys_vec);
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
                        let has_acc = jsobj.accessors.as_ref().map_or(false, |a| a.contains_key(&name));
                        if !has_acc && !self.module_namespace_objects.contains(&obj) {
                            let ic = &self.inline_caches[ic_idx];
                            if ic.state == crate::inline_cache::IcState::Monomorphic
                                && ic.cached_name.as_deref() == Some(&name)
                                && ic.cached_shape_id == Some(jsobj.shape_id)
                            {
                                if let Some(offset) = ic.cached_offset {
                                    if let Some(val) = jsobj.get_slot(offset) {
                                        self.push(val);
                                        continue;
                                    }
                                }
                            }
                        }
                    }

                    let val = match self.get_property_result(obj, &name) {
                        Ok(v) => v,
                        Err(err) => {
                            self.handle_exception_at_depth(err, target_depth)?;
                            continue;
                        }
                    };

                    if let Some(ptr) = obj.as_object_ptr() {
                        let jsobj = unsafe { &*(ptr as *mut JsObject) };
                        let has_acc = jsobj.accessors.as_ref().map_or(false, |a| a.contains_key(&name));
                        if !has_acc {
                            if let Some(offset) = self.shapes.lookup_offset(jsobj.shape_id, &name) {
                                self.inline_caches[ic_idx].record_hit(&name, jsobj.shape_id, offset);
                            }
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

                    if self.non_writable_props.contains(&(obj, name.clone())) {
                        let is_strict = self.frames.last().map(|f| f.is_strict).unwrap_or(self.is_strict);
                        if is_strict {
                            self.handle_exception_at_depth(
                                VmError::TypeError(format!("Cannot assign to read only property '{}'", name)),
                                target_depth,
                            )?;
                            continue;
                        }
                        self.push(val);
                        continue;
                    }

                    let ic_idx = name_idx as usize;
                    if ic_idx >= self.inline_caches.len() {
                        self.inline_caches.resize(ic_idx + 1, InlineCacheSlot::new());
                    }

                    if let Some(ptr) = obj.as_object_ptr() {
                        let jsobj = unsafe { &mut *(ptr as *mut JsObject) };
                        let has_acc = jsobj.accessors.as_ref().map_or(false, |a| a.contains_key(&name));
                        if !has_acc && jsobj.prototype.is_none() {
                            let ic = &self.inline_caches[ic_idx];
                            if ic.state == crate::inline_cache::IcState::Monomorphic
                                && ic.cached_name.as_deref() == Some(&name)
                                && ic.cached_shape_id == Some(jsobj.shape_id)
                            {
                                if let Some(offset) = ic.cached_offset {
                                    jsobj.set_slot(offset, val);
                                    self.push(val);
                                    continue;
                                }
                            }
                        }
                    }

                    if let Err(err) = self.set_property_result(obj, &name, val) {
                        self.handle_exception_at_depth(err, target_depth)?;
                        continue;
                    }

                    if let Some(ptr) = obj.as_object_ptr() {
                        let jsobj = unsafe { &*(ptr as *mut JsObject) };
                        let has_acc = jsobj.accessors.as_ref().map_or(false, |a| a.contains_key(&name));
                        if !has_acc && jsobj.prototype.is_none() {
                            if let Some(offset) = self.shapes.lookup_offset(jsobj.shape_id, &name) {
                                self.inline_caches[ic_idx].record_hit(&name, jsobj.shape_id, offset);
                            }
                        }
                    }

                    self.push(val);
                }
                OpCode::DefineGetter(prop_idx) => {
                    let name = match &chunk.constants[prop_idx as usize] {
                        Constant::String(s) => s.clone(),
                        _ => return Err(VmError::TypeError("Property name must be string".into())),
                    };
                    let getter = self.pop()?;
                    let obj = self.pop()?;
                    if let Some(ptr) = obj.as_object_ptr() {
                        let jsobj = unsafe { &mut *(ptr as *mut JsObject) };
                        jsobj.define_getter(name, getter);
                    }
                }
                OpCode::DefineSetter(prop_idx) => {
                    let name = match &chunk.constants[prop_idx as usize] {
                        Constant::String(s) => s.clone(),
                        _ => return Err(VmError::TypeError("Property name must be string".into())),
                    };
                    let setter = self.pop()?;
                    let obj = self.pop()?;
                    if let Some(ptr) = obj.as_object_ptr() {
                        let jsobj = unsafe { &mut *(ptr as *mut JsObject) };
                        jsobj.define_setter(name, setter);
                    }
                }
                OpCode::DefineComputedGetter => {
                    let getter_val = self.pop()?;
                    let key_val = self.pop()?;
                    let obj_val = self.pop()?;
                    let prop_key = match self.to_property_key(key_val) {
                        Ok(k) => k,
                        Err(err) => {
                            self.handle_exception_at_depth(err, target_depth)?;
                            continue;
                        }
                    };
                    if let Some(ptr) = obj_val.as_object_ptr() {
                        let obj = unsafe { &mut *(ptr as *mut JsObject) };
                        match prop_key {
                            PropertyKeyVal::String(s) => {
                                let accs = obj.accessors.get_or_insert_with(|| Box::new(std::collections::HashMap::new()));
                                accs.entry(s).or_insert(Accessor { getter: None, setter: None }).getter = Some(getter_val);
                            }
                            PropertyKeyVal::Symbol(sym) => {
                                obj.insert_symbol_accessor_getter(sym, getter_val);
                            }
                        }
                    }
                }
                OpCode::DefineComputedSetter => {
                    let setter_val = self.pop()?;
                    let key_val = self.pop()?;
                    let obj_val = self.pop()?;
                    let prop_key = match self.to_property_key(key_val) {
                        Ok(k) => k,
                        Err(err) => {
                            self.handle_exception_at_depth(err, target_depth)?;
                            continue;
                        }
                    };
                    if let Some(ptr) = obj_val.as_object_ptr() {
                        let obj = unsafe { &mut *(ptr as *mut JsObject) };
                        match prop_key {
                            PropertyKeyVal::String(s) => {
                                let accs = obj.accessors.get_or_insert_with(|| Box::new(std::collections::HashMap::new()));
                                accs.entry(s).or_insert(Accessor { getter: None, setter: None }).setter = Some(setter_val);
                            }
                            PropertyKeyVal::Symbol(sym) => {
                                obj.insert_symbol_accessor_setter(sym, setter_val);
                            }
                        }
                    }
                }
                OpCode::DefineClassElement(flags) => {
                    let is_static = (flags & 1) != 0;
                    let kind = flags & 0xFE; // 0 = method, 2 = getter, 4 = setter
                    let func_val = self.pop()?;
                    let key_val = self.pop()?;
                    let ctor_val = self.pop()?;

                    let prop_key = match self.to_property_key(key_val) {
                        Ok(k) => k,
                        Err(err) => {
                            self.handle_exception_at_depth(err, target_depth)?;
                            continue;
                        }
                    };

                    if is_static {
                        if let PropertyKeyVal::String(ref s) = prop_key {
                            if s == "prototype" {
                                let err = VmError::TypeError("Cannot define static property named prototype".to_string());
                                self.handle_exception_at_depth(err, target_depth)?;
                                continue;
                            }
                        }
                    }

                    let target = if is_static {
                        ctor_val
                    } else {
                        self.get_property(ctor_val, "prototype")
                    };

                    if let Some(target_ptr) = target.as_object_ptr() {
                        let target_obj = unsafe { &mut *(target_ptr as *mut JsObject) };
                        match kind {
                            2 => { // Getter
                                match prop_key {
                                    PropertyKeyVal::String(s) => {
                                        self.non_enumerable_props.insert((target, s.clone()));
                                        let accs = target_obj.accessors.get_or_insert_with(|| Box::new(std::collections::HashMap::new()));
                                        accs.entry(s).or_insert(Accessor { getter: None, setter: None }).getter = Some(func_val);
                                    }
                                    PropertyKeyVal::Symbol(sym) => {
                                        target_obj.insert_symbol_accessor_getter(sym, func_val);
                                    }
                                }
                            }
                            4 => { // Setter
                                match prop_key {
                                    PropertyKeyVal::String(s) => {
                                        self.non_enumerable_props.insert((target, s.clone()));
                                        let accs = target_obj.accessors.get_or_insert_with(|| Box::new(std::collections::HashMap::new()));
                                        accs.entry(s).or_insert(Accessor { getter: None, setter: None }).setter = Some(func_val);
                                    }
                                    PropertyKeyVal::Symbol(sym) => {
                                        target_obj.insert_symbol_accessor_setter(sym, func_val);
                                    }
                                }
                            }
                            _ => { // Method
                                match prop_key {
                                    PropertyKeyVal::String(s) => {
                                        self.non_enumerable_props.insert((target, s.clone()));
                                        self.set_property(target, &s, func_val);
                                    }
                                    PropertyKeyVal::Symbol(sym) => {
                                        target_obj.insert_symbol_property(sym, func_val);
                                    }
                                }
                            }
                        }
                    }
                }
                OpCode::InheritClass => {
                    let super_ctor = self.pop()?;
                    let ctor = *self.stack.last().ok_or(VmError::StackUnderflow)?;

                    if let Some(_super_ptr) = super_ctor.as_object_ptr() {
                        if let Some(ctor_ptr) = ctor.as_object_ptr() {
                            let ctor_obj = unsafe { &mut *(ctor_ptr as *mut JsObject) };
                            ctor_obj.prototype = Some(super_ctor);

                            let ctor_proto = self.get_property(ctor, "prototype");
                            let super_proto = self.get_property(super_ctor, "prototype");
                            if let Some(ctor_proto_ptr) = ctor_proto.as_object_ptr() {
                                let ctor_proto_obj = unsafe { &mut *(ctor_proto_ptr as *mut JsObject) };
                                ctor_proto_obj.prototype = Some(super_proto);
                            }
                        }
                    }
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
                        if let ObjectKind::String(s) = &jsobj.kind {
                            if let Some(i) = key.to_int32() {
                                if i >= 0 {
                                    let idx = i as usize;
                                    if let Some(ch) = s.chars().nth(idx) {
                                        let ch_str = self.alloc_string(ch.to_string());
                                        self.push(ch_str);
                                        continue;
                                    }
                                }
                            }
                        }
                        if let ObjectKind::TypedArray(ta) = &jsobj.kind {
                            if let Some(i) = key.to_int32() {
                                if i >= 0 {
                                    if let Some(val) = Self::read_typed_array_element(ta, i as usize) {
                                        self.push(val);
                                        continue;
                                    }
                                }
                            }
                        }
                    }
                    let prop_key = match self.to_property_key(key) {
                        Ok(k) => k,
                        Err(err) => {
                            self.handle_exception_at_depth(err, target_depth)?;
                            continue;
                        }
                    };
                    let val = match prop_key {
                        PropertyKeyVal::Symbol(sym) => {
                            match self.get_symbol_property_result(obj, sym) {
                                Ok(v) => v,
                                Err(err) => {
                                    self.handle_exception_at_depth(err, target_depth)?;
                                    continue;
                                }
                            }
                        }
                        PropertyKeyVal::String(key_str) => {
                            match self.get_property_result(obj, &key_str) {
                                Ok(v) => v,
                                Err(err) => {
                                    self.handle_exception_at_depth(err, target_depth)?;
                                    continue;
                                }
                            }
                        }
                    };
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
                                if i >= 0 {
                                    if Self::write_typed_array_element(ta, i as usize, val) {
                                        self.push(val);
                                        continue;
                                    }
                                }
                            }
                        }
                    }
                    let prop_key = match self.to_property_key(key) {
                        Ok(k) => k,
                        Err(err) => {
                            self.handle_exception_at_depth(err, target_depth)?;
                            continue;
                        }
                    };
                    match prop_key {
                        PropertyKeyVal::Symbol(sym) => {
                            if let Err(err) = self.set_symbol_property_result(obj, sym, val) {
                                self.handle_exception_at_depth(err, target_depth)?;
                                continue;
                            }
                        }
                        PropertyKeyVal::String(key_str) => {
                            if let Err(err) = self.set_property_result(obj, &key_str, val) {
                                self.handle_exception_at_depth(err, target_depth)?;
                                continue;
                            }
                        }
                    }
                    self.push(val);
                }

                OpCode::CreateFunction(const_idx) => {
                    let def = match &chunk.constants[const_idx as usize] {
                        Constant::Function(f) => Arc::clone(f),
                        _ => return Err(VmError::TypeError("Expected function def".into())),
                    };
                    let upvalues = self.capture_upvalues();
                    let js_fn = JsFunction {
                        name: def.name.clone(),
                        params: def.params.clone(),
                        local_names: def.local_names.clone(),
                        lexical_local_names: def.lexical_local_names.clone(),
                        chunk: Arc::new(def.chunk.clone()),
                        upvalues,
                        is_async: false,
                        is_generator: false,
                        call_count: 0,
                        jit_entry: None,
                        is_strict: def.is_strict,
                        has_param_exprs: def.has_param_exprs,
                        is_method: def.is_method,
                        is_arrow: def.is_arrow,
                    };
                    let func_val = self.alloc_function(js_fn);
                    self.push(func_val);
                }

                OpCode::CreateAsyncFunction(const_idx) => {
                    let def = match &chunk.constants[const_idx as usize] {
                        Constant::Function(f) => Arc::clone(f),
                        _ => return Err(VmError::TypeError("Expected function def".into())),
                    };
                    let upvalues = self.capture_upvalues();
                    let js_fn = JsFunction {
                        name: def.name.clone(),
                        params: def.params.clone(),
                        local_names: def.local_names.clone(),
                        lexical_local_names: def.lexical_local_names.clone(),
                        chunk: Arc::new(def.chunk.clone()),
                        upvalues,
                        is_async: true,
                        is_generator: false,
                        call_count: 0,
                        jit_entry: None,
                        is_strict: def.is_strict,
                        has_param_exprs: def.has_param_exprs,
                        is_method: def.is_method,
                        is_arrow: def.is_arrow,
                    };
                    let func_val = self.alloc_function(js_fn);
                    self.push(func_val);
                }

                OpCode::CreateGenerator(const_idx) => {
                    let def = match &chunk.constants[const_idx as usize] {
                        Constant::Function(f) => Arc::clone(f),
                        _ => return Err(VmError::TypeError("Expected function def".into())),
                    };
                    let upvalues = self.capture_upvalues();
                    let js_fn = JsFunction {
                        name: def.name.clone(),
                        params: def.params.clone(),
                        local_names: def.local_names.clone(),
                        lexical_local_names: def.lexical_local_names.clone(),
                        chunk: Arc::new(def.chunk.clone()),
                        upvalues,
                        is_async: false,
                        is_generator: true,
                        call_count: 0,
                        jit_entry: None,
                        is_strict: def.is_strict,
                        has_param_exprs: def.has_param_exprs,
                        is_method: def.is_method,
                        is_arrow: def.is_arrow,
                    };
                    let func_val = self.alloc_function(js_fn);
                    self.push(func_val);
                }

                OpCode::CreateAsyncGenerator(const_idx) => {
                    let def = match &chunk.constants[const_idx as usize] {
                        Constant::Function(f) => Arc::clone(f),
                        _ => return Err(VmError::TypeError("Expected function def".into())),
                    };
                    let upvalues = self.capture_upvalues();
                    let js_fn = JsFunction {
                        name: def.name.clone(),
                        params: def.params.clone(),
                        local_names: def.local_names.clone(),
                        lexical_local_names: def.lexical_local_names.clone(),
                        chunk: Arc::new(def.chunk.clone()),
                        upvalues,
                        is_async: true,
                        is_generator: true,
                        call_count: 0,
                        jit_entry: None,
                        is_strict: def.is_strict,
                        has_param_exprs: def.has_param_exprs,
                        is_method: def.is_method,
                        is_arrow: def.is_arrow,
                    };
                    let func_val = self.alloc_function(js_fn);
                    self.push(func_val);
                }

                OpCode::GeneratorStart => {
                    let frame = self.frames.pop().unwrap();
                    let gen_func = frame.callee_func.clone().unwrap();
                    let generator_instance = JsGenerator {
                        function: gen_func,
                        ip: frame.ip,
                        stack: frame.locals,
                        state: crate::object::GeneratorState::SuspendedStart,
                        this_val: frame.this_val,
                        arguments: frame.arguments,
                        mapped_arguments: frame.mapped_arguments,
                    };
                    let gen_val = self.alloc_generator(generator_instance);
                    self.stack.truncate(frame.base_sp);
                    return Ok(gen_val);
                }

                OpCode::Call(argc) => {
                    let mut args = Vec::with_capacity(argc as usize);
                    for _ in 0..argc {
                        args.push(self.pop()?);
                    }
                    args.reverse();
                    let callee = self.pop()?;
                    match self.execute_call_internal(callee, JsValue::undefined(), args) {
                        Ok(Some(res)) => self.push(res),
                        Ok(None) => {},
                        Err(err) => {
                            self.handle_exception_at_depth(err, target_depth)?;
                        }
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
                    match self.execute_call_internal(callee, this_val, args) {
                        Ok(Some(res)) => self.push(res),
                        Ok(None) => {},
                        Err(err) => {
                            self.handle_exception_at_depth(err, target_depth)?;
                        }
                    }
                }

                OpCode::CallSpread => {
                    let args_val = self.pop()?;
                    let callee = self.pop()?;
                    let args = self.array_to_vec_spread(args_val);
                    match self.execute_call_internal(callee, JsValue::undefined(), args) {
                        Ok(Some(res)) => self.push(res),
                        Ok(None) => {},
                        Err(err) => {
                            self.handle_exception_at_depth(err, target_depth)?;
                        }
                    }
                }

                OpCode::CallWithThisSpread => {
                    let args_val = self.pop()?;
                    let callee = self.pop()?;
                    let this_val = self.pop()?;
                    let args = self.array_to_vec_spread(args_val);
                    match self.execute_call_internal(callee, this_val, args) {
                        Ok(Some(res)) => self.push(res),
                        Ok(None) => {},
                        Err(err) => {
                            self.handle_exception_at_depth(err, target_depth)?;
                        }
                    }
                }

                OpCode::CallDirectEval(argc) => {
                    let mut args = Vec::with_capacity(argc as usize);
                    for _ in 0..argc {
                        args.push(self.pop()?);
                    }
                    args.reverse();
                    let callee = self.pop()?;
                    let is_eval = if let Some(eval_val) = self.globals.get("eval") {
                        *eval_val == callee
                    } else {
                        false
                    };
                    if is_eval {
                        self.in_direct_eval = true;
                    }
                    let res = self.execute_call_internal(callee, JsValue::undefined(), args);
                    self.in_direct_eval = false;
                    match res {
                        Ok(Some(res)) => self.push(res),
                        Ok(None) => {},
                        Err(err) => {
                            self.handle_exception_at_depth(err, target_depth)?;
                        }
                    }
                }

                OpCode::GetNewTarget => {
                    let target = self.frames.last().and_then(|f| f.new_target).unwrap_or(JsValue::undefined());
                    self.push(target);
                }

                OpCode::Construct(argc) => {
                    let mut args = Vec::with_capacity(argc as usize);
                    for _ in 0..argc {
                        args.push(self.pop()?);
                    }
                    args.reverse();
                    let callee = self.pop()?;
                    self.execute_construct(callee, args, target_depth)?;
                }

                OpCode::ConstructSpread => {
                    let args_val = self.pop()?;
                    let callee = self.pop()?;
                    let args = self.array_to_vec_spread(args_val);
                    self.execute_construct(callee, args, target_depth)?;
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
                                        self.handle_exception_at_depth(VmError::Thrown(p.result), target_depth)?;
                                        continue;
                                    }
                                    PromiseState::Pending => val,
                                }
                            } else {
                                val
                            }
                        } else {
                            let then_fn = self.get_property(val, "then");
                            if self.is_callable(then_fn) {
                                let result_val = Arc::new(std::sync::Mutex::new(None));
                                let error_val = Arc::new(std::sync::Mutex::new(None));
                                let res_clone = Arc::clone(&result_val);
                                let rej_clone = Arc::clone(&error_val);
                                let on_resolve = self.alloc_closure_native_function("resolve", 1, Arc::new(move |_vm, _this, args| {
                                    let v = args.first().copied().unwrap_or_else(JsValue::undefined);
                                    let mut l = res_clone.lock().unwrap();
                                    *l = Some(v);
                                    Ok(JsValue::undefined())
                                }));
                                let on_reject = self.alloc_closure_native_function("reject", 1, Arc::new(move |_vm, _this, args| {
                                    let e = args.first().copied().unwrap_or_else(JsValue::undefined);
                                    let mut l = rej_clone.lock().unwrap();
                                    *l = Some(e);
                                    Ok(JsValue::undefined())
                                }));
                                match self.call(then_fn, val, &[on_resolve, on_reject]) {
                                    Ok(_) => {
                                        self.drain_microtasks()?;
                                        if let Some(e) = error_val.lock().unwrap().take() {
                                            self.handle_exception_at_depth(VmError::Thrown(e), target_depth)?;
                                            continue;
                                        }
                                        let final_res = result_val.lock().unwrap().take();
                                        final_res.unwrap_or(val)
                                    }
                                    Err(err) => {
                                        self.handle_exception_at_depth(err, target_depth)?;
                                        continue;
                                    }
                                }
                            } else {
                                val
                            }
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
                    let val = self.pop().unwrap_or(JsValue::undefined());
                    if name == "default" {
                        if let Some(ptr) = val.as_object_ptr() {
                            let obj = unsafe { &*(ptr as *mut JsObject) };
                            if matches!(obj.kind, ObjectKind::Function(_) | ObjectKind::NativeFunction(_)) {
                                let curr_name = self.get_property(val, "name");
                                if curr_name.is_string() && self.to_string_value(curr_name).is_empty() {
                                    let dflt_str = self.alloc_string("default".to_string());
                                    self.set_property_direct(val, "name", dflt_str);
                                }
                            }
                        }
                    }
                    self.current_exports.insert(name.clone(), val);
                    if let Some(mod_obj) = self.current_module_obj {
                        self.set_property_direct(mod_obj, &name, val);
                        self.non_configurable_props.insert((mod_obj, name.clone()));
                    }
                }

                OpCode::ExportAll => {
                    let mod_obj = self.pop()?;
                    if let Some(ptr) = mod_obj.as_object_ptr() {
                        let obj = unsafe { &*(ptr as *mut JsObject) };
                        let mut names = self.shapes.all_properties(obj.shape_id);
                        for ((re_mod, re_name), _) in &self.module_reexports {
                            if *re_mod == mod_obj && !names.contains(re_name) {
                                names.push(re_name.clone());
                            }
                        }
                        for name in names {
                            if name != "default" && !name.starts_with("Symbol(") {
                                let val = self.get_property(mod_obj, &name);
                                if let Some(existing_mod) = self.star_exports.get(&name).copied() {
                                    if existing_mod != mod_obj {
                                        // Ambiguous export - omit from module namespace object
                                        self.current_exports.remove(&name);
                                        if let Some(cur) = self.current_module_obj {
                                            let _ = self.delete_property(cur, &name);
                                        }
                                        continue;
                                    }
                                } else {
                                    self.star_exports.insert(name.clone(), mod_obj);
                                    self.current_exports.insert(name.clone(), val);
                                    if let Some(cur) = self.current_module_obj {
                                        self.set_property_direct(cur, &name, val);
                                        self.non_configurable_props.insert((cur, name.clone()));
                                    }
                                }
                            }
                        }
                    }
                }

                OpCode::BindImport(name_idx, exp_idx) => {
                    let bind_name = match &chunk.constants[name_idx as usize] {
                        Constant::String(s) => s.clone(),
                        _ => return Err(VmError::TypeError("Import bind name must be string".into())),
                    };
                    let exp_name = match &chunk.constants[exp_idx as usize] {
                        Constant::String(s) => s.clone(),
                        _ => return Err(VmError::TypeError("Import export name must be string".into())),
                    };
                    let mod_obj = *self.peek()?;
                    self.imported_bindings.insert(bind_name.clone(), (mod_obj, exp_name.clone()));
                    let val = if exp_name == "*" {
                        mod_obj
                    } else {
                        self.get_property(mod_obj, &exp_name)
                    };
                    self.global_lexicals.insert(bind_name.clone(), val);
                    if self.globals.contains_key(&bind_name) {
                        self.globals.insert(bind_name.clone(), val);
                    }
                }

                OpCode::ReExport(target_name_idx, exp_name_idx) => {
                    let target_name = match &chunk.constants[target_name_idx as usize] {
                        Constant::String(s) => s.clone(),
                        _ => return Err(VmError::TypeError("ReExport target name must be string".into())),
                    };
                    let exp_name = match &chunk.constants[exp_name_idx as usize] {
                        Constant::String(s) => s.clone(),
                        _ => return Err(VmError::TypeError("ReExport export name must be string".into())),
                    };
                    let tgt_mod = *self.peek()?;
                    if let Some(cur_mod) = self.current_module_obj {
                        self.module_reexports.insert((cur_mod, exp_name.clone()), (tgt_mod, target_name.clone()));
                        let init_val = self.get_property(tgt_mod, &target_name);
                        self.set_property_direct(cur_mod, &exp_name, init_val);
                        self.non_configurable_props.insert((cur_mod, exp_name.clone()));
                        self.current_exports.insert(exp_name, init_val);
                    }
                }

                OpCode::ImportModule(src_idx) => {
                    let src = match &chunk.constants[src_idx as usize] {
                        Constant::String(s) => s.clone(),
                        _ => return Err(VmError::TypeError("Import source must be string".into())),
                    };
                    let file_path = if let Some(ref base) = self.base_dir {
                        base.join(&src)
                    } else {
                        std::path::PathBuf::from(&src)
                    };
                    let canonical_key = file_path.canonicalize().unwrap_or_else(|_| file_path.clone())
                        .to_string_lossy().to_string();
                    let mod_obj = if let Some(m) = self.modules.get(&canonical_key).copied() {
                        m
                    } else if let Some(m) = self.modules.get(&src).copied() {
                        m
                    } else {
                        let m = self.alloc_object();
                        self.set_prototype(m, JsValue::null());
                        if let Some(tag_sym) = self.symbol_to_string_tag {
                            let tag_val = self.alloc_string("Module".to_string());
                            self.set_symbol_property(m, tag_sym, tag_val);
                            self.non_configurable_props.insert((m, "[Symbol.toStringTag]".to_string()));
                            self.non_enumerable_props.insert((m, "[Symbol.toStringTag]".to_string()));
                            self.non_writable_props.insert((m, "[Symbol.toStringTag]".to_string()));
                        }
                        self.module_namespace_objects.insert(m);
                        self.non_extensible_objects.insert(m);
                        self.modules.insert(canonical_key.clone(), m);
                        self.modules.insert(src.clone(), m);
                        if let Some(name) = file_path.file_name() {
                            self.modules.insert(format!("./{}", name.to_string_lossy()), m);
                            self.modules.insert(name.to_string_lossy().to_string(), m);
                        }

                        if let Ok(content) = std::fs::read_to_string(&file_path) {
                            if let Ok(prog) = doms_parser::parse_module(&content) {
                                if let Ok(c) = doms_compiler::compile_strict(&prog) {
                                    let old_exports = std::mem::take(&mut self.current_exports);
                                    let old_is_mod = self.is_module;
                                    let old_base = self.base_dir.clone();
                                    let old_mod_obj = self.current_module_obj;
                                    let old_lexicals = std::mem::take(&mut self.global_lexicals);
                                    let old_globals = self.globals.clone();
                                    let old_microtasks = std::mem::take(&mut self.microtasks);
                                    let old_consts = std::mem::take(&mut self.const_bindings);
                                    let old_imported = std::mem::take(&mut self.imported_bindings);
                                    let old_star_exports = std::mem::take(&mut self.star_exports);
                                    let old_var_names = std::mem::take(&mut self.var_names);
                                    let old_script_vars = std::mem::take(&mut self.script_var_names);

                                    self.is_module = true;
                                    self.base_dir = file_path.parent().map(|p| p.to_path_buf());
                                    self.current_module_obj = Some(m);

                                    let _ = self.run(&c);

                                    self.is_module = old_is_mod;
                                    self.base_dir = old_base;
                                    self.current_module_obj = old_mod_obj;
                                    self.global_lexicals = old_lexicals;
                                    self.globals = old_globals;
                                    self.microtasks = old_microtasks;
                                    self.const_bindings = old_consts;
                                    self.imported_bindings = old_imported;
                                    self.star_exports = old_star_exports;
                                    self.var_names = old_var_names;
                                    self.script_var_names = old_script_vars;
                                    let exports = std::mem::replace(&mut self.current_exports, old_exports);
                                    for (k, v) in exports {
                                        self.set_property_direct(m, &k, v);
                                        self.non_configurable_props.insert((m, k.clone()));
                                    }
                                }
                            }
                        }
                        m
                    };
                    self.push(mod_obj);
                }

                OpCode::DynamicImport => {
                    let specifier_val = self.pop()?;
                    let src = self.to_string_value(specifier_val);
                    let file_path = if let Some(ref base) = self.base_dir {
                        base.join(&src)
                    } else {
                        std::path::PathBuf::from(&src)
                    };
                    let canonical_key = file_path.canonicalize().unwrap_or_else(|_| file_path.clone())
                        .to_string_lossy().to_string();
                    let mod_obj = if let Some(m) = self.modules.get(&canonical_key).copied() {
                        m
                    } else if let Some(m) = self.modules.get(&src).copied() {
                        m
                    } else {
                        let m = self.alloc_object();
                        self.set_prototype(m, JsValue::null());
                        if let Some(tag_sym) = self.symbol_to_string_tag {
                            let tag_val = self.alloc_string("Module".to_string());
                            self.set_symbol_property(m, tag_sym, tag_val);
                            self.non_configurable_props.insert((m, "[Symbol.toStringTag]".to_string()));
                            self.non_enumerable_props.insert((m, "[Symbol.toStringTag]".to_string()));
                            self.non_writable_props.insert((m, "[Symbol.toStringTag]".to_string()));
                        }
                        self.module_namespace_objects.insert(m);
                        self.non_extensible_objects.insert(m);
                        self.modules.insert(canonical_key.clone(), m);
                        self.modules.insert(src.clone(), m);
                        if let Some(name) = file_path.file_name() {
                            self.modules.insert(format!("./{}", name.to_string_lossy()), m);
                            self.modules.insert(name.to_string_lossy().to_string(), m);
                        }

                        if let Ok(content) = std::fs::read_to_string(&file_path) {
                            if let Ok(prog) = doms_parser::parse_module(&content).or_else(|_| doms_parser::parse(&content)) {
                                if let Ok(c) = doms_compiler::compile_strict(&prog) {
                                    let old_exports = std::mem::take(&mut self.current_exports);
                                    let old_is_mod = self.is_module;
                                    let old_base = self.base_dir.clone();
                                    let old_mod_obj = self.current_module_obj;
                                    let old_lexicals = std::mem::take(&mut self.global_lexicals);
                                    let old_globals = self.globals.clone();
                                    let old_microtasks = std::mem::take(&mut self.microtasks);
                                    let old_consts = std::mem::take(&mut self.const_bindings);
                                    let old_imported = std::mem::take(&mut self.imported_bindings);
                                    let old_star_exports = std::mem::take(&mut self.star_exports);
                                    let old_var_names = std::mem::take(&mut self.var_names);
                                    let old_script_vars = std::mem::take(&mut self.script_var_names);

                                    self.is_module = true;
                                    self.base_dir = file_path.parent().map(|p| p.to_path_buf());
                                    self.current_module_obj = Some(m);

                                    let _ = self.run(&c);

                                    self.is_module = old_is_mod;
                                    self.base_dir = old_base;
                                    self.current_module_obj = old_mod_obj;
                                    self.global_lexicals = old_lexicals;
                                    self.globals = old_globals;
                                    self.microtasks = old_microtasks;
                                    self.const_bindings = old_consts;
                                    self.imported_bindings = old_imported;
                                    self.star_exports = old_star_exports;
                                    self.var_names = old_var_names;
                                    self.script_var_names = old_script_vars;
                                    let exports = std::mem::replace(&mut self.current_exports, old_exports);
                                    for (k, v) in exports {
                                        self.set_property(m, &k, v);
                                        self.non_configurable_props.insert((m, k.clone()));
                                    }
                                }
                            }
                        }
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

                    let a_is_sym = a_prim.is_symbol() || (a_prim.is_object() && a_prim.as_object_ptr().is_some_and(|p| matches!(unsafe { &(*(p as *mut JsObject)).kind }, ObjectKind::Symbol(_))));
                    let b_is_sym = b_prim.is_symbol() || (b_prim.is_object() && b_prim.as_object_ptr().is_some_and(|p| matches!(unsafe { &(*(p as *mut JsObject)).kind }, ObjectKind::Symbol(_))));
                    if a_is_sym || b_is_sym {
                        return Err(VmError::TypeError("Cannot convert a Symbol value to a number or string".into()));
                    }

                    let a_is_str = a_prim.is_string() || (a_prim.is_object() && a_prim.as_object_ptr().is_some_and(|p| matches!(unsafe { &(*(p as *mut JsObject)).kind }, ObjectKind::String(_))));
                    let b_is_str = b_prim.is_string() || (b_prim.is_object() && b_prim.as_object_ptr().is_some_and(|p| matches!(unsafe { &(*(p as *mut JsObject)).kind }, ObjectKind::String(_))));

                    if a_is_str || b_is_str {
                        let sa = self.to_string_value(a_prim);
                        let sb = self.to_string_value(b_prim);
                        let combined = format!("{}{}", sa, sb);
                        let res = self.alloc_string(combined);
                        self.push(res);
                    } else if let (Some(ba), Some(bb)) = (self.as_bigint_val(a_prim), self.as_bigint_val(b_prim)) {
                        let res = self.alloc_bigint(&ba + &bb);
                        self.push(res);
                    } else if self.as_bigint_val(a_prim).is_some() || self.as_bigint_val(b_prim).is_some() {
                        return Err(VmError::TypeError("Cannot mix BigInt and other types, use explicit conversions".into()));
                    } else {
                        let na = self.to_number_value(a_prim)?;
                        let nb = self.to_number_value(b_prim)?;
                        self.push(JsValue::from_double(na + nb));
                    }
                }
                OpCode::Sub => {
                    let b_val = self.pop()?;
                    let a_val = self.pop()?;
                    let a_num = self.to_numeric(a_val)?;
                    let b_num = self.to_numeric(b_val)?;
                    if let (Some(ba), Some(bb)) = (self.as_bigint_val(a_num), self.as_bigint_val(b_num)) {
                        let res = self.alloc_bigint(&ba - &bb);
                        self.push(res);
                    } else if self.as_bigint_val(a_num).is_some() || self.as_bigint_val(b_num).is_some() {
                        return Err(VmError::TypeError("Cannot mix BigInt and other types, use explicit conversions".into()));
                    } else {
                        let a = a_num.to_number().unwrap_or(f64::NAN);
                        let b = b_num.to_number().unwrap_or(f64::NAN);
                        self.push(JsValue::from_double(a - b));
                    }
                }
                OpCode::Mul => {
                    let b_val = self.pop()?;
                    let a_val = self.pop()?;
                    let a_num = self.to_numeric(a_val)?;
                    let b_num = self.to_numeric(b_val)?;
                    if let (Some(ba), Some(bb)) = (self.as_bigint_val(a_num), self.as_bigint_val(b_num)) {
                        let res = self.alloc_bigint(&ba * &bb);
                        self.push(res);
                    } else if self.as_bigint_val(a_num).is_some() || self.as_bigint_val(b_num).is_some() {
                        return Err(VmError::TypeError("Cannot mix BigInt and other types, use explicit conversions".into()));
                    } else {
                        let a = a_num.to_number().unwrap_or(f64::NAN);
                        let b = b_num.to_number().unwrap_or(f64::NAN);
                        self.push(JsValue::from_double(a * b));
                    }
                }
                OpCode::Div => {
                    let b_val = self.pop()?;
                    let a_val = self.pop()?;
                    let a_num = self.to_numeric(a_val)?;
                    let b_num = self.to_numeric(b_val)?;
                    if let (Some(ba), Some(bb)) = (self.as_bigint_val(a_num), self.as_bigint_val(b_num)) {
                        if bb.is_zero() {
                            return Err(VmError::RangeError("Division by zero".into()));
                        }
                        let res = self.alloc_bigint(&ba / &bb);
                        self.push(res);
                    } else if self.as_bigint_val(a_num).is_some() || self.as_bigint_val(b_num).is_some() {
                        return Err(VmError::TypeError("Cannot mix BigInt and other types, use explicit conversions".into()));
                    } else {
                        let a = a_num.to_number().unwrap_or(f64::NAN);
                        let b = b_num.to_number().unwrap_or(f64::NAN);
                        self.push(JsValue::from_double(a / b));
                    }
                }
                OpCode::Mod => {
                    let b_val = self.pop()?;
                    let a_val = self.pop()?;
                    let a_num = self.to_numeric(a_val)?;
                    let b_num = self.to_numeric(b_val)?;
                    if let (Some(ba), Some(bb)) = (self.as_bigint_val(a_num), self.as_bigint_val(b_num)) {
                        if bb.is_zero() {
                            return Err(VmError::RangeError("Division by zero".into()));
                        }
                        let res = self.alloc_bigint(&ba % &bb);
                        self.push(res);
                    } else if self.as_bigint_val(a_num).is_some() || self.as_bigint_val(b_num).is_some() {
                        return Err(VmError::TypeError("Cannot mix BigInt and other types, use explicit conversions".into()));
                    } else {
                        let a = a_num.to_number().unwrap_or(f64::NAN);
                        let b = b_num.to_number().unwrap_or(f64::NAN);
                        self.push(JsValue::from_double(a % b));
                    }
                }
                OpCode::Exp => {
                    let b_val = self.pop()?;
                    let a_val = self.pop()?;
                    let a_num = self.to_numeric(a_val)?;
                    let b_num = self.to_numeric(b_val)?;
                    if let (Some(ba), Some(bb)) = (self.as_bigint_val(a_num), self.as_bigint_val(b_num)) {
                        if bb.is_negative() {
                            return Err(VmError::RangeError("Exponent must be non-negative".into()));
                        } else if bb.is_zero() {
                            let res = self.alloc_bigint(BigInt::one());
                            self.push(res);
                        } else {
                            let exp = bb.to_u32().ok_or_else(|| VmError::RangeError("Maximum BigInt size exceeded".into()))?;
                            let res = self.alloc_bigint(ba.pow(exp));
                            self.push(res);
                        }
                    } else if self.as_bigint_val(a_num).is_some() || self.as_bigint_val(b_num).is_some() {
                        return Err(VmError::TypeError("Cannot mix BigInt and other types, use explicit conversions".into()));
                    } else {
                        let a = a_num.to_number().unwrap_or(f64::NAN);
                        let b = b_num.to_number().unwrap_or(f64::NAN);
                        self.push(JsValue::from_double(js_exponentiate(a, b)));
                    }
                }

                OpCode::Pos => {
                    let v = self.pop()?;
                    if self.as_bigint_val(v).is_some() {
                        return Err(VmError::TypeError("Cannot convert a BigInt value to a number".into()));
                    }
                    let n = self.to_number_value(v)?;
                    self.push(JsValue::from_double(n));
                }
                OpCode::Neg => {
                    let v = self.pop()?;
                    if let Some(bi) = self.as_bigint_val(v) {
                        let res = self.alloc_bigint(-&bi);
                        self.push(res);
                    } else {
                        let n = self.to_number_value(v)?;
                        self.push(JsValue::from_double(-n));
                    }
                }
                OpCode::Void => {
                    let _ = self.pop()?;
                    self.push(JsValue::undefined());
                }
                OpCode::Delete => {
                    let _ = self.pop()?;
                    self.push(JsValue::from_boolean(true));
                }
                OpCode::DeleteProp(name_idx) => {
                    let prop = match &chunk.constants[name_idx as usize] {
                        Constant::String(s) => s.clone(),
                        _ => return Err(VmError::TypeError("Property name must be string".into())),
                    };
                    let target = self.pop()?;
                    let is_strict = self.frames.last().map(|f| f.is_strict).unwrap_or(self.is_strict);
                    if let Some(ptr) = target.as_object_ptr() {
                        let obj = unsafe { &mut *(ptr as *mut JsObject) };
                        if self.non_configurable_props.contains(&(target, prop.clone())) {
                            if is_strict {
                                self.handle_exception_at_depth(
                                    VmError::TypeError(format!("Cannot delete property '{}'", prop)),
                                    target_depth,
                                )?;
                                return Ok(JsValue::undefined());
                            } else {
                                self.push(JsValue::from_boolean(false));
                            }
                        } else {
                            self.deleted_props.insert((target, prop.clone()));
                            if let Some(accs) = &mut obj.accessors {
                                accs.remove(&prop);
                            }
                            let new_shape_id = self.shapes.remove_property(obj.shape_id, &prop);
                            obj.shape_id = new_shape_id;
                            if target == self.global_object {
                                self.globals.remove(&prop);
                            }
                            self.push(JsValue::from_boolean(true));
                        }
                    } else {
                        self.push(JsValue::from_boolean(true));
                    }
                }
                OpCode::DeleteElem => {
                    let key_val = self.pop()?;
                    let target = self.pop()?;
                    let is_strict = self.frames.last().map(|f| f.is_strict).unwrap_or(self.is_strict);
                    if let Some(ptr) = target.as_object_ptr() {
                        let obj = unsafe { &mut *(ptr as *mut JsObject) };
                        if key_val.is_symbol() || key_val.as_object_ptr().is_some_and(|p| matches!(unsafe { &(*(p as *mut JsObject)).kind }, ObjectKind::Symbol(_))) {
                            if self.module_namespace_objects.contains(&target)
                                || self.non_configurable_props.contains(&(target, "[Symbol.toStringTag]".to_string()))
                            {
                                if is_strict {
                                    self.handle_exception_at_depth(
                                        VmError::TypeError("Cannot delete property".into()),
                                        target_depth,
                                    )?;
                                    return Ok(JsValue::undefined());
                                } else {
                                    self.push(JsValue::from_boolean(false));
                                    continue;
                                }
                            }
                            if let Some(syms) = &mut obj.symbol_properties {
                                syms.remove(&key_val);
                            }
                            if let Some(accs) = &mut obj.symbol_accessors {
                                accs.remove(&key_val);
                            }
                            if let Some(order) = &mut obj.symbol_order {
                                order.retain(|s| s != &key_val);
                            }
                            self.push(JsValue::from_boolean(true));
                        } else {
                            let prop = self.to_string_value(key_val);
                            if self.non_configurable_props.contains(&(target, prop.clone())) {
                                if is_strict {
                                    self.handle_exception_at_depth(
                                        VmError::TypeError(format!("Cannot delete property '{}'", prop)),
                                        target_depth,
                                    )?;
                                    return Ok(JsValue::undefined());
                                } else {
                                    self.push(JsValue::from_boolean(false));
                                }
                            } else {
                                self.deleted_props.insert((target, prop.clone()));
                                if let Some(accs) = &mut obj.accessors {
                                    accs.remove(&prop);
                                }
                                let new_shape_id = self.shapes.remove_property(obj.shape_id, &prop);
                                obj.shape_id = new_shape_id;
                                if target == self.global_object {
                                    self.globals.remove(&prop);
                                }
                                self.sync_mapped_arguments_delete(target, &prop);
                                self.push(JsValue::from_boolean(true));
                            }
                        }
                    } else {
                        self.push(JsValue::from_boolean(true));
                    }
                }
                OpCode::DeleteIdent(name_idx) => {
                    let name = match &chunk.constants[name_idx as usize] {
                        Constant::String(s) => s.clone(),
                        _ => return Err(VmError::TypeError("Identifier name must be string".into())),
                    };
                    if name == "arguments" {
                        self.push(JsValue::from_boolean(false));
                        continue;
                    }
                    let is_strict = self.frames.last().map(|f| f.is_strict).unwrap_or(self.is_strict);
                    let mut scope_found = false;
                    let num_frames = self.frames.len();
                    for f_idx in (0..num_frames).rev() {
                        let scopes = self.frames[f_idx].scopes.clone();
                        for scope_obj in scopes.into_iter().rev() {
                            if scope_obj.is_object() && self.has_property(scope_obj, &name).unwrap_or(false) {
                                if self.non_configurable_props.contains(&(scope_obj, name.clone())) {
                                    if is_strict {
                                        self.handle_exception_at_depth(
                                            VmError::TypeError(format!("Cannot delete property '{}'", name)),
                                            target_depth,
                                        )?;
                                        return Ok(JsValue::undefined());
                                    } else {
                                        self.push(JsValue::from_boolean(false));
                                    }
                                } else {
                                    let _ = self.delete_property(scope_obj, &name);
                                    self.deleted_props.insert((scope_obj, name.clone()));
                                    self.push(JsValue::from_boolean(true));
                                }
                                scope_found = true;
                                break;
                            }
                        }
                        if scope_found { break; }
                    }
                    if scope_found {
                        continue;
                    }

                    if self.global_lexicals.contains_key(&name) || self.var_names.contains(&name) {
                        if is_strict {
                            self.handle_exception_at_depth(
                                VmError::SyntaxError(format!("Delete of unqualified identifier in strict mode: {}", name)),
                                target_depth,
                            )?;
                            return Ok(JsValue::undefined());
                        } else {
                            self.push(JsValue::from_boolean(false));
                        }
                    } else if let Some(ptr) = self.global_object.as_object_ptr() {
                        let obj = unsafe { &mut *(ptr as *mut JsObject) };
                        if self.shapes.has_property(obj.shape_id, &name) {
                            if self.non_configurable_props.contains(&(self.global_object, name.clone())) {
                                if is_strict {
                                    self.handle_exception_at_depth(
                                        VmError::TypeError(format!("Cannot delete property '{}'", name)),
                                        target_depth,
                                    )?;
                                    return Ok(JsValue::undefined());
                                } else {
                                    self.push(JsValue::from_boolean(false));
                                }
                            } else {
                                let new_shape_id = self.shapes.remove_property(obj.shape_id, &name);
                                obj.shape_id = new_shape_id;
                                self.globals.remove(&name);
                                self.push(JsValue::from_boolean(true));
                            }
                        } else {
                            self.push(JsValue::from_boolean(true));
                        }
                    } else {
                        self.push(JsValue::from_boolean(true));
                    }
                }
                OpCode::Not => {
                    let v = self.pop()?;
                    self.push(JsValue::from_boolean(!v.is_truthy()));
                }
                OpCode::BitNot => {
                    let v = self.pop()?;
                    let v_num = self.to_numeric(v)?;
                    if let Some(bi) = self.as_bigint_val(v_num) {
                        let res = self.alloc_bigint(!bi);
                        self.push(res);
                    } else {
                        let num = self.to_number_value(v_num)?;
                        let n = double_to_int32(num);
                        self.push(JsValue::from_int32(!n));
                    }
                }
                OpCode::Typeof => {
                    let v = self.pop()?;
                    let t = if v.is_symbol() {
                        "symbol"
                    } else if let Some(ptr) = v.as_heap_ptr() {
                        let obj = unsafe { &*(ptr as *mut JsObject) };
                        match &obj.kind {
                            ObjectKind::String(_) => "string",
                            ObjectKind::Symbol(_) => if v.is_symbol() { "symbol" } else { "object" },
                            ObjectKind::BigInt(_) => "bigint",
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
                    let b_val = self.pop()?;
                    let a_val = self.pop()?;
                    let a_num = self.to_numeric(a_val)?;
                    let b_num = self.to_numeric(b_val)?;
                    if let (Some(ba), Some(bb)) = (self.as_bigint_val(a_num), self.as_bigint_val(b_num)) {
                        let res = self.alloc_bigint(&ba & &bb);
                        self.push(res);
                    } else if self.as_bigint_val(a_num).is_some() || self.as_bigint_val(b_num).is_some() {
                        return Err(VmError::TypeError("Cannot mix BigInt and other types, use explicit conversions".into()));
                    } else {
                        let a_int = double_to_int32(self.to_number_value(a_num)?);
                        let b_int = double_to_int32(self.to_number_value(b_num)?);
                        self.push(JsValue::from_int32(a_int & b_int));
                    }
                }
                OpCode::BitOr => {
                    let b_val = self.pop()?;
                    let a_val = self.pop()?;
                    let a_num = self.to_numeric(a_val)?;
                    let b_num = self.to_numeric(b_val)?;
                    if let (Some(ba), Some(bb)) = (self.as_bigint_val(a_num), self.as_bigint_val(b_num)) {
                        let res = self.alloc_bigint(&ba | &bb);
                        self.push(res);
                    } else if self.as_bigint_val(a_num).is_some() || self.as_bigint_val(b_num).is_some() {
                        return Err(VmError::TypeError("Cannot mix BigInt and other types, use explicit conversions".into()));
                    } else {
                        let a_int = double_to_int32(self.to_number_value(a_num)?);
                        let b_int = double_to_int32(self.to_number_value(b_num)?);
                        self.push(JsValue::from_int32(a_int | b_int));
                    }
                }
                OpCode::BitXor => {
                    let b_val = self.pop()?;
                    let a_val = self.pop()?;
                    let a_num = self.to_numeric(a_val)?;
                    let b_num = self.to_numeric(b_val)?;
                    if let (Some(ba), Some(bb)) = (self.as_bigint_val(a_num), self.as_bigint_val(b_num)) {
                        let res = self.alloc_bigint(&ba ^ &bb);
                        self.push(res);
                    } else if self.as_bigint_val(a_num).is_some() || self.as_bigint_val(b_num).is_some() {
                        return Err(VmError::TypeError("Cannot mix BigInt and other types, use explicit conversions".into()));
                    } else {
                        let a_int = double_to_int32(self.to_number_value(a_num)?);
                        let b_int = double_to_int32(self.to_number_value(b_num)?);
                        self.push(JsValue::from_int32(a_int ^ b_int));
                    }
                }
                OpCode::Shl => {
                    let b_val = self.pop()?;
                    let a_val = self.pop()?;
                    let a_num = self.to_numeric(a_val)?;
                    let b_num = self.to_numeric(b_val)?;
                    if let (Some(ba), Some(bb)) = (self.as_bigint_val(a_num), self.as_bigint_val(b_num)) {
                        if bb.is_negative() {
                            let shift = (-bb).to_usize().ok_or_else(|| VmError::RangeError("Maximum BigInt size exceeded".into()))?;
                            let res = self.alloc_bigint(&ba >> shift);
                            self.push(res);
                        } else {
                            let shift = bb.to_usize().ok_or_else(|| VmError::RangeError("Maximum BigInt size exceeded".into()))?;
                            let res = self.alloc_bigint(&ba << shift);
                            self.push(res);
                        }
                    } else if self.as_bigint_val(a_num).is_some() || self.as_bigint_val(b_num).is_some() {
                        return Err(VmError::TypeError("Cannot mix BigInt and other types, use explicit conversions".into()));
                    } else {
                        let a = self.to_number_value(a_num)?;
                        let b = self.to_number_value(b_num)?;
                        let a_int = double_to_int32(a);
                        let b_uint = (double_to_int32(b) as u32) & 0x1F;
                        self.push(JsValue::from_int32(a_int.wrapping_shl(b_uint)));
                    }
                }
                OpCode::Shr => {
                    let b_val = self.pop()?;
                    let a_val = self.pop()?;
                    let a_num = self.to_numeric(a_val)?;
                    let b_num = self.to_numeric(b_val)?;
                    if let (Some(ba), Some(bb)) = (self.as_bigint_val(a_num), self.as_bigint_val(b_num)) {
                        if bb.is_negative() {
                            let shift = (-bb).to_usize().ok_or_else(|| VmError::RangeError("Maximum BigInt size exceeded".into()))?;
                            let res = self.alloc_bigint(&ba << shift);
                            self.push(res);
                        } else {
                            let shift = bb.to_usize().ok_or_else(|| VmError::RangeError("Maximum BigInt size exceeded".into()))?;
                            let res = self.alloc_bigint(&ba >> shift);
                            self.push(res);
                        }
                    } else if self.as_bigint_val(a_num).is_some() || self.as_bigint_val(b_num).is_some() {
                        return Err(VmError::TypeError("Cannot mix BigInt and other types, use explicit conversions".into()));
                    } else {
                        let a = self.to_number_value(a_num)?;
                        let b = self.to_number_value(b_num)?;
                        let a_int = double_to_int32(a);
                        let b_uint = (double_to_int32(b) as u32) & 0x1F;
                        self.push(JsValue::from_int32(a_int.wrapping_shr(b_uint)));
                    }
                }
                OpCode::Ushr => {
                    let b_val = self.pop()?;
                    let a_val = self.pop()?;
                    let a_num = self.to_numeric(a_val)?;
                    let b_num = self.to_numeric(b_val)?;
                    if self.as_bigint_val(a_num).is_some() || self.as_bigint_val(b_num).is_some() {
                        return Err(VmError::TypeError("BigInt has no unsigned right shift, use >> instead".into()));
                    } else {
                        let a = self.to_number_value(a_num)?;
                        let b = self.to_number_value(b_num)?;
                        let a_u32 = double_to_int32(a) as u32;
                        let b_uint = (double_to_int32(b) as u32) & 0x1F;
                        let res = a_u32.wrapping_shr(b_uint);
                        self.push(JsValue::from_double(res as f64));
                    }
                }

                OpCode::StrictEq => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    self.push(JsValue::from_boolean(self.strict_equal(a, b)));
                }
                OpCode::StrictNotEq => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    self.push(JsValue::from_boolean(!self.strict_equal(a, b)));
                }
                OpCode::Eq => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    let eq = self.abstract_equal(a, b)?;
                    self.push(JsValue::from_boolean(eq));
                }
                OpCode::NotEq => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    let eq = self.abstract_equal(a, b)?;
                    self.push(JsValue::from_boolean(!eq));
                }
                OpCode::Lt => {
                    let b_val = self.pop()?;
                    let a_val = self.pop()?;
                    let ord = self.abstract_relational_comparison(a_val, b_val)?;
                    self.push(JsValue::from_boolean(ord == Some(std::cmp::Ordering::Less)));
                }
                OpCode::Lte => {
                    let b_val = self.pop()?;
                    let a_val = self.pop()?;
                    let ord = self.abstract_relational_comparison(a_val, b_val)?;
                    self.push(JsValue::from_boolean(matches!(ord, Some(std::cmp::Ordering::Less | std::cmp::Ordering::Equal))));
                }
                OpCode::Gt => {
                    let b_val = self.pop()?;
                    let a_val = self.pop()?;
                    let ord = self.abstract_relational_comparison(a_val, b_val)?;
                    self.push(JsValue::from_boolean(ord == Some(std::cmp::Ordering::Greater)));
                }
                OpCode::Gte => {
                    let b_val = self.pop()?;
                    let a_val = self.pop()?;
                    let ord = self.abstract_relational_comparison(a_val, b_val)?;
                    self.push(JsValue::from_boolean(matches!(ord, Some(std::cmp::Ordering::Greater | std::cmp::Ordering::Equal))));
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
                    if !rhs.is_object() {
                        return Err(VmError::TypeError("Cannot use 'in' operator to search for property in non-object".into()));
                    }
                    let has = if lhs.is_symbol() {
                        let prop_key = PropertyKeyVal::Symbol(lhs);
                        let mut curr = Some(rhs);
                        let mut found = false;
                        while let Some(c_val) = curr {
                            if self.has_own_property_key(c_val, &prop_key) {
                                found = true;
                                break;
                            }
                            if let Some(c_ptr) = c_val.as_object_ptr() {
                                let c_obj = unsafe { &*(c_ptr as *mut JsObject) };
                                curr = c_obj.prototype;
                            } else {
                                break;
                            }
                        }
                        found
                    } else {
                        let key = self.to_string_value(lhs);
                        self.has_property(rhs, &key)?
                    };
                    self.push(JsValue::from_boolean(has));
                }

                OpCode::Jump(target) => {
                    let frame = self.frames.last_mut().unwrap();
                    frame.ip = target as usize;
                }
                OpCode::JumpIfFalse(target) => {
                    let cond = self.pop()?;
                    if !cond.is_truthy() {
                        let frame = self.frames.last_mut().unwrap();
                        frame.ip = target as usize;
                    }
                }
                OpCode::JumpIfTrue(target) => {
                    let cond = self.pop()?;
                    if cond.is_truthy() {
                        let frame = self.frames.last_mut().unwrap();
                        frame.ip = target as usize;
                    }
                }

                OpCode::Throw => {
                    let err = self.pop()?;
                    return Err(VmError::Thrown(err));
                }
                OpCode::PushCatch(target) => {
                    let scopes_len = self.frames.last().map(|f| f.scopes.len()).unwrap_or(0);
                    self.catch_stack.push(CatchHandler {
                        frame_idx: self.frames.len().saturating_sub(1),
                        target_ip: target as usize,
                        stack_len: self.stack.len(),
                        scopes_len,
                    });
                }
                OpCode::PopCatch => {
                    self.catch_stack.pop();
                }

                OpCode::GetLocal(slot) => {
                    let frame = self.frames.last().unwrap();
                    let val = frame.locals.get(slot as usize).copied().unwrap_or(JsValue::undefined());
                    self.push(val);
                }
                OpCode::SetLocal(slot) => {
                    let val = *self.peek()?;
                    let slot_idx = slot as usize;
                    let (env_and_name, args_and_idx) = {
                        let frame = self.frames.last_mut().unwrap();
                        if slot_idx < frame.locals.len() {
                            frame.locals[slot_idx] = val;
                        } else {
                            frame.locals.resize(slot_idx + 1, JsValue::undefined());
                            frame.locals[slot_idx] = val;
                        }
                        let env_and_name = frame.env_obj.and_then(|env| {
                            frame.local_names.get(slot_idx).cloned().map(|name| (env, name))
                        });
                        let args_and_idx = if let Some(mapped) = &frame.mapped_arguments {
                            if mapped.get(slot_idx).copied().unwrap_or(false) {
                                frame.arguments.map(|a| (a, slot_idx))
                            } else {
                                None
                            }
                        } else {
                            None
                        };
                        (env_and_name, args_and_idx)
                    };
                    if let Some((env, name)) = env_and_name {
                        self.set_property(env, &name, val);
                    }
                    if let Some((args_obj, idx)) = args_and_idx {
                        self.set_own_property(args_obj, &idx.to_string(), val);
                    }
                }
                OpCode::PushScope => {
                    let scope_obj = self.pop()?;
                    let frame = self.frames.last_mut().unwrap();
                    frame.scopes.push(scope_obj);
                }
                OpCode::PopScope => {
                    let frame = self.frames.last_mut().unwrap();
                    frame.scopes.pop();
                }
                OpCode::PushCatchScope(name_idx) => {
                    let name = match &chunk.constants[name_idx as usize] {
                        Constant::String(s) => s.clone(),
                        _ => return Err(VmError::TypeError("Catch param name must be string".into())),
                    };
                    let err = self.pop()?;
                    let obj = self.alloc_object();
                    self.set_property(obj, &name, err);
                    let frame = self.frames.last_mut().unwrap();
                    frame.scopes.push(obj);
                }
                OpCode::EnterFunctionBody => {
                    let frame = self.frames.last_mut().unwrap();
                    frame.in_param_init = false;
                }
                OpCode::DefGlobal(name_idx) => {
                    let name = match &chunk.constants[name_idx as usize] {
                        Constant::String(s) => s.clone(),
                        _ => return Err(VmError::TypeError("Global name must be string".into())),
                    };
                    if self.global_lexicals.contains_key(&name) {
                        self.handle_exception_at_depth(
                            VmError::SyntaxError(format!("Identifier '{}' has already been declared", name)),
                            target_depth,
                        )?;
                        return Ok(JsValue::undefined());
                    }
                    self.var_names.insert(name.clone());
                    if !self.globals.contains_key(&name) {
                        if self.non_extensible_objects.contains(&self.global_object) {
                            self.handle_exception_at_depth(
                                VmError::TypeError(format!("Cannot define property '{}' on non-extensible global object", name)),
                                target_depth,
                            )?;
                            return Ok(JsValue::undefined());
                        }
                        self.globals.insert(name.clone(), JsValue::undefined());
                        if self.global_object.is_object() {
                            self.set_property(self.global_object, &name, JsValue::undefined());
                            self.non_configurable_props.insert((self.global_object, name.clone()));
                        }
                    }
                }
                OpCode::DefLexical(name_idx) => {
                    let name = match &chunk.constants[name_idx as usize] {
                        Constant::String(s) => s.clone(),
                        _ => return Err(VmError::TypeError("Global name must be string".into())),
                    };
                    if name == "undefined" || name == "NaN" || name == "Infinity"
                        || self.non_configurable_props.contains(&(self.global_object, name.clone()))
                        || self.global_lexicals.contains_key(&name)
                        || self.var_names.contains(&name)
                    {
                        self.handle_exception_at_depth(
                            VmError::SyntaxError(format!("Identifier '{}' has already been declared", name)),
                            target_depth,
                        )?;
                        return Ok(JsValue::undefined());
                    }
                    self.global_lexicals.insert(name, JsValue::undefined());
                }
                OpCode::DefConst(name_idx) => {
                    let name = match &chunk.constants[name_idx as usize] {
                        Constant::String(s) => s.clone(),
                        _ => return Err(VmError::TypeError("Global name must be string".into())),
                    };
                    if name == "undefined" || name == "NaN" || name == "Infinity"
                        || self.non_configurable_props.contains(&(self.global_object, name.clone()))
                        || self.global_lexicals.contains_key(&name)
                        || self.var_names.contains(&name)
                    {
                        self.handle_exception_at_depth(
                            VmError::SyntaxError(format!("Identifier '{}' has already been declared", name)),
                            target_depth,
                        )?;
                        return Ok(JsValue::undefined());
                    }
                    self.global_lexicals.insert(name.clone(), JsValue::undefined());
                    self.const_bindings.insert(name);
                }
                OpCode::InitConst(name_idx) => {
                    let name = match &chunk.constants[name_idx as usize] {
                        Constant::String(s) => s.clone(),
                        _ => return Err(VmError::TypeError("Global name must be string".into())),
                    };
                    let val = *self.peek()?;
                    self.global_lexicals.insert(name.clone(), val);
                    self.const_bindings.insert(name);
                }
                OpCode::GetGlobalSafe(name_idx) => {
                    let name = match &chunk.constants[name_idx as usize] {
                        Constant::String(s) => s.as_str(),
                        _ => return Err(VmError::TypeError("Global name must be string".into())),
                    };
                    if let Some(val) = self.resolve_identifier(name) {
                        if val.is_uninitialized() {
                            self.handle_exception_at_depth(
                                VmError::ReferenceError(format!("Cannot access '{}' before initialization", name)),
                                target_depth,
                            )?;
                            return Ok(JsValue::undefined());
                        }
                        self.push(val);
                    } else {
                        self.push(JsValue::undefined());
                    }
                }
                OpCode::GetGlobal(name_idx) => {
                    let name = match &chunk.constants[name_idx as usize] {
                        Constant::String(s) => s.as_str(),
                        _ => return Err(VmError::TypeError("Global name must be string".into())),
                    };
                    if let Some(val) = self.resolve_identifier(name) {
                        if val.is_uninitialized() {
                            self.handle_exception_at_depth(
                                VmError::ReferenceError(format!("Cannot access '{}' before initialization", name)),
                                target_depth,
                            )?;
                            return Ok(JsValue::undefined());
                        }
                        self.push(val);
                    } else {
                        self.handle_exception_at_depth(VmError::ReferenceError(format!("{} is not defined", name)), target_depth)?;
                    }
                }
                OpCode::SetGlobalDirect(name_idx) => {
                    let name = match &chunk.constants[name_idx as usize] {
                        Constant::String(s) => s.clone(),
                        _ => return Err(VmError::TypeError("Global name must be string".into())),
                    };
                    let val = *self.peek()?;
                    let is_chunk_import = self.frames.last()
                        .map(|f| f.chunk.imported_bindings.contains(&name))
                        .unwrap_or(false);
                    if is_chunk_import {
                        self.handle_exception_at_depth(
                            VmError::TypeError(format!("Assignment to constant variable {}", name)),
                            target_depth,
                        )?;
                        return Ok(JsValue::undefined());
                    }
                    if let Some(mod_obj) = self.current_module_obj {
                        if let Some(ptr) = mod_obj.as_object_ptr() {
                            let obj = unsafe { &*(ptr as *mut JsObject) };
                            if self.shapes.has_property(obj.shape_id, &name) {
                                self.set_property_direct(mod_obj, &name, val);
                            }
                        }
                    }
                    let mod_values: Vec<JsValue> = self.modules.values().copied().collect();
                    for m in mod_values {
                        if let Some(ptr) = m.as_object_ptr() {
                            let obj = unsafe { &*(ptr as *mut JsObject) };
                            if self.shapes.has_property(obj.shape_id, &name) {
                                self.set_property_direct(m, &name, val);
                            }
                        }
                    }
                    if !self.is_strict && !self.global_lexicals.contains_key(&name) {
                        if self.globals.contains_key(&name) {
                            self.globals.insert(name.clone(), val);
                        }
                        if !self.is_module {
                            self.set_property(self.global_object, &name, val);
                        }
                    } else if let Some(lex) = self.global_lexicals.get_mut(&name) {
                        *lex = val;
                    } else {
                        if self.globals.contains_key(&name) {
                            self.globals.insert(name.clone(), val);
                        }
                        if !self.is_module {
                            self.set_property(self.global_object, &name, val);
                        }
                    }
                }
                OpCode::SetGlobal(name_idx) => {
                    let name = match &chunk.constants[name_idx as usize] {
                        Constant::String(s) => s.clone(),
                        _ => return Err(VmError::TypeError("Global name must be string".into())),
                    };
                    let val = *self.peek()?;
                    let mut assigned = false;
                    let num_frames = self.frames.len();
                    let is_indirect = self.frames.last().map(|f| f.is_indirect_eval).unwrap_or(false);
                    let is_func = self.frames.last().map(|f| f.is_function).unwrap_or(false);
                    let min_frame_idx = if is_indirect || is_func { num_frames.saturating_sub(1) } else { 0 };
                    for f_idx in (min_frame_idx..num_frames).rev() {
                        let num_scopes = self.frames[f_idx].scopes.len();
                        if !name.starts_with('@') {
                            for s_idx in (0..num_scopes).rev() {
                                let scope_val = self.frames[f_idx].scopes[s_idx];
                                if scope_val.is_object() {
                                    if self.has_property(scope_val, &name).unwrap_or(false) {
                                        self.set_property(scope_val, &name, val);
                                        for f in &mut self.frames {
                                            if f.env_obj == Some(scope_val) {
                                                for (i, l_name) in f.local_names.iter().enumerate() {
                                                    if l_name == &name && i < f.locals.len() {
                                                        f.locals[i] = val;
                                                    }
                                                }
                                            }
                                        }
                                        assigned = true;
                                        break;
                                    }
                                }
                            }
                        }
                        if assigned { break; }
                        let env_to_sync = {
                            let frame = &mut self.frames[f_idx];
                            let mut found_env = None;
                            for (i, local_name) in frame.local_names.iter().enumerate().rev() {
                                if *local_name == name {
                                    if i < frame.locals.len() {
                                        frame.locals[i] = val;
                                    }
                                    if let Some(env) = frame.env_obj {
                                        found_env = Some(env);
                                    }
                                    assigned = true;
                                    break;
                                }
                            }
                            found_env
                        };
                        if let Some(env) = env_to_sync {
                            self.set_property(env, &name, val);
                        }
                        if assigned { break; }
                    }
                    if assigned {
                        if let Some(mod_obj) = self.current_module_obj {
                            if let Some(ptr) = mod_obj.as_object_ptr() {
                                let obj = unsafe { &*(ptr as *mut JsObject) };
                                if self.shapes.has_property(obj.shape_id, &name) {
                                    self.set_property_direct(mod_obj, &name, val);
                                }
                            }
                        }
                        let mod_values: Vec<JsValue> = self.modules.values().copied().collect();
                        for m in mod_values {
                            if let Some(ptr) = m.as_object_ptr() {
                                let obj = unsafe { &*(ptr as *mut JsObject) };
                                if self.shapes.has_property(obj.shape_id, &name) {
                                    self.set_property_direct(m, &name, val);
                                }
                            }
                        }
                    }
                    if !assigned {
                        let is_chunk_import = self.frames.last()
                            .map(|f| f.chunk.imported_bindings.contains(&name))
                            .unwrap_or(false);
                        if is_chunk_import {
                            self.handle_exception_at_depth(
                                VmError::TypeError(format!("Assignment to constant variable {}", name)),
                                target_depth,
                            )?;
                            return Ok(JsValue::undefined());
                        }
                        if self.const_bindings.contains(&name) {
                            self.handle_exception_at_depth(
                                VmError::TypeError(format!("Assignment to constant variable {}", name)),
                                target_depth,
                            )?;
                            return Ok(JsValue::undefined());
                        }
                        if self.non_writable_props.contains(&(self.global_object, name.clone())) {
                            let is_strict = self.frames.last().map(|f| f.is_strict).unwrap_or(self.is_strict);
                            if is_strict {
                                self.handle_exception_at_depth(
                                    VmError::TypeError(format!("Cannot assign to read only property '{}'", name)),
                                    target_depth,
                                )?;
                                return Ok(JsValue::undefined());
                            }
                            return Ok(JsValue::undefined());
                        }
                        if self.global_lexicals.contains_key(&name) {
                            self.global_lexicals.insert(name.clone(), val);
                            if let Some(mod_obj) = self.current_module_obj {
                                if let Some(ptr) = mod_obj.as_object_ptr() {
                                    let obj = unsafe { &*(ptr as *mut JsObject) };
                                    if self.shapes.has_property(obj.shape_id, &name) {
                                        self.set_property_direct(mod_obj, &name, val);
                                    }
                                }
                            }
                            let mod_values: Vec<JsValue> = self.modules.values().copied().collect();
                            for m in mod_values {
                                if let Some(ptr) = m.as_object_ptr() {
                                    let obj = unsafe { &*(ptr as *mut JsObject) };
                                    if self.shapes.has_property(obj.shape_id, &name) {
                                        self.set_property_direct(m, &name, val);
                                    }
                                }
                            }
                            assigned = true;
                        }
                    }
                    if !assigned {
                        let is_current_strict = self.frames.last().map(|f| f.is_strict).unwrap_or(self.is_strict);
                        if self.globals.contains_key(&name) {
                            self.globals.insert(name.clone(), val);
                            if let Some(mod_obj) = self.current_module_obj {
                                if let Some(ptr) = mod_obj.as_object_ptr() {
                                    let obj = unsafe { &*(ptr as *mut JsObject) };
                                    if self.shapes.has_property(obj.shape_id, &name) {
                                        self.set_property_direct(mod_obj, &name, val);
                                    }
                                }
                            }
                            let mod_values: Vec<JsValue> = self.modules.values().copied().collect();
                            for m in mod_values {
                                if let Some(ptr) = m.as_object_ptr() {
                                    let obj = unsafe { &*(ptr as *mut JsObject) };
                                    if self.shapes.has_property(obj.shape_id, &name) {
                                        self.set_property_direct(m, &name, val);
                                    }
                                }
                            }
                            if !self.is_module && self.global_object.is_object() {
                                self.set_property(self.global_object, &name, val);
                            }
                        } else if is_current_strict {
                            self.handle_exception_at_depth(VmError::ReferenceError(format!("Assignment to undeclared variable {}", name)), target_depth)?;
                        } else if self.global_object.is_object() && {
                            let ptr = self.global_object.as_object_ptr().unwrap();
                            let obj = unsafe { &*(ptr as *mut JsObject) };
                            self.shapes.has_property(obj.shape_id, &name)
                        } {
                            self.set_property(self.global_object, &name, val);
                            self.globals.insert(name.clone(), val);
                        } else {
                            self.globals.insert(name.clone(), val);
                            if self.global_object.is_object() {
                                self.set_property(self.global_object, &name, val);
                            }
                        }
                    }
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
                    let ret_val = if self.stack.len() > finished.base_sp {
                        self.stack.pop().unwrap_or_default()
                    } else {
                        JsValue::undefined()
                    };
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
            local_names: vec!["a".into(), "b".into()],
            lexical_local_names: Vec::new(),
            chunk: Arc::new(chunk),
            upvalues: Vec::new(),
            is_async: false,
            is_generator: false,
            call_count: 0,
            jit_entry: None,
            is_strict: false,
            has_param_exprs: false,
            is_method: false,
            is_arrow: false,
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

    #[test]
    fn test_phase1_promises_and_weak_collections() {
        let mut vm = Vm::new();

        // 1. WeakMap test
        let wm_res = vm.eval("let wm = new WeakMap(); let k = {}; wm.set(k, 'value'); wm.has(k);").unwrap();
        assert_eq!(wm_res.as_boolean(), Some(true));

        // 2. WeakSet test
        let ws_res = vm.eval("let ws = new WeakSet(); let item = {}; ws.add(item); ws.has(item);").unwrap();
        assert_eq!(ws_res.as_boolean(), Some(true));

        // 3. Promise.allSettled test
        let p_res = vm.eval("let p = Promise.allSettled([Promise.resolve(10), Promise.reject('err')]); p;").unwrap();
        assert!(p_res.is_object());

        // 4. Promise.any test
        let any_res = vm.eval("let a = Promise.any([Promise.resolve(99)]); a;").unwrap();
        assert!(any_res.is_object());
    }

    #[test]
    fn test_phase2_symbols_and_global_registry() {
        let mut vm = Vm::new();

        // Unique symbols
        let res1 = vm.eval("Symbol('foo') === Symbol('foo')").unwrap();
        assert_eq!(res1.as_boolean(), Some(false));

        // Global registry
        let res2 = vm.eval("Symbol.for('bar') === Symbol.for('bar')").unwrap();
        assert_eq!(res2.as_boolean(), Some(true));

        let res3 = vm.eval("Symbol.keyFor(Symbol.for('hello')) === 'hello'").unwrap();
        assert_eq!(res3.as_boolean(), Some(true));

        // Well-known symbols
        let res4 = vm.eval("typeof Symbol.iterator === 'symbol' && typeof Symbol.toStringTag === 'symbol'").unwrap();
        assert_eq!(res4.as_boolean(), Some(true));

        // Symbol-keyed properties
        let res5 = vm.eval("let obj = {}; let s = Symbol('key'); obj[s] = 42; obj[s] === 42").unwrap();
        assert_eq!(res5.as_boolean(), Some(true));
    }

    #[test]
    fn test_phase2_typed_arrays_and_dataview() {
        let mut vm = Vm::new();

        // ArrayBuffer and isView
        let res_buf = vm.eval("let b = new ArrayBuffer(16); b.byteLength === 16 && ArrayBuffer.isView(new Uint8Array(b))").unwrap();
        assert_eq!(res_buf.as_boolean(), Some(true));

        // Uint8Array read/write/methods
        let res_ta = vm.eval(r#"
            let u8 = new Uint8Array([10, 20, 30, 40]);
            u8[1] = 99;
            let sub = u8.subarray(1, 3);
            let inc = u8.includes(99);
            let idx = u8.indexOf(40);
            let joined = u8.join('-');
            u8[1] === 99 && sub.length === 2 && sub[0] === 99 && inc === true && idx === 3 && joined === '10-99-30-40'
        "#).unwrap();
        assert_eq!(res_ta.as_boolean(), Some(true));

        // Uint8ClampedArray clamping and round-half-to-even
        let res_clamp = vm.eval(r#"
            let ca = new Uint8ClampedArray(4);
            ca[0] = 300;
            ca[1] = -10;
            ca[2] = 2.5;
            ca[3] = 3.5;
            ca[0] === 255 && ca[1] === 0 && ca[2] === 2 && ca[3] === 4
        "#).unwrap();
        assert_eq!(res_clamp.as_boolean(), Some(true));

        // DataView endianness and various getters/setters
        let res_dv = vm.eval(r#"
            let buf = new ArrayBuffer(16);
            let dv = new DataView(buf);
            dv.setInt32(0, 12345678, true);
            let read32 = dv.getInt32(0, true);
            dv.setFloat32(4, 1.5, true);
            let readF32 = dv.getFloat32(4, true);
            read32 === 12345678 && readF32 === 1.5
        "#).unwrap();
        assert_eq!(res_dv.as_boolean(), Some(true));
    }

    #[test]
    fn test_phase2_iterator_protocols() {
        let mut vm = Vm::new();

        // Array.prototype[Symbol.iterator]
        let res_arr_iter = vm.eval(r#"
            let arr = [10, 20];
            let it = arr[Symbol.iterator]();
            let step1 = it.next();
            let step2 = it.next();
            let step3 = it.next();
            step1.value === 10 && step1.done === false &&
            step2.value === 20 && step2.done === false &&
            step3.done === true
        "#).unwrap();
        assert_eq!(res_arr_iter.as_boolean(), Some(true));

        // Array.from with mapping
        let res_from = vm.eval(r#"
            let mapped = Array.from([1, 2, 3], x => x * 3);
            mapped.length === 3 && mapped[0] === 3 && mapped[1] === 6 && mapped[2] === 9
        "#).unwrap();
        assert_eq!(res_from.as_boolean(), Some(true));

        // Array.of
        let res_of = vm.eval(r#"
            let ofArr = Array.of(7, 8, 9);
            ofArr.length === 3 && ofArr[0] === 7 && ofArr[1] === 8 && ofArr[2] === 9
        "#).unwrap();
        assert_eq!(res_of.as_boolean(), Some(true));

        // String.prototype[Symbol.iterator]
        let res_str_iter = vm.eval(r#"
            let strIt = "ab"[Symbol.iterator]();
            let s1 = strIt.next();
            let s2 = strIt.next();
            let s3 = strIt.next();
            s1.value === 'a' && s2.value === 'b' && s3.done === true
        "#).unwrap();
        assert_eq!(res_str_iter.as_boolean(), Some(true));

        // Map and Set iteration via Symbol.iterator
        let res_map_iter = vm.eval(r#"
            let m = new Map();
            m.set('k', 'v');
            let mIt = m[Symbol.iterator]();
            let mEntry = mIt.next().value;
            mEntry[0] === 'k' && mEntry[1] === 'v'
        "#).unwrap();
        assert_eq!(res_map_iter.as_boolean(), Some(true));
    }

    #[test]
    fn test_phase2_atomics() {
        let mut vm = Vm::new();

        let res = vm.eval(r#"
            let sab = new SharedArrayBuffer(16);
            let ta = new Int32Array(sab);
            let isLf = Atomics.isLockFree(4);
            Atomics.store(ta, 0, 42);
            let loaded = Atomics.load(ta, 0);
            let oldAdd = Atomics.add(ta, 0, 8); // 42 -> 50
            let bitAnd = Atomics.and(ta, 0, 0xFF);
            let bitOr = Atomics.or(ta, 0, 0x100);
            let bitXor = Atomics.xor(ta, 0, 0x100);
            let cx = Atomics.compareExchange(ta, 0, 50, 100); // 50 -> 100
            let waitRes = Atomics.wait(ta, 0, 999, 10);
            let notifyRes = Atomics.notify(ta, 0, 1);

            isLf === true && loaded === 42 && oldAdd === 42 && cx === 50 &&
            Atomics.load(ta, 0) === 100 && waitRes === 'not-equal' && notifyRes === 0
        "#).unwrap();
        assert_eq!(res.as_boolean(), Some(true));
    }

    #[test]
    fn test_default_parameters() {
        let mut vm = Vm::new();
        let res = vm.eval(r#"
            function add(a, b = 10) { return a + b; }
            const mul = (x = 4, y = 5) => x * y;
            add(5) === 15 && add(5, 2) === 7 && mul() === 20 && mul(3) === 15
        "#).unwrap();
        assert_eq!(res.as_boolean(), Some(true));
    }
}
