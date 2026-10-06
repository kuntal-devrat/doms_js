#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
RESULTS_DIR="${SCRIPT_DIR}/results"
mkdir -p "${RESULTS_DIR}"

DOMS_BIN="${WORKSPACE_ROOT}/target/release/doms"
if [ ! -f "${DOMS_BIN}" ]; then
    DOMS_BIN="${WORKSPACE_ROOT}/target/debug/doms"
fi

# Build release CLI if not present
if [ ! -f "${DOMS_BIN}" ]; then
    echo "Building DOMS CLI in release mode..."
    cargo build --release --bin doms
fi

echo "============================================================"
echo "DOMS JavaScript Engine — Official Test262 Conformance Suite"
echo "============================================================"
echo "Running official tc39/test262 tests..."

# Forward all arguments directly to native test262 runner
exec "${DOMS_BIN}" test262 "$@"
