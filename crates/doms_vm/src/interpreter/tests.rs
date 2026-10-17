//! Unit tests for the interpreter.

use super::*;


#[test]
fn test_eval_simple_bytecode() {
    let mut chunk = BytecodeChunk::new();
    let c1 = chunk.add_constant(Constant::Number(10.0));
    let c2 = chunk.add_constant(Constant::Number(32.0));
    chunk.emit(OpCode::LoadConst(c1), 1);
    chunk.emit(OpCode::LoadConst(c2), 1);
    chunk.emit(OpCode::Add, 1);
    chunk.emit(OpCode::Halt, 1);

    let mut vm = Vm::new();
    let res = vm.run(&chunk).unwrap();
    assert_eq!(res.as_number(), Some(42.0));
}

#[test]
fn test_eval_object_and_properties() {
    let mut chunk = BytecodeChunk::new();
    let name_prop = chunk.add_constant(Constant::String("x".into()));
    let val_const = chunk.add_constant(Constant::Number(99.0));

    chunk.emit(OpCode::CreateObject, 1);
    chunk.emit(OpCode::Dup, 1);
    chunk.emit(OpCode::LoadConst(val_const), 1);
    chunk.emit(OpCode::SetProp(name_prop), 1);
    chunk.emit(OpCode::Pop, 1);
    chunk.emit(OpCode::GetProp(name_prop), 1);
    chunk.emit(OpCode::Halt, 1);

    let mut vm = Vm::new();
    let res = vm.run(&chunk).unwrap();
    assert_eq!(res.as_number(), Some(99.0));
}

#[test]
fn test_phase3_builtins_and_collections() {
    let mut vm = Vm::new();

    // Map test
    let map_val = vm.alloc_map();
    let set_method = vm.get_property(map_val, "set");
    let get_method = vm.get_property(map_val, "get");
    let k = vm.alloc_string("hello".into());
    let v = JsValue::from_int32(123);
    vm.call(set_method, map_val, &[k, v]).unwrap();
    let k_lookup = vm.alloc_string("hello".into());
    let got = vm.call(get_method, map_val, &[k_lookup]).unwrap();
    assert_eq!(got.as_number(), Some(123.0));

    // Set test
    let set_val = vm.alloc_set();
    let add_method = vm.get_property(set_val, "add");
    let has_method = vm.get_property(set_val, "has");
    let num = JsValue::from_int32(77);
    vm.call(add_method, set_val, &[num]).unwrap();
    let has_res = vm.call(has_method, set_val, &[num]).unwrap();
    assert_eq!(has_res.as_boolean(), Some(true));

    // WeakRef test
    let obj = vm.alloc_object();
    let wr = vm.alloc_weak_ref(obj);
    let deref_fn = vm.get_property(wr, "deref");
    let derefed = vm.call(deref_fn, wr, &[]).unwrap();
    assert_eq!(derefed, obj);

    // RegExp test
    let re = vm.alloc_regexp("doms".into(), "".into());
    let test_fn = vm.get_property(re, "test");
    let target_str = vm.alloc_string("doms_js engine".into());
    let matched = vm.call(test_fn, re, &[target_str]).unwrap();
    assert_eq!(matched.as_boolean(), Some(true));
}

#[test]
fn test_phase3_jit_hot_execution() {
    let mut vm = Vm::new();
    // Function that adds two numbers: arg0 + arg1
    let mut chunk = BytecodeChunk::new();
    chunk.emit(OpCode::GetLocal(0), 1);
    chunk.emit(OpCode::GetLocal(1), 1);
    chunk.emit(OpCode::Add, 1);
    chunk.emit(OpCode::Return, 1);

    let js_fn = JsFunction {
        name: Some("add".into()),
        params: vec!["a".into(), "b".into()],
        local_names: vec!["a".into(), "b".into()],
        lexical_local_names: Vec::new(),
        chunk: Arc::new(chunk),
        upvalues: Vec::new(),
        is_async: false,
        is_generator: false,
        call_count: 0,
        jit_entry: None,
        is_strict: false,
        has_param_exprs: false,
        is_method: false,
        is_arrow: false,
        home_object: None,
    };
    let fn_val = vm.alloc_function(js_fn);

    // Call 15 times to exceed JIT_HOT_THRESHOLD (10)
    let mut total = 0.0;
    for i in 0..15 {
        let res = vm.call(fn_val, JsValue::undefined(), &[JsValue::from_int32(i), JsValue::from_int32(1)]).unwrap();
        total += res.as_number().unwrap();
    }
    assert_eq!(total, 120.0);

    let obj = unsafe { &*(fn_val.as_object_ptr().unwrap() as *mut JsObject) };
    if let ObjectKind::Function(f) = &obj.kind {
        assert!(f.jit_entry.is_some(), "Function should have been JIT compiled");
        assert!(f.jit_entry.as_ref().unwrap().is_native(), "JIT code should be native Cranelift machine code");
    } else {
        panic!("Expected Function ObjectKind");
    }
}

