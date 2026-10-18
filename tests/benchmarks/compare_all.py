#!/usr/bin/env python3
import subprocess
import time
import os
import sys

ENGINES = {
    "DOMS (JIT)": ["target/release/doms", "run"],
    "Node.js (V8)": ["node"],
    "Deno (V8)": ["target/bench_engines/deno/bin/deno", "run"],
    "QuickJS": ["target/bench_engines/quickjs/qjs"],
    "Boa (v0.22)": ["target/bench_engines/boa"],
}

BENCHMARKS = [
    ("Bitwise Ops", "tests/benchmarks/bitops-bitwise-and.js"),
    ("Control Flow Loop", "tests/benchmarks/controlflow-loop.js"),
    ("Function Call Overhead", "tests/benchmarks/jit-function-call.js"),
    ("Recursive Fibonacci", "tests/benchmarks/fibonacci.js"),
    ("Object & Inline Cache", "tests/benchmarks/object-property-access.js"),
    ("Array Operations", "tests/benchmarks/array-manipulation.js"),
    ("Primes Sieve", "tests/benchmarks/primes-sieve.js"),
]

RUNS = 5

def measure(cmd_prefix, script):
    cmd = cmd_prefix + [script]
    times = []
    for _ in range(RUNS):
        t0 = time.perf_counter()
        res = subprocess.run(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        t1 = time.perf_counter()
        if res.returncode != 0:
            return None, res.stderr.decode("utf-8", errors="ignore")
        times.append((t1 - t0) * 1000.0) # in ms
    # return median / trimmed mean
    times.sort()
    # middle 3 average
    avg = sum(times[1:4]) / 3.0 if len(times) >= 3 else sum(times) / len(times)
    return avg, None

def main():
    print(f"=== Comprehensive Engine Speed Benchmark (Averaged over {RUNS} runs) ===")
    results = {}
    for bench_name, script_path in BENCHMARKS:
        print(f"\nRunning {bench_name} ({script_path})...")
        results[bench_name] = {}
        for eng_name, cmd in ENGINES.items():
            avg_ms, err = measure(cmd, script_path)
            if avg_ms is not None:
                results[bench_name][eng_name] = round(avg_ms, 2)
                print(f"  {eng_name:18}: {avg_ms:7.2f} ms")
            else:
                results[bench_name][eng_name] = "ERR"
                print(f"  {eng_name:18}: ERROR ({err.strip()[:40]})")

    print("\n\n" + "=" * 80)
    print("SUMMARY RESULTS TABLE (ms, lower is better)")
    print("=" * 80)
    header = f"| {'Benchmark':<24} | " + " | ".join(f"{eng:<14}" for eng in ENGINES.keys()) + " |"
    sep = f"|{'-' * 26}|" + "|".join(f"{'-' * 16}" for _ in ENGINES) + "|"
    print(header)
    print(sep)
    for bench_name, _ in BENCHMARKS:
        row = f"| {bench_name:<24} | "
        for eng in ENGINES.keys():
            val = results[bench_name].get(eng, "N/A")
            if isinstance(val, (int, float)):
                row += f"{val:12.2f} ms | "
            else:
                row += f"{str(val):<14} | "
        print(row)

if __name__ == "__main__":
    main()
