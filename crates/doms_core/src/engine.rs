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

/// Comprehensive error hierarchy for JavaScript engine execution.
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("Parse error: {0}")]
    Parser(#[from] doms_parser::ParseError),
    #[error("Compile error: {0}")]
    Compiler(#[from] doms_compiler::CompileError),
    #[error("Runtime error: {0}")]
    Runtime(#[from] doms_vm::VmError),
    #[error("Execution error: {0}")]
    Generic(String),
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

    /// Evaluate a JavaScript source string, draining microtasks, and return the result.
    ///
    /// This is a convenience method that parses, compiles, executes
    /// the source, and drains microtasks in one step.
    ///
    /// # Errors
    ///
    /// Returns an [`EngineError`] if parsing, compilation, or execution fails.
    pub fn eval(&mut self, source: &str) -> Result<JsValue, EngineError> {
        let program = doms_parser::parse(source)?;
        let chunk = doms_compiler::compile(&program)?;
        let result = self.isolate.vm.run(&chunk)?;
        let _ = self.isolate.vm.drain_microtasks();
        Ok(result)
    }

    /// Evaluate an ES6 module source string, executing it, draining microtasks,
    /// and returning the module namespace object containing all exported bindings.
    pub fn eval_module(&mut self, source: &str) -> Result<JsValue, EngineError> {
        let program = doms_parser::parse_module(source)?;
        let chunk = doms_compiler::compile(&program)?;
        let old_exports = std::mem::take(&mut self.isolate.vm.current_exports);
        let old_is_mod = self.isolate.vm.is_module;
        let old_lexicals = std::mem::take(&mut self.isolate.vm.global_lexicals);
        let old_var_names = std::mem::take(&mut self.isolate.vm.var_names);
        let old_script_vars = std::mem::take(&mut self.isolate.vm.script_var_names);

        self.isolate.vm.is_module = true;
        let run_res = self.isolate.vm.run(&chunk);
        let _ = self.isolate.vm.drain_microtasks();
        let exports = std::mem::replace(&mut self.isolate.vm.current_exports, old_exports);

        self.isolate.vm.is_module = old_is_mod;
        self.isolate.vm.global_lexicals = old_lexicals;
        self.isolate.vm.var_names = old_var_names;
        self.isolate.vm.script_var_names = old_script_vars;

        run_res?;

        let mod_obj = self.isolate.vm.alloc_object();
        for (k, v) in exports {
            self.isolate.vm.set_property(mod_obj, &k, v);
        }
        if let Some(tag_sym) = self.isolate.vm.symbol_to_string_tag {
            let tag_val = self.isolate.vm.alloc_string("Module".to_string());
            self.isolate.vm.set_symbol_property(mod_obj, tag_sym, tag_val);
        }
        Ok(mod_obj)
    }

    /// Register a module namespace object under a specifier for `import` resolution.
    pub fn register_module(&mut self, specifier: &str, module_obj: JsValue) {
        self.isolate.vm.modules.insert(specifier.to_string(), module_obj);
    }

    /// Evaluate an ES6 module and register it under the given specifier.
    pub fn load_module(&mut self, specifier: &str, source: &str) -> Result<JsValue, EngineError> {
        let mod_obj = self.eval_module(source)?;
        self.register_module(specifier, mod_obj);
        Ok(mod_obj)
    }

    /// Drain all queued microtasks.
    pub fn drain_microtasks(&mut self) -> Result<(), EngineError> {
        self.isolate.vm.drain_microtasks().map_err(EngineError::from)
    }

    /// Step the event loop once. Returns `true` if work was performed.
    pub fn step_event_loop(&mut self) -> Result<bool, EngineError> {
        self.isolate.vm.step_event_loop().map_err(EngineError::from)
    }

    /// Run the event loop until all timers and microtasks have completed.
    pub fn run_event_loop(&mut self) -> Result<(), EngineError> {
        self.isolate.vm.run_event_loop().map_err(EngineError::from)
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

    #[test]
    fn test_context_eval_error_hierarchy() {
        let engine = Engine::new();
        let mut isolate = engine.create_isolate();
        let mut ctx = isolate.create_context();

        // Syntax error -> EngineError::Parser
        let parse_err = ctx.eval("let = ;");
        assert!(matches!(parse_err, Err(EngineError::Parser(_))));

        // Runtime error -> EngineError::Runtime
        let rt_err = ctx.eval("nonExistentFunction();");
        assert!(matches!(rt_err, Err(EngineError::Runtime(_))));
    }

    #[test]
    fn test_context_microtasks_draining() {
        let engine = Engine::new();
        let mut isolate = engine.create_isolate();
        let mut ctx = isolate.create_context();

        let res = ctx.eval("var resolvedVal = 0; Promise.resolve(42).then(v => { resolvedVal = v; }); resolvedVal;");
        assert!(res.is_ok());
        // Since eval drains microtasks, the promise reaction has executed:
        let val = ctx.eval("resolvedVal").unwrap().to_int32().unwrap();
        assert_eq!(val, 42);
    }

    #[test]
    fn test_context_eval_module() {
        let engine = Engine::new();
        let mut isolate = engine.create_isolate();
        let mut ctx = isolate.create_context();

        let mod_ns = ctx.eval_module("export var a = 10; export var b = 20;").unwrap();
        assert!(mod_ns.is_object());
        let a_val = ctx.isolate.vm.get_property(mod_ns, "a").to_int32().unwrap();
        let b_val = ctx.isolate.vm.get_property(mod_ns, "b").to_int32().unwrap();
        assert_eq!(a_val, 10);
        assert_eq!(b_val, 20);
    }

    #[test]
    fn test_context_load_and_import_module() {
        let engine = Engine::new();
        let mut isolate = engine.create_isolate();
        let mut ctx = isolate.create_context();

        ctx.load_module("calc", "export var factor = 2; export function double(x) { return x * 2; }").unwrap();
        let mod_res = ctx.eval_module("import { factor, double } from 'calc'; export var res = double(21);").unwrap();
        let val = ctx.isolate.vm.get_property(mod_res, "res").to_int32();
        assert_eq!(val, Some(42));
    }
}
