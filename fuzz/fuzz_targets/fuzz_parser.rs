//! Fuzz target for doms_parser: tests robustness against arbitrary malformed JS tokens and syntax.

use std::env;
use std::fs;

fn fuzz_parser(data: &[u8]) {
    if let Ok(source) = std::str::from_utf8(data) {
        let _ = doms_parser::parse(source);
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() > 1 {
        for path in &args[1..] {
            if let Ok(bytes) = fs::read(path) {
                fuzz_parser(&bytes);
            }
        }
    } else {
        // Built-in corpus smoke test
        let test_inputs = [
            b"function f(a, b) { return a + b; }".as_slice(),
            b"var x = { a: 1, b: [2, 3], c: /regex/g };".as_slice(),
            b"try { throw 42; } catch(e) { } finally { }".as_slice(),
            b"class Foo extends Bar { constructor() { super(); } }".as_slice(),
            b"/* unclosed comment".as_slice(),
            b"\"unclosed string \x00 \xFF".as_slice(),
            b"`template ${x + 1} string`".as_slice(),
            b"\x00\xFF\xFE\xFD\x10\x20\x30".as_slice(),
        ];
        for input in test_inputs {
            fuzz_parser(input);
        }
        println!("Fuzz parser smoke test passed successfully!");
    }
}
