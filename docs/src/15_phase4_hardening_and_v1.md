# Phase 4 Hardening & 1.0 Stability Freeze

Phase 4 concludes the engine roadmap, locking in production hardening, security fuzzing, and ABI stability guarantees.

---

## 1. Roadmap Milestones Completed

- **Phase 0 (Foundation)**: Generational GC, NaN-boxed `JsValue`, AST, Compiler, VM interpreter loop.
- **Phase 1 (Language Core)**: Full object model with Hidden Classes/Shapes, Prototype chains, Closures, C ABI (`libdoms.h`).
- **Phase 2 (Async & Parallelism)**: Promises, Async/Await, ES Modules, `SharedArrayBuffer` + `Atomics`, Multi-isolate scheduler, Inline Caches.
- **Phase 3 (Performance)**: Copy-and-Patch Baseline JIT, Type feedback, Old-gen compaction, Concurrent sweeping, WeakRef/FinalizationRegistry.
- **Phase 4 (Hardening & 1.0 Freeze)**:
  - ES2024/ES2025 additions (`Object.groupBy`, `Map.groupBy`, `Promise.withResolvers`, `isWellFormed`, Set methods).
  - Memory Caging (4 GiB virtual sandbox for ArrayBuffer security).
  - Chrome DevTools Protocol (CDP) JSON-RPC debugger.
  - Source Maps v3 parser and VLQ stack symbolication.
  - Continuous fuzzing suite (`fuzz_parser`, `fuzz_compiler`, `fuzz_vm`).
  - Production browser & game engine demos.
  - Stable 1.0 C ABI freeze.

---

## 2. The 1.0 ABI Stability Guarantee

The C ABI declared in `include/doms.h` is frozen:
- `#define DOMS_ABI_VERSION 1`
- All struct handles are strictly opaque pointers (`doms_engine_t*`, `doms_isolate_t*`, `doms_context_t*`, `doms_value_t*`).
- Internal algorithmic updates to the GC, JIT, or VM will never break binary compatibility with compiled C and C++ applications.
