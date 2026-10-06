# Case Studies: Parallel Browsers & Game Engines

DOMS.js was designed from day one to excel in two demanding embedding domains:
1. **Next-Generation Parallel Browsers** (Servo-style multi-threaded architectures)
2. **Game Engines** (Bevy, Godot, SDL ECS loops)

---

## 1. Parallel Browser Architecture (Servo-Style)

In a parallel browser, page rendering, layout, and JavaScript execution do not block each other:
- **Tab Isolation**: Each tab/iframe executes in its own isolated `Isolate` with an independent young and tenured GC heap.
- **Offscreen Framebuffer Workers**: Background workers compute CSS layout and write rendered pixels directly into `SharedArrayBuffer` memory using atomic operations (`Atomics.store`).
- **Compositor**: Reads the shared framebuffer concurrently without CPU memory copy overhead.
- **Message Passing**: Tabs and workers exchange structured messages via `MessageChannel`.

See `examples/browser_prototype.rs` for a complete working prototype.

---

## 2. Game Engine Scripting Loop

In game development, engines require predictable memory usage, fast foreign function calls, and high frame rates (60–120 FPS):
- **Deterministic Ticks**: The native Rust game loop runs `onUpdate(deltaTime)` each frame.
- **Entity State Sync**: JavaScript controls high-level game logic (AI, player input, state machines), while Rust executes performance-critical physics and rendering.
- **Zero GC Pause Spikes**: Young-generation bump allocation ensures short lived temporary math objects are collected during frame boundaries without stuttering.

See `examples/game_engine_embedding.rs` for a working implementation.
