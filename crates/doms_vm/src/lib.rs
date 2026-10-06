//! # DOMS VM
//!
//! Bytecode virtual machine and interpreter for the DOMS JavaScript engine.

pub mod inline_cache;
pub mod interpreter;

pub use inline_cache::{IcState, InlineCacheSlot};
pub use interpreter::{Vm, VmError, VmValue};
