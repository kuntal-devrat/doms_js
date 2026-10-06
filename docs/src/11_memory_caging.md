# Memory Caging & Security Sandbox

To protect browser hosts and embedding applications from memory corruption, DOMS.js adopts a modern **Memory Caging** architecture inspired by Ladybird and V8.

---

## 1. The Threat Model

JavaScript engines running untrusted third-party code face vulnerability vectors:
- Out-of-bounds reads/writes in `ArrayBuffer` or `SharedArrayBuffer`
- Type confusion leading to forged object pointers
- Heap spraying or buffer overflows attempting to overwrite OS stacks, function pointers, or executable code

In traditional engines, a compromised pointer can read or write any address in the process address space.

---

## 2. The 4 GiB Virtual Memory Cage

In DOMS.js, all `ArrayBuffer` and `SharedArrayBuffer` backing allocations are strictly quarantined within a reserved virtual memory window:

```
Process Virtual Memory Space
┌──────────────────────────────────────────────────────────────┐
│  OS Stack / Code / Metadata (Untouchable by Cage)            │
├──────────────────────────────────────────────────────────────┤
│  DOMS Memory Cage (4 GiB Reserved Virtual Region)            │
│  ┌───────────────────────┬───────────────────────┬────────┐  │
│  │ ArrayBuffer #1 (1 MB) │ SharedArrayBuffer (4K)│ ...    │  │
│  └───────────────────────┴───────────────────────┴────────┘  │
│  All offsets are bounded or masked relative to cage_base      │
├──────────────────────────────────────────────────────────────┤
│  Host Application Memory (Protected from Buffer Escapes)     │
└──────────────────────────────────────────────────────────────┘
```

---

## 3. Implementation in Rust (`doms_parallel::MemoryCage`)

The cage guarantees:
1. **Base & Capacity Registration**: `cage_base` and `cage_size` (up to 4 GiB virtual capacity).
2. **Bounds Checking**: Every allocation through `CagedBuffer` verifies that reads and writes cannot exceed buffer boundaries.
3. **Address Confinement**: Raw address verification via `is_within_cage(ptr)`.
4. **Zero Overhead Fast Path**: Within hot loops, offsets are checked against the cage length in a single CPU branch.
