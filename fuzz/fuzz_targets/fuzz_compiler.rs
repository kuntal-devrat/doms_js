//! Fuzz target for doms_compiler: verifies AST to bytecode compilation stability.

use std::env;
use std::fs;

fn fuzz_compiler(data: &[u8]) {
    if let Ok(source) = std::str::from_utf8(data) {
        if let Ok(program) = doms_parser::parse(source) {
            let _ = doms_compiler::compile(&program);
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() > 1 {
        for path in &args[1..] {
            if let Ok(bytes) = fs::read(path) {
                fuzz_compiler(&bytes);
            }
        }
    } else {
        let test_inputs = [
            b"let a = 1; let b = 2; a + b * 3;".as_slice(),
            b"for (let i = 0; i < 10; i++) { if (i === 5) break; }".as_slice(),
            b"function outer() { let x = 10; return function inner() { return x; }; }".as_slice(),
            b"({ a: 1, b: [10, 20], get c() { return 42; } });".as_slice(),
        ];
        for input in test_inputs {
            fuzz_compiler(input);
        }
        println!("Fuzz compiler smoke test passed successfully!");
    }
}
