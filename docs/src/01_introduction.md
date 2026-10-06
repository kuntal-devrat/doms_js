# Chapter 1: Introduction & Philosophy

Welcome to the **DOMS.js Architecture Guide**.

DOMS is an embeddable, high-performance JavaScript engine implemented in Rust, designed from the ground up for modern parallel browsers, game engines, and application runtimes.

---

## 1. The Modern JavaScript Engine Problem

Over the last decade, JavaScript engines have divided into two extremes:

1. **Mega-Engines (V8, JavaScriptCore, SpiderMonkey):**
   - Millions of lines of C++.
   - Monolithic and deeply intertwined with specific host platforms.
   - 30+ minute build times and massive binary sizes (20–40 MB+).
   - Inaccessible to independent browser projects, game developers, or embedded systems wanting lightweight control.

2. **Indie & Embedded Engines (QuickJS, Boa, Brimstone, Nova):**
   - **QuickJS:** Brilliant in design and footprint, but uses ref-counting GC (fragile with cyclic graphs) and stalled after original development slowed down.
   - **Boa:** Impressive Test262 progress, but treated GC as a late addition for over five years, leading to architectural debt.
   - **Brimstone:** Phenomenal 97%+ conformance, but built by a solo developer without community infrastructure, public RFCs, or embedding APIs.
   - **Nova:** Novel data-oriented ideas, but remains an early-stage research prototype with a high bus factor.

---

## 2. The DOMS Tenets

DOMS was conceived to break these cycles by adhering to five uncompromised principles:

### A. GC Architecture from Day 1
A garbage-collected language cannot treat its collector as an afterthought. Value representation, object headers, pointer tagging, and thread isolation all depend on the GC design. DOMS begins with a high-performance semi-space bump allocator (Cheney scavenge) and an old generation.

### B. The C ABI is the Product
An engine is only as useful as its host boundary. Rather than bolting on an FFI layer after engine completion, DOMS defines `libdoms.h` first. Any language that speaks C can embed DOMS in fewer than 10 lines of code.

### C. Parallel-First Architecture
Modern browsers (like Servo and next-generation layout engines) require concurrent execution across multiple threads. DOMS isolates share no mutable state by default, multiplex cleanly onto thread pools, and communicate via structured cloning, `SharedArrayBuffer`, and `Atomics`.

### D. Layered, Contributor-Friendly Boundaries
Engine contributions should not require deep compiler wizardry. A contributor implementing `Array.prototype.flat()` works in `doms_runtime` and touches neither GC pointers nor bytecode JIT dispatchers.

### E. Community Governance
From day 1, DOMS is governed under a multi-maintainer model with public RFCs, automated CI, and open Discord/GitHub triage.
