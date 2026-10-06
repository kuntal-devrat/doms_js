# Contributing to DOMS

Thank you for your interest in contributing to DOMS! This document will guide you through the process of contributing, from setting up your development environment to submitting your first pull request.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Setup](#development-setup)
- [Architecture Overview](#architecture-overview)
- [Finding Work](#finding-work)
- [Making Changes](#making-changes)
- [Testing](#testing)
- [Submitting a Pull Request](#submitting-a-pull-request)
- [Code Style](#code-style)

## Code of Conduct

By participating in this project, you agree to abide by our [Code of Conduct](CODE_OF_CONDUCT.md). Please read it before contributing.

## Getting Started

### Prerequisites

- **Rust 1.85+** (install via [rustup](https://rustup.rs/))
- **Git**
- **A C compiler** (gcc or clang) — for testing the C API

### Clone the Repository

```bash
git clone https://github.com/nicodes-js/doms.git
cd doms
```

### Build

```bash
cargo build
```

### Run Tests

```bash
cargo test --workspace
```

### Run the CLI

```bash
cargo run -p doms_cli -- repl
```

## Development Setup

### Recommended IDE Setup

We recommend **VS Code** with:
- `rust-analyzer` extension
- `Even Better TOML` extension
- `CodeLLDB` extension (for debugging)

### Useful Commands

```bash
# Run all tests
cargo test --workspace

# Run tests for a specific crate
cargo test -p doms_gc
cargo test -p doms_parser

# Check formatting
cargo fmt --check

# Run linter
cargo clippy --workspace -- -D warnings

# Build documentation
cargo doc --workspace --no-deps --open

# Run Test262 conformance suite
./tests/test262/run.sh

# Build the architecture guide
cd docs && mdbook build
```

## Architecture Overview

DOMS is split into independent crates to make contributing easy. **You don't need to understand the whole engine to contribute.**

```
Crate               What it does                   Difficulty
─────────────────────────────────────────────────────────────
doms_gc             Garbage collector               🔴 Hard
doms_parser         Lexer + Parser → AST            🟡 Medium
doms_compiler       AST → Bytecode                  🟡 Medium
doms_vm             Bytecode interpreter             🟡 Medium
doms_runtime        Built-in JS objects              🟢 Easy
doms_parallel       Multi-isolate parallelism        🔴 Hard
doms_core           Ties everything together         🟡 Medium
doms_ffi            C ABI bindings                   🟡 Medium
doms_cli            REPL + script runner             🟢 Easy
```

### Best Entry Points for New Contributors

1. **`doms_runtime`** — Implement built-in methods like `Array.prototype.at()` or `String.prototype.padStart()`. Each method is a self-contained function. No GC knowledge needed.

2. **`doms_parser`** — Add parsing for new syntax. The parser is a recursive descent parser — straightforward to extend.

3. **`doms_cli`** — Improve the REPL experience, add command-line flags, improve error formatting.

4. **Documentation** — Fix typos, improve examples, add architecture diagrams.

5. **Test262** — Help categorize and triage Test262 failures.

## Finding Work

### Good First Issues

Look for issues labeled [`good first issue`](https://github.com/nicodes-js/doms/labels/good%20first%20issue). These are curated tasks that:
- Have clear scope
- Don't require deep engine knowledge
- Include guidance on where to start
- Are reviewable by the core team

### Issue Labels

| Label | Meaning |
|:---|:---|
| `good first issue` | Great for newcomers |
| `help wanted` | Core team needs help |
| `bug` | Something is broken |
| `enhancement` | New feature or improvement |
| `documentation` | Docs need work |
| `A-parser` | Related to the parser |
| `A-vm` | Related to the VM |
| `A-gc` | Related to the GC |
| `A-runtime` | Related to built-in objects |
| `A-parallel` | Related to parallelism |
| `A-ffi` | Related to the C API |

### RFC Process

Significant changes (new built-in APIs, GC algorithm changes, VM opcode additions) go through an **RFC (Request for Comments)** process:

1. Open a GitHub Discussion in the "RFCs" category
2. Describe the change, motivation, and alternatives
3. Community discussion period (at least 1 week)
4. Core team decision

## Making Changes

### Branch Naming

```
feature/short-description    # New features
fix/short-description        # Bug fixes
docs/short-description       # Documentation changes
refactor/short-description   # Code cleanup
```

### Commit Messages

We follow [Conventional Commits](https://www.conventionalcommits.org/):

```
feat(parser): add parsing for optional chaining
fix(gc): correct nursery overflow check
docs(readme): update build instructions
test(runtime): add Array.prototype.at tests
refactor(vm): extract opcode dispatch table
```

## Testing

### Unit Tests

Every module should have tests. Place them in a `#[cfg(test)] mod tests` block at the bottom of the file.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nan_boxing_double() {
        let val = JsValue::from_double(3.14);
        assert!(val.is_double());
        assert_eq!(val.as_double(), Some(3.14));
    }
}
```

### Integration Tests

Place integration tests in the `tests/` directory:

```bash
tests/
├── test262/         # ECMAScript conformance
├── integration/     # End-to-end engine tests
└── ffi/             # C API tests
```

### Test262

We track conformance against the [official ECMAScript Test Suite](https://github.com/nicodes-js/test262):

```bash
# Download test262 (one-time)
git submodule update --init tests/test262/test262

# Run conformance tests
./tests/test262/run.sh

# Run a specific test
cargo run -p doms_cli -- run tests/test262/test262/test/language/expressions/addition/...
```

## Submitting a Pull Request

1. **Fork** the repository
2. **Create a branch** from `main`
3. **Make your changes** with clear, well-tested commits
4. **Run checks locally:**
   ```bash
   cargo fmt --check
   cargo clippy --workspace -- -D warnings
   cargo test --workspace
   ```
5. **Open a PR** with:
   - Clear title (conventional commit style)
   - Description of what changed and why
   - Link to any related issues
6. **Address review feedback** promptly

### PR Review Checklist

- [ ] Code compiles without warnings
- [ ] All existing tests pass
- [ ] New tests added for new functionality
- [ ] Documentation updated if needed
- [ ] No unrelated changes included

## Code Style

### Rust

- Follow `rustfmt` defaults (we have a `rustfmt.toml`)
- Use `clippy` and fix all warnings
- Prefer `thiserror` for error types
- Document public APIs with `///` doc comments
- Use `// SAFETY:` comments above every `unsafe` block explaining the invariant

### Documentation

- Use present tense ("Add feature" not "Added feature")
- Keep line length under 100 characters in docs
- Include code examples in doc comments for public APIs

### Unsafe Code Policy

Unsafe code is **only allowed** in:
- `doms_gc` (memory management is inherently unsafe)
- `doms_ffi` (C ABI boundary)
- Performance-critical VM inner loops (with justification)

Every `unsafe` block must have a `// SAFETY:` comment. PRs with unjustified unsafe code will be rejected.

---

## Questions?

- Open a [GitHub Discussion](https://github.com/nicodes-js/doms/discussions)
- Join our [Discord](#)
- Tag `@core-team` in your issue or PR

Thank you for helping build the future of JavaScript engines! 🚀
