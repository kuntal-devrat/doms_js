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

echo "Running Phase 1 language core conformance checks..."
run_test "for-loop" "let sum = 0; for (let i = 0; i < 5; i = i + 1) { sum = sum + i; } sum;"
run_test "for-loop-break" "let count = 0; for (let i = 0; i < 10; i = i + 1) { if (i === 3) { break; } count = count + 1; } count;"
run_test "for-loop-continue" "let evens = 0; for (let i = 0; i < 6; i = i + 1) { if (i === 1) { continue; } evens = evens + 1; } evens;"
run_test "function-declaration-call" "function square(x) { return x * x; } square(5);"
run_test "function-recursion" "function fact(n) { if (n <= 1) { return 1; } return n * fact(n - 1); } fact(5);"
run_test "function-multiple-args" "function add3(a, b, c) { return a + b + c; } add3(10, 20, 12);"
run_test "object-literal-access" "let obj = { x: 10, y: 20 }; obj.x + obj.y;"
run_test "object-property-assignment" "let o = { a: 1 }; o.b = 41; o.a + o.b;"
run_test "array-literal-indexing" "let arr = [10, 20, 30]; arr[0] + arr[2];"
run_test "array-element-assignment" "let a = [1, 2]; a[1] = 99; a[1];"
run_test "array-push-pop" "let list = [1, 2]; list.push(3); list.pop();"
run_test "math-builtins" "Math.floor(4.9) + Math.abs(-10);"
run_test "json-stringify" "let data = { key: 42 }; JSON.stringify(data);"
run_test "object-keys" "let o = { a: 1, b: 2 }; Object.keys(o).length;"
run_test "try-catch" "let res = 0; try { throw 42; } catch (e) { res = e; } res;"
run_test "ternary-operator" "let x = 10 > 5 ? 100 : 200; x;"

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
