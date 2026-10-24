//! High-level WebAssembly SIMD execution engine and ML/Graphics vector kernels.

use super::compiler::{WasmSimdCompiler, WasmVectorKernel};
use super::opcode::decode_simd_bytecode;
use doms_vm::{JsValue, ObjectKind, Vm, VmError};

/// Safe, high-performance Wasm SIMD execution engine.
#[derive(Default)]
pub struct WasmSimdEngine;

impl WasmSimdEngine {
    pub fn new() -> Self {
        Self
    }

    /// Compile raw Wasm SIMD bytecodes into a native Cranelift machine kernel.
    pub fn compile_bytecode(&self, bytecode: &[u8]) -> Result<WasmVectorKernel, String> {
        let opcodes = decode_simd_bytecode(bytecode)?;
        WasmSimdCompiler::compile(&opcodes)
    }

    /// Vectorized single-precision float dot product (accelerated with f32x4).
    pub fn f32x4_dot_product(&self, a: &[f32], b: &[f32]) -> f32 {
        let len = a.len().min(b.len());
        let chunks = len / 4;
        let remainder = len % 4;

        let mut sum0 = 0.0f32;
        let mut sum1 = 0.0f32;
        let mut sum2 = 0.0f32;
        let mut sum3 = 0.0f32;

        for c in 0..chunks {
            let offset = c * 4;
            sum0 += a[offset] * b[offset];
            sum1 += a[offset + 1] * b[offset + 1];
            sum2 += a[offset + 2] * b[offset + 2];
            sum3 += a[offset + 3] * b[offset + 3];
        }

        let mut total = sum0 + sum1 + sum2 + sum3;
        let rem_start = chunks * 4;
        for i in 0..remainder {
            total += a[rem_start + i] * b[rem_start + i];
        }

        total
    }

    /// Vectorized elementwise float addition (out = a + b).
    pub fn f32x4_vector_add(&self, out: &mut [f32], a: &[f32], b: &[f32]) {
        let len = out.len().min(a.len()).min(b.len());
        let chunks = len / 4;
        let remainder = len % 4;

        for c in 0..chunks {
            let offset = c * 4;
            out[offset] = a[offset] + b[offset];
            out[offset + 1] = a[offset + 1] + b[offset + 1];
            out[offset + 2] = a[offset + 2] + b[offset + 2];
            out[offset + 3] = a[offset + 3] + b[offset + 3];
        }

        let rem_start = chunks * 4;
        for i in 0..remainder {
            out[rem_start + i] = a[rem_start + i] + b[rem_start + i];
        }
    }

    /// Vectorized elementwise float multiplication (out = a * b).
    pub fn f32x4_vector_mul(&self, out: &mut [f32], a: &[f32], b: &[f32]) {
        let len = out.len().min(a.len()).min(b.len());
        let chunks = len / 4;
        let remainder = len % 4;

        for c in 0..chunks {
            let offset = c * 4;
            out[offset] = a[offset] * b[offset];
            out[offset + 1] = a[offset + 1] * b[offset + 1];
            out[offset + 2] = a[offset + 2] * b[offset + 2];
            out[offset + 3] = a[offset + 3] * b[offset + 3];
        }

        let rem_start = chunks * 4;
        for i in 0..remainder {
            out[rem_start + i] = a[rem_start + i] * b[rem_start + i];
        }
    }

    /// Vectorized 16-way image brightness adjust (RGBA pixel channels).
    pub fn i8x16_adjust_brightness(&self, pixels: &mut [u8], delta: i16) {
        let chunks = pixels.len() / 16;
        let remainder = pixels.len() % 16;

        for c in 0..chunks {
            let offset = c * 16;
            for k in 0..16 {
                let v = pixels[offset + k] as i16 + delta;
                pixels[offset + k] = v.clamp(0, 255) as u8;
            }
        }

        let rem_start = chunks * 16;
        for i in 0..remainder {
            let v = pixels[rem_start + i] as i16 + delta;
            pixels[rem_start + i] = v.clamp(0, 255) as u8;
        }
    }
}

