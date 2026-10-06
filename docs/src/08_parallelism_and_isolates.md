# Chapter 8: Concurrency & Parallel Isolates

JavaScript is historically single-threaded, but modern browsers and high-performance applications require parallel execution. `doms_parallel` provides first-class concurrency primitives.

---

## 1. The Isolate Model

In DOMS, an **Isolate** represents an independent JavaScript world:

```text
┌─────────────────────────┐         ┌─────────────────────────┐
│        Isolate A        │         │        Isolate B        │
│  • Independent GC Heap  │         │  • Independent GC Heap  │
│  • Bytecode Interpreter │         │  • Bytecode Interpreter │
│  • Execution Context    │         │  • Execution Context    │
└────────────┬────────────┘         └────────────▲────────────┘
             │                                   │
             │       MessageChannel / PostMsg    │
             └───────────────────────────────────┘
```

- **Zero Shared Mutable State:** Isolates cannot directly reference each other's GC heaps. This eliminates global interpreter locks (GIL) and lock contention.
- **Thread Safety:** Isolates can run concurrently on separate OS threads without synchronization bottlenecks.
- **Instant Cleanup:** Terminating a worker or tab destroys its isolate and frees its entire heap instantly.

---

## 2. Message Channels & PostMessage

Isolates communicate via asynchronous `MessageChannel`s:
- Data is serialized into byte payloads via structured cloning.
- Receiver receives an isolated copy of the data.
- Non-blocking channel primitives prevent thread starvation.

---

## 3. SharedArrayBuffer & Atomics

For workloads requiring high-throughput shared memory (such as game audio rendering or parallel graphics rasterization):

1. **`SharedArrayBuffer`:** Backed by thread-safe atomic memory buffers (`Arc<Vec<AtomicU8>>`). Both isolates hold references to the same memory segment.
2. **`Atomics`:** Lock-free atomic primitives conforming to ECMAScript:
   - `Atomics.load(sab, index)`
   - `Atomics.store(sab, index, val)`
   - `Atomics.add(sab, index, val)`
   - `Atomics.sub(sab, index, val)`
   - `Atomics.compareExchange(sab, index, expected, replacement)`

---

## 4. Work-Stealing Scheduler

The `Scheduler` manages an OS thread worker pool. Rather than dedicating one OS thread per isolate (which quickly exhausts system thread limits), N isolates multiplex onto M worker threads, dynamically prioritizing user-facing UI isolates over background workers.
