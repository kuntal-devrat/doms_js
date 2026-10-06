<div align="center">

# 🚀 DOMS

### A Modular, Embeddable JavaScript Engine for Parallel Browsers

[![CI](https://github.com/nicodes-js/doms/actions/workflows/ci.yml/badge.svg)](https://github.com/nicodes-js/doms/actions)
[![Test262](https://img.shields.io/badge/Test262-0.0%25-red)](https://github.com/nicodes-js/doms)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue)](LICENSE-MIT)
[![Discord](https://img.shields.io/discord/000000000?label=Discord&logo=discord&color=5865F2)](https://discord.gg/doms)

[Architecture Guide](docs/src/SUMMARY.md) • [C API Reference](doms_ffi/include/doms.h) • [Contributing](CONTRIBUTING.md) • [Discord](#)

</div>

---

## What is DOMS?

**DOMS** is a JavaScript engine written in Rust, designed from the ground up for:

- 🔌 **Embeddability** — Stable C ABI (`libdoms.h`). Embed JS in your app with 5 lines of C.
- ⚡ **Parallelism** — First-class multi-isolate architecture with `SharedArrayBuffer`, `Atomics`, and work-stealing scheduling. Built for parallel browsers.
- 🧩 **Modularity** — Parser, compiler, VM, GC, and runtime are independent crates. Contribute to built-ins without understanding the GC.
- 🛡️ **Safety** — Rust's ownership model enforces memory safety. No `unsafe` in contributor-facing code.

## Why Another JS Engine?

| Engine | Problem DOMS Solves |
|:---|:---|
| **Boa** | Prototype GC after 5+ years. No stable C API. |
| **Nova** | Bus factor of ~1. Unproven at scale. |
| **Brimstone** | Zero community. No embedding API. Missing SharedArrayBuffer. |
| **QuickJS** | Refcounting GC doesn't scale. Original went dormant. |
| **rusty_v8** | Not an engine — it's V8 bindings. 30+ min builds. |
| **LibJS** | Welded to the Ladybird browser. Not standalone-embeddable. |

DOMS combines the best ideas: **proven NaN-boxing** (like JSC), **generational GC** (like V8), **register-based VM** (like Ignition), **parallel isolates** (like V8 but first-class), and a **stable C ABI** (like QuickJS, but better documented).

## Quick Start

### Embed in C

```c
#include <doms.h>

int main() {
    doms_engine_t* engine = doms_engine_new();
    doms_isolate_t* isolate = doms_isolate_new(engine);
    doms_context_t* ctx = doms_context_new(isolate);

    doms_value_t* result;
    doms_eval(ctx, "2 + 2", 5, &result);

    double num;
    doms_value_to_number(ctx, result, &num);
    printf("Result: %f\n", num);  // 4.000000

    doms_context_destroy(ctx);
    doms_isolate_destroy(isolate);
    doms_engine_destroy(engine);
    return 0;
}
```

### Embed in Rust

```rust
use doms_core::{Engine, Isolate, Context};

fn main() {
    let engine = Engine::new();
    let isolate = Isolate::new(&engine);
    let mut ctx = Context::new(&isolate);

    let result = ctx.eval("2 + 2").unwrap();
    println!("Result: {}", result.to_number()); // 4
}
```

### Run from CLI

```bash
cargo install doms-cli
doms run script.js
doms repl
```

## Architecture

```
┌────────────────────────────────────────────────────────┐
│  Embedding Layer   (C ABI · Rust API · Wasm API)       │
├────────────────────────────────────────────────────────┤
│  Runtime Layer     (Event Loop · Modules · Host Hooks) │
├────────────────────────────────────────────────────────┤
│  Execution Layer   (Register VM · Interpreter · JIT)   │
├────────────────────────────────────────────────────────┤
│  Frontend Layer    (Lexer · Parser · Bytecode Compiler)│
├────────────────────────────────────────────────────────┤
│  Memory Layer      (Generational GC · NaN-boxed Values)│
├────────────────────────────────────────────────────────┤
│  Parallelism Layer (Isolates · SAB · Atomics · Sched.) │
└────────────────────────────────────────────────────────┘
```

See the full [Architecture Guide](docs/src/SUMMARY.md) for deep dives.

## Project Structure

```
doms/
├── crates/
│   ├── doms_gc/          # Generational garbage collector
│   ├── doms_parser/      # Lexer + Parser → AST
│   ├── doms_compiler/    # AST → Bytecode
│   ├── doms_vm/          # Register-based bytecode interpreter
│   ├── doms_runtime/     # Built-in objects (Array, Promise, etc.)
│   └── doms_parallel/    # Isolates, SharedArrayBuffer, scheduler
├── doms_core/            # Ties everything together (Rust API)
├── doms_ffi/             # C ABI layer (libdoms.so)
├── doms_cli/             # REPL + script runner
├── docs/                 # Architecture guide (mdBook)
└── tests/                # Test262 conformance harness
```

## Current Status

> ⚠️ DOMS is in **early development** (Phase 0). It is not yet usable for production.

| Milestone | Status |
|:---|:---|
| GC nursery (bump allocation) | ✅ Implemented |
| NaN-boxed JsValue | ✅ Implemented |
| C API header (`libdoms.h`) | ✅ Designed |
| Parser | 🔨 In Progress |
| Bytecode compiler | 🔨 In Progress |
| Register VM | 🔨 In Progress |
| Test262 conformance | 📊 0.0% |

## Roadmap

| Phase | Timeline | Goal |
|:---|:---|:---|
| **Phase 0** | Months 1-3 | End-to-end `console.log("hello")` |
| **Phase 1** | Months 4-8 | 50% Test262, stable C API |
| **Phase 2** | Months 9-12 | 75% Test262, parallel isolates |
| **Phase 3** | Months 13-18 | 90% Test262, baseline JIT |
| **Phase 4** | Months 19-24 | 95%+ Test262, v1.0 release |

## Contributing

We welcome contributors at every level! See [CONTRIBUTING.md](CONTRIBUTING.md) for:
- How to set up the development environment
- Architecture overview for newcomers
- "Good First Issue" guide
- Code review process

Join our [Discord](#) to chat with the team.

## License

Dual-licensed under [MIT](LICENSE-MIT) and [Apache 2.0](LICENSE-APACHE).
