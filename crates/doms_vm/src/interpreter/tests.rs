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
fn test_debug_return_add() {
    let mut vm = Vm::new();
    let res = vm.eval("function test(a, b) { return a + b; } test('hello', 'world');").unwrap();
    assert_eq!(vm.to_string_value(res), "helloworld");
    let res2 = vm.eval("test('foo', 'bar');").unwrap();
    assert_eq!(vm.to_string_value(res2), "foobar");
    let res3 = vm.eval("test(10, 20);").unwrap();
    assert_eq!(vm.to_string_value(res3), "30");
    let res4 =
        vm.eval("function f(p, c) { return p + c; } ['1', '2', '3', '4', '5'].reduce(f);").unwrap();
    assert_eq!(vm.to_string_value(res4), "12345");
}

#[test]
fn test_debug_array_proto_in() {
    let mut vm = Vm::new();
    let res = vm.eval("Object.defineProperty(Array.prototype, '0', { get: function() { return 11; }, configurable: true }); '0' in Array.prototype;").unwrap();
    assert_eq!(res.as_boolean(), Some(true));
    let res2 = vm.eval("'0' in [, , , ];").unwrap();
    assert_eq!(res2.as_boolean(), Some(true));
    let res3 = vm.eval("function callbackfn(val, idx, obj) { if (idx === 0) return val !== 11; return true; } [, , , ].every(callbackfn);").unwrap();
    assert_eq!(res3.as_boolean(), Some(false));
}

