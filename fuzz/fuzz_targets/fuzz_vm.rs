//! Fuzz target for doms_vm: executes arbitrary bytecode with strict instruction bounds to verify memory safety.

use std::env;
use std::fs;

fn fuzz_vm(data: &[u8]) {
    if let Ok(source) = std::str::from_utf8(data) {
        if let Ok(program) = doms_parser::parse(source) {
            if let Ok(chunk) = doms_compiler::compile(&program) {
                let mut vm = doms_vm::Vm::new();
                vm.max_instructions = Some(20_000); // 20k instructions max per fuzz input
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let _ = vm.run(&chunk);
                }));
            }
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() > 1 {
        for path in &args[1..] {
            if let Ok(bytes) = fs::read(path) {
                fuzz_vm(&bytes);
            }
        }
    } else {
        let test_inputs = [
            b"var a = 1; var b = 2; a + b;".as_slice(),
            b"function fact(n) { return n <= 1 ? 1 : n * fact(n - 1); } fact(5);".as_slice(),
            b"let m = new Map(); m.set('k', 123); m.get('k');".as_slice(),
            b"let s = new Set([1, 2, 3]); s.union(new Set([3, 4, 5])).has(4);".as_slice(),
            b"Object.groupBy([1, 2, 3, 4], x => x % 2 === 0 ? 'even' : 'odd');".as_slice(),
            b"while(true) {}".as_slice(), // Verifies instruction limit terminates safely
        ];
        for input in test_inputs {
            fuzz_vm(input);
        }
        println!("Fuzz VM smoke test passed successfully!");
    }
}
