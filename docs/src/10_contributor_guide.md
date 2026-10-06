# Chapter 10: Contributor Guide & Development Workflow

DOMS is built to be approachable and welcoming to contributors across the language ecosystem.

---

## 1. Quick Start

### Prerequisites
- Modern Rust toolchain (edition 2024 support)
- Git and bash

### Building and Testing
```bash
# Clone the repository
git clone https://github.com/nicodes-js/doms.git
cd doms

# Check all crates
cargo check --workspace --all-targets

# Run the test suite
cargo test --workspace

# Run linter and formatting checks
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check

# Test CLI
cargo run -p doms_cli -- eval "1 + 1"
```

---

## 2. Where to Contribute

The project is structured so contributors can work without mastering the entire compiler pipeline:

| Level | Subsystem | Typical Tasks |
|:---|:---|:---|
| **Beginner** | `doms_runtime` | Add standard library built-ins (`Math.trunc`, `Array.prototype.at`) |
| **Intermediate** | `doms_parser` | Add new ES grammar (e.g. ternary operator `?:`, template strings) |
| **Intermediate** | `doms_compiler` | Emit bytecode for new syntax, optimize local slot allocation |
| **Advanced** | `doms_vm` | Implement inline caches, call stack frame handling |
| **Advanced** | `doms_gc` | Mark-sweep collection, compacting collector, write barriers |
| **Advanced** | `doms_parallel` | Work-stealing scheduler optimizations, isolate clustering |

---

## 3. Running Test262

Verify ECMAScript conformance:

```bash
chmod +x tests/test262/run.sh
./tests/test262/run.sh
```

Results are saved to `tests/test262/results/summary.json`.

---

## 4. RFC Process

Major features (such as JIT design, changes to the C ABI, or GC compaction strategies) must go through an RFC (Request for Comments) issue before implementation. Open an RFC with motivation, design specification, and backwards compatibility analysis.
