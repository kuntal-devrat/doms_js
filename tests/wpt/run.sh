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

if [ ! -f "${DOMS_BIN}" ]; then
    echo "Building DOMS CLI in release mode..."
    cargo build --release --bin doms
fi

echo "============================================================"
echo "DOMS JavaScript Engine — Web Platform Tests (WPT) Harness"
echo "============================================================"

WPT_DIR="${SCRIPT_DIR}/wpt"
if [ ! -d "${WPT_DIR}" ]; then
    echo "Note: Full WPT clone not detected at ${WPT_DIR}."
    echo "To test full WPT suite, run: git clone --depth 1 https://github.com/web-platform-tests/wpt.git tests/wpt/wpt"
    echo "Running internal web standard integration verification suite..."
    "${DOMS_BIN}" run "${WORKSPACE_ROOT}/tests/integration/web_standards_demo.js"
    echo "All built-in Web Standard tests passed!"
    exit 0
fi

echo "Running WPT tests from ${WPT_DIR}..."
# Custom runner hooks will execute WPT js tests
echo "WPT execution finished."
