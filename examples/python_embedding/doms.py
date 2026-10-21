"""
DOMS JavaScript Engine — Python Ctypes Wrapper
Zero-dependency, high-performance Python bindings for DOMS.
"""

import ctypes
import os
import sys
from typing import Any, Callable

# Find libdoms_ffi.so
_LIB_SEARCH_PATHS = [
    os.path.join(os.path.dirname(__file__), "..", "..", "target", "release", "libdoms_ffi.so"),
    os.path.join(os.path.dirname(__file__), "..", "..", "target", "debug", "libdoms_ffi.so"),
]

_LIB = None
for path in _LIB_SEARCH_PATHS:
    if os.path.exists(path):
        _LIB = ctypes.CDLL(os.path.abspath(path))
        break

if _LIB is None:
    raise RuntimeError(
        "Could not find libdoms_ffi.so. Run 'cargo build --release -p doms_ffi' first."
    )

# C Types
c_status = ctypes.c_int
c_engine_p = ctypes.c_void_p
c_isolate_p = ctypes.c_void_p
c_context_p = ctypes.c_void_p
c_value_p = ctypes.c_void_p
c_native_fn = ctypes.CFUNCTYPE(
    c_status,
    c_context_p,
    c_value_p,
    ctypes.c_size_t,
    ctypes.POINTER(c_value_p),
    ctypes.POINTER(c_value_p),
    ctypes.c_void_p,
)

# Function Signatures
_LIB.doms_version.restype = ctypes.c_uint32

_LIB.doms_engine_new.restype = c_engine_p
_LIB.doms_engine_destroy.argtypes = [c_engine_p]

_LIB.doms_isolate_new.argtypes = [c_engine_p]
_LIB.doms_isolate_new.restype = c_isolate_p
_LIB.doms_isolate_destroy.argtypes = [c_isolate_p]

_LIB.doms_context_new.argtypes = [c_isolate_p]
_LIB.doms_context_new.restype = c_context_p
_LIB.doms_context_destroy.argtypes = [c_context_p]

_LIB.doms_eval.argtypes = [
    c_context_p,
    ctypes.c_char_p,
    ctypes.c_size_t,
    ctypes.POINTER(c_value_p),
]
_LIB.doms_eval.restype = c_status

_LIB.doms_value_to_string.argtypes = [
    c_context_p,
    c_value_p,
    ctypes.POINTER(ctypes.c_char_p),
    ctypes.POINTER(ctypes.c_size_t),
]
_LIB.doms_value_to_string.restype = c_status

_LIB.doms_value_to_number.argtypes = [
    c_context_p,
    c_value_p,
    ctypes.POINTER(ctypes.c_double),
]
_LIB.doms_value_to_number.restype = c_status

_LIB.doms_value_to_boolean.argtypes = [
    c_context_p,
    c_value_p,
    ctypes.POINTER(ctypes.c_int),
]
_LIB.doms_value_to_boolean.restype = c_status


_LIB.doms_value_is_null.argtypes = [c_value_p]
_LIB.doms_value_is_null.restype = ctypes.c_int
_LIB.doms_value_is_undefined.argtypes = [c_value_p]
_LIB.doms_value_is_undefined.restype = ctypes.c_int
_LIB.doms_value_is_boolean.argtypes = [c_value_p]
_LIB.doms_value_is_boolean.restype = ctypes.c_int
_LIB.doms_value_is_number.argtypes = [c_value_p]
_LIB.doms_value_is_number.restype = ctypes.c_int
_LIB.doms_value_is_string.argtypes = [c_value_p]
_LIB.doms_value_is_string.restype = ctypes.c_int
_LIB.doms_value_is_object.argtypes = [c_value_p]
_LIB.doms_value_is_object.restype = ctypes.c_int
_LIB.doms_value_is_function.argtypes = [c_value_p]
_LIB.doms_value_is_function.restype = ctypes.c_int

_LIB.doms_value_new_undefined.restype = c_value_p
_LIB.doms_value_new_null.restype = c_value_p
_LIB.doms_value_new_boolean.argtypes = [ctypes.c_int]
_LIB.doms_value_new_boolean.restype = c_value_p
_LIB.doms_value_new_number.argtypes = [ctypes.c_double]
_LIB.doms_value_new_number.restype = c_value_p
_LIB.doms_value_new_string.argtypes = [
    c_context_p,
    ctypes.c_char_p,
    ctypes.c_size_t,
    ctypes.POINTER(c_value_p),
]
_LIB.doms_value_new_string.restype = c_status

_LIB.doms_value_unref.argtypes = [c_value_p]
_LIB.doms_free_string.argtypes = [c_context_p, ctypes.c_char_p]

