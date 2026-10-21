//! Baseline JIT Compiler and Speculative Optimization Engine.
//!
//! Implements Tier-1 baseline execution with Cranelift native code generation
//! and speculative specialization for high-throughput JavaScript code execution.

pub mod builder;
pub mod compiler;
pub mod types;

pub use builder::{can_compile_native, can_compile_native_osr, compile_native, compile_native_osr};
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

        let jit_fn =
            JitCompiler::compile_native(&chunk).expect("Expected native strict eq compilation");
        assert!(jit_fn.is_native());

        let mut vm = Vm::new();
        // Numbers
        let r1 = jit_fn
            .call(
                &mut vm,
                JsValue::undefined(),
                &[JsValue::from_int32(42), JsValue::from_int32(42)],
            )
            .unwrap();
        assert_eq!(r1.as_boolean(), Some(true));

        let r2 = jit_fn
            .call(
                &mut vm,
                JsValue::undefined(),
                &[JsValue::from_int32(42), JsValue::from_int32(43)],
            )
            .unwrap();
        assert_eq!(r2.as_boolean(), Some(false));

        // SSO strings: identical representation
        let s1 = vm.alloc_string("hello".to_string());
        let s2 = vm.alloc_string("hello".to_string());
        assert_eq!(s1.0, s2.0); // Identical NaN-boxed SSO bits
        let r3 = jit_fn.call(&mut vm, JsValue::undefined(), &[s1, s2]).unwrap();
        assert_eq!(r3.as_boolean(), Some(true));

        // Distinct heap allocations (> 5 bytes)
        let h1 = vm.alloc_string("hello world".to_string());
        let h2 = vm.alloc_string("hello world".to_string());
        assert_ne!(h1.0, h2.0); // Distinct heap pointers
        let r_heap = jit_fn.call(&mut vm, JsValue::undefined(), &[h1, h2]).unwrap();
        assert_eq!(r_heap.as_boolean(), Some(true));

        let s3 = vm.alloc_string("world".to_string());
        let r4 = jit_fn.call(&mut vm, JsValue::undefined(), &[s1, s3]).unwrap();
        assert_eq!(r4.as_boolean(), Some(false));

        // NaN === NaN is false
        let nan_val = JsValue::from_double(f64::NAN);
        let r5 = jit_fn.call(&mut vm, JsValue::undefined(), &[nan_val, nan_val]).unwrap();
        assert_eq!(r5.as_boolean(), Some(false));
    }

    #[test]
    fn test_monomorphic_shape_guard_and_deopt_bailout() {
        let mut vm = Vm::new();
        let obj1 = vm.alloc_object();
        vm.set_property(obj1, "x", JsValue::from_int32(100));

        let ptr1 = obj1.as_object_ptr().unwrap();
        let jsobj1 = unsafe { &*(ptr1 as *const crate::object::JsObject) };
        let shape1 = jsobj1.shape_id;
        let offset1 = vm.shapes.lookup_offset(shape1, "x").unwrap();

        // Bytecode function: function(o) { return o.x; }
        let mut chunk = BytecodeChunk::new();
        let c_x = chunk.add_constant(Constant::String("x".to_string()));
        chunk.emit(OpCode::GetLocal(0), 1);
        chunk.emit(OpCode::GetProp(c_x), 1);
        chunk.emit(OpCode::Return, 1);

        // Record monomorphic feedback at GetProp instruction (ip = 1)
        chunk.record_feedback_shape(1, shape1, offset1);

        let jit_fn =
            JitCompiler::compile_native(&chunk).expect("Expected native compile with shape guard");
        assert!(jit_fn.is_native());

        // Call with matching shape -> runs native fast-path
        let res1 = jit_fn.call(&mut vm, JsValue::undefined(), &[obj1]).unwrap();
        assert_eq!(res1.as_number(), Some(100.0));

        // Create obj2 with DIFFERENT shape: { y: 200, x: 300 }
        let obj2 = vm.alloc_object();
        vm.set_property(obj2, "y", JsValue::from_int32(200));
        vm.set_property(obj2, "x", JsValue::from_int32(300));

        // Calling with obj2 fails the monomorphic shape guard and safely deopts to interpreted fallback!
        let res2 = jit_fn.call(&mut vm, JsValue::undefined(), &[obj2]).unwrap();
        assert_eq!(res2.as_number(), Some(300.0));
    }

    #[test]
    fn test_jit_monomorphic_set_prop_and_get_prop_direct_lowering() {
        let mut vm = Vm::new();
        let obj = vm.alloc_object();
        vm.set_property(obj, "val", JsValue::from_int32(42));

        let ptr = obj.as_object_ptr().unwrap();
        let jsobj = unsafe { &*(ptr as *const crate::object::JsObject) };
        let shape = jsobj.shape_id;
        let offset = vm.shapes.lookup_offset(shape, "val").unwrap();

        // Bytecode function: function(o, v) { o.val = v; return o.val; }
        let mut chunk = BytecodeChunk::new();
        let c_val = chunk.add_constant(Constant::String("val".to_string()));
        chunk.emit(OpCode::GetLocal(0), 1);
        chunk.emit(OpCode::GetLocal(1), 1);
        chunk.emit(OpCode::SetProp(c_val), 1); // ip = 2
        chunk.emit(OpCode::Pop, 1);
        chunk.emit(OpCode::GetLocal(0), 1);
        chunk.emit(OpCode::GetProp(c_val), 1); // ip = 5
        chunk.emit(OpCode::Return, 1);

        // Record monomorphic feedback for SetProp (ip = 2) and GetProp (ip = 5)
        chunk.record_feedback_shape(2, shape, offset);
        chunk.record_feedback_shape(5, shape, offset);

        let jit_fn = JitCompiler::compile_native(&chunk)
            .expect("Expected native compile with direct IR lowered SetProp/GetProp");
        assert!(jit_fn.is_native());

        // Execute natively: updates obj.val to 999 and returns 999
        let res =
            jit_fn.call(&mut vm, JsValue::undefined(), &[obj, JsValue::from_int32(999)]).unwrap();
        assert_eq!(res.as_number(), Some(999.0));

        // Verify the object's in-object slot was actually updated directly in native IR
        let jsobj_after = unsafe { &*(ptr as *const crate::object::JsObject) };
        assert_eq!(jsobj_after.get_slot(offset).unwrap().as_number(), Some(999.0));
    }

    #[test]
    fn test_jit_polymorphic_inline_cache_direct_ir_lowering() {
        let mut vm = Vm::new();

        // 3 objects with distinct shapes:
        // obj_a: { x: 10, a: 1 } (x at offset 0)
        let obj_a = vm.alloc_object();
        vm.set_property(obj_a, "x", JsValue::from_int32(10));
        vm.set_property(obj_a, "a", JsValue::from_int32(1));
        let shape_a = unsafe {
            (*(obj_a.as_object_ptr().unwrap() as *const crate::object::JsObject)).shape_id
        };
        let offset_a = vm.shapes.lookup_offset(shape_a, "x").unwrap();

        // obj_b: { y: 2, x: 20 } (x at offset 1)
        let obj_b = vm.alloc_object();
        vm.set_property(obj_b, "y", JsValue::from_int32(2));
        vm.set_property(obj_b, "x", JsValue::from_int32(20));
        let shape_b = unsafe {
            (*(obj_b.as_object_ptr().unwrap() as *const crate::object::JsObject)).shape_id
        };
        let offset_b = vm.shapes.lookup_offset(shape_b, "x").unwrap();

        // obj_c: { z: 3, w: 4, x: 30 } (x at offset 2)
        let obj_c = vm.alloc_object();
        vm.set_property(obj_c, "z", JsValue::from_int32(3));
        vm.set_property(obj_c, "w", JsValue::from_int32(4));
        vm.set_property(obj_c, "x", JsValue::from_int32(30));
        let shape_c = unsafe {
            (*(obj_c.as_object_ptr().unwrap() as *const crate::object::JsObject)).shape_id
        };
        let offset_c = vm.shapes.lookup_offset(shape_c, "x").unwrap();

        // Bytecode function: function(o) { return o.x; }
        let mut chunk = BytecodeChunk::new();
        let c_x = chunk.add_constant(Constant::String("x".to_string()));
        chunk.emit(OpCode::GetLocal(0), 1);
        chunk.emit(OpCode::GetProp(c_x), 1); // ip = 1
        chunk.emit(OpCode::Return, 1);

        // Record polymorphic feedback at GetProp instruction (ip = 1) for all 3 shapes
        chunk.record_feedback_shape(1, shape_a, offset_a);
        chunk.record_feedback_shape(1, shape_b, offset_b);
        chunk.record_feedback_shape(1, shape_c, offset_c);

        let jit_fn = JitCompiler::compile_native(&chunk)
            .expect("Expected native compile with Polymorphic Inline Cache direct IR lowering");
        assert!(jit_fn.is_native());

        // Test all 3 polymorphic shapes execute directly in Cranelift IR!
        let res_a = jit_fn.call(&mut vm, JsValue::undefined(), &[obj_a]).unwrap();
        assert_eq!(res_a.as_number(), Some(10.0));

        let res_b = jit_fn.call(&mut vm, JsValue::undefined(), &[obj_b]).unwrap();
        assert_eq!(res_b.as_number(), Some(20.0));

        let res_c = jit_fn.call(&mut vm, JsValue::undefined(), &[obj_c]).unwrap();
        assert_eq!(res_c.as_number(), Some(30.0));

        // Unknown 4th shape: { m: 9, n: 8, p: 7, x: 40 } -> safely deopts and resumes
        let obj_d = vm.alloc_object();
        vm.set_property(obj_d, "m", JsValue::from_int32(9));
        vm.set_property(obj_d, "n", JsValue::from_int32(8));
        vm.set_property(obj_d, "p", JsValue::from_int32(7));
        vm.set_property(obj_d, "x", JsValue::from_int32(40));

        let res_d = jit_fn.call(&mut vm, JsValue::undefined(), &[obj_d]).unwrap();
        assert_eq!(res_d.as_number(), Some(40.0));
    }

    #[test]
    fn test_jit_dense_array_access() {
        let mut vm = Vm::new();
        let arr = vm.alloc_array(vec![
            JsValue::from_int32(10),
            JsValue::from_int32(20),
            JsValue::from_int32(30),
        ]);

        // function(a, i, v) { a[i] = v; return a[i]; }
        let mut chunk = BytecodeChunk::new();
        chunk.emit(OpCode::GetLocal(0), 1);
        chunk.emit(OpCode::GetLocal(1), 1);
        chunk.emit(OpCode::GetLocal(2), 1);
        chunk.emit(OpCode::SetElem, 1);
        chunk.emit(OpCode::Pop, 1);
        chunk.emit(OpCode::GetLocal(0), 1);
        chunk.emit(OpCode::GetLocal(1), 1);
        chunk.emit(OpCode::GetElem, 1);
        chunk.emit(OpCode::Return, 1);

        let jit_fn =
            JitCompiler::compile_native(&chunk).expect("Expected native array get/set compile");
        assert!(jit_fn.is_native());

        let res = jit_fn
            .call(
                &mut vm,
                JsValue::undefined(),
                &[arr, JsValue::from_int32(1), JsValue::from_int32(99)],
            )
            .unwrap();
        assert_eq!(res.as_number(), Some(99.0));

        // Verify array was actually mutated in memory
        let elem1 = vm.get_property(arr, "1");
        assert_eq!(elem1.as_number(), Some(99.0));
    }

    #[test]
    fn test_on_stack_replacement_loop_execution() {
        let mut vm = Vm::new();
        let mut chunk = BytecodeChunk::new();
        let c0 = chunk.add_constant(Constant::Number(0.0));
        let c1 = chunk.add_constant(Constant::Number(1.0));
        let c100 = chunk.add_constant(Constant::Number(100.0));

        // 0: sum = 0 (local 0)
        chunk.emit(OpCode::LoadConst(c0), 1);
        chunk.emit(OpCode::SetLocal(0), 1);
        chunk.emit(OpCode::Pop, 1);

        // 3: i = 0 (local 1)
        chunk.emit(OpCode::LoadConst(c0), 1);
        chunk.emit(OpCode::SetLocal(1), 1);
        chunk.emit(OpCode::Pop, 1);

        // 6: loop header: i < 100
        chunk.emit(OpCode::GetLocal(1), 1);
        chunk.emit(OpCode::LoadConst(c100), 1);
        chunk.emit(OpCode::Lt, 1);
        // 9: JumpIfFalse -> 21
        chunk.emit(OpCode::JumpIfFalse(21), 1);

        // 10: sum = sum + i
        chunk.emit(OpCode::GetLocal(0), 1);
        chunk.emit(OpCode::GetLocal(1), 1);
        chunk.emit(OpCode::Add, 1);
        chunk.emit(OpCode::SetLocal(0), 1);
        chunk.emit(OpCode::Pop, 1);

        // 15: i = i + 1
        chunk.emit(OpCode::GetLocal(1), 1);
        chunk.emit(OpCode::LoadConst(c1), 1);
        chunk.emit(OpCode::Add, 1);
        chunk.emit(OpCode::SetLocal(1), 1);
        chunk.emit(OpCode::Pop, 1);

        // 20: Jump -> 6 (backward jump)
        chunk.emit(OpCode::Jump(6), 1);

        // 21: return sum
        chunk.emit(OpCode::GetLocal(0), 1);
        chunk.emit(OpCode::Return, 1);

        // Run through vm.run!
        let res = vm.run(&chunk).unwrap();
        // 0 + 1 + ... + 99 = 4950
        assert_eq!(res.as_number(), Some(4950.0));

        // Verify OSR was compiled and cached for this loop target (target 6)
        assert!(vm.osr_cache.contains_key(&(chunk.id, 6)));
    }

    #[test]
    fn test_benchmark_array_manipulation_osr() {
        let code = std::fs::read_to_string("../../tests/benchmarks/array-manipulation.js")
            .or_else(|_| std::fs::read_to_string("tests/benchmarks/array-manipulation.js"))
            .unwrap();
        let program = doms_parser::parse(&code).unwrap();
        let chunk = doms_compiler::compile(&program).unwrap();
        let mut vm = Vm::new();
        let res = vm.run(&chunk).unwrap();
        assert_eq!(res.as_number(), Some(599990000.0));
    }

    #[test]
    fn test_cranelift_native_mod_and_call() {
        let mut vm = Vm::new();

        // 1. Test native modulo: function(a, b) { return a % b; }
        let mut mod_chunk = BytecodeChunk::new();
        mod_chunk.emit(OpCode::GetLocal(0), 1);
        mod_chunk.emit(OpCode::GetLocal(1), 1);
        mod_chunk.emit(OpCode::Mod, 1);
        mod_chunk.emit(OpCode::Return, 1);

        let jit_mod =
            JitCompiler::compile_native(&mod_chunk).expect("Expected native compile for mod");
        assert!(jit_mod.is_native());

        let res = jit_mod
            .call(&mut vm, JsValue::undefined(), &[JsValue::from_int32(10), JsValue::from_int32(3)])
            .unwrap();
        assert_eq!(res.as_int32(), Some(1));

        let res2 = jit_mod
            .call(&mut vm, JsValue::undefined(), &[JsValue::from_int32(14), JsValue::from_int32(5)])
            .unwrap();
        assert_eq!(res2.as_int32(), Some(4));

        // 2. Test native call trampoline: function(f, x, y) { return f(x, y); }
        let mut call_chunk = BytecodeChunk::new();
        call_chunk.emit(OpCode::GetLocal(0), 1); // callee f
        call_chunk.emit(OpCode::GetLocal(1), 1); // arg1 x
        call_chunk.emit(OpCode::GetLocal(2), 1); // arg2 y
        call_chunk.emit(OpCode::Call(2), 1);
        call_chunk.emit(OpCode::Return, 1);

        let jit_caller =
            JitCompiler::compile_native(&call_chunk).expect("Expected native compile for call");
        assert!(jit_caller.is_native());

        // Create JS function for mod_chunk
        let callee_fn = vm.alloc_function(crate::object::JsFunction {
            name: Some("mod_fn".into()),
            params: vec!["a".into(), "b".into()],
            length: 2,
            self_slot: None,
            local_names: std::sync::Arc::new(Vec::new()),
            lexical_local_names: std::sync::Arc::new(Vec::new()),
            chunk: std::sync::Arc::new(mod_chunk),
            upvalues: Vec::new(),
            is_async: false,
            is_generator: false,
            call_count: 0,
            jit_entry: Some(jit_mod),
            jit_attempted: true,
            is_strict: false,
            has_param_exprs: false,
            is_method: false,
            is_arrow: false,
            home_object: None,
            lexical_this: None,
            lexical_new_target: None,
            prototype: None,
            initial_shape: None,
            is_class_constructor: false,
            is_derived_constructor: false,
        });

        let call_res = jit_caller
            .call(
                &mut vm,
                JsValue::undefined(),
                &[callee_fn, JsValue::from_int32(23), JsValue::from_int32(7)],
            )
            .unwrap();
        assert_eq!(call_res.as_int32(), Some(2));
    }

    #[test]
    fn test_cranelift_native_construct_and_object_access() {
        let mut vm = Vm::new();
        let res = vm
            .eval(
                r#"
            function Point(x, y) {
                this.x = x;
                this.y = y;
            }
            let sum = 0;
            let i = 0;
            while (i < 100) {
                let p = new Point(i, i * 2);
                sum = sum + p.x + p.y;
                i = i + 1;
            }
            sum
        "#,
            )
            .unwrap();
        assert_eq!(res.as_number(), Some(14850.0));
    }

    #[test]
    fn test_cranelift_native_direct_self_recursion() {
        let mut vm = Vm::new();
        let res = vm
            .eval(
                r#"
            function fib(n) {
                if (n <= 1) return n;
                return fib(n - 1) + fib(n - 2);
            }
            fib(20)
        "#,
            )
            .unwrap();
        assert_eq!(res.as_int32().or(res.as_number().map(|n| n as i32)), Some(6765));
    }

    #[test]
    fn test_cranelift_typed_array_indexed_access() {
        let mut vm = Vm::new();
        let res = vm
            .eval(
                r#"
            let arr = new Float64Array(100);
            for (let i = 0; i < 100; i = i + 1) {
                arr[i] = i * 2.5;
            }
            let sum = 0;
            for (let i = 0; i < 100; i = i + 1) {
                sum = sum + arr[i];
            }
            sum
        "#,
            )
            .unwrap();
        assert_eq!(res.as_number(), Some(12375.0));
    }

    #[test]
    fn test_in_object_property_storage_8_slots() {
        let mut vm = Vm::new();
        let obj = vm.alloc_object();
        for i in 0..8 {
            vm.set_property(obj, &format!("prop{}", i), JsValue::from_int32(i * 10));
        }
        let jsobj = unsafe { &*(obj.as_object_ptr().unwrap() as *const crate::object::JsObject) };
        // All 8 properties reside directly in in_object_slots with zero extra_slots allocation!
        assert_eq!(jsobj.extra_slots.len(), 0);
        for i in 0..8 {
            assert_eq!(jsobj.in_object_slots[i].as_int32(), Some((i * 10) as i32));
        }
        // 9th property spills to extra_slots
        vm.set_property(obj, "prop8", JsValue::from_int32(80));
        let jsobj = unsafe { &*(obj.as_object_ptr().unwrap() as *const crate::object::JsObject) };
        assert_eq!(jsobj.extra_slots.len(), 1);
        assert_eq!(jsobj.extra_slots[0].as_int32(), Some(80));
    }
}
