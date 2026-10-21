//! Cranelift native machine code builder and bytecode compiler.

use super::types::*;
use crate::interpreter::{Vm, VmError};
use crate::value::{
    JsValue, PAYLOAD_MASK, TAG_BOOLEAN, TAG_HOLE, TAG_INT32, TAG_MASK, TAG_NULL, TAG_OBJECT,
    TAG_UNDEFINED,
};
use doms_compiler::bytecode::{BytecodeChunk, Constant, FeedbackSlot, OpCode};
use std::collections::HashMap;
use std::sync::Arc;

use cranelift_codegen::ir::{
    AbiParam, Endianness, InstBuilder, MemFlagsData, StackSlotData, StackSlotKind,
    condcodes::{FloatCC, IntCC},
    types,
};
use cranelift_codegen::settings::Configurable;
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{Linkage, Module};

/// Determine if a bytecode chunk can be compiled directly into Cranelift native machine code.
pub fn can_compile_native(chunk: &BytecodeChunk) -> bool {
    if chunk.instructions.is_empty() {
        return false;
    }
    for (_ip, op) in chunk.instructions.iter().enumerate() {
        match op {
            OpCode::LoadConst(idx) => match chunk.constants.get(*idx as usize) {
                Some(Constant::Number(_)) | Some(Constant::Boolean(_)) => {}
                Some(Constant::String(s)) => {
                    if s.len() > 5 || !s.is_ascii() {
                        return false;
                    }
                }
                _ => return false,
            },
            OpCode::LoadNull
            | OpCode::LoadUndefined
            | OpCode::LoadTrue
            | OpCode::LoadFalse
            | OpCode::This
            | OpCode::GetLocal(_)
            | OpCode::SetLocal(_)
            | OpCode::GetGlobal(_)
            | OpCode::GetGlobalSafe(_)
            | OpCode::SetGlobal(_)
            | OpCode::SetGlobalDirect(_)
            | OpCode::Add
            | OpCode::Sub
            | OpCode::Mul
            | OpCode::Div
            | OpCode::Mod
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
            | OpCode::Call(_)
            | OpCode::CallWithThis(_)
            | OpCode::Construct(_)
            | OpCode::Pop
            | OpCode::Dup
            | OpCode::Return
            | OpCode::Halt
            | OpCode::CreateObject
            | OpCode::CreateArray(_)
            | OpCode::GetProp(_)
            | OpCode::SetProp(_)
            | OpCode::GetElem
            | OpCode::SetElem => {}
            _ => return false,
        }
    }
    true
}

/// Determine if a bytecode chunk can be compiled for OSR starting from a loop target.
pub fn can_compile_native_osr(chunk: &BytecodeChunk, loop_target: usize) -> bool {
    if chunk.instructions.is_empty() || loop_target >= chunk.instructions.len() {
        return false;
    }
    for ip in loop_target..chunk.instructions.len() {
        let op = &chunk.instructions[ip];
        match op {
            OpCode::LoadConst(idx) => match chunk.constants.get(*idx as usize) {
                Some(Constant::Number(_)) | Some(Constant::Boolean(_)) => {}
                Some(Constant::String(s)) => {
                    if s.len() > 5 || !s.is_ascii() {
                        return false;
                    }
                }
                _ => return false,
            },
            OpCode::LoadNull
            | OpCode::LoadUndefined
            | OpCode::LoadTrue
            | OpCode::LoadFalse
            | OpCode::This
            | OpCode::GetLocal(_)
            | OpCode::SetLocal(_)
            | OpCode::GetGlobal(_)
            | OpCode::GetGlobalSafe(_)
            | OpCode::SetGlobal(_)
            | OpCode::SetGlobalDirect(_)
            | OpCode::Add
            | OpCode::Sub
            | OpCode::Mul
            | OpCode::Div
            | OpCode::Mod
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
            | OpCode::Call(_)
            | OpCode::CallWithThis(_)
            | OpCode::Pop
            | OpCode::Dup
            | OpCode::Return
            | OpCode::Halt
            | OpCode::Jump(_)
            | OpCode::JumpIfFalse(_)
            | OpCode::JumpIfTrue(_)
            | OpCode::GetProp(_)
            | OpCode::SetProp(_)
            | OpCode::GetElem
            | OpCode::SetElem => {}
            _ => return false,
        }
    }
    true
}

/// Compile a bytecode chunk into Cranelift native machine code.
pub fn compile_native(chunk: &BytecodeChunk) -> Option<JitFunction> {
    compile_native_internal(chunk, None)
}

/// Compile a bytecode chunk into Cranelift native machine code for On-Stack Replacement (OSR) at a loop header.
pub fn compile_native_osr(chunk: &BytecodeChunk, loop_target: usize) -> Option<JitFunction> {
    compile_native_internal(chunk, Some(loop_target))
}

fn compute_stack_depths(chunk: &BytecodeChunk, osr_target: Option<usize>) -> Vec<usize> {
    let num_insts = chunk.instructions.len();
    let mut depths: Vec<Option<usize>> = vec![None; num_insts + 1];
    let start_ip = osr_target.unwrap_or(0);
    depths[start_ip] = Some(0);

    let mut worklist = std::collections::VecDeque::new();
    worklist.push_back(start_ip);

    while let Some(ip) = worklist.pop_front() {
        if ip >= num_insts {
            continue;
        }
        let cur_depth = depths[ip].unwrap_or(0);
        let op = &chunk.instructions[ip];

        let (pops, pushes, branches) = match op {
            OpCode::LoadConst(_)
            | OpCode::LoadNull
            | OpCode::LoadUndefined
            | OpCode::LoadTrue
            | OpCode::LoadFalse
            | OpCode::This
            | OpCode::GetLocal(_)
            | OpCode::GetGlobal(_)
            | OpCode::GetGlobalSafe(_)
            | OpCode::CreateObject
            | OpCode::Dup => (0, 1, Vec::new()),

            OpCode::SetLocal(_) | OpCode::SetGlobal(_) | OpCode::SetGlobalDirect(_) => {
                (0, 0, Vec::new())
            }

            OpCode::Pop => (1, 0, Vec::new()),

            OpCode::Add
            | OpCode::Sub
            | OpCode::Mul
            | OpCode::Div
            | OpCode::Mod
            | OpCode::BitAnd
            | OpCode::BitOr
            | OpCode::BitXor
            | OpCode::Shl
            | OpCode::Shr
            | OpCode::Ushr
            | OpCode::Lt
            | OpCode::Lte
            | OpCode::Gt
            | OpCode::Gte
            | OpCode::Eq
            | OpCode::NotEq
            | OpCode::StrictEq
            | OpCode::StrictNotEq
            | OpCode::GetElem => (2, 1, Vec::new()),

            OpCode::BitNot | OpCode::Pos | OpCode::Neg | OpCode::Not | OpCode::GetProp(_) => {
                (1, 1, Vec::new())
            }

            OpCode::SetProp(_) => (2, 1, Vec::new()),
            OpCode::SetElem => (3, 1, Vec::new()),

            OpCode::CreateArray(n) => (*n as usize, 1, Vec::new()),
            OpCode::Call(argc) | OpCode::Construct(argc) => (*argc as usize + 1, 1, Vec::new()),
            OpCode::CallWithThis(argc) => (*argc as usize + 2, 1, Vec::new()),

            OpCode::Jump(t) => (0, 0, vec![*t as usize]),
            OpCode::JumpIfFalse(t) | OpCode::JumpIfTrue(t) => (1, 0, vec![*t as usize, ip + 1]),
            OpCode::Return | OpCode::Halt => (cur_depth.min(1), 0, Vec::new()),
            _ => (0, 0, Vec::new()),
        };

        let next_depth = cur_depth.saturating_sub(pops) + pushes;

        if !branches.is_empty() {
            for target in branches {
                if target <= num_insts && depths[target].is_none() {
                    depths[target] = Some(next_depth);
                    worklist.push_back(target);
                }
            }
        } else if !matches!(op, OpCode::Return | OpCode::Halt) && ip + 1 <= num_insts {
            if depths[ip + 1].is_none() {
                depths[ip + 1] = Some(next_depth);
                worklist.push_back(ip + 1);
            }
        }
    }

    depths.into_iter().map(|d| d.unwrap_or(0)).collect()
}

