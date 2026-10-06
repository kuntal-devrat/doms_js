# Chapter 7: VM Execution & Inline Caches

The virtual machine (`doms_vm`) is responsible for interpreting bytecode chunks and accelerating hot property accesses via inline caching.

---

## 1. Virtual Machine Mechanics

The `Vm` maintains:
- **Evaluation Stack (`Vec<VmValue>`):** Holds intermediate operands, locals, and results.
- **Globals Table:** Stores global bindings in the execution environment.
- **Heap Reference:** Connects directly to `doms_gc` for object allocation and garbage collection.
- **Inline Cache Registry:** Stores cache slots paired with property lookup sites.

---

## 2. Dispatch Loop

The interpreter executes a loop:
1. Read instruction at instruction pointer `ip`.
2. Advance `ip`.
3. Match opcode and perform stack manipulations.
4. Check for unhandled exceptions or stack underflow.
5. Exit on `Halt` or `Return`, returning the completion value.

```rust
while ip < instructions.len() {
    match instructions[ip] {
        OpCode::Add => {
            let b = self.pop()?;
            let a = self.pop()?;
            self.push(a + b);
        }
        ...
    }
}
```

---

## 3. Inline Caches (IC)

Property accesses in dynamic languages (e.g. `obj.prop`) traditionally require hash-map lookups on every access. Inline caches remember previous lookup results:

```text
               First Access
  Uninitialized ───────────► Monomorphic (Cache Shape & Offset)
                                  │
                                  ▼ Different Shape
                             Polymorphic (2-4 Shapes)
                                  │
                                  ▼ Many Shapes
                             Megamorphic (Generic Dictionary Lookup)
```

1. **Monomorphic:** Site only ever encounters one object shape. Lookup compiles to a direct memory offset read: `*(obj_ptr + cached_offset)`.
2. **Polymorphic:** Site encounters 2 to 4 distinct shapes. Small linear search through cached pairs.
3. **Megamorphic:** Site encounters many shapes; falls back to slow path dictionary search.

This mechanism forms the basis for JIT speculative optimization in Phase 3.
