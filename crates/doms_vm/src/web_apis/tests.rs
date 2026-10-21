//! Tests for the web standard host APIs.

use super::*;

#[test]
fn test_timers_and_event_loop() {
    let mut vm = Vm::new();
    let script = r#"
            var timerFired = false;
            setTimeout(function() {
                timerFired = true;
            }, 5);
        "#;
    vm.eval(script).expect("eval should succeed");
    assert_eq!(vm.globals.get("timerFired").map(|v| v.is_truthy()), Some(false));

    // Sleep 15ms so timer expires
    std::thread::sleep(Duration::from_millis(15));
    vm.run_event_loop().expect("event loop should run");
    assert_eq!(vm.globals.get("timerFired").map(|v| v.is_truthy()), Some(true));
}

#[test]
fn test_text_encoder_decoder() {
    let mut vm = Vm::new();
    let script = r#"
            var encoder = new TextEncoder();
            var bytes = encoder.encode("Hello, DOMS!");
            var decoder = new TextDecoder();
            var result = decoder.decode(bytes);
        "#;
    vm.eval(script).expect("eval should succeed");
    let res = vm.globals.get("result").copied().unwrap();
    assert_eq!(vm.to_string_value(res), "Hello, DOMS!");
}

#[test]
fn test_url_and_search_params() {
    let mut vm = Vm::new();
    let script = r#"
            var url = new URL("https://domsjs.dev:8080/api/v1/search?query=parallel&page=2#top");
            var protocol = url.protocol;
            var hostname = url.hostname;
            var port = url.port;
            var pathname = url.pathname;
            var query = url.searchParams.get("query");
            var hasPage = url.searchParams.has("page");
        "#;
    vm.eval(script).expect("eval should succeed");
    assert_eq!(vm.to_string_value(vm.globals.get("protocol").copied().unwrap()), "https:");
    assert_eq!(vm.to_string_value(vm.globals.get("hostname").copied().unwrap()), "domsjs.dev");
    assert_eq!(vm.to_string_value(vm.globals.get("port").copied().unwrap()), "8080");
    assert_eq!(vm.to_string_value(vm.globals.get("pathname").copied().unwrap()), "/api/v1/search");
    assert_eq!(vm.to_string_value(vm.globals.get("query").copied().unwrap()), "parallel");
    assert_eq!(vm.globals.get("hasPage").map(|v| v.is_truthy()), Some(true));
}

#[test]
fn test_event_target_and_custom_event() {
    let mut vm = Vm::new();
    let script = r#"
            var target = new EventTarget();
            var receivedDetail = null;
            target.addEventListener("custom_event", function(e) {
                receivedDetail = e.detail;
            });
            var event = new CustomEvent("custom_event", { detail: "DOMS_EVENT_PAYLOAD" });
            target.dispatchEvent(event);
        "#;
    vm.eval(script).expect("eval should succeed");
    let detail = vm.globals.get("receivedDetail").copied().unwrap();
    assert_eq!(vm.to_string_value(detail), "DOMS_EVENT_PAYLOAD");
}

#[test]
fn test_fetch_data_uri_response() {
    let mut vm = Vm::new();
    let script = r#"
            var fetchBody = null;
            fetch("data:text/plain,Hello DOMS Fetch").then(function(res) {
                return res.text();
            }).then(function(text) {
                fetchBody = text;
            });
        "#;
    vm.eval(script).expect("eval should succeed");
    vm.drain_microtasks().expect("drain microtasks");
    let body = vm.globals.get("fetchBody").copied().unwrap();
    assert_eq!(vm.to_string_value(body), "Hello DOMS Fetch");
}
