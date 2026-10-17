//! JIT compiler entry point and Tier-1 interpreted delegation fallback.

use super::builder;
use super::types::*;
use crate::interpreter::{Vm, VmError};
use crate::value::JsValue;
use doms_compiler::bytecode::{BytecodeChunk, OpCode};
use std::sync::Arc;

/// The Baseline JIT Compiler.
pub struct JitCompiler;

impl JitCompiler {
    pub fn new() -> Self {
        Self
    }

    /// Determine if a bytecode chunk can be compiled directly into Cranelift native machine code.
    #[inline]
    pub fn can_compile_native(chunk: &BytecodeChunk) -> bool {
        builder::can_compile_native(chunk)
    }

    /// Compile a bytecode chunk into Cranelift native machine code.
    #[inline]
    pub fn compile_native(chunk: &BytecodeChunk) -> Option<JitFunction> {
        builder::compile_native(chunk)
    }

    /// Compile a bytecode chunk using the Tier-1 interpreted delegation fallback.
    pub fn compile_interpreted(chunk: &BytecodeChunk) -> Option<JitFunction> {
        let chunk_clone = Arc::new(chunk.clone());

        // Speculative type analysis on chunk operations
        let mut specialized = SpeculativeType::Unknown;
        for op in &chunk.instructions {
            match op {
                OpCode::BitAnd | OpCode::BitOr | OpCode::BitXor | OpCode::BitNot => {
                    specialized = SpeculativeType::Integer;
                    break;
                }
                OpCode::Add | OpCode::Sub | OpCode::Mul | OpCode::Div
                    if specialized == SpeculativeType::Unknown =>
                {
                    specialized = SpeculativeType::Double;
                }
                _ => {}
            }
        }

        let exec_chunk = Arc::clone(&chunk_clone);
        let executable: JitCallable = Arc::new(move |vm: &mut Vm, this_val: JsValue, args: &[JsValue]| -> Result<JsValue, VmError> {
            let frame_chunk = Arc::clone(&exec_chunk);
            let target_depth = vm.frames.len();
            let base_sp = vm.stack.len();

            for arg in args {
                vm.push(*arg);
            }

            vm.frames.push(crate::interpreter::CallFrame {
                chunk: frame_chunk,
                ip: 0,
                base_sp,
                this_val,
                gen_val: None,
                locals: args.to_vec(),
                local_names: Vec::new(),
                lexical_local_names: Vec::new(),
                scopes: Vec::new(),
                upvalues_count: 0,
                env_obj: None,
                arguments: None,
                is_strict: false,
                new_target: None,
                is_function: true,
                is_method: false,
                is_arrow: false,
                has_param_exprs: false,
                in_param_init: false,
                is_indirect_eval: false,
                callee_func: None,
                callee_val: None,
                mapped_arguments: None,
                home_object: None,
            });

            vm.run_until_depth(target_depth)
        });

        Some(JitFunction {
            name: None,
            hotness: JIT_HOT_THRESHOLD,
            specialized_type: specialized,
            code: JitCode::Interpreted(Arc::clone(&executable)),
            executable,
        })
    }

    /// Compile a bytecode chunk into a specialized JIT block if eligible.
    ///
    /// Attempts native Cranelift machine code compilation first, then falls back to interpreted delegation.
    pub fn compile(chunk: &BytecodeChunk) -> Option<JitFunction> {
        if let Some(native_fn) = Self::compile_native(chunk) {
            return Some(native_fn);
        }
        Self::compile_interpreted(chunk)
    }
}

impl Default for JitCompiler {
    fn default() -> Self {
        Self::new()
    }
}
