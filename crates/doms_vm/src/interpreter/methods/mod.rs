//! Fast-path method dispatch helpers for the `[[Get]]` machinery.
//!
//! Each file dispatches a prototype method name to a native function closure
//! (for example `Array.prototype.push` or `Map.prototype.get`). Keeping them
//! split by builtin object keeps each file small and easy to extend.

use super::*;

// mod array;
mod finalization;
mod generator;
mod map_set;
mod string;
mod worker;
