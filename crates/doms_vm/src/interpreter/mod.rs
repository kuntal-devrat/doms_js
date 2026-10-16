//! Bytecode interpreter virtual machine.
//!
//! This module implements the core of the engine: a stack-based bytecode
//! interpreter with NaN-boxed values, call frames and hidden-class shapes.
//!
//! The implementation is split into focused submodules so contributors can
//! jump straight to the part they need:
//!
//! | Module | Responsibility |
//! |--------|----------------|
//! | [`vm`] | The [`Vm`] state itself, its constructor and stack helpers |
//! | [`types`] | [`VmError`], [`CallFrame`] and other support types |
//! | [`run`] | Execution entry points and the main dispatch loop |
//! | [`gc`] / [`alloc`] | Object tracking, tracing GC and allocation helpers |
//! | [`properties`] / [`property_access`] / [`get_property`] | Property model |
//! | [`symbols`] / [`for_in`] | Symbol keys and key enumeration |
//! | [`calls`] | Calls, construction, closures and identifier lookup |
//! | [`operators`] | Abstract operations (equality, coercion, comparison) |
//! | [`opcodes`] | Bodies of the largest dispatch-loop arms, grouped by area |
//! | [`values`] / [`elements`] | Value conversion and indexed element access |
//! | [`promises`] / [`microtasks`] | Promise machinery and the event loop |
//! | [`errors`] | Mapping interpreter errors onto JavaScript error objects |
//! | [`modules`] | `eval` and global declaration instantiation |
//! | [`methods`] | Fast-path prototype method dispatch (Array, String, Map, …) |
//!
//! Each submodule contributes `impl Vm` blocks; there is no behavioural
//! difference between a method living in one file or another.

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
use num_bigint::BigInt;
use num_traits::{Num, One, Signed, ToPrimitive, Zero};
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::fmt;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use thiserror::Error;

mod alloc;
mod calls;
mod elements;
mod errors;
mod for_in;
mod gc;
mod get_property;
mod helpers;
mod identifiers;
mod methods;
mod microtasks;
mod modules;
mod opcodes;
mod operators;
mod promises;
mod properties;
mod property_access;
mod run;
mod symbols;
mod types;
mod values;
mod vm;

#[cfg(test)]
mod tests;

pub use helpers::*;
pub use types::*;
pub use vm::Vm;
