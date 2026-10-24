//! # DOMS WebAssembly Target (`doms_wasm`)
//!
//! Provides a sandboxed JavaScript runtime target for browser environments,
//! WebAssembly runtimes (Wasmtime, Wasmer), Cloudflare Workers, and Deno/Node.js.

#![allow(clippy::not_unsafe_ptr_arg_deref)]

pub mod simd;

use doms_vm::Vm;
use simd::WasmVectorKernel;
use std::collections::HashMap;
use std::ffi::CString;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT_ISOLATE_ID: AtomicUsize = AtomicUsize::new(1);
static ISOLATES: Mutex<Option<HashMap<usize, DomsWasmSandbox>>> = Mutex::new(None);

/// High-level safe Rust sandbox for WebAssembly hosts.
pub struct DomsWasmSandbox {
    vm: Vm,
}

unsafe impl Send for DomsWasmSandbox {}
unsafe impl Sync for DomsWasmSandbox {}

impl DomsWasmSandbox {
    pub fn new() -> Self {
        let mut vm = Vm::new();
        simd::register_wasm_simd(&mut vm);
        Self { vm }
    }

    pub fn eval(&mut self, source: &str) -> Result<String, String> {
        let program = doms_parser::parse(source).map_err(|e| format!("Parse error: {}", e))?;
        let chunk =
            doms_compiler::compile(&program).map_err(|e| format!("Compile error: {}", e))?;
        let val = self.vm.run(&chunk).map_err(|e| format!("Runtime error: {}", e))?;
        Ok(self.vm.to_string_value(val))
    }

    pub fn eval_with_event_loop(&mut self, source: &str) -> Result<String, String> {
        let res = self.eval(source)?;
        let _ = self.vm.run_event_loop();
        Ok(res)
    }

    pub fn get_global(&self, name: &str) -> Option<String> {
        self.vm.globals.get(name).map(|v| self.vm.to_string_value(*v))
    }
}

impl Default for DomsWasmSandbox {
    fn default() -> Self {
        Self::new()
    }
}

// C ABI and WebAssembly exported functions

/// Allocate a C string for an FFI result.
///
/// Every string returned by the eval entry points is allocated this way, so
/// callers may uniformly release results with [`doms_wasm_free_string`].
fn result_cstring(s: impl Into<Vec<u8>>) -> *const u8 {
    CString::new(s).unwrap_or_else(|_| CString::new("Error: interior NUL").unwrap()).into_raw()
        as *const u8
}

/// Lock the isolate table, recovering from poisoning after a panic.
fn lock_isolates() -> std::sync::MutexGuard<'static, Option<HashMap<usize, DomsWasmSandbox>>> {
    match ISOLATES.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn doms_wasm_version() -> *const u8 {
    concat!("DOMS.js Wasm v", env!("CARGO_PKG_VERSION"), "\0").as_ptr()
}

#[unsafe(no_mangle)]
pub extern "C" fn doms_wasm_alloc(size: usize) -> *mut u8 {
    if size == 0 {
        return std::ptr::null_mut();
    }
    let mut vec = Vec::<u8>::new();
    if vec.try_reserve_exact(size).is_err() {
        return std::ptr::null_mut();
    }
    vec.resize(size, 0);
    let ptr = vec.as_mut_ptr();
    std::mem::forget(vec);
    ptr
}

