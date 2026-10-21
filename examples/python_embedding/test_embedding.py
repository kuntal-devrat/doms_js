#!/usr/bin/env python3
"""
DOMS JavaScript Engine — Comprehensive Python Embedding Integration Tests
100% Zero-mock testing against live `libdoms_ffi.so`.
"""

import unittest
import math
from doms import DomsEngine, DomsError


class TestDomsPythonEmbedding(unittest.TestCase):
    def setUp(self):
        self.js = DomsEngine()

    def tearDown(self):
        self.js.close()

    def test_version_and_lifecycle(self):
        self.assertGreaterEqual(self.js.version, 1)

    def test_context_manager(self):
        with DomsEngine() as engine:
            self.assertEqual(engine.eval("10 + 20"), 30)

    def test_primitive_numbers(self):
        self.assertEqual(self.js.eval("42"), 42)
        self.assertEqual(self.js.eval("-100"), -100)
        self.assertAlmostEqual(self.js.eval("3.14159"), 3.14159, places=5)
        self.assertEqual(self.js.eval("2 ** 10"), 1024)
        self.assertEqual(self.js.eval("Math.max(10, 20, 5)"), 20)
        self.assertEqual(self.js.eval("Math.sqrt(144)"), 12)

    def test_strings_and_unicode(self):
        self.assertEqual(self.js.eval("'hello world'"), "hello world")
        self.assertEqual(self.js.eval("'🚀 JavaScript in Python'"), "🚀 JavaScript in Python")
        self.assertEqual(self.js.eval("const a = 'foo'; const b = 'bar'; `${a}_${b}`"), "foo_bar")

    def test_booleans_and_null(self):
        self.assertIs(self.js.eval("true"), True)
        self.assertIs(self.js.eval("false"), False)
        self.assertIsNone(self.js.eval("null"))
        self.assertIsNone(self.js.eval("undefined"))

    def test_arrays(self):
        # Setting array from Python
        self.js.set_global("pyArray", [1, 2, 3, 4, 5])
        self.assertEqual(self.js.eval("pyArray.length"), 5)
        self.assertEqual(self.js.eval("pyArray.reduce((acc, x) => acc + x, 0)"), 15)
        self.assertEqual(self.js.eval("pyArray.map(x => x * 2).join('-')"), "2-4-6-8-10")

    def test_objects(self):
        self.js.set_global("user", {"name": "Alice", "age": 30, "admin": True})
        self.assertEqual(self.js.eval("user.name"), "Alice")
        self.assertEqual(self.js.eval("user.age"), 30)
        self.assertEqual(self.js.eval("user.admin"), True)

    def test_python_callbacks_in_js(self):
        calls = []

        def logger(msg):
            calls.append(msg)
            return len(msg)

        self.js.set_global_function("pyLog", logger)
        ret = self.js.eval("pyLog('system initialized')")
        self.assertEqual(ret, 18)
        self.assertEqual(calls, ["system initialized"])

        def multiply(x, y):
            return x * y

        self.js.set_global_function("pyMul", multiply)
        self.assertEqual(self.js.eval("pyMul(7, 8)"), 56)

    def test_call_js_function_from_python(self):
        self.js.eval("""
            function fibonacci(n) {
                if (n <= 1) return n;
                return fibonacci(n - 1) + fibonacci(n - 2);
            }
        """)
        self.assertEqual(self.js.call("fibonacci", 0), 0)
        self.assertEqual(self.js.call("fibonacci", 1), 1)
        self.assertEqual(self.js.call("fibonacci", 7), 13)
        self.assertEqual(self.js.call("fibonacci", 10), 55)

    def test_errors_and_exceptions(self):
        with self.assertRaises(DomsError):
            self.js.eval("this is invalid syntax !!!")

        with self.assertRaises(DomsError):
            self.js.eval("null.fictionalProperty")

        with self.assertRaises(DomsError):
            self.js.eval("throw new Error('custom JS error')")


if __name__ == "__main__":
    unittest.main()
