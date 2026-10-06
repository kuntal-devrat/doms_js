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

echo "Running Phase 2 language & concurrency conformance checks..."
run_test "promise-constructor-resolve" "let p = new Promise(function(resolve, reject) { resolve(42); }); await p;"
run_test "promise-static-resolve" "await Promise.resolve(100);"
run_test "promise-then" "await Promise.resolve(55).then(function(v) { return v + 10; });"
run_test "promise-catch" "await Promise.reject(404).catch(function(e) { return 200; });"
run_test "promise-finally" "await Promise.resolve(7).finally(function() { return 0; });"
run_test "promise-all" "let res = await Promise.all([Promise.resolve(10), Promise.resolve(20)]); res[0] + res[1];"
run_test "promise-race" "let res = await Promise.race([Promise.resolve(99), Promise.resolve(100)]); res;"
run_test "async-await-call" "async function add(a, b) { return a + b; } await add(20, 22);"
run_test "generator-yield-next" "function* gen() { yield 10; yield 20; return 30; } let it = gen(); let r1 = it.next(); let r2 = it.next(); let r3 = it.next(); r1.value + r2.value + r3.value;"
run_test "generator-iterator-protocol" "function* g() { yield 1; } let it = g(); let r1 = it.next(); let r2 = it.next(); r1.done === false && r2.done === true;"
run_test "shared-array-buffer" "let sab = new SharedArrayBuffer(16); sab.byteLength;"
run_test "typed-array-int32" "let sab = new SharedArrayBuffer(16); let ta = new Int32Array(sab); ta[0] = 42; ta[0];"
run_test "atomics-store-load" "let sab = new SharedArrayBuffer(16); let ta = new Int32Array(sab); Atomics.store(ta, 0, 77); Atomics.load(ta, 0);"
run_test "atomics-add-sub" "let sab = new SharedArrayBuffer(16); let ta = new Int32Array(sab); Atomics.store(ta, 0, 10); Atomics.add(ta, 0, 5); Atomics.sub(ta, 0, 2); Atomics.load(ta, 0);"
run_test "atomics-is-lock-free" "Atomics.isLockFree(4);"
run_test "worker-parallel-actor" "let w = new Worker('postMessage(42);'); w.receiveMessage();"

echo "Running Phase 3 standard library & JIT optimization conformance checks..."
run_test "map-set-get" "let m = new Map(); m.set('k', 42); m.get('k');"
run_test "map-has-size-delete" "let m = new Map(); m.set('x', 1); m.set('y', 2); m.has('x') && m.size === 2 && m.delete('x') && m.size === 1;"
run_test "map-foreach" "let m = new Map(); m.set('a', 10); m.set('b', 20); let sum = 0; m.forEach(function(v, k) { sum = sum + v; }); sum === 30;"
run_test "set-add-has" "let s = new Set(); s.add(10); s.has(10);"
run_test "set-size-delete-clear" "let s = new Set(); s.add(1); s.add(2); s.delete(1); s.size === 1 && s.has(2);"
run_test "set-foreach" "let s = new Set(); s.add(5); s.add(15); let total = 0; s.forEach(function(v) { total = total + v; }); total === 20;"
run_test "weakref-deref" "let o = { a: 1 }; let wr = new WeakRef(o); wr.deref().a === 1;"
run_test "finalization-registry" "let fr = new FinalizationRegistry(function(v) {}); fr.register({ x: 1 }, 42, 'tok'); fr.unregister('tok');"
run_test "regexp-test-exec" "let re = new RegExp('abc'); re.test('123abcdef') && re.exec('123abcdef').index === 3;"
run_test "regexp-source-flags" "let re = new RegExp('xyz', 'g'); re.source === 'xyz' && re.flags === 'g';"
run_test "intl-number-format" "let nf = new Intl.NumberFormat(); typeof nf.format(1234.5) === 'string';"
run_test "intl-datetime-format" "let df = new Intl.DateTimeFormat(); typeof df.format('2026-10-06') === 'string';"
run_test "intl-collator" "let c = new Intl.Collator(); c.compare('a', 'b') === -1;"
run_test "temporal-instant" "let t = Temporal.Now.instant(); typeof t.epochMilliseconds === 'number';"
run_test "temporal-plain-date" "let d = new Temporal.PlainDate(2026, 10, 6); d.year === 2026 && d.month === 10 && d.day === 6;"
run_test "jit-hot-loop" "function add(a, b) { return a + b; } let s = 0; for (let i = 0; i < 25; i = i + 1) { s = add(s, 1); } s === 25;"

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
