//! Chrome DevTools Protocol (CDP) Debugger subsystem for DOMS.
//!
//! Provides JSON-RPC 2.0 based debugging protocol compatible with Chrome DevTools,
//! VS Code, and browser developer tools:
//! - `Runtime` domain: `enable`, `evaluate`, `getProperties`, `runIfWaitingForDebugger`
//! - `Debugger` domain: `enable`, `setBreakpoint`, `removeBreakpoint`, `pause`, `resume`,
//!   `stepOver`, `stepInto`, `stepOut`, `getScriptSource`

use crate::interpreter::Vm;
use crate::value::JsValue;
use std::collections::HashMap;

mod dispatcher;
mod types;

#[cfg(test)]
mod tests;

pub use dispatcher::CdpDispatcher;
pub use types::{Breakpoint, CallFrameInfo, DebuggerSession, StepMode};
