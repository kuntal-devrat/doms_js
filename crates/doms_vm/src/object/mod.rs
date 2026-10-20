//! Object model, prototypes, functions, and arrays.

use crate::shape::{PropertyAttributes, ShapeId};
use crate::value::JsValue;
use doms_compiler::bytecode::BytecodeChunk;
use doms_gc::trace::{Trace, Tracer};
use num_bigint::BigInt;
use std::sync::Arc;

mod equality;
mod function;
mod generator;
mod js_object;
mod native_function;
mod object_kind;
mod promise;
mod trace;
mod typed_array;

pub use equality::*;
pub use function::*;
pub use generator::*;
pub use js_object::*;
pub use native_function::*;
pub use object_kind::*;
pub use promise::*;
pub use trace::*;
pub use typed_array::*;