#[unsafe(no_mangle)]
pub extern "C" fn doms_wasm_free(ptr: *mut u8, size: usize) {
    if !ptr.is_null() && size > 0 {
        unsafe {
            let _ = Vec::from_raw_parts(ptr, size, size);
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn doms_wasm_create_isolate() -> usize {
    let id = NEXT_ISOLATE_ID.fetch_add(1, Ordering::SeqCst);
    let mut lock = lock_isolates();
    let map = lock.get_or_insert_with(HashMap::new);
    map.insert(id, DomsWasmSandbox::new());
    id
}

#[unsafe(no_mangle)]
pub extern "C" fn doms_wasm_destroy_isolate(id: usize) {
    let mut lock = lock_isolates();
    if let Some(map) = lock.as_mut() {
        map.remove(&id);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn doms_wasm_isolate_eval(
    isolate_id: usize,
    source_ptr: *const u8,
    source_len: usize,
) -> *const u8 {
    if source_ptr.is_null() {
        return result_cstring("Error: null source pointer");
    }
    let slice = unsafe { std::slice::from_raw_parts(source_ptr, source_len) };
    let source_str = match std::str::from_utf8(slice) {
        Ok(s) => s,
        // Return a heap string just like every other result so that callers can
        // uniformly release it with `doms_wasm_free_string`.
        Err(_) => return result_cstring("Error: Invalid UTF-8"),
    };

    let mut lock = lock_isolates();
    let map = lock.get_or_insert_with(HashMap::new);
    let sandbox = map.entry(isolate_id).or_default();

    // A panic must never unwind across the C ABI boundary: that is undefined
    // behaviour. Convert it into a regular error string instead.
    let output = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        sandbox.eval_with_event_loop(source_str)
    })) {
        Ok(Ok(s)) => s,
        Ok(Err(e)) => format!("Error: {}", e),
        Err(_) => "Error: panic in evaluation".to_string(),
    };

    result_cstring(output)
}

/// Free a string previously returned by the eval entry points.
///
/// # Safety
/// `ptr` must be a pointer returned by [`doms_wasm_isolate_eval`] /
/// [`doms_wasm_eval`] (or another `result_cstring` allocation) that has not
/// already been freed. Static strings such as [`doms_wasm_version`] must not
/// be passed here.
#[unsafe(no_mangle)]
pub extern "C" fn doms_wasm_free_string(ptr: *mut std::ffi::c_char) {
    if !ptr.is_null() {
        unsafe {
            let _ = CString::from_raw(ptr);
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn doms_wasm_eval(source_ptr: *const u8, source_len: usize) -> *const u8 {
    // Default global isolate ID = 0
    doms_wasm_isolate_eval(0, source_ptr, source_len)
}

#[unsafe(no_mangle)]
pub extern "C" fn doms_wasm_simd_compile(
    bytecode_ptr: *const u8,
    bytecode_len: usize,
) -> *mut WasmVectorKernel {
    if bytecode_ptr.is_null() || bytecode_len == 0 {
        return std::ptr::null_mut();
    }
    let slice = unsafe { std::slice::from_raw_parts(bytecode_ptr, bytecode_len) };
    let engine = simd::WasmSimdEngine::new();
    match engine.compile_bytecode(slice) {
        Ok(kernel) => Box::into_raw(Box::new(kernel)),
        Err(_) => std::ptr::null_mut(),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn doms_wasm_simd_execute(
    kernel_ptr: *mut WasmVectorKernel,
    mem_ptr: *mut u8,
    mem_len: usize,
) -> i32 {
    if kernel_ptr.is_null() || mem_ptr.is_null() {
        return -1;
    }
    let kernel = unsafe { &*kernel_ptr };
    let slice = unsafe { std::slice::from_raw_parts_mut(mem_ptr, mem_len) };
    kernel.execute(slice)
}

#[unsafe(no_mangle)]
pub extern "C" fn doms_wasm_simd_free_kernel(kernel_ptr: *mut WasmVectorKernel) {
    if !kernel_ptr.is_null() {
        unsafe {
            let _ = Box::from_raw(kernel_ptr);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CStr;

    #[test]
    fn test_wasm_eval_basic() {
        let mut sandbox = DomsWasmSandbox::new();
        let res = sandbox.eval("10 + 20 * 3").expect("eval arithmetic");
        assert_eq!(res, "70");
    }

    #[test]
    fn test_wasm_web_standards_in_sandbox() {
        let mut sandbox = DomsWasmSandbox::new();
        let script = r#"
            var url = new URL("https://domsjs.dev/wasm");
            var enc = new TextEncoder();
            var dec = new TextDecoder();
            var bytes = enc.encode("DOMS_WASM");
            var decoded = dec.decode(bytes);
            var result = url.hostname + ":" + decoded;
            result;
        "#;
        let res = sandbox.eval_with_event_loop(script).expect("eval web standards");
        assert_eq!(res, "domsjs.dev:DOMS_WASM");
    }

    #[test]
    fn test_wasm_multiple_isolates() {
        let id1 = doms_wasm_create_isolate();
        let id2 = doms_wasm_create_isolate();

        let s1 = "var x = 100; x;";
        let s2 = "var x = 200; x;";

        let p1 = doms_wasm_isolate_eval(id1, s1.as_ptr(), s1.len());
        let p2 = doms_wasm_isolate_eval(id2, s2.as_ptr(), s2.len());

        let res1 = unsafe { CStr::from_ptr(p1 as *const std::ffi::c_char) }.to_str().unwrap();
        let res2 = unsafe { CStr::from_ptr(p2 as *const std::ffi::c_char) }.to_str().unwrap();

        assert_eq!(res1, "100");
        assert_eq!(res2, "200");

        doms_wasm_free_string(p1 as *mut _);
        doms_wasm_free_string(p2 as *mut _);

        doms_wasm_destroy_isolate(id1);
        doms_wasm_destroy_isolate(id2);
    }

    #[test]
    fn test_wasm_c_ffi_alloc_and_eval() {
        let script = "var answer = 42; answer;";
        let ptr = doms_wasm_alloc(script.len());
        unsafe {
            std::ptr::copy_nonoverlapping(script.as_ptr(), ptr, script.len());
        }

        let res_ptr = doms_wasm_eval(ptr, script.len());
        let res_str =
            unsafe { CStr::from_ptr(res_ptr as *const std::ffi::c_char) }.to_str().unwrap();
        assert_eq!(res_str, "42");

        doms_wasm_free_string(res_ptr as *mut _);
        doms_wasm_free(ptr, script.len());
    }

    #[test]
    fn test_wasm_invalid_utf8_result_is_freeable() {
        // The error result must be heap-allocated like every other result so
        // that `doms_wasm_free_string` can safely release it.
        let bad = [0xFFu8, 0xFE];
        let res_ptr = doms_wasm_eval(bad.as_ptr(), bad.len());
        assert!(!res_ptr.is_null());
        let msg = unsafe { CStr::from_ptr(res_ptr as *const std::ffi::c_char) }.to_str().unwrap();
        assert!(msg.starts_with("Error:"), "unexpected error message: {}", msg);
        doms_wasm_free_string(res_ptr as *mut _);
    }

    #[test]
    fn test_wasm_null_source_pointer_is_handled() {
        let res_ptr = doms_wasm_eval(std::ptr::null(), 0);
        assert!(!res_ptr.is_null());
        let msg = unsafe { CStr::from_ptr(res_ptr as *const std::ffi::c_char) }.to_str().unwrap();
        assert!(msg.starts_with("Error:"), "unexpected error message: {}", msg);
        doms_wasm_free_string(res_ptr as *mut _);
    }

    #[test]
    fn test_wasm_alloc_zero_size_returns_null() {
        assert!(doms_wasm_alloc(0).is_null());
    }

    #[test]
    fn test_wasm_simd_compile_and_execute_v128_const_and_store() {
        // v128.const (16 bytes: 1..16) then v128.store offset=0
        let mut bytecode = vec![0xFD, 0x0C];
        for b in 1..=16u8 {
            bytecode.push(b);
        }
        // v128.store: 0xFD, 0x0B, align=0, offset=0
        bytecode.extend_from_slice(&[0xFD, 0x0B, 0x00, 0x00]);

        let engine = simd::WasmSimdEngine::new();
        let kernel = engine.compile_bytecode(&bytecode).expect("compile Wasm SIMD bytecode");

        let mut mem = vec![0u8; 32];
        let status = kernel.execute(&mut mem);
        assert_eq!(status, 0);

        for (i, &byte) in mem.iter().enumerate().take(16) {
            assert_eq!(byte, (i + 1) as u8);
        }
        assert_eq!(mem[16], 0);
    }

    #[test]
    fn test_wasm_simd_ml_vector_dot_product_and_math() {
        let mut sandbox = DomsWasmSandbox::new();
        let script = r#"
            var a = new Float32Array([1.0, 2.0, 3.0, 4.0]);
            var b = new Float32Array([0.5, 0.5, 0.5, 0.5]);
            var dot = WebAssembly.simd.f32x4Dot(a, b);

            var outAdd = new Float32Array(4);
            WebAssembly.simd.f32x4Add(outAdd, a, b);

            var outMul = new Float32Array(4);
            WebAssembly.simd.f32x4Mul(outMul, a, b);

            dot + ":" + outAdd[0] + ":" + outMul[3];
        "#;
        let res = sandbox.eval(script).expect("eval SIMD ML script");
        // dot = 0.5 + 1.0 + 1.5 + 2.0 = 5.0
        // outAdd[0] = 1.0 + 0.5 = 1.5
        // outMul[3] = 4.0 * 0.5 = 2.0
        assert_eq!(res, "5:1.5:2");
    }

    #[test]
    fn test_wasm_simd_graphics_rgba_brightness() {
        let mut sandbox = DomsWasmSandbox::new();
        let script = r#"
            var pixels = new Uint8Array([10, 20, 30, 255, 40, 50, 60, 255, 70, 80, 90, 255, 100, 110, 120, 255]);
            WebAssembly.simd.i8x16AdjustBrightness(pixels, 15);
            pixels[0] + ":" + pixels[1] + ":" + pixels[3];
        "#;
        let res = sandbox.eval(script).expect("eval SIMD graphics script");
        // pixels[0] = 10 + 15 = 25
        // pixels[1] = 20 + 15 = 35
        // pixels[3] = 255 clamped = 255
        assert_eq!(res, "25:35:255");
    }

    #[test]
    fn test_wasm_c_ffi_simd_compile_and_execute() {
        let mut bytecode = vec![0xFD, 0x0C];
        bytecode.extend(std::iter::repeat_n(42, 16));
        bytecode.extend_from_slice(&[0xFD, 0x0B, 0x00, 0x00]);

        let kernel_ptr = doms_wasm_simd_compile(bytecode.as_ptr(), bytecode.len());
        assert!(!kernel_ptr.is_null());

        let mut mem = vec![0u8; 32];
        let status = doms_wasm_simd_execute(kernel_ptr, mem.as_mut_ptr(), mem.len());
        assert_eq!(status, 0);
        for &byte in mem.iter().take(16) {
            assert_eq!(byte, 42);
        }

        doms_wasm_simd_free_kernel(kernel_ptr);
    }
}
