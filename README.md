<div align="center">

<img src="assets/logo.jpg" alt="DOMS JavaScript Engine Logo" width="180" height="180" />

# DOMS

### A Superfast, Modular, Embeddable JavaScript Engine & Parallel Web Runtime in Pure Rust

[![CI](https://github.com/nicodes-js/doms/actions/workflows/ci.yml/badge.svg)](https://github.com/nicodes-js/doms/actions)
[![Test262 Conformance](https://img.shields.io/badge/Test262-75.2%25%20passing-brightgreen)](tests/test262/results/summary.json)
[![Web Standards](https://img.shields.io/badge/Web%20APIs-WHATWG%20Compatible-blue)](tests/wpt/README.md)
[![Rust Edition](https://img.shields.io/badge/Rust-2024%20Edition-orange?logo=rust)](Cargo.toml)
[![Chrome DevTools](https://img.shields.io/badge/DevTools-CDP%20Ready-green?logo=googlechrome)](crates/doms_vm/src/inspector_server.rs)
[![License](https://img.shields.io/badge/license-MIT%20%2F%20Apache--2.0-blue.svg)](LICENSE-MIT)

[Architecture Guide](docs/src/SUMMARY.md) • [C ABI Reference](doms_ffi/include/doms.h) • [Test262 Conformance](tests/test262/README.md) • [Web Platform Tests](tests/wpt/README.md) • [Contributing](CONTRIBUTING.md) • [Security](SECURITY.md)

</div>

---

## ⚡ Overview

**DOMS** (Dynamic Object & Memory System) is an embeddable, standards-compliant JavaScript and ECMAScript engine engineered from scratch in pure Rust.

Designed specifically for modern multi-core systems, headless runtimes, game engines, and next-generation browser architectures, DOMS avoids the legacy pitfalls of past engines by pairing **generational Cheney garbage collection**, **NaN-boxed 64-bit value representation**, **dynamic inline caching with hidden shapes**, and **first-class multi-isolate parallelism** from day one.

```javascript
// High-performance parallelism with SharedArrayBuffer & lock-free Atomics
const buffer = new SharedArrayBuffer(1024);
const view = new Int32Array(buffer);
Atomics.add(view, 0, 42);

// Standard Web APIs built directly into the engine
const encoder = new TextEncoder();
const bytes = encoder.encode("Fast, embeddable JavaScript!");
const url = new URL("https://domsjs.dev/engine?jit=true#fast");
console.log(`Ready on ${url.hostname}:`, bytes.length, "bytes");
```

---

## 🎯 Why DOMS? — Learning from Other Engines' Mistakes

Existing engines either carry decades of C++ baggage or made architectural trade-offs that are difficult to correct later:

| Engine | Architectural Limitation | How DOMS Solves It |
|:---|:---|:---|
| **V8 / rusty_v8** | Massive 30+ min build times, millions of lines of C++, difficult to embed cleanly. | Pure Rust, fast compilation (<2 min), zero C++ toolchain dependencies, modular crate structure. |
| **Boa** | Started as a tree-walking interpreter; retrofit a bytecode VM and GC years later. | Generational GC, NaN-boxing, register bytecode VM, and shape trees designed from **day one**. |
| **QuickJS** | Reference counting GC. Suffers from O(N) cycle collection pauses and lacks thread safety. | Generational tracing GC (Cheney nursery copying + mark-compact old gen) with zero refcount overhead. |
| **Hermes** | Stripped down for React Native (no `eval()`, no JIT, limited modern ES features). | Full ECMAScript standard target with `eval()`, ES modules, dynamic imports, and tiered JIT roadmap. |
| **LibJS (Ladybird)** | Hard-coupled to the Ladybird browser; cannot be embedded standalone without the browser tree. | Standalone embedding-first design. Zero browser coupling; clean C ABI (`doms_ffi`) and Rust API (`doms_core`). |
| **Nova** | Single-developer bus factor; experimental unproven data layout. | Industry-proven architectures (V8 shapes, JSC NaN-boxing, SpiderMonkey Cheney scavenge) with community governance. |
| **Deno / Node** | Full runtimes wrapping V8 rather than standalone lightweight JS engines. | DOMS is a lightweight, embeddable engine (<10MB binary) that boots in microseconds. |

---

## ✨ Key Features

- **🚀 Generational Tracing GC**: 2MB copying bump-allocated nursery for ephemeral objects, mark-compact for long-lived objects, and dedicated large-object space.
- **📦 Compact NaN-Boxed Values**: Every JavaScript value fits in an 8-byte 64-bit word (`JsValue`), maximizing CPU L1 cache line utilization.
- **🏎️ Shapes & Polymorphic Inline Caches**: Fast property lookup via transition trees and monomorphic/polymorphic inline cache call-sites.
- **🧵 True Parallelism & Multi-Isolates**: Run independent JavaScript isolates across OS threads with zero lock contention. Coordinate via `SharedArrayBuffer`, `Atomics`, and cross-isolate `MessageChannel`.
- **🌐 Built-in Web Standards**: `TextEncoder`, `TextDecoder`, `URL`, `URLSearchParams`, `EventTarget`, `CustomEvent`, `setTimeout`, and microtask queues.
- **🛠️ Chrome DevTools Protocol (CDP)**: Built-in WebSocket inspector server for live debugging in Chrome DevTools (`chrome://inspect`) with breakpoints, call stacks, and source maps.
- **🔌 Multi-Language Embedding**:
  - **Rust**: Safe, ergonomic API through [`doms_core`](crates/doms_core).
  - **C / C++**: Stable, zero-overhead C ABI header [`doms.h`](doms_ffi/include/doms.h).
  - **WebAssembly**: Freestanding sandboxed module via [`doms_wasm`](crates/doms_wasm).

---

## 🏛️ Engine Architecture

```
┌────────────────────────────────────────────────────────────────────────┐
│                        DOMS Execution Engine                           │
├────────────────────────────────────────────────────────────────────────┤
│  Embedding APIs:  Rust API (doms_core) · C ABI (doms_ffi) · WASM      │
├────────────────────────────────────────────────────────────────────────┤
│  DevTools & Diagnostics: Chrome DevTools Protocol (CDP) · Source Maps  │
├────────────────────────────────────────────────────────────────────────┤
│  Web Standards: URL · Encoding · EventTarget · Timers · Microtasks     │
├────────────────────────────────────────────────────────────────────────┤
│  TIER 1 VM: NaN-Boxed Register Dispatch · Shapes · Inline Caches       │
├────────────────────────────────────────────────────────────────────────┤
│  Frontend Pipeline: Lexer (UTF-8) → Parser → AST → Bytecode Compiler   │
├────────────────────────────────────┬───────────────────────────────────┤
│  Generational Garbage Collector    │  Parallelism & Memory Layer       │
│  - Nursery (Cheney Semi-Space)     │  - Threaded Multi-Isolates        │
│  - Old Generation (Mark-Compact)   │  - SharedArrayBuffer & Atomics    │
│  - Large Object Space (>64KB)      │  - MessageChannel & Memory Cages  │
└────────────────────────────────────┴───────────────────────────────────┘
```

---

## 🚀 Quick Start

### 1. Build and Run from CLI

Clone and build with standard Cargo:

```bash
git clone https://github.com/nicodes-js/doms.git
cd doms
cargo build --release
```

#### Run a JavaScript file
```bash
./target/release/doms run tests/integration/demo_phase5.js
```

#### One-liner evaluation
```bash
./target/release/doms eval "console.log(Math.sqrt(144) + 10);"
```

#### Interactive REPL
```bash
./target/release/doms repl
```

#### Live Debugging with Chrome DevTools
```bash
./target/release/doms run my_script.js --inspect=127.0.0.1:9229
# Open chrome://inspect in Google Chrome and inspect DOMS in real-time!
```

---

### 2. Embed in Rust (`doms_core`)

Add DOMS to your `Cargo.toml`:

```toml
[dependencies]
doms_core = { path = "crates/doms_core" }
```

Execute scripts in an isolated context:

```rust
use doms_core::Engine;

fn main() {
    let engine = Engine::new();
    let mut isolate = engine.create_isolate();
    let mut ctx = isolate.create_context();

    // Evaluate JavaScript expressions
    let result = ctx.eval("let x = 10; let y = 20; x * y").expect("Script failed");
    println!("Computed result: {:?}", result);
}
```

---

### 3. Embed in C / C++ (`doms_ffi`)

Include `doms.h` and link against `libdoms.so` / `libdoms.dylib` / `doms.dll`:

```c
#include <stdio.h>
#include "doms.h"

int main(void) {
    doms_engine_t* engine = doms_engine_new();
    doms_isolate_t* isolate = doms_isolate_new(engine);
    doms_context_t* ctx = doms_context_new(isolate);

    doms_value_t* result = NULL;
    doms_status_t status = doms_eval(ctx, "40 + 2", 6, &result);

    if (status == DOMS_OK) {
        double num = 0;
        doms_value_to_number(ctx, result, &num);
        printf("Result from C: %.0f\n", num); // 42
        doms_value_destroy(result);
    } else {
        printf("Error: %s\n", doms_get_error_message(ctx));
    }

    doms_context_destroy(ctx);
    doms_isolate_destroy(isolate);
    doms_engine_destroy(engine);
    return 0;
}
```

---

## 🧪 Conformance & Testing

DOMS treats rigorous compliance testing as a tier-1 requirement.

### Test262 (ECMAScript Official Test Suite)

DOMS includes an integrated multi-threaded test runner for the official [tc39/test262](https://github.com/tc39/test262) test suite:

```bash
# Run the test262 conformance test suite
./tests/test262/run.sh

# Run with custom filters and parallel threads
./tests/test262/run.sh --suite built-ins --threads 8 --max 500
```

**Latest Conformance Snapshot:**
- Evaluated: **1,529** standardized tests
- Passing: **1,150** tests
- Overall Conformance: **75.21%** across evaluated language sections

### Web Platform Tests (WPT)

Conformance against WHATWG and W3C Web Standards is validated using the WPT test suite:

```bash
./tests/wpt/run.sh
```

---

## 🏎️ Benchmarks

Benchmark scripts are located in `tests/benchmarks/`. Run all performance benchmarks with:

```bash
bash tests/benchmarks/run_bench.sh
```

Sample micro-benchmark execution (x86_64 Linux, Release build):

| Benchmark | Description | Time |
|:---|:---|:---|
| `bitops-bitwise-and.js` | 10M bitwise operations & mask calculations | ~56 ms |
| `jit-function-call.js` | Recursion, call-frame setup & teardown | ~87 ms |
| `controlflow-loop.js` | Deep nested arithmetic loops & accumulator | ~302 ms |

---

## 📂 Monorepo Crates

The codebase is organized as a clean, decoupled Cargo workspace:

```
doms/
├── assets/                   # Brand marks, vector SVGs, and graphics
├── crates/
│   ├── doms_gc/              # Generational copying nursery + mark-compact old gen
│   ├── doms_parser/          # High-throughput lexer & ECMAScript AST parser
│   ├── doms_compiler/        # AST to register bytecode compiler & Source Maps v3
│   ├── doms_vm/              # Bytecode VM, NaN-boxing, shapes, inline caches, CDP
│   ├── doms_runtime/         # Built-in globals and standard library definitions
│   ├── doms_parallel/        # Multi-isolate coordination, SharedArrayBuffer, Atomics
│   └── doms_wasm/            # Freestanding sandboxed WebAssembly target
├── doms_core/                # Ergonomic high-level Rust embedding engine API
├── doms_ffi/                 # Stable C ABI bindings and C headers (libdoms)
├── doms_cli/                 # Command-line interface, REPL, and Test262 runner
├── docs/                     # mdBook architecture book (16 chapters)
├── fuzz/                     # libFuzzer / cargo-fuzz targets for parser, compiler, VM
├── tests/
│   ├── test262/              # tc39/test262 conformance harness & results
│   ├── wpt/                  # Web Platform Tests harness & runner
│   ├── benchmarks/           # JS execution micro-benchmarks
│   └── integration/          # Engine and Web API integration test cases
└── .github/                  # CI/CD workflows, issue templates, PR template
```

---

## 🗺️ Roadmap & Milestones

- [x] **Phase 0: Foundation**: Generational nursery GC, NaN-boxing, register bytecode VM, basic built-ins.
- [x] **Phase 1: Conformance**: Test262 runner integration, 75%+ passing score across evaluated suites, C ABI.
- [x] **Phase 2: Parallelism**: Multi-isolate scheduling, `SharedArrayBuffer`, `Atomics`, cross-isolate channels.
- [x] **Phase 3: Web Standards & DevTools**: `TextEncoder`/`Decoder`, `URL`, Chrome DevTools Protocol inspector.
- [ ] **Phase 4: JIT Compilation**: Template Baseline JIT & Cranelift optimizing JIT compiler backend.
- [ ] **Phase 5: Production 1.0**: 95%+ Test262 compliance, WPT certification, hardened production embedding.

---

## 🤝 Contributing

We welcome contributions of all levels! Whether you are interested in compiler optimization, garbage collector algorithms, ECMAScript specification compliance, or Web APIs:

1. Check out our [Architecture Guide](docs/src/SUMMARY.md) to understand internal designs.
2. Read [CONTRIBUTING.md](CONTRIBUTING.md) for local environment setup, style guidelines, and PR workflow.
3. Check the [Issue Tracker](https://github.com/nicodes-js/doms/issues) for "Good First Issue" tags.

Please note that this project is governed by our [Code of Conduct](CODE_OF_CONDUCT.md).

---

## 🔒 Security

For details on reporting security vulnerabilities, please refer to our [Security Policy](SECURITY.md).

---

## 📄 License

DOMS is dual-licensed under either:

- **Apache License, Version 2.0** ([LICENSE-APACHE](LICENSE-APACHE))
- **MIT License** ([LICENSE-MIT](LICENSE-MIT))

at your option.
