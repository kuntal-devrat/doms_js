//! # DOMS Garbage Collector
//!
//! A generational garbage collector designed for the DOMS JavaScript engine.
//!
//! ## Architecture
//!
//! The GC uses a **generational** design based on the weak generational hypothesis:
//! most objects die young.
//!
//! - **Nursery (Young Generation):** Semi-space copying collector with bump allocation.
//!   New objects are allocated here. When full, live objects are copied to the other
//!   semi-space (scavenge). Objects that survive multiple scavenges are promoted to
//!   the old generation.
//!
//! - **Old Generation (Tenured):** Mark-sweep-compact collector for long-lived objects.
//!   (Phase 1 implementation — currently a simple mark-sweep.)
//!
//! - **Large Object Space:** Objects larger than `LARGE_OBJECT_THRESHOLD` are allocated
//!   directly here and never moved.
//!
//! ## Key Types
//!
//! - [`Heap`] — Top-level GC manager owning all memory spaces.
//! - [`GcRef`] — A traced reference to a GC-managed object.
//! - [`Trace`] — Trait implemented by all GC-managed types.
//! - [`GcHeader`] — Object metadata header prepended to every allocation.

pub mod gc_ref;
pub mod header;
pub mod heap;
pub mod large_object;
pub mod nursery;
pub mod old_gen;
pub mod trace;

pub use gc_ref::GcRef;
pub use header::GcHeader;
pub use heap::Heap;
pub use large_object::LargeObjectSpace;
pub use nursery::Nursery;
pub use old_gen::OldGen;
pub use trace::{Trace, Tracer};
