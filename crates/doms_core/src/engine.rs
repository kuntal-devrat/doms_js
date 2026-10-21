//! Engine, Isolate, and Context — the top-level API types.
//!
//! These form the embedding interface for the DOMS engine:
//!
//! - [`Engine`] — Process-global, manages shared resources (thread pool, etc.)
//! - [`Isolate`] — Independent JS execution environment with its own heap.
//! - [`Context`] — An execution context within an isolate (global object, etc.)

use crate::value::JsValue;
use doms_gc::heap::{Heap, HeapConfig};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Handle for asynchronously interrupting JavaScript execution from another thread.
#[derive(Debug, Clone, Default)]
pub struct InterruptHandle {
    interrupted: Arc<AtomicBool>,
}

impl InterruptHandle {
    /// Create a new un-triggered interrupt handle.
    pub fn new() -> Self {
        Self { interrupted: Arc::new(AtomicBool::new(false)) }
    }

    /// Trigger an interruption, causing the associated isolate to abort execution promptly.
    pub fn interrupt(&self) {
        self.interrupted.store(true, Ordering::Relaxed);
    }

    /// Check whether an interruption has been triggered.
    pub fn is_interrupted(&self) -> bool {
        self.interrupted.load(Ordering::Relaxed)
    }

    /// Reset the interruption signal to allow future execution.
    pub fn reset(&self) {
        self.interrupted.store(false, Ordering::Relaxed);
    }
}

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
    // Shared compiled code cache, thread pool, etc.
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
        // The heap must exist before any builtin is allocated, otherwise the
        // builtins would live in a heap that is thrown away here.
        Self { vm: doms_vm::Vm::with_heap(Heap::new(config)) }
    }

    /// Create a new execution context within this isolate.
    pub fn create_context(&mut self) -> Context<'_> {
        Context::new(self)
    }

    /// Get or create an [`InterruptHandle`] to interrupt this isolate asynchronously.
    pub fn get_or_create_interrupt_handle(&mut self) -> InterruptHandle {
        if let Some(ref flag) = self.vm.interrupt_flag {
            InterruptHandle { interrupted: Arc::clone(flag) }
        } else {
            let handle = InterruptHandle::new();
            self.vm.interrupt_flag = Some(Arc::clone(&handle.interrupted));
            handle
        }
    }

    /// Attach an external [`InterruptHandle`] to this isolate.
    pub fn set_interrupt_handle(&mut self, handle: &InterruptHandle) {
        self.vm.interrupt_flag = Some(Arc::clone(&handle.interrupted));
    }

    /// Set an execution step (fuel) limit for the VM.
    pub fn set_max_fuel(&mut self, max_instructions: Option<usize>) {
        self.vm.max_instructions = max_instructions;
    }

    /// Get the remaining fuel (instructions) before timeout, if a limit is configured.
    pub fn fuel_remaining(&self) -> Option<usize> {
        self.vm.max_instructions.map(|max| max.saturating_sub(self.vm.instruction_count))
    }

    /// Get the number of instructions executed by this isolate so far.
    pub fn fuel_consumed(&self) -> usize {
        self.vm.instruction_count
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

    /// Get the global object for this context.
    pub fn get_global(&self) -> JsValue {
        self.isolate.vm.global_object
    }

    /// Read a property from the global object.
    pub fn get_global_val(&mut self, name: &str) -> JsValue {
        let g = self.isolate.vm.global_object;
        self.isolate.vm.get_property(g, name)
    }

    /// Set a property on the global object using any type implementing [`crate::convert::IntoJs`].
    pub fn set_global(&mut self, name: &str, val: impl crate::convert::IntoJs) {
        let js_val = val.into_js(&mut self.isolate.vm);
        let g = self.isolate.vm.global_object;
        self.isolate.vm.set_property(g, name, js_val);
    }

    /// Register a native Rust function or closure into the global scope with automatic
    /// argument and return value conversion.
    pub fn set_global_fn<Args, Ret>(
        &mut self,
        name: &str,
        func: impl crate::callback::JsCallback<Args, Ret>,
    ) {
        let arity = func.arity();
        let closure: doms_vm::object::ClosureNativeFn =
            std::sync::Arc::new(move |vm, this, args| func.invoke(vm, this, args));
        let fn_val = self.isolate.vm.alloc_closure_native_function(name, arity, closure);
        let g = self.isolate.vm.global_object;
        self.isolate.vm.set_property(g, name, fn_val);
    }

    /// Register a native Rust function or closure as a method on a target object.
    pub fn set_method<Args, Ret>(
        &mut self,
        obj: JsValue,
        name: &str,
        func: impl crate::callback::JsCallback<Args, Ret>,
    ) {
        let arity = func.arity();
        let closure: doms_vm::object::ClosureNativeFn =
            std::sync::Arc::new(move |vm, this, args| func.invoke(vm, this, args));
        let fn_val = self.isolate.vm.alloc_closure_native_function(name, arity, closure);
        self.isolate.vm.set_property(obj, name, fn_val);
    }

    /// Create a new empty JavaScript object.
    pub fn create_object(&mut self) -> JsValue {
        self.isolate.vm.alloc_object()
    }

    /// Create a new JavaScript array containing the specified items.
    pub fn create_array<T: crate::convert::IntoJs>(
        &mut self,
        items: impl IntoIterator<Item = T>,
    ) -> JsValue {
        let elements = items.into_iter().map(|item| item.into_js(&mut self.isolate.vm)).collect();
        self.isolate.vm.alloc_array(elements)
    }

    /// Get a property from an object.
    pub fn get_property(&mut self, obj: JsValue, key: &str) -> JsValue {
        self.isolate.vm.get_property(obj, key)
    }

    /// Set a property on an object.
    pub fn set_property(&mut self, obj: JsValue, key: &str, val: impl crate::convert::IntoJs) {
        let js_val = val.into_js(&mut self.isolate.vm);
        self.isolate.vm.set_property(obj, key, js_val);
    }

    /// Call a JavaScript function value.
    pub fn call(
        &mut self,
        func: JsValue,
        this_val: JsValue,
        args: &[JsValue],
    ) -> Result<JsValue, EngineError> {
        self.isolate.vm.call(func, this_val, args).map_err(EngineError::from)
    }

    /// Call a named method on an object.
    pub fn call_method(
        &mut self,
        obj: JsValue,
        method: &str,
        args: &[JsValue],
    ) -> Result<JsValue, EngineError> {
        let func = self.isolate.vm.get_property(obj, method);
        if !self.isolate.vm.is_callable(func) {
            return Err(EngineError::Generic(format!("Property '{}' is not callable", method)));
        }
        self.isolate.vm.call(func, obj, args).map_err(EngineError::from)
    }

    /// Evaluate a JavaScript expression and convert the result directly to a Rust type.
    pub fn eval_as<T: crate::convert::FromJs>(&mut self, source: &str) -> Result<T, EngineError> {
        let val = self.eval(source)?;
        T::from_js(val, &mut self.isolate.vm).map_err(EngineError::Generic)
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

        ctx.load_module(
            "calc",
            "export var factor = 2; export function double(x) { return x * 2; }",
        )
        .unwrap();
        let mod_res = ctx
            .eval_module("import { factor, double } from 'calc'; export var res = double(21);")
            .unwrap();
        let val = ctx.isolate.vm.get_property(mod_res, "res").to_int32();
        assert_eq!(val, Some(42));
    }

    #[test]
    fn test_interrupt_handle() {
        let engine = Engine::new();
        let mut isolate = engine.create_isolate();
        let handle = isolate.get_or_create_interrupt_handle();
        assert!(!handle.is_interrupted());

        handle.interrupt();
        assert!(handle.is_interrupted());

        let mut ctx = isolate.create_context();
        // Since interrupted is set, a long loop aborts promptly
        let res = ctx.eval("while (true) {}");
        assert!(matches!(res, Err(EngineError::Runtime(_))));
    }

    #[test]
    fn test_fuel_metering() {
        let engine = Engine::new();
        let mut isolate = engine.create_isolate();
        isolate.set_max_fuel(Some(100));

        let mut ctx = isolate.create_context();
        // A loop exceeding 100 instructions fails with step limit exceeded
        let res = ctx.eval("var x = 0; while (true) { x++; }");
        assert!(matches!(res, Err(EngineError::Runtime(_))));
        assert!(isolate.fuel_consumed() > 100);
    }

    #[test]
    fn test_type_conversions() {
        use crate::convert::{FromJs, IntoJs};
        let engine = Engine::new();
        let mut isolate = engine.create_isolate();

        let val_bool = true.into_js(&mut isolate.vm);
        assert!(bool::from_js(val_bool, &mut isolate.vm).unwrap());

        let val_i32 = 42i32.into_js(&mut isolate.vm);
        assert_eq!(i32::from_js(val_i32, &mut isolate.vm).unwrap(), 42);

        let val_str = "hello doms".into_js(&mut isolate.vm);
        assert_eq!(String::from_js(val_str, &mut isolate.vm).unwrap(), "hello doms");

        let opt_some: Option<i32> = Some(99);
        let val_opt = opt_some.into_js(&mut isolate.vm);
        assert_eq!(Option::<i32>::from_js(val_opt, &mut isolate.vm).unwrap(), Some(99));

        let opt_none: Option<i32> = None;
        let val_none = opt_none.into_js(&mut isolate.vm);
        assert_eq!(Option::<i32>::from_js(val_none, &mut isolate.vm).unwrap(), None);
    }

    #[test]
    fn test_isolate_with_config_builtins_survive_gc() {
        let engine = Engine::new();
        // A custom (small) heap must still host the builtins allocated during
        // VM construction — otherwise GC would sweep them and property reads
        // on prototypes would fail.
        let mut isolate = engine.create_isolate_with_config(doms_gc::heap::HeapConfig {
            nursery_size: 64 * 1024,
            old_gen_size: 256 * 1024,
        });

        let mut ctx = isolate.create_context();
        let res = ctx.eval("'abc'.toUpperCase() === 'ABC'").unwrap();
        assert_eq!(res.as_boolean(), Some(true));

        // Force a full collection and verify builtins are still reachable.
        ctx.isolate.vm.scavenge_nursery();
        ctx.isolate.vm.gc_collect();

        let res = ctx
            .eval("[1, 2, 3].map(function (x) { return x * 2; }).join(',') === '2,4,6'")
            .unwrap();
        assert_eq!(res.as_boolean(), Some(true));
    }

    #[test]
    fn test_native_function_binding_and_eval_as() {
        let engine = Engine::new();
        let mut isolate = engine.create_isolate();
        let mut ctx = isolate.create_context();

        // Register 2-arg function
        ctx.set_global_fn("add", |a: i32, b: i32| a + b);

        // Register 1-arg function returning String
        ctx.set_global_fn("greet", |name: String| format!("Hello, {}!", name));

        // Register function returning Result for error throwing
        ctx.set_global_fn("safe_divide", |a: f64, b: f64| -> Result<f64, String> {
            if b == 0.0 { Err("Division by zero".into()) } else { Ok(a / b) }
        });

        let sum: i32 = ctx.eval_as("add(15, 27)").unwrap();
        assert_eq!(sum, 42);

        let msg: String = ctx.eval_as("greet('Rust Embedding')").unwrap();
        assert_eq!(msg, "Hello, Rust Embedding!");

        let div: f64 = ctx.eval_as("safe_divide(100, 4)").unwrap();
        assert_eq!(div, 25.0);

        let div_err = ctx.eval("safe_divide(10, 0)");
        assert!(div_err.is_err());
    }

    #[test]
    fn test_object_and_array_manipulation() {
        let engine = Engine::new();
        let mut isolate = engine.create_isolate();
        let mut ctx = isolate.create_context();

        // Create object, set properties, call method
        let user = ctx.create_object();
        ctx.set_property(user, "name", "Alice");
        ctx.set_property(user, "age", 30);
        ctx.set_method(user, "celebrateBirthday", || "Happy Birthday!");

        ctx.set_global("currentUser", user);

        let name: String = ctx.eval_as("currentUser.name").unwrap();
        let age: i32 = ctx.eval_as("currentUser.age").unwrap();
        let bday: String = ctx.eval_as("currentUser.celebrateBirthday()").unwrap();

        assert_eq!(name, "Alice");
        assert_eq!(age, 30);
        assert_eq!(bday, "Happy Birthday!");

        // Array creation and conversion
        let arr = ctx.create_array(vec![10, 20, 30, 40]);
        ctx.set_global("myNumbers", arr);
        let doubled_sum: i32 = ctx.eval_as("myNumbers.reduce((a, b) => a + b, 0)").unwrap();
        assert_eq!(doubled_sum, 100);

        let vec_out: Vec<i32> = ctx.eval_as("[1, 2, 3, 4, 5]").unwrap();
        assert_eq!(vec_out, vec![1, 2, 3, 4, 5]);
    }
}