#[test]
fn test_phase1_promises_and_weak_collections() {
    let mut vm = Vm::new();

    // 1. WeakMap test
    let wm_res = vm.eval("let wm = new WeakMap(); let k = {}; wm.set(k, 'value'); wm.has(k);").unwrap();
    assert_eq!(wm_res.as_boolean(), Some(true));

    // 2. WeakSet test
    let ws_res = vm.eval("let ws = new WeakSet(); let item = {}; ws.add(item); ws.has(item);").unwrap();
    assert_eq!(ws_res.as_boolean(), Some(true));

    // 3. Promise.allSettled test
    let p_res = vm.eval("let p = Promise.allSettled([Promise.resolve(10), Promise.reject('err')]); p;").unwrap();
    assert!(p_res.is_object());

    // 4. Promise.any test
    let any_res = vm.eval("let a = Promise.any([Promise.resolve(99)]); a;").unwrap();
    assert!(any_res.is_object());
}

#[test]
fn test_phase2_symbols_and_global_registry() {
    let mut vm = Vm::new();

    // Unique symbols
    let res1 = vm.eval("Symbol('foo') === Symbol('foo')").unwrap();
    assert_eq!(res1.as_boolean(), Some(false));

    // Global registry
    let res2 = vm.eval("Symbol.for('bar') === Symbol.for('bar')").unwrap();
    assert_eq!(res2.as_boolean(), Some(true));

    let res3 = vm.eval("Symbol.keyFor(Symbol.for('hello')) === 'hello'").unwrap();
    assert_eq!(res3.as_boolean(), Some(true));

    // Well-known symbols
    let res4 = vm.eval("typeof Symbol.iterator === 'symbol' && typeof Symbol.toStringTag === 'symbol'").unwrap();
    assert_eq!(res4.as_boolean(), Some(true));

    // Symbol-keyed properties
    let res5 = vm.eval("let obj = {}; let s = Symbol('key'); obj[s] = 42; obj[s] === 42").unwrap();
    assert_eq!(res5.as_boolean(), Some(true));
}

