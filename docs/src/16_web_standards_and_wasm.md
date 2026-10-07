# Phase 5: Web Standards Host APIs, Live DevTools & WebAssembly

Phase 5 expands DOMS.js from a core ECMAScript engine into a full-featured browser and cloud runtime, providing standard Web Host APIs, remote debugging over Chrome DevTools Protocol, and native WebAssembly embedding.

---

## 1. Web Standards Host APIs

DOMS.js provides zero-dependency implementations of modern web APIs in `crates/doms_vm/src/web_apis.rs`:

### A. Timers & Event Loop
- **`setTimeout(callback, delayMs, ...args)`** & **`clearTimeout(timerId)`**
- **`setInterval(callback, delayMs, ...args)`** & **`clearInterval(timerId)`**
- **`queueMicrotask(callback)`**
- Integrated with `Vm::step_event_loop()` and `Vm::run_event_loop()` for non-blocking browser render frames or CLI execution.

```javascript
var timerFired = false;
setTimeout(function() {
    timerFired = true;
    console.log("Async Task Executed!");
}, 10);
```

### B. Encoding (`TextEncoder` & `TextDecoder`)
- **`TextEncoder`**: Encodes UTF-8 strings into `Uint8Array` backed by memory-caged `SharedArrayBuffer`.
- **`TextDecoder`**: Decodes UTF-8 byte streams into JavaScript strings.

```javascript
var encoder = new TextEncoder();
var bytes = encoder.encode("Hello from DOMS.js!");
var decoder = new TextDecoder();
console.log(decoder.decode(bytes)); // "Hello from DOMS.js!"
```

### C. URL & URLSearchParams
- **`URL`**: Full WHATWG URL parser exposing `href`, `origin`, `protocol`, `host`, `hostname`, `port`, `pathname`, `search`, `hash`, and `searchParams`.
- **`URLSearchParams`**: Query parameter manipulation (`get`, `getAll`, `has`, `set`, `append`, `delete`, `toString`).

```javascript
var url = new URL("https://domsjs.dev:8080/search?engine=doms&jit=true#bench");
console.log(url.hostname);                // "domsjs.dev"
console.log(url.port);                    // "8080"
console.log(url.searchParams.get("jit")); // "true"
```

### D. Events & EventTarget
- **`EventTarget`**: Standard event dispatcher with `addEventListener`, `removeEventListener`, and `dispatchEvent`.
- **`Event`** & **`CustomEvent`**: Support for custom event payloads (`detail`), cancelable events, and `preventDefault()`.

```javascript
var bus = new EventTarget();
bus.addEventListener("custom", function(e) {
    console.log("Received:", e.detail);
});
bus.dispatchEvent(new CustomEvent("custom", { detail: { score: 99 } }));
```

### E. Fetch API & Responses
- **`fetch(url, [options])`**: Dispatches HTTP 1.1 requests or processes `data:` URLs, returning a `Promise<Response>`.
- **`Response`**: Provides `status`, `statusText`, `ok`, `headers`, and body consumer promises (`.text()`, `.json()`, `.arrayBuffer()`).
- **`Headers`**: Case-insensitive HTTP header container.

---

## 2. Live Chrome DevTools Protocol Inspector (`--inspect`)

`crates/doms_vm/src/inspector_server.rs` implements a zero-dependency TCP & WebSocket server supporting Chrome DevTools Protocol (CDP):

### Starting the Inspector
```bash
# Launch script with live DevTools listening on default port 9229
doms run script.js --inspect

# Break on first instruction and wait for DevTools connection
doms run script.js --inspect-brk

# Bind to custom host and port
doms run script.js --inspect=127.0.0.1:9330
```

### Discovery Endpoints
- **`GET /json/version`**: Returns browser identification:
  ```json
  {"Browser":"DOMS/0.1.0","Protocol-Version":"1.3"}
  ```
- **`GET /json` or `GET /json/list`**: Returns target discovery metadata:
  ```json
  [{
    "description":"DOMS.js V8/CDP target",
    "devtoolsFrontendUrl":"devtools://devtools/bundled/js_app.html?experiments=true&v8only=true&ws=127.0.0.1:9229/devtools/page/doms",
    "id":"doms-isolate-1",
    "title":"DOMS.js Runtime",
    "type":"node",
    "url":"file:///path/to/script.js",
    "webSocketDebuggerUrl":"ws://127.0.0.1:9229/devtools/page/doms"
  }]
  ```

### RFC 6455 WebSocket Framing
- Standalone SHA-1 and base64 computation for `Sec-WebSocket-Accept` handshake.
- Unmasks client frames and encodes server JSON-RPC responses.
- Routes commands to `CdpDispatcher` (`Debugger.setBreakpoint`, `Debugger.stepOver`, `Debugger.resume`, `Runtime.evaluate`).

---

## 3. WebAssembly Target (`doms_wasm`)

`crates/doms_wasm` enables embedding DOMS.js in browsers, Cloudflare Workers, and Wasm runtimes without native compilation dependencies:

### Exported C ABI Functions
| Function | Description |
|:---|:---|
| `doms_wasm_version()` | Returns version string pointer |
| `doms_wasm_alloc(size)` | Allocates memory in WebAssembly linear memory |
| `doms_wasm_free(ptr, size)` | Frees allocated WebAssembly memory |
| `doms_wasm_eval(ptr, len)` | Evaluates script in global isolate |
| `doms_wasm_create_isolate()` | Spawns a completely isolated JS environment |
| `doms_wasm_isolate_eval(id, ptr, len)` | Evaluates code inside a specific isolate |
| `doms_wasm_destroy_isolate(id)` | Frees isolate resources |
| `doms_wasm_free_string(ptr)` | Deallocates returned result strings |

### Browser Demo
A complete interactive WebAssembly sandbox UI is provided in `crates/doms_wasm/demo/index.html` and `crates/doms_wasm/demo/doms_wasm.js`.