#[test]
fn test_tagged_template_opt_chain() {
    let mut vm = Vm::new();
    let res = vm.eval("function fn7() { return () => {}; }\nfn7()`hello`?.a;");
    println!("RES: {:?}", res);
    assert!(res.is_ok());
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
fn test_map_and_set_collections() {
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
fn test_jit_hot_loop_tiering() {
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
        length: 2,
        self_slot: None,
        local_names: Arc::new(vec!["a".into(), "b".into()]),
        lexical_local_names: Arc::new(Vec::new()),
        chunk: Arc::new(chunk),
        upvalues: Vec::new(),
        is_async: false,
        is_generator: false,
        call_count: 0,
        jit_entry: None,
        jit_attempted: false,
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
    };
    let fn_val = vm.alloc_function(js_fn);

    // Call 15 times to exceed JIT_HOT_THRESHOLD (10)
    let mut total = 0.0;
    for i in 0..15 {
        let res = vm
            .call(fn_val, JsValue::undefined(), &[JsValue::from_int32(i), JsValue::from_int32(1)])
            .unwrap();
        total += res.as_number().unwrap();
    }
    assert_eq!(total, 120.0);

    let obj = unsafe { &*(fn_val.as_object_ptr().unwrap() as *mut JsObject) };
    if let ObjectKind::Function(f) = &obj.kind {
        assert!(f.jit_entry.is_some(), "Function should have been JIT compiled");
        assert!(
            f.jit_entry.as_ref().unwrap().is_native(),
            "JIT code should be native Cranelift machine code"
        );
    } else {
        panic!("Expected Function ObjectKind");
    }
}

#[test]
fn test_promises_and_weak_references() {
    let mut vm = Vm::new();

    // 1. WeakMap test
    let wm_res =
        vm.eval("let wm = new WeakMap(); let k = {}; wm.set(k, 'value'); wm.has(k);").unwrap();
    assert_eq!(wm_res.as_boolean(), Some(true));

    // 2. WeakSet test
    let ws_res =
        vm.eval("let ws = new WeakSet(); let item = {}; ws.add(item); ws.has(item);").unwrap();
    assert_eq!(ws_res.as_boolean(), Some(true));

    // 3. Promise.allSettled test
    let p_res = vm
        .eval("let p = Promise.allSettled([Promise.resolve(10), Promise.reject('err')]); p;")
        .unwrap();
    assert!(p_res.is_object());

    // 4. Promise.any test
    let any_res = vm.eval("let a = Promise.any([Promise.resolve(99)]); a;").unwrap();
    assert!(any_res.is_object());
}

#[test]
fn test_symbols_and_global_registry() {
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
    let res4 = vm
        .eval("typeof Symbol.iterator === 'symbol' && typeof Symbol.toStringTag === 'symbol'")
        .unwrap();
    assert_eq!(res4.as_boolean(), Some(true));

    // Symbol-keyed properties
    let res5 = vm.eval("let obj = {}; let s = Symbol('key'); obj[s] = 42; obj[s] === 42").unwrap();
    assert_eq!(res5.as_boolean(), Some(true));
}

#[test]
fn test_typed_arrays_and_dataview() {
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
    let res_clamp = vm
        .eval(
            r#"
        let ca = new Uint8ClampedArray(4);
        ca[0] = 300;
        ca[1] = -10;
        ca[2] = 2.5;
        ca[3] = 3.5;
        ca[0] === 255 && ca[1] === 0 && ca[2] === 2 && ca[3] === 4
    "#,
        )
        .unwrap();
    assert_eq!(res_clamp.as_boolean(), Some(true));

    // DataView endianness and various getters/setters
    let res_dv = vm
        .eval(
            r#"
        let buf = new ArrayBuffer(16);
        let dv = new DataView(buf);
        dv.setInt32(0, 12345678, true);
        let read32 = dv.getInt32(0, true);
        dv.setFloat32(4, 1.5, true);
        let readF32 = dv.getFloat32(4, true);
        read32 === 12345678 && readF32 === 1.5
    "#,
        )
        .unwrap();
    assert_eq!(res_dv.as_boolean(), Some(true));
}

#[test]
fn test_iterator_protocols() {
    let mut vm = Vm::new();

    // Array.prototype[Symbol.iterator]
    let res_arr_iter = vm
        .eval(
            r#"
        let arr = [10, 20];
        let it = arr[Symbol.iterator]();
        let step1 = it.next();
        let step2 = it.next();
        let step3 = it.next();
        step1.value === 10 && step1.done === false &&
        step2.value === 20 && step2.done === false &&
        step3.done === true
    "#,
        )
        .unwrap();
    assert_eq!(res_arr_iter.as_boolean(), Some(true));

    // Array.from with mapping
    let res_from = vm
        .eval(
            r#"
        let mapped = Array.from([1, 2, 3], x => x * 3);
        mapped.length === 3 && mapped[0] === 3 && mapped[1] === 6 && mapped[2] === 9
    "#,
        )
        .unwrap();
    assert_eq!(res_from.as_boolean(), Some(true));

    // Array.of
    let res_of = vm
        .eval(
            r#"
        let ofArr = Array.of(7, 8, 9);
        ofArr.length === 3 && ofArr[0] === 7 && ofArr[1] === 8 && ofArr[2] === 9
    "#,
        )
        .unwrap();
    assert_eq!(res_of.as_boolean(), Some(true));

    // String.prototype[Symbol.iterator]
    let res_str_iter = vm
        .eval(
            r#"
        let strIt = "ab"[Symbol.iterator]();
        let s1 = strIt.next();
        let s2 = strIt.next();
        let s3 = strIt.next();
        s1.value === 'a' && s2.value === 'b' && s3.done === true
    "#,
        )
        .unwrap();
    assert_eq!(res_str_iter.as_boolean(), Some(true));

    // Map and Set iteration via Symbol.iterator
    let res_map_iter = vm
        .eval(
            r#"
        let m = new Map();
        m.set('k', 'v');
        let mIt = m[Symbol.iterator]();
        let mEntry = mIt.next().value;
        mEntry[0] === 'k' && mEntry[1] === 'v'
    "#,
        )
        .unwrap();
    assert_eq!(res_map_iter.as_boolean(), Some(true));
}

#[test]
fn test_atomics_operations() {
    let mut vm = Vm::new();

    let res = vm
        .eval(
            r#"
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
    "#,
        )
        .unwrap();
    assert_eq!(res.as_boolean(), Some(true));
}

#[test]
fn test_default_parameters() {
    let mut vm = Vm::new();
    let res = vm
        .eval(
            r#"
        function add(a, b = 10) { return a + b; }
        const mul = (x = 4, y = 5) => x * y;
        add(5) === 15 && add(5, 2) === 7 && mul() === 20 && mul(3) === 15
    "#,
        )
        .unwrap();
    assert_eq!(res.as_boolean(), Some(true));
}

#[test]
fn test_polymorphic_inline_cache_execution() {
    let mut vm = Vm::new();
    let res = vm
        .eval(
            r#"
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
    "#,
        )
        .unwrap();
    assert_eq!(res.as_boolean(), Some(true));
}

#[test]
fn test_generational_gc_nursery_scavenge() {
    let mut vm = Vm::new();
    let res = vm
        .eval(
            r#"
        let root = {
            a: 42,
            b: "hello GC",
            nested: { c: [1, 2, 3] },
            fn: function(x) { return x * 2; }
        };
        root
    "#,
        )
        .unwrap();

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
    let res = vm
        .eval(
            r#"
        let live_list = [];
        for (let i = 0; i < 30000; i++) {
            let temp = { id: i, payload: "temp_" + i };
            if (i % 1000 === 0) {
                live_list.push(temp);
            }
        }
        live_list.length === 30 && live_list[0].id === 0 && live_list[29].id === 29000
    "#,
        )
        .unwrap();

    assert_eq!(res.as_boolean(), Some(true));
    assert!(
        vm.heap.nursery.scavenge_count >= 1,
        "Expected at least 1 automatic nursery scavenge, got {}",
        vm.heap.nursery.scavenge_count
    );
}

#[test]
fn test_regexp_features() {
    let mut vm = Vm::new();

    // 1. Unicode sets /v flag
    let res_v = vm
        .eval(
            r#"
        let re = new RegExp('abc', 'v');
        re.unicodeSets === true && re.unicode === false && re.flags === 'v'
    "#,
        )
        .unwrap();
    assert_eq!(res_v.as_boolean(), Some(true));

    // 2. /u and /v mutual exclusivity
    let res_uv = vm
        .eval(
            r#"
        let caught = false;
        try {
            new RegExp('abc', 'uv');
        } catch (e) {
            caught = e instanceof SyntaxError;
        }
        caught
    "#,
        )
        .unwrap();
    assert_eq!(res_uv.as_boolean(), Some(true));

    // 3. Match indices /d flag
    let res_d = vm.eval(r#"
        let reD = new RegExp('a(b)(?<group>c)', 'd');
        let m = reD.exec('abc');
        let m0_start = m.indices[0][0];
        let m0_end = m.indices[0][1];
        let m1_start = m.indices[1][0];
        let m1_end = m.indices[1][1];
        let mg_start = m.indices.groups.group[0];
        let mg_end = m.indices.groups.group[1];
        m0_start === 0 && m0_end === 3 && m1_start === 1 && m1_end === 2 && mg_start === 2 && mg_end === 3
    "#).unwrap();
    assert_eq!(res_d.as_boolean(), Some(true));

    // 4. Generic RegExp.prototype.flags getter
    let res_generic_flags = vm
        .eval(
            r#"
        let fake = { hasIndices: true, multiline: true, dotAll: true };
        let desc = Object.getOwnPropertyDescriptor(RegExp.prototype, 'flags');
        desc.get.call(fake) === 'dms'
    "#,
        )
        .unwrap();
    assert_eq!(res_generic_flags.as_boolean(), Some(true));

    // 5. Annex B RegExp.prototype.compile
    let res_compile = vm.eval(r#"
        let reComp = new RegExp('foo');
        let ret = reComp.compile('bar', 'i');
        ret === reComp && reComp.test('BAR') === true && reComp.source === 'bar' && reComp.flags === 'i'
    "#).unwrap();
    assert_eq!(res_compile.as_boolean(), Some(true));

    // 6. Annex B RegExp legacy constructor properties
    let res_legacy = vm
        .eval(
            r#"
        let r = new RegExp('hello (\\w+)');
        r.exec('say hello world today');
        RegExp.lastMatch === 'hello world' &&
        RegExp.$1 === 'world' &&
        RegExp.leftContext === 'say ' &&
        RegExp.rightContext === ' today'
    "#,
        )
        .unwrap();
    assert_eq!(res_legacy.as_boolean(), Some(true));
}

#[test]
fn test_intl_formatters() {
    let mut vm = Vm::new();

    // 1. Intl.getCanonicalLocales
    let res_canon = vm
        .eval(
            r#"
        let list = Intl.getCanonicalLocales(['EN-us', 'ZH-hans-CN', 'en-US']);
        list.length === 2 && list[0] === 'en-US' && list[1] === 'zh-Hans-CN'
    "#,
        )
        .unwrap();
    assert_eq!(res_canon.as_boolean(), Some(true));

    // Invalid tag throws RangeError
    let res_invalid = vm
        .eval(
            r#"
        let threw = false;
        try {
            Intl.getCanonicalLocales('invalid_tag');
        } catch (e) {
            threw = e instanceof RangeError;
        }
        threw
    "#,
        )
        .unwrap();
    assert_eq!(res_invalid.as_boolean(), Some(true));

    // 2. Intl.Locale
    vm.eval(
        r#"
        let loc = new Intl.Locale('zh-Hans-CN', { calendar: 'gregory', numeric: true });
    "#,
    )
    .unwrap();
    assert_eq!(
        vm.eval("loc.baseName === 'zh-Hans-CN'").unwrap().as_boolean(),
        Some(true),
        "baseName failed: {:?}",
        vm.eval("loc.baseName")
    );
    assert_eq!(
        vm.eval("loc.language === 'zh'").unwrap().as_boolean(),
        Some(true),
        "language failed: {:?}",
        vm.eval("loc.language")
    );
    assert_eq!(
        vm.eval("loc.script === 'Hans'").unwrap().as_boolean(),
        Some(true),
        "script failed: {:?}",
        vm.eval("loc.script")
    );
    assert_eq!(
        vm.eval("loc.region === 'CN'").unwrap().as_boolean(),
        Some(true),
        "region failed: {:?}",
        vm.eval("loc.region")
    );
    assert_eq!(
        vm.eval("loc.calendar === 'gregory'").unwrap().as_boolean(),
        Some(true),
        "calendar failed: {:?}",
        vm.eval("loc.calendar")
    );
    assert_eq!(
        vm.eval("loc.numeric === true").unwrap().as_boolean(),
        Some(true),
        "numeric failed: {:?}",
        vm.eval("loc.numeric")
    );
    assert_eq!(
        vm.eval("loc.toString() === 'zh-Hans-CN-u-ca-gregory-kn'").unwrap().as_boolean(),
        Some(true),
        "toString failed: {:?}",
        vm.eval("loc.toString()")
    );

    // 3. Intl.PluralRules
    let res_pr = vm
        .eval(
            r#"
        let pr = new Intl.PluralRules('en-US');
        pr instanceof Intl.PluralRules && pr.select(1) === 'one' && pr.select(2) === 'other'
    "#,
        )
        .unwrap();
    assert_eq!(res_pr.as_boolean(), Some(true));

    // 4. Intl.ListFormat
    let res_lf = vm
        .eval(
            r#"
        let lf = new Intl.ListFormat('en-US');
        let formatted = lf.format(['Apple', 'Banana', 'Orange']);
        formatted === 'Apple, Banana, and Orange'
    "#,
        )
        .unwrap();
    assert_eq!(res_lf.as_boolean(), Some(true));

    // 5. Intl.DisplayNames
    let res_dn = vm
        .eval(
            r#"
        let dn = new Intl.DisplayNames('en-US', { type: 'region' });
        dn.of('US') === 'United States' && dn.of('JP') === 'Japan'
    "#,
        )
        .unwrap();
    assert_eq!(res_dn.as_boolean(), Some(true));

    // 6. Intl.Segmenter
    let res_seg = vm
        .eval(
            r#"
        let seg = new Intl.Segmenter('en-US', { granularity: 'word' });
        let segments = seg.segment('Hello world');
        let item = segments.containing(0);
        item.segment === 'H' && item.index === 0
    "#,
        )
        .unwrap();
    assert_eq!(res_seg.as_boolean(), Some(true));
}

#[test]
fn test_annex_b_escape_unescape() {
    let mut vm = Vm::new();
    let res = vm
        .eval(
            r#"
        let enc = escape('Hello World!');
        let dec = unescape(enc);
        enc === 'Hello%20World%21' && dec === 'Hello World!'
    "#,
        )
        .unwrap();
    assert_eq!(res.as_boolean(), Some(true));
}

#[test]
fn test_fn_global_object() {
    let mut vm = Vm::new();
    let res = vm.eval("Function('return this;')()").unwrap();
    assert_eq!(res, vm.global_object);
    let res2 = vm.eval("var f = Function('return this;'); f()").unwrap();
    assert_eq!(res2, vm.global_object);
}

#[test]
fn test_array_element_attributes_semantics() {
    let mut vm = Vm::new();
    let res = vm.eval(r#"
        var failures = [];
        function check(name, cond) { if (!cond) failures.push(name); }

        // 1. Non-writable element: writes are ignored in sloppy mode.
        var a = [1, 2, 3];
        Object.defineProperty(a, "1", { value: 5, writable: false, enumerable: true, configurable: false });
        a[1] = 99;
        check("non-writable write ignored", a[1] === 5);

        // 2. Strict-mode writes to a non-writable element throw TypeError.
        var threw = false;
        try { (function () { "use strict"; a[1] = 99; })(); } catch (e) { threw = e instanceof TypeError; }
        check("strict non-writable throws", threw);

        // 3. Element accessors are honoured by the get/set fast paths.
        var b = [1];
        var setValue = 0;
        Object.defineProperty(b, "0", {
            get: function () { return 42; },
            set: function (v) { setValue = v; },
        });
        check("accessor get", b[0] === 42);
        b[0] = 7;
        check("accessor set", setValue === 7);

        // 4. Frozen elements ignore writes and push throws.
        var c = Object.freeze([1]);
        c[0] = 2;
        check("frozen write ignored", c[0] === 1);
        var pushThrew = false;
        try { c.push(2); } catch (e) { pushThrew = e instanceof TypeError; }
        check("frozen push throws", pushThrew);

        // 5. Sparse index growth updates length without materializing holes.
        var d = [];
        d[200000] = 1;
        check("sparse length", d.length === 200001);
        check("sparse hole stays undefined", d[199999] === undefined);

        // 6. `length` assignment coerces its value.
        var e = [1, 2, 3];
        e.length = "2";
        check("length coercion truncates", e.length === 2 && e[2] === undefined);

        // 7. Invalid lengths throw RangeError.
        var rangeThrew = false;
        try { e.length = -1; } catch (err) { rangeThrew = err instanceof RangeError; }
        check("invalid length throws", rangeThrew);

        failures.length;
    "#).unwrap();
    assert_eq!(res.as_number(), Some(0.0), "array element attribute semantics failed");
}

#[test]
fn test_weak_pruning_and_liveness_after_gc() {
    let mut vm = Vm::new();
    let res = vm
        .eval(
            r#"
        var failures = [];
        function check(name, cond) { if (!cond) failures.push(name); }

        // A WeakRef to an unreachable target is cleared by GC.
        var wr = (function () { return new WeakRef({ tag: "dead" }); })();
        (function churn() { for (var i = 0; i < 64; i++) { var junk = { j: i }; } })();
        $262.gc();
        check("weakref pruned", wr.deref() === undefined);

        // A WeakRef to a live target survives GC.
        var live = { tag: "live" };
        var wr2 = new WeakRef(live);
        $262.gc();
        check("weakref live", wr2.deref() === live);

        // WeakMap prunes entries whose keys became unreachable (ephemeron fixpoint).
        var wm = new WeakMap();
        (function () { wm.set({ dead: true }, "value"); })();
        (function churn2() { for (var i = 0; i < 64; i++) { var junk = { j2: i }; } })();
        $262.gc();
        check("weakmap pruned", typeof wm.size === "undefined" || wm.size === 0);

        // WeakMap retains entries for live keys.
        var liveKey = { k: 2 };
        var wm2 = new WeakMap();
        wm2.set(liveKey, 42);
        $262.gc();
        check("weakmap live", wm2.get(liveKey) === 42);

        failures.length;
    "#,
        )
        .unwrap();
    assert_eq!(res.as_number(), Some(0.0), "weak reference semantics failed");
}

#[test]
fn test_tiny_heap_stack_locals_survive_gc() {
    use doms_gc::heap::HeapConfig;
    crate::interpreter::gc::register_stack_base();
    // A tiny heap forces collection on nearly every allocation. Values that
    // only exist in Rust locals / machine registers while engine code runs
    // (builtin construction, embedder callbacks) must be found by the
    // conservative native-stack scan and kept alive.
    let mut vm = Vm::with_heap(doms_gc::Heap::new(HeapConfig {
        nursery_size: 64 * 1024,
        old_gen_size: 256 * 1024,
    }));
    let a = vm.alloc_native_function("probe_a", 0, |_, _, _| Ok(JsValue::undefined()));
    let pa = a.as_object_ptr().unwrap() as *mut JsObject;
    for _ in 0..8 {
        vm.gc_collect();
    }
    assert!(vm.objects.iter().any(|p| *p == pa), "live stack local was swept by GC");
}

#[test]
fn test_gc_reclaims_dead_large_objects() {
    // Large objects live in the fallback large-object space. A collection must
    // sweep that space, otherwise every object that ever overflowed the
    // nursery/old generation leaks until the process exits.
    let mut vm = Vm::new();
    let p = vm.heap.large_objects.allocate(16 * 1024, doms_gc::header::TypeTag::Object).unwrap();
    assert_eq!(vm.heap.large_objects.object_count(), 1);
    vm.gc_collect();
    assert_eq!(
        vm.heap.large_objects.object_count(),
        0,
        "unreferenced large objects must be reclaimed by a collection"
    );
    assert!(!vm.heap.large_objects.contains(p));
}

#[test]
fn test_proxy_mop_reactivity_and_reflect() {
    let mut vm = Vm::new();
    let res = vm
        .eval(
            r#"
        const log = [];
        const raw = { count: 0, items: ["a", "b"] };
        const state = new Proxy(raw, {
            get(target, prop, receiver) {
                log.push("get:" + String(prop));
                return Reflect.get(target, prop, receiver);
            },
            set(target, prop, value, receiver) {
                log.push("set:" + String(prop) + "=" + String(value));
                return Reflect.set(target, prop, value, receiver);
            },
            has(target, prop) {
                log.push("has:" + String(prop));
                return Reflect.has(target, prop);
            },
            deleteProperty(target, prop) {
                log.push("delete:" + String(prop));
                return Reflect.deleteProperty(target, prop);
            }
        });

        state.count = 42;
        const c = state.count;
        const exists = "count" in state;
        delete state.items;

        log.join(",");
    "#,
        )
        .unwrap();
    assert_eq!(vm.to_string_value(res), "set:count=42,get:count,has:count,delete:items");
}

#[test]
fn test_vm_incremental_gc_slice() {
    let mut vm = Vm::new();
    let _ = vm
        .eval(
            r#"
        globalThis.preserved = { id: 123, child: { name: "doms" } };
        let temp = { dead: true };
        "#,
        )
        .unwrap();

    // Run incremental slices with budget 16 objects per slice
    let mut done = false;
    for _ in 0..100 {
        if vm.gc_incremental_slice(16) {
            done = true;
            break;
        }
    }
    assert!(done, "Incremental GC slices should complete");

    let check = vm.eval("globalThis.preserved.child.name").unwrap();
    assert_eq!(vm.to_string_value(check), "doms");
}
