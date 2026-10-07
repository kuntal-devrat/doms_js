//! # DOMS WebAssembly Target (`doms_wasm`)
//!
//! Provides a sandboxed JavaScript runtime target for browser environments,
//! WebAssembly runtimes (Wasmtime, Wasmer), Cloudflare Workers, and Deno/Node.js.

#![allow(clippy::not_unsafe_ptr_arg_deref)]

use doms_vm::Vm;
use std::collections::HashMap;
use std::ffi::CString;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

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
        Self { vm: Vm::new() }
    }

    pub fn eval(&mut self, source: &str) -> Result<String, String> {
        let program = doms_parser::parse(source).map_err(|e| format!("Parse error: {}", e))?;
        let chunk = doms_compiler::compile(&program).map_err(|e| format!("Compile error: {}", e))?;
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

// ── C ABI / WebAssembly Exported Functions ──

#[unsafe(no_mangle)]
pub extern "C" fn doms_wasm_version() -> *const u8 {
    c"DOMS.js Wasm v1.0.0 (Phase 5)".as_ptr() as *const u8
}

#[unsafe(no_mangle)]
pub extern "C" fn doms_wasm_alloc(size: usize) -> *mut u8 {
    let mut vec = Vec::with_capacity(size);
    let ptr = vec.as_mut_ptr();
    std::mem::forget(vec);
    ptr
}

#[unsafe(no_mangle)]
pub extern "C" fn doms_wasm_free(ptr: *mut u8, size: usize) {
    if !ptr.is_null() && size > 0 {
        unsafe {
            let _ = Vec::from_raw_parts(ptr, 0, size);
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn doms_wasm_create_isolate() -> usize {
    let id = NEXT_ISOLATE_ID.fetch_add(1, Ordering::SeqCst);
    let mut lock = ISOLATES.lock().unwrap();
    let map = lock.get_or_insert_with(HashMap::new);
    map.insert(id, DomsWasmSandbox::new());
    id
}

#[unsafe(no_mangle)]
pub extern "C" fn doms_wasm_destroy_isolate(id: usize) {
    let mut lock = ISOLATES.lock().unwrap();
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
        return std::ptr::null();
    }
    let slice = unsafe { std::slice::from_raw_parts(source_ptr, source_len) };
    let source_str = match std::str::from_utf8(slice) {
        Ok(s) => s,
        Err(_) => return c"Error: Invalid UTF-8".as_ptr() as *const u8,
    };

    let mut lock = ISOLATES.lock().unwrap();
    let map = lock.get_or_insert_with(HashMap::new);
    let sandbox = map.entry(isolate_id).or_default();

    let output = match sandbox.eval_with_event_loop(source_str) {
        Ok(s) => s,
        Err(e) => format!("Error: {}", e),
    };

    let c_str = CString::new(output).unwrap_or_else(|_| CString::new("Error: CString conversion").unwrap());
    c_str.into_raw() as *const u8
}

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
        let res_str = unsafe { CStr::from_ptr(res_ptr as *const std::ffi::c_char) }.to_str().unwrap();
        assert_eq!(res_str, "42");

        doms_wasm_free_string(res_ptr as *mut _);
        doms_wasm_free(ptr, script.len());
    }
}
