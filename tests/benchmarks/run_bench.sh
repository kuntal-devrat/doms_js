#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

DOMS_BIN="${WORKSPACE_ROOT}/target/release/doms"
if [ ! -f "${DOMS_BIN}" ]; then
    DOMS_BIN="${WORKSPACE_ROOT}/target/debug/doms"
fi

echo "=========================================="
echo "DOMS JavaScript Engine — Benchmark Suite"
echo "Binary: ${DOMS_BIN}"
echo "=========================================="

run_benchmark() {
    local file="$1"
    local name="$(basename "$file")"
    
    echo -n "Running ${name}... "
    local start_time
    start_time=$(date +%s%N)
    
    local res
    res=$("${DOMS_BIN}" run "${file}")
    
    local end_time
    end_time=$(date +%s%N)
    local elapsed_ms=$(( (end_time - start_time) / 1000000 ))
    echo "${elapsed_ms} ms (Result: ${res})"
}

for bench in "${SCRIPT_DIR}"/*.js; do
    if [ -f "$bench" ]; then
        run_benchmark "$bench"
    fi
done

echo "=========================================="
echo "All benchmarks completed."
echo "=========================================="