#[test]
fn test_phase2_typed_arrays_and_dataview() {
    let mut vm = Vm::new();

    // ArrayBuffer and isView
    let res_buf = vm.eval("let b = new ArrayBuffer(16); b.byteLength === 16 && ArrayBuffer.isView(new Uint8Array(b))").unwrap();
    assert_eq!(res_buf.as_boolean(), Some(true));

    // Uint8Array read/write/methods
    let res_ta = vm.eval(r#"
        let u8 = new Uint8Array([10, 20, 30, 40]);
        u8[1] = 99;
        let sub = u8.subarray(1, 3);
        let inc = u8.includes(99);
        let idx = u8.indexOf(40);
        let joined = u8.join('-');
        u8[1] === 99 && sub.length === 2 && sub[0] === 99 && inc === true && idx === 3 && joined === '10-99-30-40'
    "#).unwrap();
    assert_eq!(res_ta.as_boolean(), Some(true));

    // Uint8ClampedArray clamping and round-half-to-even
    let res_clamp = vm.eval(r#"
        let ca = new Uint8ClampedArray(4);
        ca[0] = 300;
        ca[1] = -10;
        ca[2] = 2.5;
        ca[3] = 3.5;
        ca[0] === 255 && ca[1] === 0 && ca[2] === 2 && ca[3] === 4
    "#).unwrap();
    assert_eq!(res_clamp.as_boolean(), Some(true));

    // DataView endianness and various getters/setters
    let res_dv = vm.eval(r#"
        let buf = new ArrayBuffer(16);
        let dv = new DataView(buf);
        dv.setInt32(0, 12345678, true);
        let read32 = dv.getInt32(0, true);
        dv.setFloat32(4, 1.5, true);
        let readF32 = dv.getFloat32(4, true);
        read32 === 12345678 && readF32 === 1.5
    "#).unwrap();
    assert_eq!(res_dv.as_boolean(), Some(true));
}

#[test]
fn test_phase2_iterator_protocols() {
    let mut vm = Vm::new();

    // Array.prototype[Symbol.iterator]
    let res_arr_iter = vm.eval(r#"
        let arr = [10, 20];
        let it = arr[Symbol.iterator]();
        let step1 = it.next();
        let step2 = it.next();
        let step3 = it.next();
        step1.value === 10 && step1.done === false &&
        step2.value === 20 && step2.done === false &&
        step3.done === true
    "#).unwrap();
    assert_eq!(res_arr_iter.as_boolean(), Some(true));

    // Array.from with mapping
    let res_from = vm.eval(r#"
        let mapped = Array.from([1, 2, 3], x => x * 3);
        mapped.length === 3 && mapped[0] === 3 && mapped[1] === 6 && mapped[2] === 9
    "#).unwrap();
    assert_eq!(res_from.as_boolean(), Some(true));

    // Array.of
    let res_of = vm.eval(r#"
        let ofArr = Array.of(7, 8, 9);
        ofArr.length === 3 && ofArr[0] === 7 && ofArr[1] === 8 && ofArr[2] === 9
    "#).unwrap();
    assert_eq!(res_of.as_boolean(), Some(true));

    // String.prototype[Symbol.iterator]
    let res_str_iter = vm.eval(r#"
        let strIt = "ab"[Symbol.iterator]();
        let s1 = strIt.next();
        let s2 = strIt.next();
        let s3 = strIt.next();
        s1.value === 'a' && s2.value === 'b' && s3.done === true
    "#).unwrap();
    assert_eq!(res_str_iter.as_boolean(), Some(true));

    // Map and Set iteration via Symbol.iterator
    let res_map_iter = vm.eval(r#"
        let m = new Map();
        m.set('k', 'v');
        let mIt = m[Symbol.iterator]();
        let mEntry = mIt.next().value;
        mEntry[0] === 'k' && mEntry[1] === 'v'
    "#).unwrap();
    assert_eq!(res_map_iter.as_boolean(), Some(true));
}

#[test]
fn test_phase2_atomics() {
    let mut vm = Vm::new();

    let res = vm.eval(r#"
        let sab = new SharedArrayBuffer(16);
        let ta = new Int32Array(sab);
        let isLf = Atomics.isLockFree(4);
        Atomics.store(ta, 0, 42);
        let loaded = Atomics.load(ta, 0);
        let oldAdd = Atomics.add(ta, 0, 8); // 42 -> 50
        let bitAnd = Atomics.and(ta, 0, 0xFF);
        let bitOr = Atomics.or(ta, 0, 0x100);
        let bitXor = Atomics.xor(ta, 0, 0x100);
        let cx = Atomics.compareExchange(ta, 0, 50, 100); // 50 -> 100
        let waitRes = Atomics.wait(ta, 0, 999, 10);
        let notifyRes = Atomics.notify(ta, 0, 1);

        isLf === true && loaded === 42 && oldAdd === 42 && cx === 50 &&
        Atomics.load(ta, 0) === 100 && waitRes === 'not-equal' && notifyRes === 0
    "#).unwrap();
    assert_eq!(res.as_boolean(), Some(true));
}

#[test]
fn test_default_parameters() {
    let mut vm = Vm::new();
    let res = vm.eval(r#"
        function add(a, b = 10) { return a + b; }
        const mul = (x = 4, y = 5) => x * y;
        add(5) === 15 && add(5, 2) === 7 && mul() === 20 && mul(3) === 15
    "#).unwrap();
    assert_eq!(res.as_boolean(), Some(true));
}

#[test]
fn test_polymorphic_inline_cache_execution() {
    let mut vm = Vm::new();
    let res = vm.eval(r#"
        function getX(obj) { return obj.x; }
        let p1 = { x: 10 };
        let p2 = { a: 1, x: 20 };
        let p3 = { a: 1, b: 2, x: 30 };
        let p4 = { a: 1, b: 2, c: 3, x: 40 };

        let sum = 0;
        for (let i = 0; i < 100; i++) {
            sum += getX(p1);
            sum += getX(p2);
            sum += getX(p3);
            sum += getX(p4);
        }
        sum === 10000
    "#).unwrap();
    assert_eq!(res.as_boolean(), Some(true));
}

#[test]
fn test_generational_gc_nursery_scavenge() {
    let mut vm = Vm::new();
    let res = vm.eval(r#"
        let root = {
            a: 42,
            b: "hello GC",
            nested: { c: [1, 2, 3] },
            fn: function(x) { return x * 2; }
        };
        root
    "#).unwrap();

    let root_ptr = res.as_object_ptr().expect("Expected object pointer");
    assert!(vm.heap.nursery.contains(root_ptr));

    // Perform an explicit nursery scavenge pass
    let prev_scavenges = vm.heap.nursery.scavenge_count;
    vm.scavenge_nursery();
    assert_eq!(vm.heap.nursery.scavenge_count, prev_scavenges + 1);

    // Verify object survived and pointers were updated
    let updated_root = vm.eval("root.a === 42 && root.b === 'hello GC' && root.nested.c[1] === 2 && root.fn(21) === 42").unwrap();
    assert_eq!(updated_root.as_boolean(), Some(true));
}

#[test]
fn test_generational_gc_auto_scavenge_under_allocation_pressure() {
    let mut vm = Vm::new();
    // Allocate 30,000 objects to exceed the 4MB nursery and trigger automatic minor GC
    let res = vm.eval(r#"
        let live_list = [];
        for (let i = 0; i < 30000; i++) {
            let temp = { id: i, payload: "temp_" + i };
            if (i % 1000 === 0) {
                live_list.push(temp);
            }
        }
        live_list.length === 30 && live_list[0].id === 0 && live_list[29].id === 29000
    "#).unwrap();

    assert_eq!(res.as_boolean(), Some(true));
    assert!(vm.heap.nursery.scavenge_count >= 1, "Expected at least 1 automatic nursery scavenge, got {}", vm.heap.nursery.scavenge_count);
}

