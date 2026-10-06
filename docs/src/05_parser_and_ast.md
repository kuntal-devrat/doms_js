# Chapter 5: Lexer, Parser & AST

The parsing subsystem (`doms_parser`) transforms raw JavaScript text into a structured Abstract Syntax Tree (AST).

---

## 1. The Lexer

The `Lexer` scans input UTF-8 source streams into discrete `Token`s:

- **Keywords:** `let`, `const`, `var`, `function`, `if`, `else`, `while`, `return`, `typeof`, `true`, `false`, `null`, `undefined`
- **Operators & Punctuation:** Arithmetic, comparisons, logical, bitwise, assignment, delimiters
- **Identifiers & Literals:** Number floats/integers, single/double quoted strings with escape sequences
- **Trivia Handling:** Whitespace, line comments (`//`), and block comments (`/* */`) are discarded while maintaining line and column trackers for error diagnostics.

---

## 2. Pratt Parsing Algorithm

DOMS employs a **Pratt (Top-Down Operator Precedence) Parser** for expressions. Each infix and postfix operator is assigned left and right binding powers (`l_bp`, `r_bp`):

```text
Operator                     Precedence Level    Associativity
─────────────────────────────────────────────────────────────
Assignment (=)               1                   Right-to-Left
Logical OR (||)              2                   Left-to-Right
Logical AND (&&)             3                   Left-to-Right
Bitwise OR / XOR / AND       4 - 6               Left-to-Right
Equality (==, !=, ===, !==)  7                   Left-to-Right
Relational (<, <=, >, >=)    8                   Left-to-Right
Additive (+, -)              9                   Left-to-Right
Multiplicative (*, /, %)     10                  Left-to-Right
Unary (!, -, ~, typeof)      Prefix              Right-to-Left
Calls and Member Access      Postfix             Left-to-Right
```

Pratt parsing avoids deep recursion call stacks and simplifies adding new ECMAScript operators (e.g. exponentiation `**`, nullish coalescing `??`).

---

## 3. AST Structure

Statements and expressions are cleanly partitioned:

- [`Program`]: Body vector of statements.
- [`Statement`]:
  - `VarDecl`: Variable binding declaration (`let`, `const`, `var`).
  - `FunctionDecl`: Named function declaration with parameter list and body.
  - `If`: Branch condition, then-block, and optional else-block.
  - `While`: Loop condition and body.
  - `Return`: Optional return expression.
  - `Block`: Scoped statement list.
  - `Expression`: Expression statement evaluated for side-effects.
- [`Expression`]:
  - `Binary`: Operation between two sub-expressions.
  - `Unary`: Operation on a single sub-expression.
  - `Assignment`: Target identifier and assigned expression.
  - `Call`: Callee and arguments list.
  - `Member`: Object and property access.
  - `Literal`: Static constant values.
