//! Engine, Isolate, and Context — the top-level API types.
//!
//! These form the embedding interface for the DOMS engine:
//!
//! - [`Engine`] — Process-global, manages shared resources (thread pool, etc.)
//! - [`Isolate`] — Independent JS execution environment with its own heap.
//! - [`Context`] — An execution context within an isolate (global object, etc.)

use crate::value::JsValue;
use doms_gc::heap::{Heap, HeapConfig};

/// Process-global engine instance.
///
/// Create one `Engine` per process. It manages shared resources like the
/// thread pool for parallel isolates.
///
/// # Example
///
/// ```rust
/// use doms_core::Engine;
///
/// let engine = Engine::new();
/// let isolate = engine.create_isolate();
/// ```
pub struct Engine {
    // TODO: Thread pool, shared compiled code cache, etc.
    _private: (),
}

impl Engine {
    /// Create a new engine instance.
    pub fn new() -> Self {
        Self { _private: () }
    }

    /// Create a new isolate associated with this engine.
    pub fn create_isolate(&self) -> Isolate {
        Isolate::new()
    }

    /// Create a new isolate with custom heap configuration.
    pub fn create_isolate_with_config(&self, config: HeapConfig) -> Isolate {
        Isolate::with_config(config)
    }
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

/// An independent JavaScript execution environment.
///
/// Each isolate has its own GC heap, bytecode cache, and global object.
/// Isolates can run on different OS threads — they share no mutable state.
///
/// This is the foundation of DOMS's parallelism model: each Web Worker,
/// tab, or iframe gets its own isolate.
pub struct Isolate {
    /// The VM instance for this isolate.
    pub vm: doms_vm::Vm,
}

impl Isolate {
    /// Create a new isolate with default configuration.
    pub fn new() -> Self {
        Self { vm: doms_vm::Vm::new() }
    }

    /// Create a new isolate with custom heap configuration.
    pub fn with_config(config: HeapConfig) -> Self {
        let mut vm = doms_vm::Vm::new();
        vm.heap = Heap::new(config);
        Self { vm }
    }

    /// Create a new execution context within this isolate.
    pub fn create_context(&mut self) -> Context<'_> {
        Context::new(self)
    }
}

impl Default for Isolate {
    fn default() -> Self {
        Self::new()
    }
}

/// An execution context within an isolate.
///
/// A context represents a global scope: it has its own global object and
/// built-in prototypes. Multiple contexts can share an isolate (like
/// iframes sharing a process).
pub struct Context<'isolate> {
    /// Reference to the owning isolate.
    pub isolate: &'isolate mut Isolate,
}

impl<'isolate> Context<'isolate> {
    /// Create a new context within the given isolate.
    pub fn new(isolate: &'isolate mut Isolate) -> Self {
        Self { isolate }
    }

    /// Evaluate a JavaScript source string and return the result.
    ///
    /// This is a convenience method that parses, compiles, and executes
    /// the source in one step.
    ///
    /// # Errors
    ///
    /// Returns an error string if parsing, compilation, or execution fails.
    pub fn eval(&mut self, source: &str) -> Result<JsValue, String> {
        let program = doms_parser::parse(source).map_err(|e| e.to_string())?;
        let chunk = doms_compiler::compile(&program).map_err(|e| e.to_string())?;
        self.isolate.vm.run(&chunk).map_err(|e| e.to_string())
    }

    /// Get a reference to the isolate's heap.
    pub fn heap(&self) -> &Heap {
        &self.isolate.vm.heap
    }

    /// Get a mutable reference to the isolate's heap.
    pub fn heap_mut(&mut self) -> &mut Heap {
        &mut self.isolate.vm.heap
    }

    /// Get a reference to the VM instance.
    pub fn vm(&self) -> &doms_vm::Vm {
        &self.isolate.vm
    }

    /// Get a mutable reference to the VM instance.
    pub fn vm_mut(&mut self) -> &mut doms_vm::Vm {
        &mut self.isolate.vm
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_engine_creation() {
        let _engine = Engine::new();
    }

    #[test]
    fn test_isolate_creation() {
        let engine = Engine::new();
        let _isolate = engine.create_isolate();
    }

    #[test]
    fn test_context_eval() {
        let engine = Engine::new();
        let mut isolate = engine.create_isolate();
        let mut ctx = isolate.create_context();

        let result = ctx.eval("1 + 1");
        assert!(result.is_ok());
    }

    #[test]
    fn test_multiple_isolates() {
        let engine = Engine::new();
        let _iso1 = engine.create_isolate();
        let _iso2 = engine.create_isolate();
        // Both isolates are independent — no shared state.
    }
}
