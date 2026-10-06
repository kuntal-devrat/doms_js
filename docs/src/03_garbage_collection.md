# Chapter 3: Memory Management & GC Design

DOMS implements a **generational garbage collector** (`doms_gc`) grounded in the weak generational hypothesis: *most objects die young*.

---

## 1. Heap Partitioning

The heap is divided into three distinct physical spaces:

```text
┌─────────────────────────────────────────────────────────────┐
│                          DOMS HEAP                          │
│                                                             │
│  Young Generation (Nursery)         Old Generation          │
│  ┌───────────────┬───────────────┐  ┌────────────────────┐  │
│  │  From-Space   │   To-Space    │  │ Mark-Sweep-Compact │  │
│  │  (4 MB Bump)  │  (4 MB Bump)  │  │ (Long-lived data)  │  │
│  └───────────────┴───────────────┘  └────────────────────┘  │
│                                                             │
│  Large Object Space (LOS)                                   │
│  ┌───────────────────────────────────────────────────────┐  │
│  │ Allocations > 8 KB (Pinned, individually tracked)     │  │
│  └───────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

---

## 2. Object Header Layout

Every GC-managed allocation begins with an 8-byte aligned [`GcHeader`]:

```text
Bits 0..31  : Allocation size in bytes (including header)
Bits 32..39 : Type tag (Object, String, Array, Function, Raw, etc.)
Bits 40..47 : Flags
              - Bit 0: FORWARDED (Cheney copying phase)
              - Bit 1: MARKED (Old-gen mark phase)
              - Bit 2: PINNED (Large object space, never moved)
              - Bit 3: TENURED (Promoted to old generation)
Bits 48..63 : Age / Scavenge survival counter
```

When an object is copied during scavenging, the memory after the header is repurposed to store the **forwarding pointer** to its new location in To-Space.

---

## 3. Semi-Space Nursery & Bump Allocation

New objects are allocated in the nursery's `from_space` in **O(1)** time using a bump pointer:

```rust
let aligned_size = (size + HEADER_SIZE + GC_ALIGN - 1) & !(GC_ALIGN - 1);
if bump_ptr + aligned_size <= limit {
    let result = bump_ptr;
    bump_ptr += aligned_size;
    return Some(result);
}
// Nursery full: trigger scavenge
```

---

## 4. Cheney's Scavenging Algorithm

During collection:
1. Roots (stack frames, global handles) are scanned.
2. Referenced young objects are copied from `from_space` into `to_space`.
3. A forwarding pointer is written to the old header in `from_space`.
4. If another reference points to the same object, it is updated directly from the forwarding pointer.
5. The roles of `from_space` and `to_space` are swapped.
6. Dead young objects are reclaimed instantly by resetting `from_space.bump_ptr = 0`.

---

## 5. The `Trace` Trait

Any Rust struct stored in GC memory implements `Trace`:

```rust
pub trait Trace {
    fn trace(&self, tracer: &mut Tracer);
}
```

This guarantees safety without exposing low-level pointer arithmetic to engine contributors.
