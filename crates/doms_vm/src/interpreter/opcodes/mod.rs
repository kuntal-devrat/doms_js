//! Bodies of the largest bytecode dispatch arms.
//!
//! [`crate::interpreter::run`] keeps the single `match` jump table so dispatch
//! stays one branch; the heavyweight arm bodies live here to keep the loop
//! readable. Each file groups the opcodes of one area.

use super::*;

mod access;
mod calls;
mod class;
mod comparison;
mod control;
mod delete;
mod functions;
mod global;
mod modules;
mod objects;
mod operators;
mod variables;
