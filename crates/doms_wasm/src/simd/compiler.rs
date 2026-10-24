//! Cranelift native JIT compiler for WebAssembly SIMD (v128) bytecodes.

use super::opcode::WasmSimdOpcode;
use cranelift_codegen::ir::{
    AbiParam, InstBuilder, MemFlagsData, StackSlotData, StackSlotKind, types,
};
use cranelift_codegen::settings::Configurable;
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{Linkage, Module};
use std::sync::Arc;

pub type NativeSimdFn = unsafe extern "C" fn(*mut u8, usize) -> i32;

/// Compiled native Wasm SIMD kernel.
pub struct WasmVectorKernel {
    pub func_ptr: NativeSimdFn,
    pub _module: Arc<std::sync::Mutex<JITModule>>,
}

unsafe impl Send for WasmVectorKernel {}
unsafe impl Sync for WasmVectorKernel {}

impl WasmVectorKernel {
    #[inline]
    pub fn execute(&self, memory: &mut [u8]) -> i32 {
        unsafe { (self.func_ptr)(memory.as_mut_ptr(), memory.len()) }
    }
}

/// Cranelift-backed compiler for Wasm vector (v128) bytecodes.
pub struct WasmSimdCompiler;

impl WasmSimdCompiler {
    pub fn compile(opcodes: &[WasmSimdOpcode]) -> Result<WasmVectorKernel, String> {
        let mut flag_builder = cranelift_codegen::settings::builder();
        flag_builder.set("opt_level", "speed").map_err(|e| e.to_string())?;

        let isa_builder =
            cranelift_native::builder().map_err(|e| format!("Host ISA error: {}", e))?;
        let isa = isa_builder
            .finish(cranelift_codegen::settings::Flags::new(flag_builder))
            .map_err(|e| format!("Target ISA build error: {}", e))?;

        let builder_obj = JITBuilder::with_isa(isa, cranelift_module::default_libcall_names());
        let mut module = JITModule::new(builder_obj);

        let mut sig = module.make_signature();
        sig.params.push(AbiParam::new(types::I64)); // mem_ptr
        sig.params.push(AbiParam::new(types::I64)); // mem_len
        sig.returns.push(AbiParam::new(types::I32)); // status / result

        let func_id = module
            .declare_function("wasm_simd_kernel", Linkage::Export, &sig)
            .map_err(|e| format!("Declare error: {}", e))?;

        let mut ctx = module.make_context();
        ctx.func.signature = sig;
        let mut fn_builder_ctx = FunctionBuilderContext::new();
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut fn_builder_ctx);

        let entry_block = builder.create_block();
        builder.append_block_params_for_function_params(entry_block);
        builder.switch_to_block(entry_block);

        let mem_ptr = builder.block_params(entry_block)[0];
        let _mem_len = builder.block_params(entry_block)[1];

        let mut val_stack: Vec<cranelift_codegen::ir::Value> = Vec::new();

