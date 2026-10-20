//! Tests for the CDP debugger dispatcher.

use super::*;

#[test]
fn test_cdp_runtime_enable_and_evaluate() {
    let mut dispatcher = CdpDispatcher::new();
    let mut vm = Vm::new();

    let req = r#"{"id":1,"method":"Runtime.enable","params":{}}"#;
    let res = dispatcher.handle_request(req, &mut vm);
    assert!(res.contains(r#""id":1"#));
    assert!(dispatcher.session.enabled);

    let eval_req = r#"{"id":2,"method":"Runtime.evaluate","params":{"expression":"10 + 20"}}"#;
    let eval_res = dispatcher.handle_request(eval_req, &mut vm);
    assert!(eval_res.contains(r#""id":2"#));
    assert!(eval_res.contains(r#""value":30"#));
}

#[test]
fn test_cdp_debugger_breakpoints() {
    let mut dispatcher = CdpDispatcher::new();
    let mut vm = Vm::new();

    let bp_req = r#"{"id":3,"method":"Debugger.setBreakpoint","params":{"location":{"scriptId":"test.js","lineNumber":42}}}"#;
    let bp_res = dispatcher.handle_request(bp_req, &mut vm);
    assert!(bp_res.contains(r#""breakpointId":"bp_1""#));
    assert!(dispatcher.session.has_breakpoint_at("test.js", 42));

    let rm_req =
        r#"{"id":4,"method":"Debugger.removeBreakpoint","params":{"breakpointId":"bp_1"}}"#;
    let rm_res = dispatcher.handle_request(rm_req, &mut vm);
    assert!(rm_res.contains(r#""id":4"#));
    assert!(!dispatcher.session.has_breakpoint_at("test.js", 42));
}

#[test]
fn test_cdp_stepping_and_pausing() {
    let mut dispatcher = CdpDispatcher::new();
    let mut vm = Vm::new();

    let pause_req = r#"{"id":5,"method":"Debugger.pause","params":{}}"#;
    let _ = dispatcher.handle_request(pause_req, &mut vm);
    assert!(dispatcher.session.paused);

    let step_req = r#"{"id":6,"method":"Debugger.stepOver","params":{}}"#;
    let _ = dispatcher.handle_request(step_req, &mut vm);
    assert_eq!(dispatcher.session.step_mode, Some(StepMode::Over));
    assert!(!dispatcher.session.paused);
}
