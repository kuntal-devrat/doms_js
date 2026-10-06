# Chapter 6: Bytecode & Compiler

The `doms_compiler` crate compiles an AST [`Program`] into a sequence of executable virtual machine bytecode instructions stored in a [`BytecodeChunk`].

---

## 1. Instruction Set Architecture

DOMS uses a compact stack-oriented bytecode format designed for quick dispatch and direct translation to copy-and-patch JIT code:

### Literals & Constants
- `LoadConst(u16)`: Load indexed constant from pool.
- `LoadNull`, `LoadUndefined`, `LoadTrue`, `LoadFalse`: Load singleton primitives.

### Arithmetic & Logic
- `Add`, `Sub`, `Mul`, `Div`, `Mod`: Numeric arithmetic (and string concatenation on `Add`).
- `Neg`, `Not`, `BitNot`, `Typeof`: Unary operations.
- `BitAnd`, `BitOr`, `BitXor`: 32-bit bitwise logic.
- `Eq`, `NotEq`, `StrictEq`, `StrictNotEq`, `Lt`, `Lte`, `Gt`, `Gte`: Comparisons.

### Variable Operations
- `GetLocal(u16)` / `SetLocal(u16)`: Fast stack slot access.
- `GetGlobal(u16)` / `SetGlobal(u16)`: Named global environment access.

### Control Flow
- `Jump(i32)`: Unconditional relative/absolute jump.
- `JumpIfFalse(i32)`: Conditional jump when stack top is falsy.
- `JumpIfTrue(i32)`: Conditional jump when stack top is truthy.

### Subroutines & Stack
- `Call(u8)`: Function invocation with argument count.
- `Return`: Return from current activation frame.
- `Pop`, `Dup`, `Halt`: Stack manipulation and termination.

---

## 2. Lexical Scoping and Symbol Tables

The compiler maintains a lexical `SymbolTable`:

1. Entering a block or function increments `scope_depth`.
2. Variables declared with `let` or `const` receive stack-relative slot numbers.
3. Exiting a scope counts out-of-scope locals and emits matching `Pop` instructions to clean the stack.
4. Unresolved identifiers at depth 0 fall back to global lookup instructions (`GetGlobal` / `SetGlobal`).

---

## 3. Jump Back-Patching

Control flow structures (`if`, `else`, `while`) emit placeholder jump targets that are patched once the target instruction index is generated:

```rust
let jump_false = chunk.emit(OpCode::JumpIfFalse(0), line);
self.compile_statement(then_branch)?;
let target = chunk.instructions.len() as i32;
chunk.patch_jump(jump_false, target);
```
