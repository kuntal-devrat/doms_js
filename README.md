<div align="center">

<img src="assets/logo.jpg" alt="DOMS Logo" width="120" height="120" />

# DOMS

An embeddable JavaScript and ECMAScript engine written in Rust.

[![CI](https://github.com/kuntal-devrat/doms_js/actions/workflows/ci.yml/badge.svg)](https://github.com/kuntal-devrat/doms_js/actions)
[![License](https://img.shields.io/badge/license-MIT%20%2F%20Apache--2.0-blue.svg)](#license)
[![Rust](https://img.shields.io/badge/rust-2024%20edition-orange.svg)](Cargo.toml)

[Architecture Guide](docs/src/SUMMARY.md) • [C ABI Reference](crates/doms_ffi/include/doms.h) • [Test262 Harness](tests/test262/README.md) • [Contributing](CONTRIBUTING.md)

</div>

---

## Overview

DOMS is an embeddable, modular ECMAScript engine implemented from scratch in pure Rust. It is architected as a set of decoupled workspace crates designed for standalone embedding into game engines, serverless runtimes, native applications, and browser runtimes.

### Core Architecture

- **Register-Based Bytecode VM**: NaN-boxed 64-bit value representation (`JsValue`) that fits any JavaScript value into a single machine word.
- **Generational Garbage Collection**: 2 MB bump-allocated semi-space nursery for ephemeral objects (Cheney scavenge), mark-compact for long-lived objects, and dedicated large-object pages.
- **Hidden Classes & Inline Caches**: Shape transition trees with monomorphic and polymorphic inline caches (PIC) for fast property access.
- **Multi-Isolate Parallelism**: Independent isolates with thread-safe concurrency via `SharedArrayBuffer`, `Atomics`, memory cages, and cross-isolate message channels.
- **Web Standards**: Built-in support for `TextEncoder`, `TextDecoder`, `URL`, `URLSearchParams`, `EventTarget`, `CustomEvent`, timers (`setTimeout`, `setInterval`), and microtask queues.
- **Chrome DevTools Protocol (CDP)**: Built-in WebSocket inspector server enabling live debugging in Chrome DevTools (`chrome://inspect`) with breakpoints, call frames, and source maps.
- **Multi-Language Embedding**: Ergonomic high-level Rust API via `doms_core` and a stable, zero-overhead C ABI header via `doms_ffi`.

---

## Quick Start

### Building from Source

Prerequisites: Rust stable (2024 edition).

```bash
git clone https://github.com/kuntal-devrat/doms_js.git
cd doms_js
cargo build --release
```

### CLI Usage

Execute a JavaScript file:

```bash
./target/release/doms run path/to/script.js
```

Evaluate an expression:

```bash
./target/release/doms eval "console.log(Math.sqrt(144) + 10);"
```

Start an interactive REPL:

```bash
./target/release/doms repl
```

Debug using Chrome DevTools:

```bash
./target/release/doms run script.js --inspect=127.0.0.1:9229
# Open chrome://inspect in Google Chrome to inspect and debug
```

---

## Embedding

### Rust API (`doms_core`)

Add `doms_core` to your `Cargo.toml`:

```toml
[dependencies]
doms_core = { path = "crates/doms_core" }
```

Execute JavaScript within an isolated context:

```rust
use doms_core::Engine;

fn main() {
    let engine = Engine::new();
    let mut isolate = engine.create_isolate();
    let mut ctx = isolate.create_context();

    let result = ctx.eval("let x = 10; let y = 20; x * y").expect("Evaluation failed");
    println!("Result: {:?}", result);
}
```

### C / C++ ABI (`doms_ffi`)

Include `doms.h` and link against `libdoms.so` (Linux), `libdoms.dylib` (macOS), or `doms.dll` (Windows):

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
        double val = 0;
        doms_value_to_number(ctx, result, &val);
        printf("Result from C: %.0f\n", val); // 42
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

## Workspace Crates

The codebase is structured into modular crates within a single Cargo workspace:

| Crate | Purpose |
|:---|:---|
| [`doms_core`](crates/doms_core) | High-level Rust embedding API (`Engine`, `Isolate`, `Context`) |
| [`doms_vm`](crates/doms_vm) | Bytecode virtual machine, object layout, builtins, and CDP inspector |
| [`doms_compiler`](crates/doms_compiler) | AST-to-bytecode compiler, feedback vectors, and source map generation |
| [`doms_parser`](crates/doms_parser) | UTF-8 lexer and recursive-descent / Pratt parser |
| [`doms_gc`](crates/doms_gc) | Generational garbage collector (Cheney nursery copying + mark-compact) |
| [`doms_parallel`](crates/doms_parallel) | Isolates, SharedArrayBuffer, lock-free Atomics, and task scheduler |
| [`doms_ffi`](crates/doms_ffi) | C ABI bindings and C header (`doms.h`) for foreign language embedding |
| [`doms_wasm`](crates/doms_wasm) | Freestanding WebAssembly compilation target |
| [`doms_cli`](crates/doms_cli) | CLI binary, REPL, and Test262 test harness |

---

## Testing & Conformance

### Unit Tests

Run the test suite across all workspace crates:

```bash
cargo test --workspace
```

### ECMAScript Test262 Conformance

DOMS integrates a multi-threaded test runner for the official [tc39/test262](https://github.com/tc39/test262) specification suite:

```bash
# Run the test262 test suite
./tests/test262/run.sh

# Run with specific filters
./tests/test262/run.sh --suite built-ins --threads 4
```

Current test results and pass rates are tracked in [`tests/test262/results/summary.json`](tests/test262/results/summary.json).

---

## Documentation

The project includes an mdBook architecture manual covering the engine internals:

```bash
mdbook build docs
# Open docs/book/index.html in your browser
```

Rust API documentation can be generated with:

```bash
cargo doc --workspace --no-deps --open
```

---

## Contributing

Contributions are welcome. Please read [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines on code style, testing requirements, and the pull request workflow.

This project is governed by our [Code of Conduct](CODE_OF_CONDUCT.md). For security vulnerabilities, refer to [SECURITY.md](SECURITY.md).

---

## License

DOMS is dual-licensed under either:

- **Apache License, Version 2.0** ([LICENSE-APACHE](LICENSE-APACHE))
- **MIT License** ([LICENSE-MIT](LICENSE-MIT))

at your option.
