#!/usr/bin/env python3
"""
DOMS JavaScript Engine — Python Embedding Example
Demonstrates evaluating JS, registering Python callbacks, and calling JS functions.
"""

from doms import DomsEngine

def main():
    print("=== DOMS JavaScript Engine — Python Embedding Showcase ===\n")

    with DomsEngine() as js:
        print(f"Loaded DOMS Engine (ABI version: {js.version})")

        # 1. Simple expression evaluation
        res = js.eval("40 + 2")
        print(f"[1] Basic eval: 40 + 2 -> {res} (type: {type(res).__name__})")

        # 2. String manipulation and template literals
        greeting = js.eval("const name = 'Python'; `Hello from DOMS to ${name}!`")
        print(f"[2] String eval: -> '{greeting}'")

        # 3. Expose Python function to JavaScript
        def py_add(a, b):
            print(f"    [Inside Python py_add] received {a} + {b}")
            return a + b

        js.set_global_function("pythonAdd", py_add)
        calc = js.eval("pythonAdd(123, 456)")
        print(f"[3] JS invoked Python function pythonAdd(123, 456) -> {calc}")

        # 4. Set global data from Python
        js.set_global("dataset", [10, 20, 30, 40, 50])
        total = js.eval("dataset.reduce((a, b) => a + b, 0)")
        print(f"[4] Python injected array 'dataset', JS computed sum -> {total}")

        # 5. Define a function in JS and call it directly from Python
        js.eval("""
            function transform(items, multiplier) {
                return items.map(x => x * multiplier).join(', ');
            }
        """)
        formatted = js.call("transform", [1, 2, 3, 4], 10)
        print(f"[5] Python called JS transform([1, 2, 3, 4], 10) -> '{formatted}'")

    print("\n=== Python Embedding Showcase completed successfully! ===")

if __name__ == "__main__":
    main()
