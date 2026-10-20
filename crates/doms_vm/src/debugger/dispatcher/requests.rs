use super::*;

impl CdpDispatcher {
    /// Handle an incoming JSON-RPC request from DevTools / client.
    pub fn handle_request(&mut self, req_json: &str, vm: &mut Vm) -> String {
        let (id, method, params) = Self::parse_json_rpc(req_json);

        let id_val = id.as_deref().unwrap_or("null");

        match method.as_deref() {
            Some("Runtime.enable") => {
                self.session.enabled = true;
                format!(r#"{{"id":{},"result":{{}}}}"#, id_val)
            }
            Some("Runtime.disable") => {
                self.session.enabled = false;
                format!(r#"{{"id":{},"result":{{}}}}"#, id_val)
            }
            Some("Runtime.evaluate") => {
                let expr = params
                    .get("expression")
                    .and_then(|v| Self::extract_json_string(v))
                    .unwrap_or_default();

                match vm.eval(&expr) {
                    Ok(val) => {
                        let (type_str, val_str) = Self::format_remote_object(val, vm);
                        format!(
                            r#"{{"id":{},"result":{{"result":{{"type":"{}","value":{}}}}}}}"#,
                            id_val, type_str, val_str
                        )
                    }
                    Err(err) => {
                        format!(
                            r#"{{"id":{},"result":{{"exceptionDetails":{{"text":"{}"}}}}}}"#,
                            id_val, err
                        )
                    }
                }
            }
            Some("Runtime.runIfWaitingForDebugger") => {
                self.session.paused = false;
                format!(r#"{{"id":{},"result":{{}}}}"#, id_val)
            }
            Some("Debugger.enable") => {
                self.session.enabled = true;
                format!(r#"{{"id":{},"result":{{"debuggerId":"doms-debugger-1"}}}}"#, id_val)
            }
            Some("Debugger.disable") => {
                self.session.enabled = false;
                format!(r#"{{"id":{},"result":{{}}}}"#, id_val)
            }
            Some("Debugger.setBreakpoint") => {
                let script_id = params
                    .get("location")
                    .and_then(|l| Self::extract_json_field(l, "scriptId"))
                    .and_then(|s| Self::extract_json_string(&s))
                    .unwrap_or_else(|| "1".into());
                let line: usize = params
                    .get("location")
                    .and_then(|l| Self::extract_json_field(l, "lineNumber"))
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0);

                let bp_id = self.session.set_breakpoint(&script_id, line, 0);
                format!(
                    r#"{{"id":{},"result":{{"breakpointId":"{}","actualLocation":{{"scriptId":"{}","lineNumber":{},"columnNumber":0}}}}}}"#,
                    id_val, bp_id, script_id, line
                )
            }
            Some("Debugger.removeBreakpoint") => {
                let bp_id = params
                    .get("breakpointId")
                    .and_then(|v| Self::extract_json_string(v))
                    .unwrap_or_default();
                self.session.remove_breakpoint(&bp_id);
                format!(r#"{{"id":{},"result":{{}}}}"#, id_val)
            }
            Some("Debugger.pause") => {
                self.session.paused = true;
                format!(r#"{{"id":{},"result":{{}}}}"#, id_val)
            }
            Some("Debugger.resume") => {
                self.session.paused = false;
                self.session.step_mode = None;
                format!(r#"{{"id":{},"result":{{}}}}"#, id_val)
            }
            Some("Debugger.stepOver") => {
                self.session.step_mode = Some(StepMode::Over);
                self.session.paused = false;
                format!(r#"{{"id":{},"result":{{}}}}"#, id_val)
            }
            Some("Debugger.stepInto") => {
                self.session.step_mode = Some(StepMode::Into);
                self.session.paused = false;
                format!(r#"{{"id":{},"result":{{}}}}"#, id_val)
            }
            Some("Debugger.stepOut") => {
                self.session.step_mode = Some(StepMode::Out);
                self.session.paused = false;
                format!(r#"{{"id":{},"result":{{}}}}"#, id_val)
            }
            Some("Debugger.getScriptSource") => {
                let script_id = params
                    .get("scriptId")
                    .and_then(|v| Self::extract_json_string(v))
                    .unwrap_or_default();
                let src = self.session.scripts.get(&script_id).cloned().unwrap_or_default();
                format!(
                    r#"{{"id":{},"result":{{"scriptSource":"{}"}}}}"#,
                    id_val,
                    src.replace('\n', "\\n").replace('"', "\\\"")
                )
            }
            _ => {
                format!(
                    r#"{{"id":{},"error":{{"code":-32601,"message":"Method not found"}}}}"#,
                    id_val
                )
            }
        }
    }

    /// Dispatch non-evaluation requests without requiring mutable VM access.
    pub fn dispatch(&mut self, req_json: &str) -> Option<String> {
        let (id, method, params) = Self::parse_json_rpc(req_json);
        let id_val = id.as_deref().unwrap_or("null");

        match method.as_deref() {
            Some("Runtime.enable") => {
                self.session.enabled = true;
                Some(format!(r#"{{"id":{},"result":{{}}}}"#, id_val))
            }
            Some("Runtime.disable") => {
                self.session.enabled = false;
                Some(format!(r#"{{"id":{},"result":{{}}}}"#, id_val))
            }
            Some("Runtime.runIfWaitingForDebugger") => {
                self.session.paused = false;
                Some(format!(r#"{{"id":{},"result":{{}}}}"#, id_val))
            }
            Some("Debugger.enable") => {
                self.session.enabled = true;
                Some(format!(r#"{{"id":{},"result":{{"debuggerId":"doms-debugger-1"}}}}"#, id_val))
            }
            Some("Debugger.disable") => {
                self.session.enabled = false;
                Some(format!(r#"{{"id":{},"result":{{}}}}"#, id_val))
            }
            Some("Debugger.setBreakpoint") => {
                let script_id = params
                    .get("location")
                    .and_then(|l| Self::extract_json_field(l, "scriptId"))
                    .and_then(|s| Self::extract_json_string(&s))
                    .unwrap_or_else(|| "1".into());
                let line: usize = params
                    .get("location")
                    .and_then(|l| Self::extract_json_field(l, "lineNumber"))
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0);
                let bp_id = self.session.set_breakpoint(&script_id, line, 0);
                Some(format!(
                    r#"{{"id":{},"result":{{"breakpointId":"{}","actualLocation":{{"scriptId":"{}","lineNumber":{},"columnNumber":0}}}}}}"#,
                    id_val, bp_id, script_id, line
                ))
            }
            Some("Debugger.removeBreakpoint") => {
                let bp_id = params
                    .get("breakpointId")
                    .and_then(|v| Self::extract_json_string(v))
                    .unwrap_or_default();
                self.session.remove_breakpoint(&bp_id);
                Some(format!(r#"{{"id":{},"result":{{}}}}"#, id_val))
            }
            Some("Debugger.pause") => {
                self.session.paused = true;
                Some(format!(r#"{{"id":{},"result":{{}}}}"#, id_val))
            }
            Some("Debugger.resume") => {
                self.session.paused = false;
                self.session.step_mode = None;
                Some(format!(r#"{{"id":{},"result":{{}}}}"#, id_val))
            }
            Some("Debugger.stepOver") => {
                self.session.step_mode = Some(StepMode::Over);
                self.session.paused = false;
                Some(format!(r#"{{"id":{},"result":{{}}}}"#, id_val))
            }
            Some("Debugger.stepInto") => {
                self.session.step_mode = Some(StepMode::Into);
                self.session.paused = false;
                Some(format!(r#"{{"id":{},"result":{{}}}}"#, id_val))
            }
            Some("Debugger.stepOut") => {
                self.session.step_mode = Some(StepMode::Out);
                self.session.paused = false;
                Some(format!(r#"{{"id":{},"result":{{}}}}"#, id_val))
            }
            _ => Some(format!(r#"{{"id":{},"result":{{}}}}"#, id_val)),
        }
    }
}