        for op in opcodes {
            match op {
                WasmSimdOpcode::V128Const(bytes) => {
                    let slot = builder.create_sized_stack_slot(StackSlotData::new(
                        StackSlotKind::ExplicitSlot,
                        16,
                        4,
                    ));
                    let slot_addr = builder.ins().stack_addr(types::I64, slot, 0);
                    for (offset, byte) in bytes.iter().enumerate() {
                        let b_val = builder.ins().iconst(types::I8, *byte as i64);
                        builder
                            .ins()
                            .store(MemFlagsData::trusted(), b_val, slot_addr, offset as i32);
                    }
                    let vec_val =
                        builder
                            .ins()
                            .load(types::I8X16, MemFlagsData::trusted(), slot_addr, 0);
                    val_stack.push(vec_val);
                }
                WasmSimdOpcode::V128Load { offset } => {
                    let base = if let Some(b) = val_stack.pop() {
                        b
                    } else {
                        builder.ins().iconst(types::I64, 0)
                    };
                    let base_i64 = if builder.func.dfg.value_type(base) != types::I64 {
                        builder.ins().uextend(types::I64, base)
                    } else {
                        base
                    };
                    let addr = builder.ins().iadd(mem_ptr, base_i64);
                    let loaded = builder.ins().load(
                        types::I8X16,
                        MemFlagsData::trusted(),
                        addr,
                        *offset as i32,
                    );
                    val_stack.push(loaded);
                }
                WasmSimdOpcode::V128Store { offset } => {
                    if let Some(val) = val_stack.pop() {
                        let base = if let Some(b) = val_stack.pop() {
                            b
                        } else {
                            builder.ins().iconst(types::I64, 0)
                        };
                        let base_i64 = if builder.func.dfg.value_type(base) != types::I64 {
                            builder.ins().uextend(types::I64, base)
                        } else {
                            base
                        };
                        let addr = builder.ins().iadd(mem_ptr, base_i64);
                        builder
                            .ins()
                            .store(MemFlagsData::trusted(), val, addr, *offset as i32);
                    }
                }
                WasmSimdOpcode::I8x16Splat => {
                    let scalar = val_stack.pop().unwrap_or_else(|| {
                        builder.ins().iconst(types::I8, 0)
                    });
                    let s_i8 = if builder.func.dfg.value_type(scalar) != types::I8 {
                        builder.ins().ireduce(types::I8, scalar)
                    } else {
                        scalar
                    };
                    let v = builder.ins().splat(types::I8X16, s_i8);
                    val_stack.push(v);
                }
                WasmSimdOpcode::I16x8Splat => {
                    let scalar = val_stack.pop().unwrap_or_else(|| {
                        builder.ins().iconst(types::I16, 0)
                    });
                    let s_i16 = if builder.func.dfg.value_type(scalar) != types::I16 {
                        builder.ins().ireduce(types::I16, scalar)
                    } else {
                        scalar
                    };
                    let v = builder.ins().splat(types::I16X8, s_i16);
                    val_stack.push(v);
                }
                WasmSimdOpcode::I32x4Splat => {
                    let scalar = val_stack.pop().unwrap_or_else(|| {
                        builder.ins().iconst(types::I32, 0)
                    });
                    let s_i32 = if builder.func.dfg.value_type(scalar) != types::I32 {
                        builder.ins().ireduce(types::I32, scalar)
                    } else {
                        scalar
                    };
                    let v = builder.ins().splat(types::I32X4, s_i32);
                    val_stack.push(v);
                }
                WasmSimdOpcode::I64x2Splat => {
                    let scalar = val_stack.pop().unwrap_or_else(|| {
                        builder.ins().iconst(types::I64, 0)
                    });
                    let v = builder.ins().splat(types::I64X2, scalar);
                    val_stack.push(v);
                }
                WasmSimdOpcode::F32x4Splat => {
                    let scalar = val_stack.pop().unwrap_or_else(|| {
                        builder.ins().f32const(0.0)
                    });
                    let v = builder.ins().splat(types::F32X4, scalar);
                    val_stack.push(v);
                }
                WasmSimdOpcode::F64x2Splat => {
                    let scalar = val_stack.pop().unwrap_or_else(|| {
                        builder.ins().f64const(0.0)
                    });
                    let v = builder.ins().splat(types::F64X2, scalar);
                    val_stack.push(v);
                }
                WasmSimdOpcode::I8x16Add | WasmSimdOpcode::I16x8Add | WasmSimdOpcode::I32x4Add | WasmSimdOpcode::I64x2Add => {
                    let b = val_stack.pop().unwrap_or_else(|| builder.ins().iconst(types::I32X4, 0));
                    let a = val_stack.pop().unwrap_or_else(|| builder.ins().iconst(types::I32X4, 0));
                    let res = builder.ins().iadd(a, b);
                    val_stack.push(res);
                }
                WasmSimdOpcode::I8x16Sub | WasmSimdOpcode::I16x8Sub | WasmSimdOpcode::I32x4Sub | WasmSimdOpcode::I64x2Sub => {
                    let b = val_stack.pop().unwrap_or_else(|| builder.ins().iconst(types::I32X4, 0));
                    let a = val_stack.pop().unwrap_or_else(|| builder.ins().iconst(types::I32X4, 0));
                    let res = builder.ins().isub(a, b);
                    val_stack.push(res);
                }
                WasmSimdOpcode::I16x8Mul | WasmSimdOpcode::I32x4Mul | WasmSimdOpcode::I64x2Mul => {
                    let b = val_stack.pop().unwrap_or_else(|| builder.ins().iconst(types::I32X4, 0));
                    let a = val_stack.pop().unwrap_or_else(|| builder.ins().iconst(types::I32X4, 0));
                    let res = builder.ins().imul(a, b);
                    val_stack.push(res);
                }
                WasmSimdOpcode::F32x4Add | WasmSimdOpcode::F64x2Add => {
                    let b = val_stack.pop().unwrap_or_else(|| builder.ins().f32const(0.0));
                    let a = val_stack.pop().unwrap_or_else(|| builder.ins().f32const(0.0));
                    let res = builder.ins().fadd(a, b);
                    val_stack.push(res);
                }
                WasmSimdOpcode::F32x4Sub | WasmSimdOpcode::F64x2Sub => {
                    let b = val_stack.pop().unwrap_or_else(|| builder.ins().f32const(0.0));
                    let a = val_stack.pop().unwrap_or_else(|| builder.ins().f32const(0.0));
                    let res = builder.ins().fsub(a, b);
                    val_stack.push(res);
                }
                WasmSimdOpcode::F32x4Mul | WasmSimdOpcode::F64x2Mul => {
                    let b = val_stack.pop().unwrap_or_else(|| builder.ins().f32const(0.0));
                    let a = val_stack.pop().unwrap_or_else(|| builder.ins().f32const(0.0));
                    let res = builder.ins().fmul(a, b);
                    val_stack.push(res);
                }
                WasmSimdOpcode::F32x4Div | WasmSimdOpcode::F64x2Div => {
                    let b = val_stack.pop().unwrap_or_else(|| builder.ins().f32const(0.0));
                    let a = val_stack.pop().unwrap_or_else(|| builder.ins().f32const(0.0));
                    let res = builder.ins().fdiv(a, b);
                    val_stack.push(res);
                }
                WasmSimdOpcode::F32x4Sqrt | WasmSimdOpcode::F64x2Sqrt => {
                    let a = val_stack.pop().unwrap_or_else(|| builder.ins().f32const(0.0));
                    let res = builder.ins().sqrt(a);
                    val_stack.push(res);
                }
                WasmSimdOpcode::F32x4Min | WasmSimdOpcode::F64x2Min => {
                    let b = val_stack.pop().unwrap_or_else(|| builder.ins().f32const(0.0));
                    let a = val_stack.pop().unwrap_or_else(|| builder.ins().f32const(0.0));
                    let res = builder.ins().fmin(a, b);
                    val_stack.push(res);
                }
                WasmSimdOpcode::F32x4Max | WasmSimdOpcode::F64x2Max => {
                    let b = val_stack.pop().unwrap_or_else(|| builder.ins().f32const(0.0));
                    let a = val_stack.pop().unwrap_or_else(|| builder.ins().f32const(0.0));
                    let res = builder.ins().fmax(a, b);
                    val_stack.push(res);
                }
                WasmSimdOpcode::V128And => {
                    let b = val_stack.pop().unwrap_or_else(|| builder.ins().iconst(types::I32X4, 0));
                    let a = val_stack.pop().unwrap_or_else(|| builder.ins().iconst(types::I32X4, 0));
                    let res = builder.ins().band(a, b);
                    val_stack.push(res);
                }
                WasmSimdOpcode::V128Or => {
                    let b = val_stack.pop().unwrap_or_else(|| builder.ins().iconst(types::I32X4, 0));
                    let a = val_stack.pop().unwrap_or_else(|| builder.ins().iconst(types::I32X4, 0));
                    let res = builder.ins().bor(a, b);
                    val_stack.push(res);
                }
                WasmSimdOpcode::V128Xor => {
                    let b = val_stack.pop().unwrap_or_else(|| builder.ins().iconst(types::I32X4, 0));
                    let a = val_stack.pop().unwrap_or_else(|| builder.ins().iconst(types::I32X4, 0));
                    let res = builder.ins().bxor(a, b);
                    val_stack.push(res);
                }
                WasmSimdOpcode::V128Not => {
                    let a = val_stack.pop().unwrap_or_else(|| builder.ins().iconst(types::I32X4, 0));
                    let res = builder.ins().bnot(a);
                    val_stack.push(res);
                }
                WasmSimdOpcode::V128BitSelect => {
                    let c = val_stack.pop().unwrap_or_else(|| builder.ins().iconst(types::I32X4, 0));
                    let b = val_stack.pop().unwrap_or_else(|| builder.ins().iconst(types::I32X4, 0));
                    let a = val_stack.pop().unwrap_or_else(|| builder.ins().iconst(types::I32X4, 0));
                    let res = builder.ins().bitselect(c, a, b);
                    val_stack.push(res);
                }
                WasmSimdOpcode::I32x4ExtractLane(lane) => {
                    let v = val_stack.pop().unwrap_or_else(|| builder.ins().iconst(types::I32X4, 0));
                    let extracted = builder.ins().extractlane(v, *lane);
                    val_stack.push(extracted);
                }
                WasmSimdOpcode::I32x4ReplaceLane(lane) => {
                    let scalar = val_stack.pop().unwrap_or_else(|| builder.ins().iconst(types::I32, 0));
                    let v = val_stack.pop().unwrap_or_else(|| builder.ins().iconst(types::I32X4, 0));
                    let replaced = builder.ins().insertlane(v, scalar, *lane);
                    val_stack.push(replaced);
                }
                WasmSimdOpcode::F32x4ExtractLane(lane) => {
                    let v = val_stack.pop().unwrap_or_else(|| builder.ins().f32const(0.0));
                    let extracted = builder.ins().extractlane(v, *lane);
                    val_stack.push(extracted);
                }
                WasmSimdOpcode::F32x4ReplaceLane(lane) => {
                    let scalar = val_stack.pop().unwrap_or_else(|| builder.ins().f32const(0.0));
                    let v = val_stack.pop().unwrap_or_else(|| builder.ins().f32const(0.0));
                    let replaced = builder.ins().insertlane(v, scalar, *lane);
                    val_stack.push(replaced);
                }
                WasmSimdOpcode::I32x4DotI16x8S => {
                    let b = val_stack.pop().unwrap_or_else(|| builder.ins().iconst(types::I16X8, 0));
                    let a = val_stack.pop().unwrap_or_else(|| builder.ins().iconst(types::I16X8, 0));
                    let zero = builder.ins().iconst(types::I32, 0);
                    let mut res = builder.ins().splat(types::I32X4, zero);

                    for k in 0..4u8 {
                        let a_low = builder.ins().extractlane(a, 2 * k);
                        let b_low = builder.ins().extractlane(b, 2 * k);
                        let a_low32 = builder.ins().sextend(types::I32, a_low);
                        let b_low32 = builder.ins().sextend(types::I32, b_low);
                        let prod0 = builder.ins().imul(a_low32, b_low32);

                        let a_high = builder.ins().extractlane(a, 2 * k + 1);
                        let b_high = builder.ins().extractlane(b, 2 * k + 1);
                        let a_high32 = builder.ins().sextend(types::I32, a_high);
                        let b_high32 = builder.ins().sextend(types::I32, b_high);
                        let prod1 = builder.ins().imul(a_high32, b_high32);

                        let sum = builder.ins().iadd(prod0, prod1);
                        res = builder.ins().insertlane(res, sum, k);
                    }
                    val_stack.push(res);
                }
                _ => {}
            }
        }

        let ret_code = builder.ins().iconst(types::I32, 0);
        builder.ins().return_(&[ret_code]);
        builder.seal_block(entry_block);
        builder.finalize(module.target_config());

        module
            .define_function(func_id, &mut ctx)
            .map_err(|e| format!("Define error: {}", e))?;
        module.clear_context(&mut ctx);
        module.finalize_definitions().map_err(|e| format!("Finalize error: {}", e))?;

        let func_ptr = unsafe {
            let code_ptr = module.get_finalized_function(func_id);
            std::mem::transmute::<*const u8, NativeSimdFn>(code_ptr)
        };

        Ok(WasmVectorKernel {
            func_ptr,
            _module: Arc::new(std::sync::Mutex::new(module)),
        })
    }
}
