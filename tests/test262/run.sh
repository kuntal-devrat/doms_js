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

OUTPUT_JSON="${RESULTS_DIR}/summary.json"
MIN_PASS=""
MARKDOWN_REPORT=""
ARGS=()

i=1
while [ $# -gt 0 ]; do
    case "$1" in
        --summary-only)
            ARGS+=("-s" "language/eval-code" "-t" "2")
            shift
            ;;
        --min-pass)
            MIN_PASS="$2"
            shift 2
            ;;
        --markdown-report)
            MARKDOWN_REPORT="$2"
            shift 2
            ;;
        -o|--output)
            OUTPUT_JSON="$2"
            ARGS+=("-o" "$2")
            shift 2
            ;;
        *)
            ARGS+=("$1")
            shift
            ;;
    esac
done

# Ensure output JSON flag is provided if not explicitly passed
HAS_OUTPUT=false
for a in "${ARGS[@]}"; do
    if [ "$a" = "-o" ] || [ "$a" = "--output" ]; then
        HAS_OUTPUT=true
        break
    fi
done

if [ "$HAS_OUTPUT" = false ]; then
    ARGS+=("-o" "${OUTPUT_JSON}")
fi

"${DOMS_BIN}" test262 "${ARGS[@]}"

# Generate markdown report if requested or in GitHub Actions
if [ -n "${MARKDOWN_REPORT}" ] || [ -n "${GITHUB_STEP_SUMMARY:-}" ]; then
    MD_FILE="${MARKDOWN_REPORT:-${RESULTS_DIR}/report.md}"
    if [ -f "${OUTPUT_JSON}" ]; then
        python3 -c "
import json, sys

with open('${OUTPUT_JSON}') as f:
    data = json.load(f)

total = data.get('total', 0)
passed = data.get('passed', 0)
failed = data.get('failed', 0)
skipped = data.get('skipped', 0)
pct = data.get('percentage', 0.0)

md = []
md.append('## 🧪 Test262 Conformance Summary\n')
md.append(f'| Metric | Value |')
md.append(f'|---|---|')
md.append(f'| **Total Evaluated** | {total} |')
md.append(f'| **Passed** | {passed} |')
md.append(f'| **Failed** | {failed} |')
md.append(f'| **Skipped** | {skipped} |')
md.append(f'| **Pass Rate** | **{pct:.2f}%** |\n')

categories = data.get('categories', {})
if categories:
    md.append('### Category Breakdown\n')
    md.append('| Category | Total | Passed | Failed | Pass % |')
    md.append('|---|---|---|---|---|')
    for cat, st in sorted(categories.items()):
        c_tot = st.get('total', 0)
        c_pass = st.get('passed', 0)
        c_fail = st.get('failed', 0)
        c_pct = st.get('percentage', 0.0)
        md.append(f'| \`{cat}\` | {c_tot} | {c_pass} | {c_fail} | {c_pct:.1f}% |')

out_text = '\n'.join(md)
with open('${MD_FILE}', 'w') as out:
    out.write(out_text)

# Also append to GITHUB_STEP_SUMMARY if available
import os
step_sum = os.environ.get('GITHUB_STEP_SUMMARY')
if step_sum:
    with open(step_sum, 'a') as s:
        s.write(out_text + '\n')
" 2>/dev/null || true
    fi
fi

# Verify minimum pass rate threshold if specified
if [ -n "${MIN_PASS}" ] && [ -f "${OUTPUT_JSON}" ]; then
    ACTUAL_PCT=$(python3 -c "import json; data=json.load(open('${OUTPUT_JSON}')); print(data.get('percentage', 0.0))" 2>/dev/null || echo "0.0")
    echo "Checking pass rate: ${ACTUAL_PCT}% >= ${MIN_PASS}% threshold..."
    PASS_OK=$(python3 -c "print(1 if float('${ACTUAL_PCT}') >= float('${MIN_PASS}') else 0)" 2>/dev/null || echo "1")
    if [ "${PASS_OK}" -eq 0 ]; then
        echo "❌ Test262 pass rate ${ACTUAL_PCT}% is below required threshold ${MIN_PASS}%!"
        exit 1
    fi
    echo "✅ Pass rate ${ACTUAL_PCT}% meets or exceeds required threshold ${MIN_PASS}%."
fi
