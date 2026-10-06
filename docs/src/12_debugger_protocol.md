# Chrome DevTools Protocol (CDP) & Debugging

DOMS.js provides first-class debugging through a built-in **Chrome DevTools Protocol (CDP)** engine.

---

## 1. Supported Domains

DOMS implements JSON-RPC 2.0 communication over WebSocket or TCP streams, supporting standard developer tools:

### A. `Runtime` Domain
- `Runtime.enable` / `Runtime.disable`: Starts/stops runtime telemetry.
- `Runtime.evaluate`: Evaluates arbitrary JavaScript expressions within the active Isolate context and returns remote object descriptors (`type`, `value`, `description`).
- `Runtime.runIfWaitingForDebugger`: Signals the engine to resume execution if held at startup.
- `Runtime.consoleAPICalled`: Forwards console logging events.

### B. `Debugger` Domain
- `Debugger.enable` / `Debugger.disable`: Activates breakpoint scanning and stepping hooks.
- `Debugger.setBreakpoint`: Sets a breakpoint at a given `(scriptId, lineNumber, columnNumber)`.
- `Debugger.removeBreakpoint`: Removes an active breakpoint by ID.
- `Debugger.pause`: Suspends VM execution before the next bytecode instruction.
- `Debugger.resume`: Continues VM execution.
- `Debugger.stepOver` / `Debugger.stepInto` / `Debugger.stepOut`: Stepping controllers.
- `Debugger.getScriptSource`: Retrieves original or transpiled script text.

---

## 2. Using the Debugger (`CdpDispatcher`)

```rust
use doms_vm::{CdpDispatcher, Vm};

let mut dispatcher = CdpDispatcher::new();
let mut vm = Vm::new();

// Client sends JSON-RPC request
let req = r#"{"id":1,"method":"Runtime.evaluate","params":{"expression":"2 + 2"}}"#;
let response_json = dispatcher.handle_request(req, &mut vm);
// Response: {"id":1,"result":{"result":{"type":"number","value":4}}}
```
