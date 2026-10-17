//! Baseline JIT Compiler and Speculative Optimization Engine.
//!
//! Implements Tier-1 baseline execution with Cranelift native code generation
//! and speculative specialization for high-throughput JavaScript code execution.

pub mod builder;
pub mod compiler;
pub mod types;

pub use builder::{can_compile_native, compile_native};
pub use compiler::JitCompiler;
pub use types::*;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interpreter::Vm;
    use crate::value::JsValue;
    use doms_compiler::bytecode::{BytecodeChunk, Constant, OpCode};

    #[test]
    fn test_jit_compiler_compilation() {
        let mut chunk = BytecodeChunk::new();
        chunk.emit(OpCode::LoadTrue, 1);
        chunk.emit(OpCode::Return, 1);

        let jit_fn = JitCompiler::compile(&chunk);
        assert!(jit_fn.is_some());
        let jit_fn = jit_fn.unwrap();
        assert_eq!(jit_fn.hotness, JIT_HOT_THRESHOLD);
        assert!(jit_fn.is_native());
    }

    #[test]
    fn test_cranelift_native_arithmetic_execution() {
        // Compile: function(a, b) { return a + b * 2; }
        let mut chunk = BytecodeChunk::new();
        let c2 = chunk.add_constant(Constant::Number(2.0));
        chunk.emit(OpCode::GetLocal(0), 1);
        chunk.emit(OpCode::GetLocal(1), 1);
        chunk.emit(OpCode::LoadConst(c2), 1);
        chunk.emit(OpCode::Mul, 1);
        chunk.emit(OpCode::Add, 1);
        chunk.emit(OpCode::Return, 1);

        let jit_fn = JitCompiler::compile_native(&chunk).expect("Expected native compilation");
        assert!(jit_fn.is_native());

        let mut vm = Vm::new();
        let a = JsValue::from_int32(10);
        let b = JsValue::from_int32(5);
        let res = jit_fn.call(&mut vm, JsValue::undefined(), &[a, b]).unwrap();
        assert_eq!(res.as_number(), Some(20.0));
    }

    #[test]
    fn test_cranelift_native_loop_execution() {
        let mut chunk = BytecodeChunk::new();
        let c0 = chunk.add_constant(Constant::Number(0.0));
        let c1 = chunk.add_constant(Constant::Number(1.0));

        // 0: sum = 0 (local 1)
        chunk.emit(OpCode::LoadConst(c0), 1);
        chunk.emit(OpCode::SetLocal(1), 1);
        chunk.emit(OpCode::Pop, 1);

        // 3: i = 0 (local 2)
        chunk.emit(OpCode::LoadConst(c0), 1);
        chunk.emit(OpCode::SetLocal(2), 1);
        chunk.emit(OpCode::Pop, 1);

        // 6: loop head: i < n (n is local 0)
        chunk.emit(OpCode::GetLocal(2), 1);
        chunk.emit(OpCode::GetLocal(0), 1);
        chunk.emit(OpCode::Lt, 1);
        // 9: JumpIfFalse -> 21 (exit)
        chunk.emit(OpCode::JumpIfFalse(21), 1);

        // 10: sum = sum + i
        chunk.emit(OpCode::GetLocal(1), 1);
        chunk.emit(OpCode::GetLocal(2), 1);
        chunk.emit(OpCode::Add, 1);
        chunk.emit(OpCode::SetLocal(1), 1);
        chunk.emit(OpCode::Pop, 1);

        // 15: i = i + 1
        chunk.emit(OpCode::GetLocal(2), 1);
        chunk.emit(OpCode::LoadConst(c1), 1);
        chunk.emit(OpCode::Add, 1);
        chunk.emit(OpCode::SetLocal(2), 1);
        chunk.emit(OpCode::Pop, 1);

        // 20: Jump back to loop head (6)
        chunk.emit(OpCode::Jump(6), 1);

        // 21: return sum
        chunk.emit(OpCode::GetLocal(1), 1);
        chunk.emit(OpCode::Return, 1);

        let jit_fn = JitCompiler::compile_native(&chunk).expect("Expected native loop compilation");
        assert!(jit_fn.is_native());

        let mut vm = Vm::new();
        let n = JsValue::from_int32(10);
        let res = jit_fn.call(&mut vm, JsValue::undefined(), &[n]).unwrap();
        // 0 + 1 + 2 + ... + 9 = 45
        assert_eq!(res.as_number(), Some(45.0));
    }

    #[test]
    fn test_cranelift_native_strict_equality() {
        let mut chunk = BytecodeChunk::new();
        chunk.emit(OpCode::GetLocal(0), 1);
        chunk.emit(OpCode::GetLocal(1), 1);
        chunk.emit(OpCode::StrictEq, 1);
        chunk.emit(OpCode::Return, 1);

        let jit_fn = JitCompiler::compile_native(&chunk).expect("Expected native strict eq compilation");
        assert!(jit_fn.is_native());

        let mut vm = Vm::new();
        // Numbers
        let r1 = jit_fn.call(&mut vm, JsValue::undefined(), &[JsValue::from_int32(42), JsValue::from_int32(42)]).unwrap();
        assert_eq!(r1.as_boolean(), Some(true));

        let r2 = jit_fn.call(&mut vm, JsValue::undefined(), &[JsValue::from_int32(42), JsValue::from_int32(43)]).unwrap();
        assert_eq!(r2.as_boolean(), Some(false));

        // Strings with distinct allocations
        let s1 = vm.alloc_string("hello".to_string());
        let s2 = vm.alloc_string("hello".to_string());
        assert_ne!(s1.0, s2.0); // Distinct heap pointers
        let r3 = jit_fn.call(&mut vm, JsValue::undefined(), &[s1, s2]).unwrap();
        assert_eq!(r3.as_boolean(), Some(true));

        let s3 = vm.alloc_string("world".to_string());
        let r4 = jit_fn.call(&mut vm, JsValue::undefined(), &[s1, s3]).unwrap();
        assert_eq!(r4.as_boolean(), Some(false));

        // NaN === NaN is false
        let nan_val = JsValue::from_double(f64::NAN);
        let r5 = jit_fn.call(&mut vm, JsValue::undefined(), &[nan_val, nan_val]).unwrap();
        assert_eq!(r5.as_boolean(), Some(false));
    }
}
