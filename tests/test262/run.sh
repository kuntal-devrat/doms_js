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

# Map --summary-only or default to safe parameters
ARGS=()
for arg in "$@"; do
    if [ "$arg" = "--summary-only" ]; then
        ARGS+=("-s" "language/eval-code" "-t" "2")
    else
        ARGS+=("$arg")
    fi
done

if [ ${#ARGS[@]} -eq 0 ]; then
    exec "${DOMS_BIN}" test262 -t 2
else
    exec "${DOMS_BIN}" test262 "${ARGS[@]}"
fi

