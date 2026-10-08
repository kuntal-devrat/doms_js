# DOMS Web Platform Tests (WPT) Conformance Harness

This directory contains the runner and test harness for the [Web Platform Tests (WPT)](https://github.com/web-platform-tests/wpt) suite against DOMS.js web standard implementations.

## Relevant WPT Test Suites for DOMS

Because DOMS is an embeddable JavaScript engine and runtime, we focus on JavaScript-accessible Web Standards:

| Standard | WPT Directory | DOMS Component |
|:---|:---|:---|
| **Encoding API** | `encoding/` | `TextEncoder`, `TextDecoder` |
| **URL API** | `url/` | `URL`, `URLSearchParams` |
| **DOM Events** | `dom/events/` | `EventTarget`, `Event`, `CustomEvent` |
| **High Resolution Time** | `hr-time/` | `performance.now()`, `DOMHighResTimeStamp` |
| **Timers & Event Loop** | `html/webappapis/timers/` | `setTimeout`, `clearTimeout`, `setInterval` |
| **Console Standard** | `console/` | `console.log`, `info`, `warn`, `error` |
| **Web Messaging** | `webmessaging/` | `MessageChannel`, `MessagePort`, `postMessage` |

## Usage

### 1. Download WPT Subsets

To fetch only the runtime-relevant test suites (saving multi-gigabyte clones):

```bash
git clone --depth 1 --filter=blob:none --sparse https://github.com/web-platform-tests/wpt.git tests/wpt/wpt
cd tests/wpt/wpt
git sparse-checkout set encoding url dom/events hr-time console
```

### 2. Run WPT Tests

```bash
./tests/wpt/run.sh
```

Or run a specific suite:

```bash
./tests/wpt/run.sh --suite url
./tests/wpt/run.sh --suite encoding
```

### 3. Continuous Integration

WPT conformance runs in GitHub Actions CI under `.github/workflows/ci.yml`, storing results and tracking regressions in `tests/wpt/results/summary.json`.