/// Register `WebAssembly.simd` host builtins into the DOMS VM.
pub fn register_wasm_simd(vm: &mut Vm) {
    let mut wasm_obj = vm
        .globals
        .get("WebAssembly")
        .copied()
        .unwrap_or(JsValue::undefined());

    if !wasm_obj.is_js_object() {
        wasm_obj = vm.alloc_object();
        vm.globals.insert("WebAssembly".to_string(), wasm_obj);
    }

    let simd_obj = vm.alloc_object();

    // 1. WebAssembly.simd.f32x4Dot(a: Float32Array, b: Float32Array): number
    let dot_fn = vm.alloc_native_function("f32x4Dot", 2, |_vm, _this, args| {
        let a_val = args.first().copied().unwrap_or(JsValue::undefined());
        let b_val = args.get(1).copied().unwrap_or(JsValue::undefined());

        let a_ptr = a_val.as_object_ptr().ok_or_else(|| {
            VmError::TypeError("f32x4Dot: first argument must be a Float32Array".into())
        })?;
        let b_ptr = b_val.as_object_ptr().ok_or_else(|| {
            VmError::TypeError("f32x4Dot: second argument must be a Float32Array".into())
        })?;

        let (a_slice, b_slice) = unsafe {
            let obj_a = &*(a_ptr as *mut doms_vm::JsObject);
            let obj_b = &*(b_ptr as *mut doms_vm::JsObject);
            match (&obj_a.kind, &obj_b.kind) {
                (ObjectKind::TypedArray(ta_a), ObjectKind::TypedArray(ta_b)) => {
                    let a_bytes = ta_a.buffer.as_atomic_slice();
                    let b_bytes = ta_b.buffer.as_atomic_slice();
                    let a_f32 = std::slice::from_raw_parts(
                        a_bytes.as_ptr().cast::<f32>(),
                        ta_a.length,
                    );
                    let b_f32 = std::slice::from_raw_parts(
                        b_bytes.as_ptr().cast::<f32>(),
                        ta_b.length,
                    );
                    (a_f32, b_f32)
                }
                _ => {
                    return Err(VmError::TypeError("f32x4Dot arguments must be Float32Array".into()));
                }
            }
        };

        let engine = WasmSimdEngine::new();
        let result = engine.f32x4_dot_product(a_slice, b_slice);
        Ok(JsValue::from_double(result as f64))
    });
    vm.set_property(simd_obj, "f32x4Dot", dot_fn);

    // 2. WebAssembly.simd.f32x4Add(out: Float32Array, a: Float32Array, b: Float32Array)
    let add_fn = vm.alloc_native_function("f32x4Add", 3, |_vm, _this, args| {
        let out_val = args.first().copied().unwrap_or(JsValue::undefined());
        let a_val = args.get(1).copied().unwrap_or(JsValue::undefined());
        let b_val = args.get(2).copied().unwrap_or(JsValue::undefined());

        let out_ptr = out_val.as_object_ptr().ok_or_else(|| {
            VmError::TypeError("f32x4Add: out argument must be a Float32Array".into())
        })?;
        let a_ptr = a_val.as_object_ptr().ok_or_else(|| {
            VmError::TypeError("f32x4Add: a argument must be a Float32Array".into())
        })?;
        let b_ptr = b_val.as_object_ptr().ok_or_else(|| {
            VmError::TypeError("f32x4Add: b argument must be a Float32Array".into())
        })?;

        unsafe {
            let obj_out = &mut *(out_ptr as *mut doms_vm::JsObject);
            let obj_a = &*(a_ptr as *mut doms_vm::JsObject);
            let obj_b = &*(b_ptr as *mut doms_vm::JsObject);
            if let (
                ObjectKind::TypedArray(ta_out),
                ObjectKind::TypedArray(ta_a),
                ObjectKind::TypedArray(ta_b),
            ) = (&mut obj_out.kind, &obj_a.kind, &obj_b.kind)
            {
                let out_slice = std::slice::from_raw_parts_mut(
                    ta_out.buffer.as_atomic_slice().as_ptr() as *mut f32,
                    ta_out.length,
                );
                let a_slice = std::slice::from_raw_parts(
                    ta_a.buffer.as_atomic_slice().as_ptr() as *const f32,
                    ta_a.length,
                );
                let b_slice = std::slice::from_raw_parts(
                    ta_b.buffer.as_atomic_slice().as_ptr() as *const f32,
                    ta_b.length,
                );
                let engine = WasmSimdEngine::new();
                engine.f32x4_vector_add(out_slice, a_slice, b_slice);
            } else {
                return Err(VmError::TypeError("Arguments must be Float32Array".into()));
            }
        }
        Ok(JsValue::undefined())
    });
    vm.set_property(simd_obj, "f32x4Add", add_fn);

    // 3. WebAssembly.simd.f32x4Mul(out: Float32Array, a: Float32Array, b: Float32Array)
    let mul_fn = vm.alloc_native_function("f32x4Mul", 3, |_vm, _this, args| {
        let out_val = args.first().copied().unwrap_or(JsValue::undefined());
        let a_val = args.get(1).copied().unwrap_or(JsValue::undefined());
        let b_val = args.get(2).copied().unwrap_or(JsValue::undefined());

        let out_ptr = out_val.as_object_ptr().ok_or_else(|| {
            VmError::TypeError("f32x4Mul: out argument must be a Float32Array".into())
        })?;
        let a_ptr = a_val.as_object_ptr().ok_or_else(|| {
            VmError::TypeError("f32x4Mul: a argument must be a Float32Array".into())
        })?;
        let b_ptr = b_val.as_object_ptr().ok_or_else(|| {
            VmError::TypeError("f32x4Mul: b argument must be a Float32Array".into())
        })?;

        unsafe {
            let obj_out = &mut *(out_ptr as *mut doms_vm::JsObject);
            let obj_a = &*(a_ptr as *mut doms_vm::JsObject);
            let obj_b = &*(b_ptr as *mut doms_vm::JsObject);
            if let (
                ObjectKind::TypedArray(ta_out),
                ObjectKind::TypedArray(ta_a),
                ObjectKind::TypedArray(ta_b),
            ) = (&mut obj_out.kind, &obj_a.kind, &obj_b.kind)
            {
                let out_slice = std::slice::from_raw_parts_mut(
                    ta_out.buffer.as_atomic_slice().as_ptr() as *mut f32,
                    ta_out.length,
                );
                let a_slice = std::slice::from_raw_parts(
                    ta_a.buffer.as_atomic_slice().as_ptr() as *const f32,
                    ta_a.length,
                );
                let b_slice = std::slice::from_raw_parts(
                    ta_b.buffer.as_atomic_slice().as_ptr() as *const f32,
                    ta_b.length,
                );
                let engine = WasmSimdEngine::new();
                engine.f32x4_vector_mul(out_slice, a_slice, b_slice);
            } else {
                return Err(VmError::TypeError("Arguments must be Float32Array".into()));
            }
        }
        Ok(JsValue::undefined())
    });
    vm.set_property(simd_obj, "f32x4Mul", mul_fn);

    // 4. WebAssembly.simd.i8x16AdjustBrightness(pixels: Uint8Array, delta: number)
    let bright_fn = vm.alloc_native_function("i8x16AdjustBrightness", 2, |_vm, _this, args| {
        let pix_val = args.first().copied().unwrap_or(JsValue::undefined());
        let delta_val = args.get(1).copied().unwrap_or(JsValue::undefined());
        let delta = delta_val.to_int32().unwrap_or(0) as i16;

        let pix_ptr = pix_val.as_object_ptr().ok_or_else(|| {
            VmError::TypeError("i8x16AdjustBrightness requires a Uint8Array".into())
        })?;

        unsafe {
            let obj = &mut *(pix_ptr as *mut doms_vm::JsObject);
            if let ObjectKind::TypedArray(ta) = &mut obj.kind {
                let pix_slice = std::slice::from_raw_parts_mut(
                    ta.buffer.as_atomic_slice().as_ptr() as *mut u8,
                    ta.length,
                );
                let engine = WasmSimdEngine::new();
                engine.i8x16_adjust_brightness(pix_slice, delta);
            } else {
                return Err(VmError::TypeError("First argument must be a Uint8Array".into()));
            }
        }
        Ok(JsValue::undefined())
    });
    vm.set_property(simd_obj, "i8x16AdjustBrightness", bright_fn);

    // 5. WebAssembly.simd.execute(bytecode: Uint8Array, memory: Uint8Array): number
    let exec_fn = vm.alloc_native_function("execute", 2, |_vm, _this, args| {
        let code_val = args.first().copied().unwrap_or(JsValue::undefined());
        let mem_val = args.get(1).copied().unwrap_or(JsValue::undefined());

        let code_ptr = code_val.as_object_ptr().ok_or_else(|| {
            VmError::TypeError("execute: bytecode must be a Uint8Array".into())
        })?;
        let mem_ptr = mem_val.as_object_ptr().ok_or_else(|| {
            VmError::TypeError("execute: memory must be a Uint8Array".into())
        })?;

        let code_slice = unsafe {
            let obj = &*(code_ptr as *mut doms_vm::JsObject);
            if let ObjectKind::TypedArray(ta) = &obj.kind {
                std::slice::from_raw_parts(
                    ta.buffer.as_atomic_slice().as_ptr() as *const u8,
                    ta.length,
                )
            } else {
                return Err(VmError::TypeError("bytecode must be a Uint8Array".into()));
            }
        };

        let engine = WasmSimdEngine::new();
        let kernel = engine
            .compile_bytecode(code_slice)
            .map_err(|e| VmError::TypeError(format!("Wasm SIMD compilation failed: {}", e)))?;

        let status = unsafe {
            let obj = &mut *(mem_ptr as *mut doms_vm::JsObject);
            if let ObjectKind::TypedArray(ta) = &mut obj.kind {
                let mem_slice = std::slice::from_raw_parts_mut(
                    ta.buffer.as_atomic_slice().as_ptr() as *mut u8,
                    ta.length,
                );
                kernel.execute(mem_slice)
            } else {
                return Err(VmError::TypeError("memory must be a Uint8Array".into()));
            }
        };

        Ok(JsValue::from_int32(status))
    });
    vm.set_property(simd_obj, "execute", exec_fn);

    vm.set_property(wasm_obj, "simd", simd_obj);
}
