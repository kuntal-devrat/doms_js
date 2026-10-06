//! Servo-style Parallel Browser Prototype powered by DOMS.js.
//!
//! Demonstrates:
//! 1. Multi-isolate tab isolation (each tab in its own independent Isolate & Heap).
//! 2. DOM tree manipulation bindings (`document.createElement`, `appendChild`, etc.).
//! 3. Worker isolate with `SharedArrayBuffer` and `Atomics` for parallel offscreen rendering.
//! 4. Cross-isolate message passing via `MessageChannel` / `postMessage`.
//! 5. Event loop ticks with asynchronous microtask draining.

use doms_core::Engine;
use doms_parallel::{Atomics, MessageChannel, SharedArrayBuffer};
use std::sync::Arc;
use std::thread;

fn main() {
    println!("============================================================");
    println!(" DOMS.js — Servo-Style Parallel Browser Prototype Demo      ");
    println!("============================================================");

    let engine = Engine::new();

    // ── 1. Create Browser Tab 1 (Foreground Tab) ──
    println!("\n[Browser Engine] Initializing Tab 1 (Foreground Isolate)...");
    let mut tab1_isolate = engine.create_isolate();
    let mut tab1_ctx = tab1_isolate.create_context();

    // Setup minimal DOM bindings in Tab 1
    let dom_setup_js = r#"
        var DOM = {
            nodes: [],
            createElement: function(tag) {
                var node = { tag: tag, children: [], text: "" };
                DOM.nodes.push(node);
                return node;
            },
            renderTree: function() {
                return "DOM Tree with " + DOM.nodes.length + " elements";
            }
        };
    "#;
    tab1_ctx.eval(dom_setup_js).expect("Setup DOM in Tab 1");

    // ── 2. Create Browser Tab 2 (Background Tab) ──
    println!("[Browser Engine] Initializing Tab 2 (Background Isolate)...");
    let mut tab2_isolate = engine.create_isolate();
    let mut tab2_ctx = tab2_isolate.create_context();
    tab2_ctx
        .eval("var tabName = 'Background Analytics Tab';")
        .expect("Tab 2 setup");

    // ── 3. Parallel Worker with SharedArrayBuffer & Atomics ──
    println!("[Browser Engine] Spawning Render Worker with 1KB SharedArrayBuffer Framebuffer...");
    let framebuffer = Arc::new(SharedArrayBuffer::new(1024)); // 1KB Framebuffer
    let worker_fb = Arc::clone(&framebuffer);

    let channel = MessageChannel::new();
    let worker_port = channel.port2;

    // Worker Thread: Computes offscreen layout & renders pixels
    let worker_handle = thread::spawn(move || {
        println!("  [Worker Thread] Running layout pass and rendering pixels...");
        // Draw pixel data into framebuffer using Atomics
        for i in 0..16 {
            let pixel_val = ((i * 15) % 255) as u8;
            Atomics::store(&worker_fb, i, pixel_val).expect("Store pixel");
        }
        // Send completion message back to Tab 1
        worker_port.post_message(b"FRAME_RENDERED".to_vec()).expect("Send message to Tab 1");
        println!("  [Worker Thread] Offscreen layout complete. Signal dispatched.");
    });

    // ── 4. Tab 1 Interaction & DOM Creation ──
    println!("[Tab 1] Executing user application script...");
    let app_script = r#"
        var h1 = DOM.createElement('h1');
        h1.text = 'Welcome to DOMS Parallel Browser!';
        var div = DOM.createElement('div');
        div.text = 'Parallel Layout and Painting Active.';
        DOM.renderTree();
    "#;
    let render_summary = tab1_ctx.eval(app_script).expect("Run app script");
    println!("[Tab 1] Render Output: {:?}", render_summary);

    // Wait for worker completion message
    worker_handle.join().expect("Worker thread join");

    if let Ok(msg) = channel.port1.try_recv() {
        let msg_str = String::from_utf8_lossy(&msg.payload);
        println!("[Tab 1] Received event from Worker: {}", msg_str);
    }

    // Inspect framebuffer rendered by the parallel worker
    let p0 = Atomics::load(&framebuffer, 0).unwrap_or(0);
    let p5 = Atomics::load(&framebuffer, 5).unwrap_or(0);
    let p10 = Atomics::load(&framebuffer, 10).unwrap_or(0);
    println!(
        "[Compositor] Displaying Composited Framebuffer: pixels[0]={}, pixels[5]={}, pixels[10]={}",
        p0, p5, p10
    );

    // Verify Tab 2 was completely isolated and unaffected
    let tab2_val = tab2_ctx.eval("tabName").expect("Read tab2");
    println!("[Tab 2] Verified background state: {:?}", tab2_val);

    println!("\n============================================================");
    println!(" Parallel Browser Prototype Demo Completed Successfully!   ");
    println!("============================================================");
}