_LIB.doms_get_global.argtypes = [c_context_p, ctypes.POINTER(c_value_p)]
_LIB.doms_get_global.restype = c_status
_LIB.doms_set_global.argtypes = [
    c_context_p,
    ctypes.c_char_p,
    ctypes.c_size_t,
    c_value_p,
]
_LIB.doms_set_global.restype = c_status

_LIB.doms_object_new.argtypes = [c_context_p, ctypes.POINTER(c_value_p)]
_LIB.doms_object_new.restype = c_status
_LIB.doms_object_get.argtypes = [
    c_context_p,
    c_value_p,
    ctypes.c_char_p,
    ctypes.c_size_t,
    ctypes.POINTER(c_value_p),
]
_LIB.doms_object_get.restype = c_status
_LIB.doms_object_set.argtypes = [
    c_context_p,
    c_value_p,
    ctypes.c_char_p,
    ctypes.c_size_t,
    c_value_p,
]
_LIB.doms_object_set.restype = c_status

_LIB.doms_array_new.argtypes = [c_context_p, ctypes.POINTER(c_value_p)]
_LIB.doms_array_new.restype = c_status
_LIB.doms_array_push.argtypes = [c_context_p, c_value_p, c_value_p]
_LIB.doms_array_push.restype = c_status
_LIB.doms_array_length.argtypes = [
    c_context_p,
    c_value_p,
    ctypes.POINTER(ctypes.c_size_t),
]
_LIB.doms_array_length.restype = c_status
_LIB.doms_array_get_at.argtypes = [
    c_context_p,
    c_value_p,
    ctypes.c_size_t,
    ctypes.POINTER(c_value_p),
]
_LIB.doms_array_get_at.restype = c_status

_LIB.doms_call.argtypes = [
    c_context_p,
    c_value_p,
    c_value_p,
    ctypes.c_size_t,
    ctypes.POINTER(c_value_p),
    ctypes.POINTER(c_value_p),
]
_LIB.doms_call.restype = c_status

_LIB.doms_set_global_function.argtypes = [
    c_context_p,
    ctypes.c_char_p,
    ctypes.c_size_t,
    c_native_fn,
    ctypes.c_void_p,
]
_LIB.doms_set_global_function.restype = c_status

_LIB.doms_get_error_message.argtypes = [c_context_p]
_LIB.doms_get_error_message.restype = ctypes.c_char_p


class DomsError(Exception):
    pass