fn compile_native_internal(
    chunk: &BytecodeChunk,
    osr_target: Option<usize>,
) -> Option<JitFunction> {
    if let Some(target) = osr_target {
        if !can_compile_native_osr(chunk, target) {
            return None;
        }
    } else if !can_compile_native(chunk) {
        return None;
    }

    let mut flag_builder = cranelift_codegen::settings::builder();
    flag_builder.set("opt_level", "speed").ok()?;
    let isa_builder = cranelift_native::builder().ok()?;
    let isa = isa_builder.finish(cranelift_codegen::settings::Flags::new(flag_builder)).ok()?;
    let mut builder_obj = JITBuilder::with_isa(isa, cranelift_module::default_libcall_names());
    builder_obj.symbol("jit_strict_eq", jit_strict_eq as *const u8);
    builder_obj.symbol("jit_strict_neq", jit_strict_neq as *const u8);
    builder_obj.symbol("jit_get_prop_monomorphic", jit_get_prop_monomorphic as *const u8);
    builder_obj.symbol("jit_set_prop_monomorphic", jit_set_prop_monomorphic as *const u8);
    builder_obj.symbol("jit_get_elem", jit_get_elem as *const u8);
    builder_obj.symbol("jit_get_elem_dense", jit_get_elem_dense as *const u8);
    builder_obj.symbol("jit_set_elem", jit_set_elem as *const u8);
    builder_obj.symbol("jit_set_elem_dense", jit_set_elem_dense as *const u8);
    builder_obj.symbol("jit_alloc_array", jit_alloc_array as *const u8);
    builder_obj.symbol("jit_alloc_array_with_elements", jit_alloc_array_with_elements as *const u8);
    builder_obj.symbol("jit_alloc_object", jit_alloc_object as *const u8);
    builder_obj.symbol("jit_mod", jit_mod as *const u8);
    builder_obj.symbol("jit_call_trampoline", jit_call_trampoline as *const u8);
    builder_obj.symbol("jit_construct_trampoline", jit_construct_trampoline as *const u8);
    builder_obj.symbol("jit_array_length", jit_array_length as *const u8);
    builder_obj.symbol("jit_array_push_fast", jit_array_push_fast as *const u8);
    builder_obj.symbol("jit_get_global", jit_get_global as *const u8);
    builder_obj.symbol("jit_get_global_safe", jit_get_global_safe as *const u8);
    builder_obj.symbol("jit_set_global", jit_set_global as *const u8);
    builder_obj.symbol("jit_osr_yield", jit_osr_yield as *const u8);
    let mut module = JITModule::new(builder_obj);

    let mut strict_eq_sig = module.make_signature();
    strict_eq_sig.params.push(AbiParam::new(types::I64));
    strict_eq_sig.params.push(AbiParam::new(types::I64));
    strict_eq_sig.returns.push(AbiParam::new(types::I64));
    let strict_eq_func_id =
        module.declare_function("jit_strict_eq", Linkage::Import, &strict_eq_sig).ok()?;

    let mut strict_neq_sig = module.make_signature();
    strict_neq_sig.params.push(AbiParam::new(types::I64));
    strict_neq_sig.params.push(AbiParam::new(types::I64));
    strict_neq_sig.returns.push(AbiParam::new(types::I64));
    let strict_neq_func_id =
        module.declare_function("jit_strict_neq", Linkage::Import, &strict_neq_sig).ok()?;

    let mut get_prop_sig = module.make_signature();
    get_prop_sig.params.push(AbiParam::new(types::I64));
    get_prop_sig.params.push(AbiParam::new(types::I32));
    get_prop_sig.params.push(AbiParam::new(types::I32));
    get_prop_sig.returns.push(AbiParam::new(types::I64));
    let get_prop_func_id =
        module.declare_function("jit_get_prop_monomorphic", Linkage::Import, &get_prop_sig).ok()?;

    let mut set_prop_sig = module.make_signature();
    set_prop_sig.params.push(AbiParam::new(types::I64));
    set_prop_sig.params.push(AbiParam::new(types::I64));
    set_prop_sig.params.push(AbiParam::new(types::I32));
    set_prop_sig.params.push(AbiParam::new(types::I32));
    set_prop_sig.returns.push(AbiParam::new(types::I64));
    let set_prop_func_id =
        module.declare_function("jit_set_prop_monomorphic", Linkage::Import, &set_prop_sig).ok()?;

    let mut get_elem_sig = module.make_signature();
    get_elem_sig.params.push(AbiParam::new(types::I64));
    get_elem_sig.params.push(AbiParam::new(types::I64));
    get_elem_sig.returns.push(AbiParam::new(types::I64));
    let get_elem_func_id =
        module.declare_function("jit_get_elem", Linkage::Import, &get_elem_sig).ok()?;

    let mut get_elem_dense_sig = module.make_signature();
    get_elem_dense_sig.params.push(AbiParam::new(types::I64));
    get_elem_dense_sig.params.push(AbiParam::new(types::I32));
    get_elem_dense_sig.returns.push(AbiParam::new(types::I64));
    let get_elem_dense_func_id =
        module.declare_function("jit_get_elem_dense", Linkage::Import, &get_elem_dense_sig).ok()?;

    let mut set_elem_sig = module.make_signature();
    set_elem_sig.params.push(AbiParam::new(types::I64));
    set_elem_sig.params.push(AbiParam::new(types::I64));
    set_elem_sig.params.push(AbiParam::new(types::I64));
    set_elem_sig.returns.push(AbiParam::new(types::I64));
    let set_elem_func_id =
        module.declare_function("jit_set_elem", Linkage::Import, &set_elem_sig).ok()?;

    let mut set_elem_dense_sig = module.make_signature();
    set_elem_dense_sig.params.push(AbiParam::new(types::I64));
    set_elem_dense_sig.params.push(AbiParam::new(types::I32));
    set_elem_dense_sig.params.push(AbiParam::new(types::I64));
    set_elem_dense_sig.returns.push(AbiParam::new(types::I64));
    let set_elem_dense_func_id =
        module.declare_function("jit_set_elem_dense", Linkage::Import, &set_elem_dense_sig).ok()?;

    let mut alloc_array_sig = module.make_signature();
    alloc_array_sig.params.push(AbiParam::new(types::I64));
    alloc_array_sig.returns.push(AbiParam::new(types::I64));
    let alloc_array_func_id =
        module.declare_function("jit_alloc_array", Linkage::Import, &alloc_array_sig).ok()?;

    let mut alloc_array_with_elements_sig = module.make_signature();
    alloc_array_with_elements_sig.params.push(AbiParam::new(types::I64));
    alloc_array_with_elements_sig.params.push(AbiParam::new(types::I64));
    alloc_array_with_elements_sig.returns.push(AbiParam::new(types::I64));
    let alloc_array_with_elements_func_id = module
        .declare_function(
            "jit_alloc_array_with_elements",
            Linkage::Import,
            &alloc_array_with_elements_sig,
        )
        .ok()?;

    let mut alloc_object_sig = module.make_signature();
    alloc_object_sig.returns.push(AbiParam::new(types::I64));
    let alloc_object_func_id =
        module.declare_function("jit_alloc_object", Linkage::Import, &alloc_object_sig).ok()?;

    let mut mod_sig = module.make_signature();
    mod_sig.params.push(AbiParam::new(types::I64));
    mod_sig.params.push(AbiParam::new(types::I64));
    mod_sig.returns.push(AbiParam::new(types::I64));
    let mod_func_id = module.declare_function("jit_mod", Linkage::Import, &mod_sig).ok()?;

    let mut call_sig = module.make_signature();
    call_sig.params.push(AbiParam::new(types::I64));
    call_sig.params.push(AbiParam::new(types::I64));
    call_sig.params.push(AbiParam::new(types::I64));
    call_sig.params.push(AbiParam::new(types::I64));
    call_sig.returns.push(AbiParam::new(types::I64));
    let call_func_id =
        module.declare_function("jit_call_trampoline", Linkage::Import, &call_sig).ok()?;

    let mut construct_sig = module.make_signature();
    construct_sig.params.push(AbiParam::new(types::I64));
    construct_sig.params.push(AbiParam::new(types::I64));
    construct_sig.params.push(AbiParam::new(types::I64));
    construct_sig.returns.push(AbiParam::new(types::I64));
    let construct_func_id = module
        .declare_function("jit_construct_trampoline", Linkage::Import, &construct_sig)
        .ok()?;

    let mut array_length_sig = module.make_signature();
    array_length_sig.params.push(AbiParam::new(types::I64));
    array_length_sig.returns.push(AbiParam::new(types::I64));
    let array_length_func_id =
        module.declare_function("jit_array_length", Linkage::Import, &array_length_sig).ok()?;

    let mut array_push_fast_sig = module.make_signature();
    array_push_fast_sig.params.push(AbiParam::new(types::I64));
    array_push_fast_sig.params.push(AbiParam::new(types::I64));
    array_push_fast_sig.returns.push(AbiParam::new(types::I64));
    let array_push_fast_func_id = module
        .declare_function("jit_array_push_fast", Linkage::Import, &array_push_fast_sig)
        .ok()?;

    let mut get_global_sig = module.make_signature();
    get_global_sig.params.push(AbiParam::new(types::I64));
    get_global_sig.params.push(AbiParam::new(types::I64));
    get_global_sig.returns.push(AbiParam::new(types::I64));
    let get_global_func_id =
        module.declare_function("jit_get_global", Linkage::Import, &get_global_sig).ok()?;

    let mut get_global_safe_sig = module.make_signature();
    get_global_safe_sig.params.push(AbiParam::new(types::I64));
    get_global_safe_sig.params.push(AbiParam::new(types::I64));
    get_global_safe_sig.returns.push(AbiParam::new(types::I64));
    let get_global_safe_func_id = module
        .declare_function("jit_get_global_safe", Linkage::Import, &get_global_safe_sig)
        .ok()?;

    let mut set_global_sig = module.make_signature();
    set_global_sig.params.push(AbiParam::new(types::I64));
    set_global_sig.params.push(AbiParam::new(types::I64));
    set_global_sig.params.push(AbiParam::new(types::I64));
    set_global_sig.returns.push(AbiParam::new(types::I64));
    let set_global_func_id =
        module.declare_function("jit_set_global", Linkage::Import, &set_global_sig).ok()?;

    let mut osr_yield_sig = module.make_signature();
    osr_yield_sig.params.push(AbiParam::new(types::I32));
    osr_yield_sig.returns.push(AbiParam::new(types::I64));
    let osr_yield_func_id =
        module.declare_function("jit_osr_yield", Linkage::Import, &osr_yield_sig).ok()?;

    let mut sig = module.make_signature();
    sig.params.push(AbiParam::new(types::I64)); // args_ptr
    sig.params.push(AbiParam::new(types::I64)); // num_args
    sig.params.push(AbiParam::new(types::I64)); // this_val
    sig.returns.push(AbiParam::new(types::I64)); // return u64

    let self_func_id = module.declare_function("doms_jit_entry", Linkage::Export, &sig).ok()?;

    let mut ctx = module.make_context();
    ctx.func.signature = sig;
    let mut fn_builder_ctx = FunctionBuilderContext::new();
    let mut builder = FunctionBuilder::new(&mut ctx.func, &mut fn_builder_ctx);

    let self_func_ref = module.declare_func_in_func(self_func_id, &mut builder.func);
    let strict_eq_func_ref = module.declare_func_in_func(strict_eq_func_id, &mut builder.func);
    let strict_neq_func_ref = module.declare_func_in_func(strict_neq_func_id, &mut builder.func);
    let get_prop_func_ref = module.declare_func_in_func(get_prop_func_id, &mut builder.func);
    let set_prop_func_ref = module.declare_func_in_func(set_prop_func_id, &mut builder.func);
    let get_elem_func_ref = module.declare_func_in_func(get_elem_func_id, &mut builder.func);
    let get_elem_dense_func_ref =
        module.declare_func_in_func(get_elem_dense_func_id, &mut builder.func);
    let set_elem_func_ref = module.declare_func_in_func(set_elem_func_id, &mut builder.func);
    let set_elem_dense_func_ref =
        module.declare_func_in_func(set_elem_dense_func_id, &mut builder.func);
    let alloc_array_func_ref = module.declare_func_in_func(alloc_array_func_id, &mut builder.func);
    let alloc_array_with_elements_func_ref =
        module.declare_func_in_func(alloc_array_with_elements_func_id, &mut builder.func);
    let alloc_object_func_ref =
        module.declare_func_in_func(alloc_object_func_id, &mut builder.func);
    let mod_func_ref = module.declare_func_in_func(mod_func_id, &mut builder.func);
    let call_func_ref = module.declare_func_in_func(call_func_id, &mut builder.func);
    let construct_func_ref = module.declare_func_in_func(construct_func_id, &mut builder.func);
    let array_length_func_ref =
        module.declare_func_in_func(array_length_func_id, &mut builder.func);
    let array_push_fast_func_ref =
        module.declare_func_in_func(array_push_fast_func_id, &mut builder.func);
    let get_global_func_ref = module.declare_func_in_func(get_global_func_id, &mut builder.func);
    let get_global_safe_func_ref =
        module.declare_func_in_func(get_global_safe_func_id, &mut builder.func);
    let set_global_func_ref = module.declare_func_in_func(set_global_func_id, &mut builder.func);
    let osr_yield_func_ref = module.declare_func_in_func(osr_yield_func_id, &mut builder.func);

    // Find jump targets (basic block leaders)
    let num_insts = chunk.instructions.len();
    let mut is_leader = vec![false; num_insts + 1];
    is_leader[0] = true;

    if let Some(target) = osr_target {
        if target <= num_insts {
            is_leader[target] = true;
        }
    }

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
    if let Some(slot) = chunk.self_slot {
        max_local = max_local.max(slot + 1);
    }
    let num_locals = (max_local as usize).max(chunk.local_names.len()).max(1);

    // Entry block: function entry arguments setup
    let entry_block = builder.create_block();
    builder.append_block_params_for_function_params(entry_block);
    builder.switch_to_block(entry_block);

    let args_ptr = builder.block_params(entry_block)[0];
    let num_args = builder.block_params(entry_block)[1];
    let this_val = builder.block_params(entry_block)[2];

    let this_var = builder.declare_var(types::I64);
    builder.def_var(this_var, this_val);

    // Declare and initialize local variables from args_ptr
    let undef_val = builder.ins().iconst(types::I64, TAG_UNDEFINED as i64);
    let mut local_vars = Vec::with_capacity(num_locals);
    for i in 0..num_locals {
        let var = builder.declare_var(types::I64);
        local_vars.push(var);

        let offset = (i * 8) as i32;
        let idx_val = builder.ins().iconst(types::I64, i as i64);
        let has_arg = builder.ins().icmp(IntCC::UnsignedLessThan, idx_val, num_args);
        let is_self_slot = chunk.self_slot.map(|s| s as usize == i).unwrap_or(false);
        let has_val = if is_self_slot {
            let one = builder.ins().iconst(types::I32, 1);
            builder.ins().icmp_imm_s(IntCC::Equal, one, 1)
        } else {
            has_arg
        };
        let arg_val = builder.ins().load(types::I64, MemFlagsData::trusted(), args_ptr, offset);
        let init_val = builder.ins().select(has_val, arg_val, undef_val);
        builder.def_var(var, init_val);
    }

    // Declare stack variables
    let mut stack_vars = Vec::with_capacity(64);
    for _ in 0..64 {
        let var = builder.declare_var(types::I64);
        builder.def_var(var, undef_val);
        stack_vars.push(var);
    }

    // Declare call arguments stack slot for native function calls
    let call_args_slot = builder.create_sized_stack_slot(StackSlotData::new(
        StackSlotKind::ExplicitSlot,
        (16 * 8) as u32,
        3,
    ));
    let call_args_ptr = builder.ins().stack_addr(types::I64, call_args_slot, 0);

    // Jump to first instruction block or OSR loop header
    if let Some(target) = osr_target {
        let target_block = *blocks.get(&target)?;
        let block_zero = blocks[&0];
        let one = builder.ins().iconst(types::I32, 1);
        builder.ins().brif(one, target_block, &[], block_zero, &[]);
    } else {
        let block_zero = blocks[&0];
        builder.ins().jump(block_zero, &[]);
    }
    builder.seal_block(entry_block);

    let precomputed_depths = compute_stack_depths(chunk, osr_target);
    let mut stack_depth: usize = precomputed_depths[osr_target.unwrap_or(0)];
    let mut block_terminated = false;
    let bitcast_flags = MemFlagsData::new().with_endianness(Endianness::Little);

    for ip in 0..num_insts {
        if let Some(&blk) = blocks.get(&ip) {
            if ip > 0 && !block_terminated {
                builder.ins().jump(blk, &[]);
            }
            builder.switch_to_block(blk);
            block_terminated = false;
            stack_depth = precomputed_depths[ip];
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
                    Constant::String(s) => {
                        if s.len() <= 5 && s.is_ascii() {
                            JsValue::try_from_sso_string(s).unwrap_or_else(JsValue::undefined)
                        } else {
                            JsValue::undefined()
                        }
                    }
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
            OpCode::This => {
                let val = builder.use_var(this_var);
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
                    let offset = (slot as i32) * 8;
                    builder.ins().store(MemFlagsData::trusted(), val, args_ptr, offset);
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
            OpCode::Add | OpCode::Sub | OpCode::Mul | OpCode::Div | OpCode::Mod => {
                if stack_depth >= 2 {
                    stack_depth -= 2;
                    let lhs = builder.use_var(stack_vars[stack_depth]);
                    let rhs = builder.use_var(stack_vars[stack_depth + 1]);

                    let tag_mask = builder.ins().iconst(types::I64, TAG_MASK as i64);
                    let tag_int = builder.ins().iconst(types::I64, TAG_INT32 as i64);
                    let l_tag = builder.ins().band(lhs, tag_mask);
                    let r_tag = builder.ins().band(rhs, tag_mask);
                    let l_is_int = builder.ins().icmp(IntCC::Equal, l_tag, tag_int);
                    let r_is_int = builder.ins().icmp(IntCC::Equal, r_tag, tag_int);
                    let is_both_int = builder.ins().band(l_is_int, r_is_int);

                    let dbl_limit =
                        builder.ins().iconst(types::I64, crate::value::DOUBLE_LIMIT as i64);
                    let l_is_dbl = builder.ins().icmp(IntCC::UnsignedLessThan, lhs, dbl_limit);
                    let r_is_dbl = builder.ins().icmp(IntCC::UnsignedLessThan, rhs, dbl_limit);
                    let l_is_num = builder.ins().bor(l_is_int, l_is_dbl);
                    let r_is_num = builder.ins().bor(r_is_int, r_is_dbl);
                    let is_both_num = builder.ins().band(l_is_num, r_is_num);

                    let num_ok_block = builder.create_block();
                    let deopt_block = builder.create_block();
                    builder.ins().brif(is_both_num, num_ok_block, &[], deopt_block, &[]);

                    builder.switch_to_block(deopt_block);
                    let deopt_val = builder.ins().iconst(types::I64, JIT_DEOPT_SENTINEL as i64);
                    for i in 0..num_locals {
                        let v = builder.use_var(local_vars[i]);
                        builder.ins().store(MemFlagsData::trusted(), v, args_ptr, (i * 8) as i32);
                    }
                    builder.ins().return_(&[deopt_val]);
                    builder.seal_block(deopt_block);

                    builder.switch_to_block(num_ok_block);
                    builder.seal_block(num_ok_block);

                    let l32 = builder.ins().ireduce(types::I32, lhs);
                    let r32 = builder.ins().ireduce(types::I32, rhs);

                    let l64_s = builder.ins().sextend(types::I64, l32);
                    let r64_s = builder.ins().sextend(types::I64, r32);

                    let res = match op {
                        OpCode::Add | OpCode::Sub | OpCode::Mul => {
                            let (calc64_s, calc32) = match op {
                                OpCode::Add => {
                                    let s64 = builder.ins().iadd(l64_s, r64_s);
                                    let s32 = builder.ins().ireduce(types::I32, s64);
                                    (s64, s32)
                                }
                                OpCode::Sub => {
                                    let d64 = builder.ins().isub(l64_s, r64_s);
                                    let d32 = builder.ins().ireduce(types::I32, d64);
                                    (d64, d32)
                                }
                                OpCode::Mul => {
                                    let p64 = builder.ins().imul(l64_s, r64_s);
                                    let p32 = builder.ins().ireduce(types::I32, p64);
                                    (p64, p32)
                                }
                                _ => unreachable!(),
                            };
                            let calc32_reext = builder.ins().sextend(types::I64, calc32);
                            let no_overflow =
                                builder.ins().icmp(IntCC::Equal, calc64_s, calc32_reext);
                            let use_int = builder.ins().band(is_both_int, no_overflow);

                            let fast_int_block = builder.create_block();
                            let slow_dbl_block = builder.create_block();
                            let merge_block = builder.create_block();
                            let arith_res_var = builder.declare_var(types::I64);

                            builder.ins().brif(use_int, fast_int_block, &[], slow_dbl_block, &[]);

                            // Fast pure integer path: hardware integer ALU, 0 float conversions
                            builder.switch_to_block(fast_int_block);
                            let res64_u = builder.ins().uextend(types::I64, calc32);
                            let int_res = builder.ins().bor(tag_int, res64_u);
                            builder.def_var(arith_res_var, int_res);
                            builder.ins().jump(merge_block, &[]);
                            builder.seal_block(fast_int_block);

                            // Slow float path: handles doubles and integer overflows
                            builder.switch_to_block(slow_dbl_block);
                            let l_f64_from_int = builder.ins().fcvt_from_sint(types::F64, l32);
                            let l_f64_raw = builder.ins().bitcast(types::F64, bitcast_flags, lhs);
                            let l_f64 = builder.ins().select(l_is_int, l_f64_from_int, l_f64_raw);

                            let r_f64_from_int = builder.ins().fcvt_from_sint(types::F64, r32);
                            let r_f64_raw = builder.ins().bitcast(types::F64, bitcast_flags, rhs);
                            let r_f64 = builder.ins().select(r_is_int, r_f64_from_int, r_f64_raw);

                            let f64_res = match op {
                                OpCode::Add => builder.ins().fadd(l_f64, r_f64),
                                OpCode::Sub => builder.ins().fsub(l_f64, r_f64),
                                OpCode::Mul => builder.ins().fmul(l_f64, r_f64),
                                _ => unreachable!(),
                            };
                            let dbl_res = builder.ins().bitcast(types::I64, bitcast_flags, f64_res);
                            builder.def_var(arith_res_var, dbl_res);
                            builder.ins().jump(merge_block, &[]);
                            builder.seal_block(slow_dbl_block);

                            builder.switch_to_block(merge_block);
                            builder.seal_block(merge_block);
                            builder.use_var(arith_res_var)
                        }
                        OpCode::Div => {
                            let l_f64_from_int = builder.ins().fcvt_from_sint(types::F64, l32);
                            let l_f64_raw = builder.ins().bitcast(types::F64, bitcast_flags, lhs);
                            let l_f64 = builder.ins().select(l_is_int, l_f64_from_int, l_f64_raw);

                            let r_f64_from_int = builder.ins().fcvt_from_sint(types::F64, r32);
                            let r_f64_raw = builder.ins().bitcast(types::F64, bitcast_flags, rhs);
                            let r_f64 = builder.ins().select(r_is_int, r_f64_from_int, r_f64_raw);

                            let div_f64 = builder.ins().fdiv(l_f64, r_f64);
                            builder.ins().bitcast(types::I64, bitcast_flags, div_f64)
                        }
                        OpCode::Mod => {
                            let is_zero = builder.ins().icmp_imm_s(IntCC::Equal, r32, 0);
                            let is_min =
                                builder.ins().icmp_imm_s(IntCC::Equal, l32, i32::MIN as i64);
                            let is_neg1 = builder.ins().icmp_imm_s(IntCC::Equal, r32, -1);
                            let is_overflow = builder.ins().band(is_min, is_neg1);
                            let is_bad = builder.ins().bor(is_zero, is_overflow);
                            let not_bad = builder.ins().bnot(is_bad);
                            let can_int_rem = builder.ins().band(is_both_int, not_bad);

                            let mod_res_var = builder.declare_var(types::I64);
                            let int_block = builder.create_block();
                            let dbl_block = builder.create_block();
                            let merge_block = builder.create_block();

                            builder.ins().brif(can_int_rem, int_block, &[], dbl_block, &[]);

                            // int_block: hardware srem, zero C-ABI call overhead!
                            builder.switch_to_block(int_block);
                            let rem32 = builder.ins().srem(l32, r32);
                            let is_rem_zero = builder.ins().icmp_imm_s(IntCC::Equal, rem32, 0);
                            let is_l_neg = builder.ins().icmp_imm_s(IntCC::SignedLessThan, l32, 0);
                            let is_neg_zero = builder.ins().band(is_rem_zero, is_l_neg);
                            let neg_zero_f64 =
                                builder.ins().iconst(types::I64, (-0.0f64).to_bits() as i64);
                            let rem64 = builder.ins().uextend(types::I64, rem32);
                            let int_res = builder.ins().bor(tag_int, rem64);
                            let safe_int = builder.ins().select(is_neg_zero, neg_zero_f64, int_res);
                            builder.def_var(mod_res_var, safe_int);
                            builder.ins().jump(merge_block, &[]);
                            builder.seal_block(int_block);

                            // dbl_block: fallback C-ABI call for floats or edge cases
                            builder.switch_to_block(dbl_block);
                            let dbl_call = builder.ins().call(mod_func_ref, &[lhs, rhs]);
                            let dbl_res = builder.inst_results(dbl_call)[0];
                            builder.def_var(mod_res_var, dbl_res);
                            builder.ins().jump(merge_block, &[]);
                            builder.seal_block(dbl_block);

                            // merge_block
                            builder.switch_to_block(merge_block);
                            builder.seal_block(merge_block);
                            builder.use_var(mod_res_var)
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

                    let tag_mask = builder.ins().iconst(types::I64, TAG_MASK as i64);
                    let tag_int = builder.ins().iconst(types::I64, TAG_INT32 as i64);
                    let l_tag = builder.ins().band(lhs, tag_mask);
                    let r_tag = builder.ins().band(rhs, tag_mask);
                    let l_is_int = builder.ins().icmp(IntCC::Equal, l_tag, tag_int);
                    let r_is_int = builder.ins().icmp(IntCC::Equal, r_tag, tag_int);
                    let is_both_int = builder.ins().band(l_is_int, r_is_int);

                    let l32_raw = builder.ins().ireduce(types::I32, lhs);
                    let r32_raw = builder.ins().ireduce(types::I32, rhs);

                    let fast_block = builder.create_block();
                    let slow_block = builder.create_block();
                    let merge_block = builder.create_block();
                    let res_var = builder.declare_var(types::I64);

                    builder.ins().brif(is_both_int, fast_block, &[], slow_block, &[]);

                    // Fast pure integer path
                    builder.switch_to_block(fast_block);
                    let fast_res32 = match op {
                        OpCode::BitAnd => builder.ins().band(l32_raw, r32_raw),
                        OpCode::BitOr => builder.ins().bor(l32_raw, r32_raw),
                        OpCode::BitXor => builder.ins().bxor(l32_raw, r32_raw),
                        _ => unreachable!(),
                    };
                    let fast_res64 = builder.ins().uextend(types::I64, fast_res32);
                    let fast_res = builder.ins().bor(tag_int, fast_res64);
                    builder.def_var(res_var, fast_res);
                    builder.ins().jump(merge_block, &[]);
                    builder.seal_block(fast_block);

                    // Slow path with f64 bitcast and saturation
                    builder.switch_to_block(slow_block);
                    let l_f64_raw = builder.ins().bitcast(types::F64, bitcast_flags, lhs);
                    let l32_from_f64 = builder.ins().fcvt_to_sint_sat(types::I32, l_f64_raw);
                    let l32 = builder.ins().select(l_is_int, l32_raw, l32_from_f64);

                    let r_f64_raw = builder.ins().bitcast(types::F64, bitcast_flags, rhs);
                    let r32_from_f64 = builder.ins().fcvt_to_sint_sat(types::I32, r_f64_raw);
                    let r32 = builder.ins().select(r_is_int, r32_raw, r32_from_f64);

                    let slow_res32 = match op {
                        OpCode::BitAnd => builder.ins().band(l32, r32),
                        OpCode::BitOr => builder.ins().bor(l32, r32),
                        OpCode::BitXor => builder.ins().bxor(l32, r32),
                        _ => unreachable!(),
                    };
                    let slow_res64 = builder.ins().uextend(types::I64, slow_res32);
                    let slow_res = builder.ins().bor(tag_int, slow_res64);
                    builder.def_var(res_var, slow_res);
                    builder.ins().jump(merge_block, &[]);
                    builder.seal_block(slow_block);

                    builder.switch_to_block(merge_block);
                    builder.seal_block(merge_block);
                    let res = builder.use_var(res_var);
                    builder.def_var(stack_vars[stack_depth], res);
                    stack_depth += 1;
                }
            }
            OpCode::BitNot => {
                if stack_depth > 0 {
                    let val = builder.use_var(stack_vars[stack_depth - 1]);
                    let tag_mask = builder.ins().iconst(types::I64, TAG_MASK as i64);
                    let tag_int = builder.ins().iconst(types::I64, TAG_INT32 as i64);
                    let v_tag = builder.ins().band(val, tag_mask);
                    let v_is_int = builder.ins().icmp(IntCC::Equal, v_tag, tag_int);

                    let v32_raw = builder.ins().ireduce(types::I32, val);
                    let v_f64_raw = builder.ins().bitcast(types::F64, bitcast_flags, val);
                    let v32_from_f64 = builder.ins().fcvt_to_sint_sat(types::I32, v_f64_raw);
                    let v32 = builder.ins().select(v_is_int, v32_raw, v32_from_f64);

                    let not_v32 = builder.ins().bnot(v32);
                    let res64 = builder.ins().uextend(types::I64, not_v32);
                    let res = builder.ins().bor(tag_int, res64);
                    builder.def_var(stack_vars[stack_depth - 1], res);
                }
            }
            OpCode::Shl | OpCode::Shr | OpCode::Ushr => {
                if stack_depth >= 2 {
                    stack_depth -= 2;
                    let lhs = builder.use_var(stack_vars[stack_depth]);
                    let rhs = builder.use_var(stack_vars[stack_depth + 1]);

                    let tag_mask = builder.ins().iconst(types::I64, TAG_MASK as i64);
                    let tag_int = builder.ins().iconst(types::I64, TAG_INT32 as i64);
                    let l_tag = builder.ins().band(lhs, tag_mask);
                    let r_tag = builder.ins().band(rhs, tag_mask);
                    let l_is_int = builder.ins().icmp(IntCC::Equal, l_tag, tag_int);
                    let r_is_int = builder.ins().icmp(IntCC::Equal, r_tag, tag_int);
                    let is_both_int = builder.ins().band(l_is_int, r_is_int);

                    let l32_raw = builder.ins().ireduce(types::I32, lhs);
                    let r32_raw = builder.ins().ireduce(types::I32, rhs);

                    let fast_block = builder.create_block();
                    let slow_block = builder.create_block();
                    let merge_block = builder.create_block();
                    let res_var = builder.declare_var(types::I64);

                    builder.ins().brif(is_both_int, fast_block, &[], slow_block, &[]);

                    // Fast pure integer path
                    builder.switch_to_block(fast_block);
                    let shift_fast = builder.ins().band_imm_u(r32_raw, 31);
                    let fast_res32 = match op {
                        OpCode::Shl => builder.ins().ishl(l32_raw, shift_fast),
                        OpCode::Shr => builder.ins().sshr(l32_raw, shift_fast),
                        OpCode::Ushr => builder.ins().ushr(l32_raw, shift_fast),
                        _ => unreachable!(),
                    };
                    let fast_res64 = builder.ins().uextend(types::I64, fast_res32);
                    let fast_res = builder.ins().bor(tag_int, fast_res64);
                    builder.def_var(res_var, fast_res);
                    builder.ins().jump(merge_block, &[]);
                    builder.seal_block(fast_block);

                    // Slow path with f64 bitcast and saturation
                    builder.switch_to_block(slow_block);
                    let l_f64_raw = builder.ins().bitcast(types::F64, bitcast_flags, lhs);
                    let l32_from_f64 = builder.ins().fcvt_to_sint_sat(types::I32, l_f64_raw);
                    let l32 = builder.ins().select(l_is_int, l32_raw, l32_from_f64);

                    let r_f64_raw = builder.ins().bitcast(types::F64, bitcast_flags, rhs);
                    let r32_from_f64 = builder.ins().fcvt_to_sint_sat(types::I32, r_f64_raw);
                    let r32 = builder.ins().select(r_is_int, r32_raw, r32_from_f64);

                    let shift = builder.ins().band_imm_u(r32, 31);
                    let slow_res32 = match op {
                        OpCode::Shl => builder.ins().ishl(l32, shift),
                        OpCode::Shr => builder.ins().sshr(l32, shift),
                        OpCode::Ushr => builder.ins().ushr(l32, shift),
                        _ => unreachable!(),
                    };
                    let slow_res64 = builder.ins().uextend(types::I64, slow_res32);
                    let slow_res = builder.ins().bor(tag_int, slow_res64);
                    builder.def_var(res_var, slow_res);
                    builder.ins().jump(merge_block, &[]);
                    builder.seal_block(slow_block);

                    builder.switch_to_block(merge_block);
                    builder.seal_block(merge_block);
                    let res = builder.use_var(res_var);
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
                    let l_is_int = builder.ins().icmp(IntCC::Equal, l_tag, tag_int);
                    let r_is_int = builder.ins().icmp(IntCC::Equal, r_tag, tag_int);
                    let is_both_int = builder.ins().band(l_is_int, r_is_int);

                    let dbl_limit =
                        builder.ins().iconst(types::I64, crate::value::DOUBLE_LIMIT as i64);
                    let l_is_dbl = builder.ins().icmp(IntCC::UnsignedLessThan, lhs, dbl_limit);
                    let r_is_dbl = builder.ins().icmp(IntCC::UnsignedLessThan, rhs, dbl_limit);
                    let l_is_num = builder.ins().bor(l_is_int, l_is_dbl);
                    let r_is_num = builder.ins().bor(r_is_int, r_is_dbl);
                    let is_both_num = builder.ins().band(l_is_num, r_is_num);

                    let num_ok_block = builder.create_block();
                    let deopt_block = builder.create_block();
                    builder.ins().brif(is_both_num, num_ok_block, &[], deopt_block, &[]);

                    builder.switch_to_block(deopt_block);
                    let deopt_val = builder.ins().iconst(types::I64, JIT_DEOPT_SENTINEL as i64);
                    for i in 0..num_locals {
                        let v = builder.use_var(local_vars[i]);
                        builder.ins().store(MemFlagsData::trusted(), v, args_ptr, (i * 8) as i32);
                    }
                    builder.ins().return_(&[deopt_val]);
                    builder.seal_block(deopt_block);

                    builder.switch_to_block(num_ok_block);
                    builder.seal_block(num_ok_block);

                    let l32 = builder.ins().ireduce(types::I32, lhs);
                    let r32 = builder.ins().ireduce(types::I32, rhs);

                    let (int_cc, flt_cc) = match op {
                        OpCode::Lt => (IntCC::SignedLessThan, FloatCC::LessThan),
                        OpCode::Lte => (IntCC::SignedLessThanOrEqual, FloatCC::LessThanOrEqual),
                        OpCode::Gt => (IntCC::SignedGreaterThan, FloatCC::GreaterThan),
                        OpCode::Gte => {
                            (IntCC::SignedGreaterThanOrEqual, FloatCC::GreaterThanOrEqual)
                        }
                        _ => unreachable!(),
                    };

                    let fast_block = builder.create_block();
                    let slow_block = builder.create_block();
                    let merge_block = builder.create_block();
                    let cmp_var = builder.declare_var(types::I8);

                    builder.ins().brif(is_both_int, fast_block, &[], slow_block, &[]);

                    // Fast pure integer comparison
                    builder.switch_to_block(fast_block);
                    let cmp_int = builder.ins().icmp(int_cc, l32, r32);
                    builder.def_var(cmp_var, cmp_int);
                    builder.ins().jump(merge_block, &[]);
                    builder.seal_block(fast_block);

                    // Slow float comparison
                    builder.switch_to_block(slow_block);
                    let l_f64_from_int = builder.ins().fcvt_from_sint(types::F64, l32);
                    let l_f64_raw = builder.ins().bitcast(types::F64, bitcast_flags, lhs);
                    let l_f64 = builder.ins().select(l_is_int, l_f64_from_int, l_f64_raw);

                    let r_f64_from_int = builder.ins().fcvt_from_sint(types::F64, r32);
                    let r_f64_raw = builder.ins().bitcast(types::F64, bitcast_flags, rhs);
                    let r_f64 = builder.ins().select(r_is_int, r_f64_from_int, r_f64_raw);

                    let cmp_flt = builder.ins().fcmp(flt_cc, l_f64, r_f64);
                    builder.def_var(cmp_var, cmp_flt);
                    builder.ins().jump(merge_block, &[]);
                    builder.seal_block(slow_block);

                    builder.switch_to_block(merge_block);
                    builder.seal_block(merge_block);
                    let cmp = builder.use_var(cmp_var);
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
                    let is_zero_int =
                        builder.ins().icmp_imm_u(IntCC::Equal, cond, TAG_INT32 as i64);
                    let is_zero_f64 = builder.ins().icmp_imm_u(IntCC::Equal, cond, 0i64);
                    let is_null = builder.ins().icmp_imm_u(IntCC::Equal, cond, TAG_NULL as i64);
                    let is_undef =
                        builder.ins().icmp_imm_u(IntCC::Equal, cond, TAG_UNDEFINED as i64);
                    let f1 = builder.ins().bor(is_false, is_zero_int);
                    let f2 = builder.ins().bor(is_zero_f64, is_null);
                    let f3 = builder.ins().bor(f1, f2);
                    let is_falsy = builder.ins().bor(f3, is_undef);

                    let true_val =
                        builder.ins().iconst(types::I64, JsValue::from_boolean(true).0 as i64);
                    let false_val =
                        builder.ins().iconst(types::I64, JsValue::from_boolean(false).0 as i64);
                    let res = builder.ins().select(is_falsy, true_val, false_val);
                    builder.def_var(stack_vars[stack_depth - 1], res);
                }
            }
            OpCode::Jump(target) => {
                let t = target as usize;
                if let Some(loop_target) = osr_target {
                    if t < loop_target {
                        let t_val = builder.ins().iconst(types::I32, t as i64);
                        let _ = builder.ins().call(osr_yield_func_ref, &[t_val]);
                        let deopt_val = builder.ins().iconst(types::I64, JIT_DEOPT_SENTINEL as i64);
                        for i in 0..num_locals {
                            let v = builder.use_var(local_vars[i]);
                            builder.ins().store(
                                MemFlagsData::trusted(),
                                v,
                                args_ptr,
                                (i * 8) as i32,
                            );
                        }
                        builder.ins().return_(&[deopt_val]);
                        block_terminated = true;
                        continue;
                    }
                }
                let target_block = blocks[&t];
                builder.ins().jump(target_block, &[]);
                block_terminated = true;
            }
            OpCode::JumpIfFalse(target) => {
                if stack_depth > 0 {
                    stack_depth -= 1;
                    let cond = builder.use_var(stack_vars[stack_depth]);
                    let is_false = builder.ins().icmp_imm_u(IntCC::Equal, cond, TAG_BOOLEAN as i64);
                    let is_zero_int =
                        builder.ins().icmp_imm_u(IntCC::Equal, cond, TAG_INT32 as i64);
                    let is_zero_f64 = builder.ins().icmp_imm_u(IntCC::Equal, cond, 0i64);
                    let is_null = builder.ins().icmp_imm_u(IntCC::Equal, cond, TAG_NULL as i64);
                    let is_undef =
                        builder.ins().icmp_imm_u(IntCC::Equal, cond, TAG_UNDEFINED as i64);
                    let f1 = builder.ins().bor(is_false, is_zero_int);
                    let f2 = builder.ins().bor(is_zero_f64, is_null);
                    let f3 = builder.ins().bor(f1, f2);
                    let is_falsy = builder.ins().bor(f3, is_undef);

                    let t = target as usize;
                    let next_block = blocks[&(ip + 1)];
                    if let Some(loop_target) = osr_target {
                        if t < loop_target {
                            let yield_block = builder.create_block();
                            builder.ins().brif(is_falsy, yield_block, &[], next_block, &[]);

                            builder.switch_to_block(yield_block);
                            let t_val = builder.ins().iconst(types::I32, t as i64);
                            let _ = builder.ins().call(osr_yield_func_ref, &[t_val]);
                            let deopt_val =
                                builder.ins().iconst(types::I64, JIT_DEOPT_SENTINEL as i64);
                            for i in 0..num_locals {
                                let v = builder.use_var(local_vars[i]);
                                builder.ins().store(
                                    MemFlagsData::trusted(),
                                    v,
                                    args_ptr,
                                    (i * 8) as i32,
                                );
                            }
                            builder.ins().return_(&[deopt_val]);
                            builder.seal_block(yield_block);

                            block_terminated = true;
                            continue;
                        }
                    }

                    let target_block = blocks[&t];
                    builder.ins().brif(is_falsy, target_block, &[], next_block, &[]);
                    block_terminated = true;
                }
            }
            OpCode::JumpIfTrue(target) => {
                if stack_depth > 0 {
                    stack_depth -= 1;
                    let cond = builder.use_var(stack_vars[stack_depth]);
                    let is_false = builder.ins().icmp_imm_u(IntCC::Equal, cond, TAG_BOOLEAN as i64);
                    let is_zero_int =
                        builder.ins().icmp_imm_u(IntCC::Equal, cond, TAG_INT32 as i64);
                    let is_zero_f64 = builder.ins().icmp_imm_u(IntCC::Equal, cond, 0i64);
                    let is_null = builder.ins().icmp_imm_u(IntCC::Equal, cond, TAG_NULL as i64);
                    let is_undef =
                        builder.ins().icmp_imm_u(IntCC::Equal, cond, TAG_UNDEFINED as i64);
                    let f1 = builder.ins().bor(is_false, is_zero_int);
                    let f2 = builder.ins().bor(is_zero_f64, is_null);
                    let f3 = builder.ins().bor(f1, f2);
                    let is_falsy = builder.ins().bor(f3, is_undef);

                    let t = target as usize;
                    let next_block = blocks[&(ip + 1)];
                    if let Some(loop_target) = osr_target {
                        if t < loop_target {
                            let yield_block = builder.create_block();
                            builder.ins().brif(is_falsy, next_block, &[], yield_block, &[]);

                            builder.switch_to_block(yield_block);
                            let t_val = builder.ins().iconst(types::I32, t as i64);
                            let _ = builder.ins().call(osr_yield_func_ref, &[t_val]);
                            let deopt_val =
                                builder.ins().iconst(types::I64, JIT_DEOPT_SENTINEL as i64);
                            for i in 0..num_locals {
                                let v = builder.use_var(local_vars[i]);
                                builder.ins().store(
                                    MemFlagsData::trusted(),
                                    v,
                                    args_ptr,
                                    (i * 8) as i32,
                                );
                            }
                            builder.ins().return_(&[deopt_val]);
                            builder.seal_block(yield_block);

                            block_terminated = true;
                            continue;
                        }
                    }

                    let target_block = blocks[&t];
                    builder.ins().brif(is_falsy, next_block, &[], target_block, &[]);
                    block_terminated = true;
                }
            }
            OpCode::GetGlobal(name_idx) => {
                if let Some(Constant::String(s)) = chunk.constants.get(name_idx as usize) {
                    let name_ptr = builder.ins().iconst(types::I64, s.as_ptr() as usize as i64);
                    let name_len = builder.ins().iconst(types::I64, s.len() as i64);
                    let call = builder.ins().call(get_global_func_ref, &[name_ptr, name_len]);
                    let res = builder.inst_results(call)[0];

                    let deopt_val = builder.ins().iconst(types::I64, JIT_DEOPT_SENTINEL as i64);
                    let is_deopt = builder.ins().icmp(IntCC::Equal, res, deopt_val);

                    let ok_block = builder.create_block();
                    let deopt_block = builder.create_block();

                    builder.ins().brif(is_deopt, deopt_block, &[], ok_block, &[]);

                    builder.switch_to_block(deopt_block);
                    for i in 0..num_locals {
                        let v = builder.use_var(local_vars[i]);
                        builder.ins().store(MemFlagsData::trusted(), v, args_ptr, (i * 8) as i32);
                    }
                    builder.ins().return_(&[deopt_val]);
                    builder.seal_block(deopt_block);

                    builder.switch_to_block(ok_block);
                    builder.seal_block(ok_block);

                    builder.def_var(stack_vars[stack_depth], res);
                    stack_depth += 1;
                } else {
                    let deopt_val = builder.ins().iconst(types::I64, JIT_DEOPT_SENTINEL as i64);
                    for i in 0..num_locals {
                        let v = builder.use_var(local_vars[i]);
                        builder.ins().store(MemFlagsData::trusted(), v, args_ptr, (i * 8) as i32);
                    }
                    builder.ins().return_(&[deopt_val]);
                    block_terminated = true;
                }
            }
            OpCode::GetGlobalSafe(name_idx) => {
                if let Some(Constant::String(s)) = chunk.constants.get(name_idx as usize) {
                    let name_ptr = builder.ins().iconst(types::I64, s.as_ptr() as usize as i64);
                    let name_len = builder.ins().iconst(types::I64, s.len() as i64);
                    let call = builder.ins().call(get_global_safe_func_ref, &[name_ptr, name_len]);
                    let res = builder.inst_results(call)[0];
                    builder.def_var(stack_vars[stack_depth], res);
                    stack_depth += 1;
                } else {
                    let undef_val = builder.ins().iconst(types::I64, TAG_UNDEFINED as i64);
                    builder.def_var(stack_vars[stack_depth], undef_val);
                    stack_depth += 1;
                }
            }
            OpCode::SetGlobal(name_idx) | OpCode::SetGlobalDirect(name_idx) => {
                if stack_depth > 0 {
                    let val = builder.use_var(stack_vars[stack_depth - 1]);
                    if let Some(Constant::String(s)) = chunk.constants.get(name_idx as usize) {
                        let name_ptr = builder.ins().iconst(types::I64, s.as_ptr() as usize as i64);
                        let name_len = builder.ins().iconst(types::I64, s.len() as i64);
                        let call =
                            builder.ins().call(set_global_func_ref, &[name_ptr, name_len, val]);
                        let res = builder.inst_results(call)[0];

                        let deopt_val = builder.ins().iconst(types::I64, JIT_DEOPT_SENTINEL as i64);
                        let is_deopt = builder.ins().icmp(IntCC::Equal, res, deopt_val);

                        let ok_block = builder.create_block();
                        let deopt_block = builder.create_block();

                        builder.ins().brif(is_deopt, deopt_block, &[], ok_block, &[]);

                        builder.switch_to_block(deopt_block);
                        for i in 0..num_locals {
                            let v = builder.use_var(local_vars[i]);
                            builder.ins().store(
                                MemFlagsData::trusted(),
                                v,
                                args_ptr,
                                (i * 8) as i32,
                            );
                        }
                        builder.ins().return_(&[deopt_val]);
                        builder.seal_block(deopt_block);

                        builder.switch_to_block(ok_block);
                        builder.seal_block(ok_block);
                    }
                }
            }
            OpCode::GetProp(name_idx) => {
                if stack_depth > 0 {
                    stack_depth -= 1;
                    let obj = builder.use_var(stack_vars[stack_depth]);
                    let slot = chunk.get_feedback_slot(ip);
                    let prop_name = chunk.constants.get(name_idx as usize).and_then(|c| match c {
                        Constant::String(s) => Some(s.as_str()),
                        _ => None,
                    });
                    if matches!(slot, Some(FeedbackSlot::ArrayLength))
                        || prop_name == Some("length")
                    {
                        let call = builder.ins().call(array_length_func_ref, &[obj]);
                        let res = builder.inst_results(call)[0];

                        let deopt_val = builder.ins().iconst(types::I64, JIT_DEOPT_SENTINEL as i64);
                        let is_deopt = builder.ins().icmp(IntCC::Equal, res, deopt_val);

                        let ok_block = builder.create_block();
                        let deopt_block = builder.create_block();

                        builder.ins().brif(is_deopt, deopt_block, &[], ok_block, &[]);

                        builder.switch_to_block(deopt_block);
                        for i in 0..num_locals {
                            let v = builder.use_var(local_vars[i]);
                            builder.ins().store(
                                MemFlagsData::trusted(),
                                v,
                                args_ptr,
                                (i * 8) as i32,
                            );
                        }
                        builder.ins().return_(&[deopt_val]);
                        builder.seal_block(deopt_block);

                        builder.switch_to_block(ok_block);
                        builder.seal_block(ok_block);

                        builder.def_var(stack_vars[stack_depth], res);
                        stack_depth += 1;
                    } else if matches!(slot, Some(FeedbackSlot::ArrayPush))
                        || prop_name == Some("push")
                    {
                        let push_fn = builder.ins().iconst(types::I64, TAG_ARRAY_PUSH_FN as i64);
                        builder.def_var(stack_vars[stack_depth], push_fn);
                        stack_depth += 1;
                    } else {
                        let mono_buf;
                        let ic_entries: Option<&[(u32, u32)]> = match slot.as_ref() {
                            Some(&FeedbackSlot::Monomorphic { shape_id, offset })
                                if offset < crate::object::IN_OBJECT_SLOTS as u32 =>
                            {
                                mono_buf = [(shape_id, offset)];
                                Some(&mono_buf[..])
                            }
                            Some(FeedbackSlot::Polymorphic { entries })
                                if !entries.is_empty()
                                    && entries.len() <= 4
                                    && entries.iter().all(|(_, off)| {
                                        *off < crate::object::IN_OBJECT_SLOTS as u32
                                    }) =>
                            {
                                Some(entries.as_slice())
                            }
                            _ => None,
                        };
                        if let Some(entries) = ic_entries {
                            let deopt_block = builder.create_block();
                            let tag_check_ok = builder.create_block();
                            let ok_block = builder.create_block();

                            // 1. Tag check: is `obj` an object pointer?
                            let tag_mask = builder.ins().iconst(types::I64, TAG_MASK as i64);
                            let obj_tag = builder.ins().band(obj, tag_mask);
                            let tag_expected = builder.ins().iconst(types::I64, TAG_OBJECT as i64);
                            let is_obj = builder.ins().icmp(IntCC::Equal, obj_tag, tag_expected);
                            builder.ins().brif(is_obj, tag_check_ok, &[], deopt_block, &[]);

                            // 2. Shape check: load shape_id at offset 0 (4 bytes, types::I32)
                            builder.switch_to_block(tag_check_ok);
                            builder.seal_block(tag_check_ok);
                            let payload_mask =
                                builder.ins().iconst(types::I64, PAYLOAD_MASK as i64);
                            let ptr = builder.ins().band(obj, payload_mask);
                            let actual_shape =
                                builder.ins().load(types::I32, MemFlagsData::trusted(), ptr, 0);

                            let mut current_block = tag_check_ok;
                            for (i, &(shape_id, offset)) in entries.iter().enumerate() {
                                if i > 0 {
                                    builder.switch_to_block(current_block);
                                    builder.seal_block(current_block);
                                }
                                let hit_block = builder.create_block();
                                let next_block = if i + 1 < entries.len() {
                                    builder.create_block()
                                } else {
                                    deopt_block
                                };

                                let exp_shape = builder.ins().iconst(types::I32, shape_id as i64);
                                let is_match =
                                    builder.ins().icmp(IntCC::Equal, actual_shape, exp_shape);
                                builder.ins().brif(is_match, hit_block, &[], next_block, &[]);

                                // In hit_block: load slot and check for hole
                                builder.switch_to_block(hit_block);
                                builder.seal_block(hit_block);
                                let slot_offset = (8 + (offset * 8)) as i32;
                                let loaded_val = builder.ins().load(
                                    types::I64,
                                    MemFlagsData::trusted(),
                                    ptr,
                                    slot_offset,
                                );
                                let hole_val = builder.ins().iconst(types::I64, TAG_HOLE as i64);
                                let is_hole =
                                    builder.ins().icmp(IntCC::Equal, loaded_val, hole_val);
                                let not_hole = builder.create_block();
                                builder.ins().brif(is_hole, deopt_block, &[], not_hole, &[]);

                                builder.switch_to_block(not_hole);
                                builder.seal_block(not_hole);
                                builder.def_var(stack_vars[stack_depth], loaded_val);
                                builder.ins().jump(ok_block, &[]);

                                current_block = next_block;
                            }

                            // 3. Deopt block:
                            builder.switch_to_block(deopt_block);
                            for i in 0..num_locals {
                                let v = builder.use_var(local_vars[i]);
                                builder.ins().store(
                                    MemFlagsData::trusted(),
                                    v,
                                    args_ptr,
                                    (i * 8) as i32,
                                );
                            }
                            let deopt_val =
                                builder.ins().iconst(types::I64, JIT_DEOPT_SENTINEL as i64);
                            builder.ins().return_(&[deopt_val]);
                            builder.seal_block(deopt_block);

                            // 4. Success block:
                            builder.switch_to_block(ok_block);
                            builder.seal_block(ok_block);
                            stack_depth += 1;
                        } else {
                            let slot_info = match slot {
                                Some(FeedbackSlot::Monomorphic { shape_id, offset }) => {
                                    Some((shape_id, offset))
                                }
                                Some(FeedbackSlot::Polymorphic { entries })
                                    if !entries.is_empty() =>
                                {
                                    Some(entries.last().copied().unwrap())
                                }
                                _ => None,
                            };
                            if let Some((shape_id, offset)) = slot_info {
                                let s_val = builder.ins().iconst(types::I32, shape_id as i64);
                                let o_val = builder.ins().iconst(types::I32, offset as i64);
                                let call =
                                    builder.ins().call(get_prop_func_ref, &[obj, s_val, o_val]);
                                let res = builder.inst_results(call)[0];

                                let deopt_val =
                                    builder.ins().iconst(types::I64, JIT_DEOPT_SENTINEL as i64);
                                let is_deopt = builder.ins().icmp(IntCC::Equal, res, deopt_val);

                                let ok_block = builder.create_block();
                                let deopt_block = builder.create_block();

                                builder.ins().brif(is_deopt, deopt_block, &[], ok_block, &[]);

                                builder.switch_to_block(deopt_block);
                                for i in 0..num_locals {
                                    let v = builder.use_var(local_vars[i]);
                                    builder.ins().store(
                                        MemFlagsData::trusted(),
                                        v,
                                        args_ptr,
                                        (i * 8) as i32,
                                    );
                                }
                                builder.ins().return_(&[deopt_val]);
                                builder.seal_block(deopt_block);

                                builder.switch_to_block(ok_block);
                                builder.seal_block(ok_block);

                                builder.def_var(stack_vars[stack_depth], res);
                                stack_depth += 1;
                            } else {
                                let ok_block = builder.create_block();
                                let deopt_block = builder.create_block();
                                let deopt_val =
                                    builder.ins().iconst(types::I64, JIT_DEOPT_SENTINEL as i64);
                                let one = builder.ins().iconst(types::I32, 1);
                                builder.ins().brif(one, deopt_block, &[], ok_block, &[]);

                                builder.switch_to_block(deopt_block);
                                for i in 0..num_locals {
                                    let v = builder.use_var(local_vars[i]);
                                    builder.ins().store(
                                        MemFlagsData::trusted(),
                                        v,
                                        args_ptr,
                                        (i * 8) as i32,
                                    );
                                }
                                builder.ins().return_(&[deopt_val]);
                                builder.seal_block(deopt_block);

                                builder.switch_to_block(ok_block);
                                builder.seal_block(ok_block);

                                builder.def_var(stack_vars[stack_depth], deopt_val);
                                stack_depth += 1;
                            }
                        }
                    }
                }
            }
            OpCode::SetProp(_name_idx) => {
                if stack_depth >= 2 {
                    stack_depth -= 2;
                    let obj = builder.use_var(stack_vars[stack_depth]);
                    let val = builder.use_var(stack_vars[stack_depth + 1]);
                    let slot = chunk.get_feedback_slot(ip);
                    let mono_buf;
                    let ic_entries: Option<&[(u32, u32)]> = match slot.as_ref() {
                        Some(&FeedbackSlot::Monomorphic { shape_id, offset })
                            if offset < crate::object::IN_OBJECT_SLOTS as u32 =>
                        {
                            mono_buf = [(shape_id, offset)];
                            Some(&mono_buf[..])
                        }
                        Some(FeedbackSlot::Polymorphic { entries })
                            if !entries.is_empty()
                                && entries.len() <= 4
                                && entries.iter().all(|(_, off)| {
                                    *off < crate::object::IN_OBJECT_SLOTS as u32
                                }) =>
                        {
                            Some(entries.as_slice())
                        }
                        _ => None,
                    };
                    if let Some(entries) = ic_entries {
                        let deopt_block = builder.create_block();
                        let tag_check_ok = builder.create_block();
                        let ok_block = builder.create_block();

                        // 1. Tag check: is `obj` an object pointer?
                        let tag_mask = builder.ins().iconst(types::I64, TAG_MASK as i64);
                        let obj_tag = builder.ins().band(obj, tag_mask);
                        let tag_expected = builder.ins().iconst(types::I64, TAG_OBJECT as i64);
                        let is_obj = builder.ins().icmp(IntCC::Equal, obj_tag, tag_expected);
                        builder.ins().brif(is_obj, tag_check_ok, &[], deopt_block, &[]);

                        // 2. Shape check: load shape_id at offset 0 (4 bytes, types::I32)
                        builder.switch_to_block(tag_check_ok);
                        builder.seal_block(tag_check_ok);
                        let payload_mask = builder.ins().iconst(types::I64, PAYLOAD_MASK as i64);
                        let ptr = builder.ins().band(obj, payload_mask);
                        let actual_shape =
                            builder.ins().load(types::I32, MemFlagsData::trusted(), ptr, 0);

                        let mut current_block = tag_check_ok;
                        for (i, &(shape_id, offset)) in entries.iter().enumerate() {
                            if i > 0 {
                                builder.switch_to_block(current_block);
                                builder.seal_block(current_block);
                            }
                            let hit_block = builder.create_block();
                            let next_block = if i + 1 < entries.len() {
                                builder.create_block()
                            } else {
                                deopt_block
                            };

                            let exp_shape = builder.ins().iconst(types::I32, shape_id as i64);
                            let is_match =
                                builder.ins().icmp(IntCC::Equal, actual_shape, exp_shape);
                            builder.ins().brif(is_match, hit_block, &[], next_block, &[]);

                            // In hit_block: store slot
                            builder.switch_to_block(hit_block);
                            builder.seal_block(hit_block);
                            let slot_offset = (8 + (offset * 8)) as i32;
                            builder.ins().store(MemFlagsData::trusted(), val, ptr, slot_offset);
                            builder.ins().jump(ok_block, &[]);

                            current_block = next_block;
                        }

                        // 3. Deopt block:
                        builder.switch_to_block(deopt_block);
                        for i in 0..num_locals {
                            let v = builder.use_var(local_vars[i]);
                            builder.ins().store(
                                MemFlagsData::trusted(),
                                v,
                                args_ptr,
                                (i * 8) as i32,
                            );
                        }
                        let deopt_val = builder.ins().iconst(types::I64, JIT_DEOPT_SENTINEL as i64);
                        builder.ins().return_(&[deopt_val]);
                        builder.seal_block(deopt_block);

                        // 4. Success block:
                        builder.switch_to_block(ok_block);
                        builder.seal_block(ok_block);
                        builder.def_var(stack_vars[stack_depth], val);
                        stack_depth += 1;
                    } else {
                        let slot_info = match slot {
                            Some(FeedbackSlot::Monomorphic { shape_id, offset }) => {
                                Some((shape_id, offset))
                            }
                            Some(FeedbackSlot::Polymorphic { entries }) if !entries.is_empty() => {
                                Some(entries.last().copied().unwrap())
                            }
                            _ => None,
                        };
                        if let Some((shape_id, offset)) = slot_info {
                            let s_val = builder.ins().iconst(types::I32, shape_id as i64);
                            let o_val = builder.ins().iconst(types::I32, offset as i64);
                            let call =
                                builder.ins().call(set_prop_func_ref, &[obj, val, s_val, o_val]);
                            let res = builder.inst_results(call)[0];

                            let deopt_val =
                                builder.ins().iconst(types::I64, JIT_DEOPT_SENTINEL as i64);
                            let is_deopt = builder.ins().icmp(IntCC::Equal, res, deopt_val);

                            let ok_block = builder.create_block();
                            let deopt_block = builder.create_block();

                            builder.ins().brif(is_deopt, deopt_block, &[], ok_block, &[]);

                            builder.switch_to_block(deopt_block);
                            for i in 0..num_locals {
                                let v = builder.use_var(local_vars[i]);
                                builder.ins().store(
                                    MemFlagsData::trusted(),
                                    v,
                                    args_ptr,
                                    (i * 8) as i32,
                                );
                            }
                            builder.ins().return_(&[deopt_val]);
                            builder.seal_block(deopt_block);

                            builder.switch_to_block(ok_block);
                            builder.seal_block(ok_block);

                            builder.def_var(stack_vars[stack_depth], res);
                            stack_depth += 1;
                        } else {
                            let ok_block = builder.create_block();
                            let deopt_block = builder.create_block();
                            let deopt_val =
                                builder.ins().iconst(types::I64, JIT_DEOPT_SENTINEL as i64);
                            let one = builder.ins().iconst(types::I32, 1);
                            builder.ins().brif(one, deopt_block, &[], ok_block, &[]);

                            builder.switch_to_block(deopt_block);
                            for i in 0..num_locals {
                                let v = builder.use_var(local_vars[i]);
                                builder.ins().store(
                                    MemFlagsData::trusted(),
                                    v,
                                    args_ptr,
                                    (i * 8) as i32,
                                );
                            }
                            builder.ins().return_(&[deopt_val]);
                            builder.seal_block(deopt_block);

                            builder.switch_to_block(ok_block);
                            builder.seal_block(ok_block);

                            builder.def_var(stack_vars[stack_depth], deopt_val);
                            stack_depth += 1;
                        }
                    }
                }
            }
            OpCode::GetElem => {
                if stack_depth >= 2 {
                    stack_depth -= 2;
                    let obj = builder.use_var(stack_vars[stack_depth]);
                    let key = builder.use_var(stack_vars[stack_depth + 1]);

                    let tag_mask = builder.ins().iconst(types::I64, TAG_MASK as i64);
                    let tag_int = builder.ins().iconst(types::I64, TAG_INT32 as i64);
                    let k_tag = builder.ins().band(key, tag_mask);
                    let k_is_int = builder.ins().icmp(IntCC::Equal, k_tag, tag_int);
                    let k_i32 = builder.ins().ireduce(types::I32, key);
                    let k_non_neg =
                        builder.ins().icmp_imm_s(IntCC::SignedGreaterThanOrEqual, k_i32, 0);
                    let is_dense_key = builder.ins().band(k_is_int, k_non_neg);

                    let dense_block = builder.create_block();
                    let fallback_block = builder.create_block();
                    let merge_block = builder.create_block();

                    builder.ins().brif(is_dense_key, dense_block, &[], fallback_block, &[]);

                    // dense_block: fast path with raw index and buffer cache
                    builder.switch_to_block(dense_block);
                    let d_call = builder.ins().call(get_elem_dense_func_ref, &[obj, k_i32]);
                    let d_res = builder.inst_results(d_call)[0];
                    builder.def_var(stack_vars[stack_depth], d_res);
                    builder.ins().jump(merge_block, &[]);
                    builder.seal_block(dense_block);

                    // fallback_block: generic get_elem
                    builder.switch_to_block(fallback_block);
                    let f_call = builder.ins().call(get_elem_func_ref, &[obj, key]);
                    let f_res = builder.inst_results(f_call)[0];
                    builder.def_var(stack_vars[stack_depth], f_res);
                    builder.ins().jump(merge_block, &[]);
                    builder.seal_block(fallback_block);

                    // merge_block
                    builder.switch_to_block(merge_block);
                    builder.seal_block(merge_block);
                    let res = builder.use_var(stack_vars[stack_depth]);

                    let deopt_val = builder.ins().iconst(types::I64, JIT_DEOPT_SENTINEL as i64);
                    let is_deopt = builder.ins().icmp(IntCC::Equal, res, deopt_val);

                    let ok_block = builder.create_block();
                    let deopt_block = builder.create_block();

                    builder.ins().brif(is_deopt, deopt_block, &[], ok_block, &[]);

                    builder.switch_to_block(deopt_block);
                    builder.ins().return_(&[deopt_val]);
                    builder.seal_block(deopt_block);

                    builder.switch_to_block(ok_block);
                    builder.seal_block(ok_block);

                    builder.def_var(stack_vars[stack_depth], res);
                    stack_depth += 1;
                }
            }
            OpCode::SetElem => {
                if stack_depth >= 3 {
                    stack_depth -= 3;
                    let obj = builder.use_var(stack_vars[stack_depth]);
                    let key = builder.use_var(stack_vars[stack_depth + 1]);
                    let val = builder.use_var(stack_vars[stack_depth + 2]);

                    let tag_mask = builder.ins().iconst(types::I64, TAG_MASK as i64);
                    let tag_int = builder.ins().iconst(types::I64, TAG_INT32 as i64);
                    let k_tag = builder.ins().band(key, tag_mask);
                    let k_is_int = builder.ins().icmp(IntCC::Equal, k_tag, tag_int);
                    let k_i32 = builder.ins().ireduce(types::I32, key);
                    let k_non_neg =
                        builder.ins().icmp_imm_s(IntCC::SignedGreaterThanOrEqual, k_i32, 0);
                    let is_dense_key = builder.ins().band(k_is_int, k_non_neg);

                    let dense_block = builder.create_block();
                    let fallback_block = builder.create_block();
                    let merge_block = builder.create_block();

                    builder.ins().brif(is_dense_key, dense_block, &[], fallback_block, &[]);

                    // dense_block
                    builder.switch_to_block(dense_block);
                    let d_call = builder.ins().call(set_elem_dense_func_ref, &[obj, k_i32, val]);
                    let d_res = builder.inst_results(d_call)[0];
                    builder.def_var(stack_vars[stack_depth], d_res);
                    builder.ins().jump(merge_block, &[]);
                    builder.seal_block(dense_block);

                    // fallback_block
                    builder.switch_to_block(fallback_block);
                    let f_call = builder.ins().call(set_elem_func_ref, &[obj, key, val]);
                    let f_res = builder.inst_results(f_call)[0];
                    builder.def_var(stack_vars[stack_depth], f_res);
                    builder.ins().jump(merge_block, &[]);
                    builder.seal_block(fallback_block);

                    // merge_block
                    builder.switch_to_block(merge_block);
                    builder.seal_block(merge_block);
                    let res = builder.use_var(stack_vars[stack_depth]);

                    let deopt_val = builder.ins().iconst(types::I64, JIT_DEOPT_SENTINEL as i64);
                    let is_deopt = builder.ins().icmp(IntCC::Equal, res, deopt_val);

                    let ok_block = builder.create_block();
                    let deopt_block = builder.create_block();

                    builder.ins().brif(is_deopt, deopt_block, &[], ok_block, &[]);

                    builder.switch_to_block(deopt_block);
                    builder.ins().return_(&[deopt_val]);
                    builder.seal_block(deopt_block);

                    builder.switch_to_block(ok_block);
                    builder.seal_block(ok_block);

                    builder.def_var(stack_vars[stack_depth], res);
                    stack_depth += 1;
                }
            }
            OpCode::CreateObject => {
                let call = builder.ins().call(alloc_object_func_ref, &[]);
                let res = builder.inst_results(call)[0];
                builder.def_var(stack_vars[stack_depth], res);
                stack_depth += 1;
            }
            OpCode::CreateArray(count) => {
                let c = count as usize;
                if stack_depth >= c && c <= 16 {
                    let res = if c == 0 {
                        let count_val = builder.ins().iconst(types::I64, 0);
                        let call = builder.ins().call(alloc_array_func_ref, &[count_val]);
                        builder.inst_results(call)[0]
                    } else {
                        for i in 0..c {
                            let arg_val = builder.use_var(stack_vars[stack_depth - c + i]);
                            builder.ins().store(
                                MemFlagsData::trusted(),
                                arg_val,
                                call_args_ptr,
                                (i * 8) as i32,
                            );
                        }
                        stack_depth -= c;
                        let count_val = builder.ins().iconst(types::I64, c as i64);
                        let call = builder
                            .ins()
                            .call(alloc_array_with_elements_func_ref, &[call_args_ptr, count_val]);
                        builder.inst_results(call)[0]
                    };
                    builder.def_var(stack_vars[stack_depth], res);
                    stack_depth += 1;
                }
            }
            OpCode::Call(argc) => {
                let argc_usize = argc as usize;
                if stack_depth >= argc_usize + 1 && argc_usize <= 16 {
                    for i in 0..argc_usize {
                        let arg_val = builder.use_var(stack_vars[stack_depth - argc_usize + i]);
                        builder.ins().store(
                            MemFlagsData::trusted(),
                            arg_val,
                            call_args_ptr,
                            (i * 8) as i32,
                        );
                    }
                    let callee_val = builder.use_var(stack_vars[stack_depth - argc_usize - 1]);
                    stack_depth -= argc_usize + 1;

                    let undef_this = builder.ins().iconst(types::I64, TAG_UNDEFINED as i64);
                    let argc_val = builder.ins().iconst(types::I64, argc_usize as i64);
                    let deopt_val = builder.ins().iconst(types::I64, JIT_DEOPT_SENTINEL as i64);

                    let ok_block = builder.create_block();
                    let deopt_block = builder.create_block();
                    let res_var = builder.declare_var(types::I64);

                    if let Some(self_slot_idx) = chunk.self_slot {
                        let self_val = builder.use_var(local_vars[self_slot_idx as usize]);
                        let is_self = builder.ins().icmp(IntCC::Equal, callee_val, self_val);

                        let self_call_block = builder.create_block();
                        let general_call_block = builder.create_block();

                        builder.ins().brif(is_self, self_call_block, &[], general_call_block, &[]);

                        // Direct native self recursive call
                        builder.switch_to_block(self_call_block);
                        let self_offset = (self_slot_idx as i32) * 8;
                        builder.ins().store(
                            MemFlagsData::trusted(),
                            self_val,
                            call_args_ptr,
                            self_offset,
                        );

                        let self_call_inst = builder
                            .ins()
                            .call(self_func_ref, &[call_args_ptr, argc_val, undef_this]);
                        let self_res = builder.inst_results(self_call_inst)[0];
                        let is_deopt_self = builder.ins().icmp(IntCC::Equal, self_res, deopt_val);

                        let self_ok_block = builder.create_block();
                        builder.ins().brif(is_deopt_self, deopt_block, &[], self_ok_block, &[]);

                        builder.switch_to_block(self_ok_block);
                        builder.def_var(res_var, self_res);
                        builder.ins().jump(ok_block, &[]);
                        builder.seal_block(self_call_block);
                        builder.seal_block(self_ok_block);

                        // General trampoline call
                        builder.switch_to_block(general_call_block);
                        let call_inst = builder.ins().call(
                            call_func_ref,
                            &[callee_val, undef_this, call_args_ptr, argc_val],
                        );
                        let call_res = builder.inst_results(call_inst)[0];
                        let is_deopt_gen = builder.ins().icmp(IntCC::Equal, call_res, deopt_val);

                        let gen_ok_block = builder.create_block();
                        builder.ins().brif(is_deopt_gen, deopt_block, &[], gen_ok_block, &[]);

                        builder.switch_to_block(gen_ok_block);
                        builder.def_var(res_var, call_res);
                        builder.ins().jump(ok_block, &[]);
                        builder.seal_block(general_call_block);
                        builder.seal_block(gen_ok_block);
                    } else {
                        let call_inst = builder.ins().call(
                            call_func_ref,
                            &[callee_val, undef_this, call_args_ptr, argc_val],
                        );
                        let call_res = builder.inst_results(call_inst)[0];
                        let is_deopt = builder.ins().icmp(IntCC::Equal, call_res, deopt_val);

                        let gen_ok_block = builder.create_block();
                        builder.ins().brif(is_deopt, deopt_block, &[], gen_ok_block, &[]);

                        builder.switch_to_block(gen_ok_block);
                        builder.def_var(res_var, call_res);
                        builder.ins().jump(ok_block, &[]);
                        builder.seal_block(gen_ok_block);
                    }

                    builder.switch_to_block(deopt_block);
                    for i in 0..num_locals {
                        let v = builder.use_var(local_vars[i]);
                        builder.ins().store(MemFlagsData::trusted(), v, args_ptr, (i * 8) as i32);
                    }
                    builder.ins().return_(&[deopt_val]);
                    builder.seal_block(deopt_block);

                    builder.switch_to_block(ok_block);
                    builder.seal_block(ok_block);

                    let final_res = builder.use_var(res_var);
                    builder.def_var(stack_vars[stack_depth], final_res);
                    stack_depth += 1;
                }
            }
            OpCode::CallWithThis(argc) => {
                let argc_usize = argc as usize;
                if stack_depth >= argc_usize + 2 && argc_usize <= 16 {
                    for i in 0..argc_usize {
                        let arg_val = builder.use_var(stack_vars[stack_depth - argc_usize + i]);
                        builder.ins().store(
                            MemFlagsData::trusted(),
                            arg_val,
                            call_args_ptr,
                            (i * 8) as i32,
                        );
                    }
                    let callee_val = builder.use_var(stack_vars[stack_depth - argc_usize - 1]);
                    let this_val = builder.use_var(stack_vars[stack_depth - argc_usize - 2]);
                    stack_depth -= argc_usize + 2;

                    let call_res = if argc_usize >= 1 {
                        let push_tag = builder.ins().iconst(types::I64, TAG_ARRAY_PUSH_FN as i64);
                        let is_push = builder.ins().icmp(IntCC::Equal, callee_val, push_tag);

                        let push_block = builder.create_block();
                        let call_block = builder.create_block();
                        let merge_block = builder.create_block();
                        let res_var = builder.declare_var(types::I64);

                        builder.ins().brif(is_push, push_block, &[], call_block, &[]);

                        // Inlined push path
                        builder.switch_to_block(push_block);
                        let mut push_res = this_val;
                        for i in 0..argc_usize {
                            let arg_i = builder.ins().load(
                                types::I64,
                                MemFlagsData::trusted(),
                                call_args_ptr,
                                (i * 8) as i32,
                            );
                            let push_call =
                                builder.ins().call(array_push_fast_func_ref, &[this_val, arg_i]);
                            push_res = builder.inst_results(push_call)[0];
                        }
                        builder.def_var(res_var, push_res);
                        builder.ins().jump(merge_block, &[]);
                        builder.seal_block(push_block);

                        // General call path
                        builder.switch_to_block(call_block);
                        let argc_val = builder.ins().iconst(types::I64, argc_usize as i64);
                        let call_inst = builder
                            .ins()
                            .call(call_func_ref, &[callee_val, this_val, call_args_ptr, argc_val]);
                        let normal_res = builder.inst_results(call_inst)[0];
                        builder.def_var(res_var, normal_res);
                        builder.ins().jump(merge_block, &[]);
                        builder.seal_block(call_block);

                        // Merge block
                        builder.switch_to_block(merge_block);
                        let final_res = builder.use_var(res_var);
                        builder.seal_block(merge_block);
                        final_res
                    } else {
                        let argc_val = builder.ins().iconst(types::I64, argc_usize as i64);
                        let call_inst = builder
                            .ins()
                            .call(call_func_ref, &[callee_val, this_val, call_args_ptr, argc_val]);
                        builder.inst_results(call_inst)[0]
                    };

                    let deopt_val = builder.ins().iconst(types::I64, JIT_DEOPT_SENTINEL as i64);
                    let is_deopt = builder.ins().icmp(IntCC::Equal, call_res, deopt_val);

                    let ok_block = builder.create_block();
                    let deopt_block = builder.create_block();

                    builder.ins().brif(is_deopt, deopt_block, &[], ok_block, &[]);

                    builder.switch_to_block(deopt_block);
                    for i in 0..num_locals {
                        let v = builder.use_var(local_vars[i]);
                        builder.ins().store(MemFlagsData::trusted(), v, args_ptr, (i * 8) as i32);
                    }
                    builder.ins().return_(&[deopt_val]);
                    builder.seal_block(deopt_block);

                    builder.switch_to_block(ok_block);
                    builder.seal_block(ok_block);

                    builder.def_var(stack_vars[stack_depth], call_res);
                    stack_depth += 1;
                }
            }
            OpCode::Construct(argc) => {
                let argc_usize = argc as usize;
                if stack_depth >= argc_usize + 1 && argc_usize <= 16 {
                    for i in 0..argc_usize {
                        let arg_val = builder.use_var(stack_vars[stack_depth - argc_usize + i]);
                        builder.ins().store(
                            MemFlagsData::trusted(),
                            arg_val,
                            call_args_ptr,
                            (i * 8) as i32,
                        );
                    }
                    let callee_val = builder.use_var(stack_vars[stack_depth - argc_usize - 1]);
                    stack_depth -= argc_usize + 1;

                    let argc_val = builder.ins().iconst(types::I64, argc_usize as i64);
                    let call_inst = builder
                        .ins()
                        .call(construct_func_ref, &[callee_val, call_args_ptr, argc_val]);
                    let call_res = builder.inst_results(call_inst)[0];

                    let deopt_val = builder.ins().iconst(types::I64, JIT_DEOPT_SENTINEL as i64);
                    let is_deopt = builder.ins().icmp(IntCC::Equal, call_res, deopt_val);

                    let ok_block = builder.create_block();
                    let deopt_block = builder.create_block();

                    builder.ins().brif(is_deopt, deopt_block, &[], ok_block, &[]);

                    builder.switch_to_block(deopt_block);
                    builder.ins().return_(&[deopt_val]);
                    builder.seal_block(deopt_block);

                    builder.switch_to_block(ok_block);
                    builder.seal_block(ok_block);

                    builder.def_var(stack_vars[stack_depth], call_res);
                    stack_depth += 1;
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
            _ => {
                if let Some(target) = osr_target {
                    if ip >= target {
                        let deopt_val = builder.ins().iconst(types::I64, JIT_DEOPT_SENTINEL as i64);
                        builder.ins().return_(&[deopt_val]);
                        block_terminated = true;
                    }
                }
            }
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

    module.define_function(self_func_id, &mut ctx).ok()?;
    module.clear_context(&mut ctx);
    module.finalize_definitions().ok()?;

    let code_ptr = module.get_finalized_function(self_func_id);
    let native_fn: NativeJitFn = unsafe { std::mem::transmute(code_ptr) };
    let module_arc = Arc::new(std::sync::Mutex::new(module));
    let chunk_clone = Arc::new(chunk.clone());
    let executable: JitCallable = Arc::new(
        move |vm: &mut Vm, this_val: JsValue, args: &[JsValue]| -> Result<JsValue, VmError> {
            let frame_chunk = Arc::clone(&chunk_clone);
            let target_depth = vm.frames.len();
            let base_sp = vm.stack.len();

            for arg in args {
                vm.push(*arg);
            }

            let local_names = Arc::clone(&frame_chunk.local_names);
            let lexical_local_names = Arc::clone(&frame_chunk.lexical_local_names);

            vm.frames.push(crate::interpreter::CallFrame {
                chunk: frame_chunk,
                ip: 0,
                base_sp,
                this_val,
                gen_val: None,
                locals: args.to_vec(),
                local_names,
                lexical_local_names,
                scopes: Vec::new(),
                upvalues_count: 0,
                env_obj: None,
                arguments: None,
                raw_args: Some(args.to_vec()),
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
        },
    );

    Some(JitFunction {
        name: None,
        hotness: JIT_HOT_THRESHOLD,
        specialized_type: SpeculativeType::Integer,
        code: JitCode::Native { func_ptr: native_fn, num_locals, _module: module_arc },
        executable,
    })
}
