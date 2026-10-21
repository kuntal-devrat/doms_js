//! DOMS JavaScript Engine — Idiomatic Rust Embedding Showcase
//!
//! Demonstrates:
//! 1. Creating an Engine and Isolate.
//! 2. Exposing multi-argument Rust closures to JavaScript (`set_global_fn`).
//! 3. Creating and configuring objects and arrays with the Rust builder API.
//! 4. Calling JavaScript functions from Rust (`call_method`, `call`).
//! 5. Type conversion and evaluation with `eval_as::<T>()`.

use doms_core::Engine;
use std::collections::HashMap;

fn main() {
    println!("=== DOMS JavaScript Engine — Rust Embedding Showcase ===\n");

    let engine = Engine::new();
    let mut isolate = engine.create_isolate();
    let mut ctx = isolate.create_context();

    // 1. Register a multi-argument Rust closure
    ctx.set_global_fn("rustCompute", |a: i64, b: i64, op: String| -> i64 {
        match op.as_str() {
            "add" => a + b,
            "sub" => a - b,
            "mul" => a * b,
            _ => 0,
        }
    });
    println!("[1] Registered Rust closure: rustCompute(a, b, op)");

    // 2. Evaluate JS code calling the Rust closure
    let result: i64 = ctx.eval_as("rustCompute(20, 22, 'add')").unwrap();
    println!("[2] Evaluated JS calling Rust closure -> {}", result);
    assert_eq!(result, 42);

    // 3. Create an object and inject into the JS isolate
    let config = ctx.create_object();
    ctx.set_property(config, "appName", "RustHostApp");
    ctx.set_property(config, "workerCount", 8_i64);
    ctx.set_property(config, "debugMode", true);
    ctx.set_method(config, "status", || "Running normally");
    ctx.set_global("appConfig", config);
    println!("[3] Injected Rust object: appConfig = {{ appName, workerCount, debugMode, status() }}");

    // 4. Create an array and process in JS
    let list = ctx.create_array(vec![10_i64, 20, 30, 40]);
    ctx.set_global("dataItems", list);

    let sum: f64 = ctx.eval_as("dataItems.reduce((a, b) => a + b, 0)").unwrap();
    println!("[4] Injected Rust array: dataItems, JS computed sum -> {}", sum);
    assert_eq!(sum, 100.0);

    // 5. Evaluate into structured Rust types: Vec<i64> and HashMap<String, String>
    let squares: Vec<i64> = ctx.eval_as("[1, 2, 3, 4, 5].map(x => x * x)").unwrap();
    println!("[5] Evaluated JS array directly into Vec<i64>: {:?}", squares);
    assert_eq!(squares, vec![1, 4, 9, 16, 25]);

    let headers: HashMap<String, String> = ctx.eval_as(
        "({ 'content-type': 'application/json', 'x-custom-engine': 'DOMS' })"
    ).unwrap();
    println!("[6] Evaluated JS object directly into HashMap<String, String>:");
    for (k, v) in &headers {
        println!("    {}: {}", k, v);
    }

    println!("\n=== Rust Embedding Showcase completed successfully! ===");
}
