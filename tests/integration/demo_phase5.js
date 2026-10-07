// DOMS.js Phase 5 Feature Demo
console.log("=== DOMS.js Phase 5 Engine Demo ===");

// 1. TextEncoder & TextDecoder
var encoder = new TextEncoder();
var bytes = encoder.encode("Hello from DOMS.js Web Standards!");
var decoder = new TextDecoder();
console.log("Encoded/Decoded:", decoder.decode(bytes));

// 2. URL & URLSearchParams
var url = new URL("https://domsjs.dev:8443/benchmarks?engine=doms&jit=true#fast");
console.log("URL Origin:", url.origin);
console.log("URL Pathname:", url.pathname);
console.log("URL SearchParam 'engine':", url.searchParams.get("engine"));

// 3. EventTarget & CustomEvent
var bus = new EventTarget();
bus.addEventListener("benchmark_completed", function(e) {
    console.log("Event dispatched! Detail:", e.detail);
});
bus.dispatchEvent(new CustomEvent("benchmark_completed", { detail: "Score: 100/100" }));

// 4. Timers and Event Loop
setTimeout(function() {
    console.log("Async Task 1 completed (10ms delay)");
}, 10);

console.log("Script execution finished (waiting for event loop...)");
