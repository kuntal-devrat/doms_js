//! Cranelift native machine code builder and bytecode compiler.

use super::types::*;
use crate::interpreter::{Vm, VmError};
use crate::value::{
    JsValue, TAG_BOOLEAN, TAG_INT32, TAG_MASK, TAG_NULL, TAG_UNDEFINED,
};
use doms_compiler::bytecode::{BytecodeChunk, Constant, OpCode};
use std::collections::HashMap;
use std::sync::Arc;

use cranelift_codegen::ir::{
    condcodes::{FloatCC, IntCC},
    types, AbiParam, Endianness, InstBuilder, MemFlagsData,
};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{Linkage, Module};

    /// Determine if a bytecode chunk can be compiled directly into Cranelift native machine code.
    pub fn can_compile_native(chunk: &BytecodeChunk) -> bool {
        if chunk.instructions.is_empty() {
            return false;
        }
        for op in &chunk.instructions {
            match op {
                OpCode::LoadConst(idx) => {
                    match chunk.constants.get(*idx as usize) {
                        Some(Constant::Number(_)) | Some(Constant::Boolean(_)) => {}
                        _ => return false,
                    }
                }
                OpCode::LoadNull
                | OpCode::LoadUndefined
                | OpCode::LoadTrue
                | OpCode::LoadFalse
                | OpCode::GetLocal(_)
                | OpCode::SetLocal(_)
                | OpCode::Add
                | OpCode::Sub
                | OpCode::Mul
                | OpCode::Div
                | OpCode::BitAnd
                | OpCode::BitOr
                | OpCode::BitXor
                | OpCode::BitNot
                | OpCode::Shl
                | OpCode::Shr
                | OpCode::Ushr
                | OpCode::Pos
                | OpCode::Neg
                | OpCode::Not
                | OpCode::Lt
                | OpCode::Lte
                | OpCode::Gt
                | OpCode::Gte
                | OpCode::Eq
                | OpCode::NotEq
                | OpCode::StrictEq
                | OpCode::StrictNotEq
                | OpCode::Jump(_)
                | OpCode::JumpIfFalse(_)
                | OpCode::JumpIfTrue(_)
                | OpCode::Pop
                | OpCode::Dup
                | OpCode::Return
                | OpCode::Halt => {}
                _ => return false,
            }
        }
        true
    }

    /// Compile a bytecode chunk into Cranelift native machine code.
    pub fn compile_native(chunk: &BytecodeChunk) -> Option<JitFunction> {
        if !can_compile_native(chunk) {
            return None;
        }

        let mut builder_obj = JITBuilder::new(cranelift_module::default_libcall_names()).ok()?;
        builder_obj.symbol("jit_strict_eq", jit_strict_eq as *const u8);
        builder_obj.symbol("jit_strict_neq", jit_strict_neq as *const u8);
        let mut module = JITModule::new(builder_obj);

        let mut strict_eq_sig = module.make_signature();
        strict_eq_sig.params.push(AbiParam::new(types::I64));
        strict_eq_sig.params.push(AbiParam::new(types::I64));
        strict_eq_sig.returns.push(AbiParam::new(types::I64));
        let strict_eq_func_id = module.declare_function("jit_strict_eq", Linkage::Import, &strict_eq_sig).ok()?;

        let mut strict_neq_sig = module.make_signature();
        strict_neq_sig.params.push(AbiParam::new(types::I64));
        strict_neq_sig.params.push(AbiParam::new(types::I64));
        strict_neq_sig.returns.push(AbiParam::new(types::I64));
        let strict_neq_func_id = module.declare_function("jit_strict_neq", Linkage::Import, &strict_neq_sig).ok()?;

        let mut sig = module.make_signature();
        sig.params.push(AbiParam::new(types::I64)); // args_ptr
        sig.params.push(AbiParam::new(types::I64)); // num_args
        sig.returns.push(AbiParam::new(types::I64)); // return u64

        let mut ctx = module.make_context();
        ctx.func.signature = sig;
        let mut fn_builder_ctx = FunctionBuilderContext::new();
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut fn_builder_ctx);

        let strict_eq_func_ref = module.declare_func_in_func(strict_eq_func_id, &mut builder.func);
        let strict_neq_func_ref = module.declare_func_in_func(strict_neq_func_id, &mut builder.func);

        // Find jump targets (basic block leaders)
        let num_insts = chunk.instructions.len();
        let mut is_leader = vec![false; num_insts + 1];
        is_leader[0] = true;

        for (ip, op) in chunk.instructions.iter().enumerate() {
            match op {
                OpCode::Jump(target) | OpCode::JumpIfFalse(target) | OpCode::JumpIfTrue(target) => {
                    let t = *target as usize;
                    if t <= num_insts {
                        is_leader[t] = true;
                    }
                    if ip + 1 <= num_insts {
                        is_leader[ip + 1] = true;
                    }
                }
                OpCode::Return | OpCode::Halt => {
                    if ip + 1 <= num_insts {
                        is_leader[ip + 1] = true;
                    }
                }
                _ => {}
            }
        }

        let mut blocks: HashMap<usize, cranelift_codegen::ir::Block> = HashMap::new();
        for (ip, &leader) in is_leader.iter().enumerate() {
            if leader && ip < num_insts {
                blocks.insert(ip, builder.create_block());
            }
        }

        // Find max local variable index used
        let mut max_local: u16 = 0;
        for op in &chunk.instructions {
            match op {
                OpCode::GetLocal(slot) | OpCode::SetLocal(slot) => {
                    max_local = max_local.max(*slot + 1);
                }
                _ => {}
            }
        }
        let num_locals = (max_local as usize).max(8);

        // Entry block: function entry arguments setup
        let entry_block = builder.create_block();
        builder.append_block_params_for_function_params(entry_block);
        builder.switch_to_block(entry_block);

        let args_ptr = builder.block_params(entry_block)[0];
        let num_args = builder.block_params(entry_block)[1];

        // Declare and initialize local variables from args_ptr
        let undef_val = builder.ins().iconst(types::I64, TAG_UNDEFINED as i64);
        let mut local_vars = Vec::with_capacity(num_locals);
        for i in 0..num_locals {
            let var = builder.declare_var(types::I64);
            local_vars.push(var);

            let offset = (i * 8) as i32;
            let idx_val = builder.ins().iconst(types::I64, i as i64);
            let has_arg = builder.ins().icmp(IntCC::UnsignedLessThan, idx_val, num_args);
            let arg_val = builder.ins().load(types::I64, MemFlagsData::trusted(), args_ptr, offset);
            let init_val = builder.ins().select(has_arg, arg_val, undef_val);
            builder.def_var(var, init_val);
        }

        // Declare stack variables
        let mut stack_vars = Vec::with_capacity(64);
        for _ in 0..64 {
            let var = builder.declare_var(types::I64);
            builder.def_var(var, undef_val);
            stack_vars.push(var);
        }

        // Jump to first instruction block
        let block_zero = blocks[&0];
        builder.ins().jump(block_zero, &[]);
        builder.seal_block(entry_block);

        let mut stack_depth: usize = 0;
        let mut block_terminated = false;
        let bitcast_flags = MemFlagsData::new().with_endianness(Endianness::Little);

        for ip in 0..num_insts {
            if let Some(&blk) = blocks.get(&ip) {
                if ip > 0 && !block_terminated {
                    builder.ins().jump(blk, &[]);
                }
                builder.switch_to_block(blk);
                block_terminated = false;
            }

            if block_terminated {
                continue;
            }

            let op = chunk.instructions[ip];
            match op {
                OpCode::LoadConst(idx) => {
                    let js_val = match &chunk.constants[idx as usize] {
                        Constant::Number(n) => {
                            if n.fract() == 0.0 && *n >= i32::MIN as f64 && *n <= i32::MAX as f64 {
                                JsValue::from_int32(*n as i32)
                            } else {
                                JsValue::from_double(*n)
                            }
                        }
                        Constant::Boolean(b) => JsValue::from_boolean(*b),
                        _ => JsValue::undefined(),
                    };
                    let val = builder.ins().iconst(types::I64, js_val.0 as i64);
                    builder.def_var(stack_vars[stack_depth], val);
                    stack_depth += 1;
                }
                OpCode::LoadTrue => {
                    let val = builder.ins().iconst(types::I64, JsValue::from_boolean(true).0 as i64);
                    builder.def_var(stack_vars[stack_depth], val);
                    stack_depth += 1;
                }
                OpCode::LoadFalse => {
                    let val = builder.ins().iconst(types::I64, JsValue::from_boolean(false).0 as i64);
                    builder.def_var(stack_vars[stack_depth], val);
                    stack_depth += 1;
                }
                OpCode::LoadNull => {
                    let val = builder.ins().iconst(types::I64, JsValue::null().0 as i64);
                    builder.def_var(stack_vars[stack_depth], val);
                    stack_depth += 1;
                }
                OpCode::LoadUndefined => {
                    let val = builder.ins().iconst(types::I64, JsValue::undefined().0 as i64);
                    builder.def_var(stack_vars[stack_depth], val);
                    stack_depth += 1;
                }
                OpCode::GetLocal(slot) => {
                    let val = builder.use_var(local_vars[slot as usize]);
                    builder.def_var(stack_vars[stack_depth], val);
                    stack_depth += 1;
                }
                OpCode::SetLocal(slot) => {
                    if stack_depth > 0 {
                        let val = builder.use_var(stack_vars[stack_depth - 1]);
                        builder.def_var(local_vars[slot as usize], val);
                    }
                }
                OpCode::Pop => {
                    if stack_depth > 0 {
                        stack_depth -= 1;
                    }
                }
                OpCode::Dup => {
                    if stack_depth > 0 {
                        let val = builder.use_var(stack_vars[stack_depth - 1]);
                        builder.def_var(stack_vars[stack_depth], val);
                        stack_depth += 1;
                    }
                }
                OpCode::Add | OpCode::Sub | OpCode::Mul | OpCode::Div => {
                    if stack_depth >= 2 {
                        stack_depth -= 2;
                        let lhs = builder.use_var(stack_vars[stack_depth]);
                        let rhs = builder.use_var(stack_vars[stack_depth + 1]);

                        let tag_mask = builder.ins().iconst(types::I64, TAG_MASK as i64);
                        let tag_int = builder.ins().iconst(types::I64, TAG_INT32 as i64);
                        let l_tag = builder.ins().band(lhs, tag_mask);
                        let r_tag = builder.ins().band(rhs, tag_mask);
                        let both_tag = builder.ins().bor(l_tag, r_tag);
                        let is_both_int = builder.ins().icmp(IntCC::Equal, both_tag, tag_int);

                        let l32 = builder.ins().ireduce(types::I32, lhs);
                        let r32 = builder.ins().ireduce(types::I32, rhs);

                        let l_f64_from_int = builder.ins().fcvt_from_sint(types::F64, l32);
                        let l_is_int = builder.ins().icmp(IntCC::Equal, l_tag, tag_int);
                        let l_f64_raw = builder.ins().bitcast(types::F64, bitcast_flags, lhs);
                        let l_f64 = builder.ins().select(l_is_int, l_f64_from_int, l_f64_raw);

                        let r_f64_from_int = builder.ins().fcvt_from_sint(types::F64, r32);
                        let r_is_int = builder.ins().icmp(IntCC::Equal, r_tag, tag_int);
                        let r_f64_raw = builder.ins().bitcast(types::F64, bitcast_flags, rhs);
                        let r_f64 = builder.ins().select(r_is_int, r_f64_from_int, r_f64_raw);

                        let res = match op {
                            OpCode::Add => {
                                let sum32 = builder.ins().iadd(l32, r32);
                                let sum64 = builder.ins().uextend(types::I64, sum32);
                                let int_res = builder.ins().bor(tag_int, sum64);
                                let sum_f64 = builder.ins().fadd(l_f64, r_f64);
                                let dbl_res = builder.ins().bitcast(types::I64, bitcast_flags, sum_f64);
                                builder.ins().select(is_both_int, int_res, dbl_res)
                            }
                            OpCode::Sub => {
                                let diff32 = builder.ins().isub(l32, r32);
                                let diff64 = builder.ins().uextend(types::I64, diff32);
                                let int_res = builder.ins().bor(tag_int, diff64);
                                let diff_f64 = builder.ins().fsub(l_f64, r_f64);
                                let dbl_res = builder.ins().bitcast(types::I64, bitcast_flags, diff_f64);
                                builder.ins().select(is_both_int, int_res, dbl_res)
                            }
                            OpCode::Mul => {
                                let prod32 = builder.ins().imul(l32, r32);
                                let prod64 = builder.ins().uextend(types::I64, prod32);
                                let int_res = builder.ins().bor(tag_int, prod64);
                                let prod_f64 = builder.ins().fmul(l_f64, r_f64);
                                let dbl_res = builder.ins().bitcast(types::I64, bitcast_flags, prod_f64);
                                builder.ins().select(is_both_int, int_res, dbl_res)
                            }
                            OpCode::Div => {
                                let div_f64 = builder.ins().fdiv(l_f64, r_f64);
                                builder.ins().bitcast(types::I64, bitcast_flags, div_f64)
                            }
                            _ => unreachable!(),
                        };

                        builder.def_var(stack_vars[stack_depth], res);
                        stack_depth += 1;
                    }
                }
                OpCode::BitAnd | OpCode::BitOr | OpCode::BitXor => {
                    if stack_depth >= 2 {
                        stack_depth -= 2;
                        let lhs = builder.use_var(stack_vars[stack_depth]);
                        let rhs = builder.use_var(stack_vars[stack_depth + 1]);
                        let l32 = builder.ins().ireduce(types::I32, lhs);
                        let r32 = builder.ins().ireduce(types::I32, rhs);
                        let res32 = match op {
                            OpCode::BitAnd => builder.ins().band(l32, r32),
                            OpCode::BitOr => builder.ins().bor(l32, r32),
                            OpCode::BitXor => builder.ins().bxor(l32, r32),
                            _ => unreachable!(),
                        };
                        let tag_int = builder.ins().iconst(types::I64, TAG_INT32 as i64);
                        let res64 = builder.ins().uextend(types::I64, res32);
                        let res = builder.ins().bor(tag_int, res64);
                        builder.def_var(stack_vars[stack_depth], res);
                        stack_depth += 1;
                    }
                }
                OpCode::BitNot => {
                    if stack_depth > 0 {
                        let val = builder.use_var(stack_vars[stack_depth - 1]);
                        let v32 = builder.ins().ireduce(types::I32, val);
                        let not32 = builder.ins().bnot(v32);
                        let tag_int = builder.ins().iconst(types::I64, TAG_INT32 as i64);
                        let res64 = builder.ins().uextend(types::I64, not32);
                        let res = builder.ins().bor(tag_int, res64);
                        builder.def_var(stack_vars[stack_depth - 1], res);
                    }
                }
                OpCode::Shl | OpCode::Shr | OpCode::Ushr => {
                    if stack_depth >= 2 {
                        stack_depth -= 2;
                        let lhs = builder.use_var(stack_vars[stack_depth]);
                        let rhs = builder.use_var(stack_vars[stack_depth + 1]);
                        let l32 = builder.ins().ireduce(types::I32, lhs);
                        let r32 = builder.ins().ireduce(types::I32, rhs);
                        let shift = builder.ins().band_imm_u(r32, 31);
                        let res32 = match op {
                            OpCode::Shl => builder.ins().ishl(l32, shift),
                            OpCode::Shr => builder.ins().sshr(l32, shift),
                            OpCode::Ushr => builder.ins().ushr(l32, shift),
                            _ => unreachable!(),
                        };
                        let tag_int = builder.ins().iconst(types::I64, TAG_INT32 as i64);
                        let res64 = builder.ins().uextend(types::I64, res32);
                        let res = builder.ins().bor(tag_int, res64);
                        builder.def_var(stack_vars[stack_depth], res);
                        stack_depth += 1;
                    }
                }
                OpCode::Lt | OpCode::Lte | OpCode::Gt | OpCode::Gte => {
                    if stack_depth >= 2 {
                        stack_depth -= 2;
                        let lhs = builder.use_var(stack_vars[stack_depth]);
                        let rhs = builder.use_var(stack_vars[stack_depth + 1]);

                        let tag_mask = builder.ins().iconst(types::I64, TAG_MASK as i64);
                        let tag_int = builder.ins().iconst(types::I64, TAG_INT32 as i64);
                        let l_tag = builder.ins().band(lhs, tag_mask);
                        let r_tag = builder.ins().band(rhs, tag_mask);
                        let both_tag = builder.ins().bor(l_tag, r_tag);
                        let is_both_int = builder.ins().icmp(IntCC::Equal, both_tag, tag_int);

                        let l32 = builder.ins().ireduce(types::I32, lhs);
                        let r32 = builder.ins().ireduce(types::I32, rhs);

                        let l_f64_from_int = builder.ins().fcvt_from_sint(types::F64, l32);
                        let l_is_int = builder.ins().icmp(IntCC::Equal, l_tag, tag_int);
                        let l_f64_raw = builder.ins().bitcast(types::F64, bitcast_flags, lhs);
                        let l_f64 = builder.ins().select(l_is_int, l_f64_from_int, l_f64_raw);

                        let r_f64_from_int = builder.ins().fcvt_from_sint(types::F64, r32);
                        let r_is_int = builder.ins().icmp(IntCC::Equal, r_tag, tag_int);
                        let r_f64_raw = builder.ins().bitcast(types::F64, bitcast_flags, rhs);
                        let r_f64 = builder.ins().select(r_is_int, r_f64_from_int, r_f64_raw);

                        let (int_cc, flt_cc) = match op {
                            OpCode::Lt => (IntCC::SignedLessThan, FloatCC::LessThan),
                            OpCode::Lte => (IntCC::SignedLessThanOrEqual, FloatCC::LessThanOrEqual),
                            OpCode::Gt => (IntCC::SignedGreaterThan, FloatCC::GreaterThan),
                            OpCode::Gte => (IntCC::SignedGreaterThanOrEqual, FloatCC::GreaterThanOrEqual),
                            _ => unreachable!(),
                        };

                        let cmp_int = builder.ins().icmp(int_cc, l32, r32);
                        let cmp_flt = builder.ins().fcmp(flt_cc, l_f64, r_f64);
                        let cmp = builder.ins().select(is_both_int, cmp_int, cmp_flt);

                        let tag_bool = builder.ins().iconst(types::I64, TAG_BOOLEAN as i64);
                        let cmp64 = builder.ins().uextend(types::I64, cmp);
                        let res = builder.ins().bor(tag_bool, cmp64);
                        builder.def_var(stack_vars[stack_depth], res);
                        stack_depth += 1;
                    }
                }
                OpCode::StrictEq | OpCode::Eq => {
                    if stack_depth >= 2 {
                        stack_depth -= 2;
                        let lhs = builder.use_var(stack_vars[stack_depth]);
                        let rhs = builder.use_var(stack_vars[stack_depth + 1]);
                        let call = builder.ins().call(strict_eq_func_ref, &[lhs, rhs]);
                        let res = builder.inst_results(call)[0];
                        builder.def_var(stack_vars[stack_depth], res);
                        stack_depth += 1;
                    }
                }
                OpCode::StrictNotEq | OpCode::NotEq => {
                    if stack_depth >= 2 {
                        stack_depth -= 2;
                        let lhs = builder.use_var(stack_vars[stack_depth]);
                        let rhs = builder.use_var(stack_vars[stack_depth + 1]);
                        let call = builder.ins().call(strict_neq_func_ref, &[lhs, rhs]);
                        let res = builder.inst_results(call)[0];
                        builder.def_var(stack_vars[stack_depth], res);
                        stack_depth += 1;
                    }
                }
                OpCode::Neg => {
                    if stack_depth > 0 {
                        let val = builder.use_var(stack_vars[stack_depth - 1]);
                        let tag_mask = builder.ins().iconst(types::I64, TAG_MASK as i64);
                        let tag_int = builder.ins().iconst(types::I64, TAG_INT32 as i64);
                        let v_tag = builder.ins().band(val, tag_mask);
                        let is_int = builder.ins().icmp(IntCC::Equal, v_tag, tag_int);
                        let v32 = builder.ins().ireduce(types::I32, val);
                        let neg32 = builder.ins().ineg(v32);
                        let neg64 = builder.ins().uextend(types::I64, neg32);
                        let int_res = builder.ins().bor(tag_int, neg64);

                        let v_f64_from_int = builder.ins().fcvt_from_sint(types::F64, v32);
                        let v_f64_raw = builder.ins().bitcast(types::F64, bitcast_flags, val);
                        let v_f64 = builder.ins().select(is_int, v_f64_from_int, v_f64_raw);
                        let neg_f64 = builder.ins().fneg(v_f64);
                        let dbl_res = builder.ins().bitcast(types::I64, bitcast_flags, neg_f64);

                        let res = builder.ins().select(is_int, int_res, dbl_res);
                        builder.def_var(stack_vars[stack_depth - 1], res);
                    }
                }
                OpCode::Pos => {
                    // Identity for already numerical primitives in this fast path
                }
                OpCode::Not => {
                    if stack_depth > 0 {
                        let cond = builder.use_var(stack_vars[stack_depth - 1]);
                        let is_false = builder.ins().icmp_imm_u(IntCC::Equal, cond, TAG_BOOLEAN as i64);
                        let is_zero_int = builder.ins().icmp_imm_u(IntCC::Equal, cond, TAG_INT32 as i64);
                        let is_zero_f64 = builder.ins().icmp_imm_u(IntCC::Equal, cond, 0i64);
                        let is_null = builder.ins().icmp_imm_u(IntCC::Equal, cond, TAG_NULL as i64);
                        let is_undef = builder.ins().icmp_imm_u(IntCC::Equal, cond, TAG_UNDEFINED as i64);
                        let f1 = builder.ins().bor(is_false, is_zero_int);
                        let f2 = builder.ins().bor(is_zero_f64, is_null);
                        let f3 = builder.ins().bor(f1, f2);
                        let is_falsy = builder.ins().bor(f3, is_undef);

                        let true_val = builder.ins().iconst(types::I64, JsValue::from_boolean(true).0 as i64);
                        let false_val = builder.ins().iconst(types::I64, JsValue::from_boolean(false).0 as i64);
                        let res = builder.ins().select(is_falsy, true_val, false_val);
                        builder.def_var(stack_vars[stack_depth - 1], res);
                    }
                }
                OpCode::Jump(target) => {
                    let target_block = blocks[&(target as usize)];
                    builder.ins().jump(target_block, &[]);
                    block_terminated = true;
                }
                OpCode::JumpIfFalse(target) => {
                    if stack_depth > 0 {
                        stack_depth -= 1;
                        let cond = builder.use_var(stack_vars[stack_depth]);
                        let is_false = builder.ins().icmp_imm_u(IntCC::Equal, cond, TAG_BOOLEAN as i64);
                        let is_zero_int = builder.ins().icmp_imm_u(IntCC::Equal, cond, TAG_INT32 as i64);
                        let is_zero_f64 = builder.ins().icmp_imm_u(IntCC::Equal, cond, 0i64);
                        let is_null = builder.ins().icmp_imm_u(IntCC::Equal, cond, TAG_NULL as i64);
                        let is_undef = builder.ins().icmp_imm_u(IntCC::Equal, cond, TAG_UNDEFINED as i64);
                        let f1 = builder.ins().bor(is_false, is_zero_int);
                        let f2 = builder.ins().bor(is_zero_f64, is_null);
                        let f3 = builder.ins().bor(f1, f2);
                        let is_falsy = builder.ins().bor(f3, is_undef);

                        let target_block = blocks[&(target as usize)];
                        let next_block = blocks[&(ip + 1)];
                        builder.ins().brif(is_falsy, target_block, &[], next_block, &[]);
                        block_terminated = true;
                    }
                }
                OpCode::JumpIfTrue(target) => {
                    if stack_depth > 0 {
                        stack_depth -= 1;
                        let cond = builder.use_var(stack_vars[stack_depth]);
                        let is_false = builder.ins().icmp_imm_u(IntCC::Equal, cond, TAG_BOOLEAN as i64);
                        let is_zero_int = builder.ins().icmp_imm_u(IntCC::Equal, cond, TAG_INT32 as i64);
                        let is_zero_f64 = builder.ins().icmp_imm_u(IntCC::Equal, cond, 0i64);
                        let is_null = builder.ins().icmp_imm_u(IntCC::Equal, cond, TAG_NULL as i64);
                        let is_undef = builder.ins().icmp_imm_u(IntCC::Equal, cond, TAG_UNDEFINED as i64);
                        let f1 = builder.ins().bor(is_false, is_zero_int);
                        let f2 = builder.ins().bor(is_zero_f64, is_null);
                        let f3 = builder.ins().bor(f1, f2);
                        let is_falsy = builder.ins().bor(f3, is_undef);

                        let target_block = blocks[&(target as usize)];
                        let next_block = blocks[&(ip + 1)];
                        builder.ins().brif(is_falsy, next_block, &[], target_block, &[]);
                        block_terminated = true;
                    }
                }
                OpCode::Return | OpCode::Halt => {
                    let ret_val = if stack_depth > 0 {
                        builder.use_var(stack_vars[stack_depth - 1])
                    } else {
                        builder.ins().iconst(types::I64, TAG_UNDEFINED as i64)
                    };
                    builder.ins().return_(&[ret_val]);
                    block_terminated = true;
                }
                _ => {}
            }
        }

        if !block_terminated {
            let ret_val = if stack_depth > 0 {
                builder.use_var(stack_vars[stack_depth - 1])
            } else {
                builder.ins().iconst(types::I64, TAG_UNDEFINED as i64)
            };
            builder.ins().return_(&[ret_val]);
        }

        // Seal all blocks and finalize
        for blk in blocks.values() {
            builder.seal_block(*blk);
        }
        builder.finalize(module.target_config());

        let func_id = module.declare_function("doms_jit_entry", Linkage::Export, &ctx.func.signature).ok()?;
        module.define_function(func_id, &mut ctx).ok()?;
        module.clear_context(&mut ctx);
        module.finalize_definitions().ok()?;

        let code_ptr = module.get_finalized_function(func_id);
        let native_fn: NativeJitFn = unsafe { std::mem::transmute(code_ptr) };
        let module_arc = Arc::new(std::sync::Mutex::new(module));
        let module_for_closure = Arc::clone(&module_arc);

        let executable: JitCallable = Arc::new(move |_vm: &mut Vm, _this: JsValue, args: &[JsValue]| -> Result<JsValue, VmError> {
            let _keep_alive = &module_for_closure;
            let mut raw_args: Vec<u64> = args.iter().map(|v| v.0).collect();
            if raw_args.len() < num_locals {
                raw_args.resize(num_locals, TAG_UNDEFINED);
            }
            let res = unsafe { (native_fn)(raw_args.as_ptr(), raw_args.len()) };
            Ok(JsValue(res))
        });

        Some(JitFunction {
            name: None,
            hotness: JIT_HOT_THRESHOLD,
            specialized_type: SpeculativeType::Integer,
            code: JitCode::Native {
                func_ptr: native_fn,
                num_locals,
                _module: module_arc,
            },
            executable,
        })
    }
