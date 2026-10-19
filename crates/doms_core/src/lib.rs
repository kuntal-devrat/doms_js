//! # DOMS Core
//!
//! The central crate that ties together all DOMS engine subsystems.
//! This provides the primary Rust API for embedding the engine.
//!
//! ## Key Types
//!
//! - [`JsValue`] — NaN-boxed JavaScript value representation.
//! - [`Engine`] — Process-global engine instance.
//! - [`Isolate`] — Independent JavaScript execution environment.
//! - [`Context`] — Execution context within an isolate.

pub mod convert;
pub mod engine;
pub mod value;

pub use convert::{FromJs, IntoJs};
pub use engine::{Context, Engine, EngineError, InterruptHandle, Isolate};
pub use value::JsValue;

