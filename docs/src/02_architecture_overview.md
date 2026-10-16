# Chapter 2: Architecture Overview

The DOMS engine is structured as a decoupled multi-crate workspace. Each subsystem has strict interfaces, ensuring independent testability, fast compile times, and clean domain separation.

---

## 1. Workspace Subsystems

```text
doms/
├── Cargo.toml                  # Workspace root
├── crates/
│   ├── doms_gc/                # Generational GC (Nursery bump allocator + Old Gen)
│   ├── doms_parser/            # Lexer + Pratt Parser -> AST
│   ├── doms_compiler/          # AST -> Bytecode compiler & symbol resolution
│   ├── doms_vm/                # Stack virtual machine & inline caching
│   ├── doms_runtime/           # ECMAScript built-ins & global environments
│   ├── doms_parallel/          # Isolates, messaging, SharedArrayBuffer & Atomics
│   ├── doms_core/              # High-level Rust API (Engine, Isolate, Context, JsValue)
│   ├── doms_ffi/               # C ABI export (libdoms.so / libdoms.a / doms.h)
│   └── doms_cli/               # Command-line driver, runner, and interactive REPL
└── tests/
    └── test262/                # ECMAScript conformance test suite
```

---

## 2. End-to-End Execution Pipeline

```text
  JavaScript Source
         │
         ▼
  ┌──────────────┐
  │  doms_parser │ ──► Lexer (tokens) ──► Pratt Parser (AST)
  └──────────────┘
         │
         ▼
  ┌──────────────┐
  │doms_compiler │ ──► Symbol Table (locals/globals) ──► BytecodeChunk
  └──────────────┘
         │
         ▼
  ┌──────────────┐
  │   doms_vm    │ ──► Bytecode Interpreter ──► Inline Caches (property access)
  └──────────────┘           │
         ▲                   ▼
         │             ┌──────────────┐
         └──────────── │   doms_gc    │ (Object allocations, Cheney scavenge)
                       └──────────────┘
```

---

## 3. Separation of Concerns

1. **Parser Independence:** `doms_parser` has zero dependency on `doms_gc` or `doms_vm`. It produces pure AST structures that can be serialized or inspected.
2. **Bytecode Portability:** `doms_compiler` targets an instruction format that abstract machines or baseline JITs can consume.
3. **Memory Safety:** Heap pointers inside `doms_vm` are tracked via `GcHeader` and `Trace` traits, preventing dangling pointers.
4. **Clean Host Boundary:** Both Rust hosts (`doms_core`) and C/C++ hosts (`doms_ffi`) consume identical semantic execution.