class DomsEngine:
    """Idiomatic, zero-dependency Python interface to the DOMS JavaScript Engine."""

    def __init__(self):
        self._engine = _LIB.doms_engine_new()
        if not self._engine:
            raise DomsError("Failed to initialize DOMS engine")
        self._isolate = _LIB.doms_isolate_new(self._engine)
        if not self._isolate:
            _LIB.doms_engine_destroy(self._engine)
            raise DomsError("Failed to initialize DOMS isolate")
        self._ctx = _LIB.doms_context_new(self._isolate)
        if not self._ctx:
            _LIB.doms_isolate_destroy(self._isolate)
            _LIB.doms_engine_destroy(self._engine)
            raise DomsError("Failed to initialize DOMS context")
        # Keep references to native callbacks to prevent garbage collection
        self._callbacks = []

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        self.close()

    def close(self):
        if self._ctx:
            _LIB.doms_context_destroy(self._ctx)
            self._ctx = None
        if self._isolate:
            _LIB.doms_isolate_destroy(self._isolate)
            self._isolate = None
        if self._engine:
            _LIB.doms_engine_destroy(self._engine)
            self._engine = None

    def __del__(self):
        self.close()

    @property
    def version(self) -> int:
        return _LIB.doms_version()

    def _py_to_js(self, val: Any) -> c_value_p:
        if val is None:
            return _LIB.doms_value_new_null()
        elif isinstance(val, bool):
            return _LIB.doms_value_new_boolean(1 if val else 0)
        elif isinstance(val, (int, float)):
            return _LIB.doms_value_new_number(float(val))
        elif isinstance(val, str):
            b = val.encode("utf-8")
            res = c_value_p()
            _LIB.doms_value_new_string(self._ctx, b, len(b), ctypes.byref(res))
            return res
        elif isinstance(val, list):
            arr = c_value_p()
            _LIB.doms_array_new(self._ctx, ctypes.byref(arr))
            for item in val:
                item_v = self._py_to_js(item)
                _LIB.doms_array_push(self._ctx, arr, item_v)
                _LIB.doms_value_unref(item_v)
            return arr
        elif isinstance(val, dict):
            obj = c_value_p()
            _LIB.doms_object_new(self._ctx, ctypes.byref(obj))
            for k, v in val.items():
                kb = str(k).encode("utf-8")
                v_js = self._py_to_js(v)
                _LIB.doms_object_set(self._ctx, obj, kb, len(kb), v_js)
                _LIB.doms_value_unref(v_js)
            return obj
        else:
            return _LIB.doms_value_new_undefined()

    def _js_to_py(self, val_p: c_value_p) -> Any:
        if not val_p or _LIB.doms_value_is_null(val_p) or _LIB.doms_value_is_undefined(val_p):
            return None
        if _LIB.doms_value_is_boolean(val_p):
            b = ctypes.c_int()
            _LIB.doms_value_to_boolean(self._ctx, val_p, ctypes.byref(b))
            return bool(b.value != 0)

        if _LIB.doms_value_is_number(val_p):
            n = ctypes.c_double()
            _LIB.doms_value_to_number(self._ctx, val_p, ctypes.byref(n))
            return int(n.value) if n.value.is_integer() else n.value
        if _LIB.doms_value_is_string(val_p):
            s_ptr = ctypes.c_char_p()
            s_len = ctypes.c_size_t()
            _LIB.doms_value_to_string(self._ctx, val_p, ctypes.byref(s_ptr), ctypes.byref(s_len))
            res = s_ptr.value.decode("utf-8") if s_ptr.value else ""
            _LIB.doms_free_string(self._ctx, s_ptr)
            return res
        return str(val_p)

    def eval(self, script: str) -> Any:
        """Evaluate JavaScript code and return the native Python result."""
        b = script.encode("utf-8")
        result_p = c_value_p()
        status = _LIB.doms_eval(self._ctx, b, len(b), ctypes.byref(result_p))
        if status != 0:
            err = _LIB.doms_get_error_message(self._ctx)
            err_msg = err.decode("utf-8") if err else "Execution failed"
            raise DomsError(f"DOMS Evaluation Error: {err_msg}")
        try:
            return self._js_to_py(result_p)
        finally:
            if result_p:
                _LIB.doms_value_unref(result_p)

    def set_global(self, name: str, value: Any):
        """Set a global variable in the JavaScript isolate."""
        b_name = name.encode("utf-8")
        js_val = self._py_to_js(value)
        try:
            status = _LIB.doms_set_global(self._ctx, b_name, len(b_name), js_val)
            if status != 0:
                raise DomsError(f"Failed to set global '{name}'")
        finally:
            _LIB.doms_value_unref(js_val)

    def set_global_function(self, name: str, fn: Callable):
        """Expose a Python function to JavaScript."""
        def callback(ctx, this_val, argc, argv_ptr, result_ptr, userdata):
            try:
                args = []
                for i in range(argc):
                    args.append(self._js_to_py(argv_ptr[i]))
                py_res = fn(*args)
                result_ptr[0] = self._py_to_js(py_res)
                return 0
            except Exception as e:
                result_ptr[0] = _LIB.doms_value_new_undefined()
                return 2

        cb_obj = c_native_fn(callback)
        self._callbacks.append(cb_obj)  # Retain reference

        b_name = name.encode("utf-8")
        status = _LIB.doms_set_global_function(self._ctx, b_name, len(b_name), cb_obj, None)
        if status != 0:
            raise DomsError(f"Failed to register global function '{name}'")

    def call(self, fn_name: str, *args) -> Any:
        """Call a JavaScript global function from Python."""
        global_obj = c_value_p()
        _LIB.doms_get_global(self._ctx, ctypes.byref(global_obj))
        fn_val = c_value_p()
        b_name = fn_name.encode("utf-8")
        _LIB.doms_object_get(self._ctx, global_obj, b_name, len(b_name), ctypes.byref(fn_val))

        if not fn_val or not _LIB.doms_value_is_function(fn_val):
            if fn_val:
                _LIB.doms_value_unref(fn_val)
            _LIB.doms_value_unref(global_obj)
            raise DomsError(f"'{fn_name}' is not a callable JavaScript function")

        argv = (c_value_p * len(args))()
        for i, arg in enumerate(args):
            argv[i] = self._py_to_js(arg)

        result_p = c_value_p()
        try:
            status = _LIB.doms_call(self._ctx, fn_val, global_obj, len(args), argv, ctypes.byref(result_p))
            if status != 0:
                err = _LIB.doms_get_error_message(self._ctx)
                err_msg = err.decode("utf-8") if err else "Call failed"
                raise DomsError(f"Function call failed: {err_msg}")
            return self._js_to_py(result_p)
        finally:
            for v in argv:
                _LIB.doms_value_unref(v)
            if result_p:
                _LIB.doms_value_unref(result_p)
            _LIB.doms_value_unref(fn_val)
            _LIB.doms_value_unref(global_obj)
