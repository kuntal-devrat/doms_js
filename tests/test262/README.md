# DOMS Test262 Conformance Test Harness

This directory contains the conformance test harness for running ECMAScript specification tests against the DOMS engine.

## Usage

Run the Phase 0 conformance test suite:

```bash
./tests/test262/run.sh
```

To run against a full Test262 suite clone:

```bash
git clone --depth 1 https://github.com/tc39/test262.git tests/test262/test262
./tests/test262/run.sh
```

## Conformance Tracking

Results and pass rates are automatically tracked in CI and output to `results/summary.json`.
