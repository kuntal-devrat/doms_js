# Chapter 4: Value Representation (NaN-Boxing)

In high-performance runtimes, value representation dictates CPU cache efficiency and instruction throughput. DOMS uses **NaN-boxing** for all JavaScript values represented by [`JsValue`].

---

## 1. IEEE 754 Floating-Point Structure

A standard double-precision float is 64 bits wide:

```text
 1 bit        11 bits                              52 bits
┌──────┬──────────────────────┬─────────────────────────────────────────────────┐
│ Sign │       Exponent       │                    Mantissa                     │
└──────┴──────────────────────┴─────────────────────────────────────────────────┘
```

When all exponent bits are `1` and the mantissa is non-zero, the value represents a `NaN` (Not-a-Number). In standard hardware, only one canonical pattern is needed for `NaN`. The remaining ~`2^52` bit combinations provide unused space to pack other JavaScript types.

---

## 2. The DOMS Tagging Layout

DOMS allocates the upper 16 bits for type tags and the lower 48 bits for payloads:

| Type Tag (Bits 48–63) | Type Representation | Payload (Bits 0–47) |
|:---|:---|:---|
| `< 0xFFF9` | **f64 Double** | IEEE 754 Double precision float |
| `0xFFF9` | **Int32 (Smi)** | 32-bit signed integer |
| `0xFFFA` | **Boolean** | Bit 0: `true` (1) / `false` (0) |
| `0xFFFB` | **Null** | None (0) |
| `0xFFFC` | **Undefined** | None (0) |
| `0xFFFD` | **Object Pointer** | 48-bit GC heap pointer |
| `0xFFFE` | **String Pointer** | 48-bit GC heap pointer |
| `0xFFFF` | **Symbol Pointer** | 48-bit GC heap pointer |

---

## 3. Benefits of NaN-Boxing

1. **Zero-Allocation Primitives:**
   Numbers, booleans, `null`, and `undefined` require zero allocations and fit directly in CPU registers (`u64`).
2. **Small Integer (Smi) Optimization:**
   Loop counters and array indices fit into the 32-bit integer payload without converting to floating point.
3. **Hardware Alignment:**
   Current 64-bit architectures (x86_64, aarch64) use 48-bit virtual address spaces, guaranteeing pointers fit into bits 0..47.
4. **Copy Semantics:**
   `JsValue` implements Rust's `Copy` and `Clone` traits, incurring no ref-count overhead or atomic locking.
