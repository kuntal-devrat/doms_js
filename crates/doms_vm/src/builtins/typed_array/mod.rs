//! Typed array constructors and shared typed-array machinery.

use super::*;

mod constructors;
mod iterator;
mod methods;
mod register;

pub(super) use iterator::{create_array_iterator, IterKind};
pub(super) use register::register_typed_arrays;
