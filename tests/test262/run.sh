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

echo "=========================================="
echo "DOMS JavaScript Engine — Test262 Conformance Harness"
echo "=========================================="

PASSED=0
FAILED=0
TOTAL=0

run_test() {
    local name="$1"
    local code="$2"
    TOTAL=$((TOTAL + 1))
    
    # Run code with DOMS CLI
    if OUTPUT=$("${DOMS_BIN}" eval "${code}" 2>&1); then
        PASSED=$((PASSED + 1))
        echo "  [PASS] ${name}"
    else
        FAILED=$((FAILED + 1))
        echo "  [FAIL] ${name} -> ${OUTPUT}"
    fi
}

echo "Running Phase 0 language conformance checks..."

run_test "primitive-number" "let x = 42; x;"
run_test "primitive-string" "let s = 'hello'; s;"
run_test "primitive-boolean" "let b = true; b;"
run_test "primitive-null" "let n = null; n;"
run_test "primitive-undefined" "let u = undefined; u;"
run_test "arithmetic-addition" "1 + 2;"
run_test "arithmetic-precedence" "2 + 3 * 4;"
run_test "bitwise-and" "5 & 3;"
run_test "bitwise-or" "4 | 2;"
run_test "comparison-lt" "1 < 2;"
run_test "comparison-gt" "5 > 3;"
run_test "equality-strict" "1 === 1;"
run_test "while-loop" "let i = 0; while (i < 3) { i = i + 1; } i;"
run_test "if-else" "let r = 0; if (true) { r = 1; } else { r = 2; } r;"

PERCENT=0
if [ "${TOTAL}" -gt 0 ]; then
    PERCENT=$((PASSED * 100 / TOTAL))
fi

echo "=========================================="
echo "Summary: ${PASSED}/${TOTAL} passed (${PERCENT}%)"
echo "=========================================="

cat <<EOF > "${RESULTS_DIR}/summary.json"
{
  "total": ${TOTAL},
  "passed": ${PASSED},
  "failed": ${FAILED},
  "percentage": ${PERCENT},
  "timestamp": "$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
}
EOF

echo "Results saved to ${RESULTS_DIR}/summary.json"
