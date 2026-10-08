# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-10-08

### Added
- **Core Engine Architecture**: Multi-crate modular monorepo (`doms_core`, `doms_vm`, `doms_compiler`, `doms_parser`, `doms_gc`, `doms_parallel`, `doms_ffi`, `doms_cli`, `doms_wasm`).
- **Memory Management**: Generational Cheney copying nursery GC, mark-compact old generation, large object space, and interior pointer tracing support.
- **Bytecode Virtual Machine**: Register-based opcode instruction set, NaN-boxed 64-bit value representations, inline property caches, and shape transition trees.
- **Parallelism Primitives**: SharedArrayBuffer, lock-free Atomics, cross-isolate MessageChannels, and work-stealing thread pools.
- **Web Standards**: Built-in implementations of `TextEncoder`, `TextDecoder`, `URL`, `URLSearchParams`, `EventTarget`, `CustomEvent`, timers, and microtask queue.
- **Debugging & Tooling**: Chrome DevTools Protocol (CDP) WebSocket inspector server (`--inspect`, `--inspect-brk`), Source Map v3 lookup, and interactive REPL.
- **ECMAScript Conformance**: Multi-threaded Test262 conformance test runner with regression snapshotting and summary reporting.
- **C ABI & WASM**: Full C bindings header (`doms_ffi`) and freestanding WebAssembly module (`doms_wasm`) for sandboxed embedding.

### Changed
- Standardized version flags across CLI and REPL to dynamic workspace versioning.
- Ensured C ABI `doms_get_error_message` returns valid null-terminated C string pointers.
- Hardened WebAssembly allocation and deallocation to ensure zero-initialized buffer semantics.

### Fixed
- Fixed parser issue where ES module `import`/`export` statements were evaluated in script context.
- Fixed Cheney nursery scavenge forwarding references.
