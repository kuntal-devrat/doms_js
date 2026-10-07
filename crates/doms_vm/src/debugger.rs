//! Chrome DevTools Protocol (CDP) Debugger subsystem for DOMS.
//!
//! Provides JSON-RPC 2.0 based debugging protocol compatible with Chrome DevTools,
//! VS Code, and browser developer tools:
//! - `Runtime` domain: `enable`, `evaluate`, `getProperties`, `runIfWaitingForDebugger`
//! - `Debugger` domain: `enable`, `setBreakpoint`, `removeBreakpoint`, `pause`, `resume`,
//!   `stepOver`, `stepInto`, `stepOut`, `getScriptSource`

use crate::interpreter::Vm;
use crate::value::JsValue;
use std::collections::HashMap;

/// Stepping modes supported by the debugger.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepMode {
    Into,
    Over,
    Out,
}

/// A breakpoint set at a specific source location.
#[derive(Debug, Clone)]
pub struct Breakpoint {
    pub id: String,
    pub script_id: String,
    pub line_number: usize,
    pub column_number: usize,
}

/// Stack frame information for debugger inspection.
#[derive(Debug, Clone)]
pub struct CallFrameInfo {
    pub call_frame_id: String,
    pub function_name: String,
    pub script_id: String,
    pub line_number: usize,
    pub column_number: usize,
}

/// Internal debugger state tracking breakpoints, scripts, and pause conditions.
pub struct DebuggerSession {
    pub enabled: bool,
    pub paused: bool,
    pub breakpoints: HashMap<String, Breakpoint>,
    pub scripts: HashMap<String, String>,
    pub step_mode: Option<StepMode>,
    pub next_bp_id: usize,
}

impl DebuggerSession {
    pub fn new() -> Self {
        Self {
            enabled: false,
            paused: false,
            breakpoints: HashMap::new(),
            scripts: HashMap::new(),
            step_mode: None,
            next_bp_id: 1,
        }
    }

    /// Register a parsed script for DevTools inspection.
    pub fn register_script(&mut self, script_id: &str, source: &str) {
        self.scripts.insert(script_id.to_string(), source.to_string());
    }

    /// Set a breakpoint in a registered script.
    pub fn set_breakpoint(&mut self, script_id: &str, line: usize, col: usize) -> String {
        let id = format!("bp_{}", self.next_bp_id);
        self.next_bp_id += 1;
        self.breakpoints.insert(
            id.clone(),
            Breakpoint {
                id: id.clone(),
                script_id: script_id.to_string(),
                line_number: line,
                column_number: col,
            },
        );
        id
    }

    /// Remove a registered breakpoint.
    pub fn remove_breakpoint(&mut self, bp_id: &str) -> bool {
        self.breakpoints.remove(bp_id).is_some()
    }

    /// Check if a breakpoint matches the given location.
    pub fn has_breakpoint_at(&self, script_id: &str, line: usize) -> bool {
        self.breakpoints
            .values()
            .any(|bp| bp.script_id == script_id && bp.line_number == line)
    }
}

impl Default for DebuggerSession {
    fn default() -> Self {
        Self::new()
    }
}

/// Dispatcher for Chrome DevTools Protocol JSON-RPC requests.
pub struct CdpDispatcher {
    pub session: DebuggerSession,
}

impl CdpDispatcher {
    pub fn new() -> Self {
        Self {
            session: DebuggerSession::new(),
        }
    }

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
                format!(
                    r#"{{"id":{},"result":{{"debuggerId":"doms-debugger-1"}}}}"#,
                    id_val
                )
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
                    id_val, src.replace('\n', "\\n").replace('"', "\\\"")
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

    /// Emit an event notification for script parsing.
    pub fn emit_script_parsed(&self, script_id: &str, url: &str) -> String {
        format!(
            r#"{{"method":"Debugger.scriptParsed","params":{{"scriptId":"{}","url":"{}","startLine":0,"startColumn":0}}}}"#,
            script_id, url
        )
    }

    /// Emit a paused event notification with call frames.
    pub fn emit_paused(&self, reason: &str, frames: &[CallFrameInfo]) -> String {
        let mut frames_json = String::from("[");
        for (i, f) in frames.iter().enumerate() {
            if i > 0 {
                frames_json.push(',');
            }
            frames_json.push_str(&format!(
                r#"{{"callFrameId":"{}","functionName":"{}","location":{{"scriptId":"{}","lineNumber":{},"columnNumber":{}}}}}"#,
                f.call_frame_id, f.function_name, f.script_id, f.line_number, f.column_number
            ));
        }
        frames_json.push(']');

        format!(
            r#"{{"method":"Debugger.paused","params":{{"callFrames":{},"reason":"{}"}}}}"#,
            frames_json, reason
        )
    }

    // ── Simple JSON Parsing & Formatting Helpers ──

    fn parse_json_rpc(json: &str) -> (Option<String>, Option<String>, HashMap<String, String>) {
        let mut id = None;
        let mut method = None;
        let mut params = HashMap::new();

        if let Some(id_val) = Self::extract_json_field(json, "id") {
            id = Some(id_val.trim().to_string());
        }
        if let Some(m_val) = Self::extract_json_field(json, "method") {
            method = Self::extract_json_string(&m_val);
        }
        if let Some(p_block) = Self::extract_json_field(json, "params") {
            // Extract top-level param keys
            for key in &["expression", "scriptId", "breakpointId", "location"] {
                if let Some(val) = Self::extract_json_field(&p_block, key) {
                    params.insert(key.to_string(), val);
                }
            }
        }

        (id, method, params)
    }

    fn extract_json_field(json: &str, field: &str) -> Option<String> {
        let search = format!("\"{}\"", field);
        let pos = json.find(&search)?;
        let rest = &json[pos + search.len()..];
        let colon_pos = rest.find(':')?;
        let after_colon = rest[colon_pos + 1..].trim_start();

        if after_colon.starts_with('{') {
            let mut depth = 0;
            let mut end = 0;
            for (idx, ch) in after_colon.chars().enumerate() {
                if ch == '{' {
                    depth += 1;
                } else if ch == '}' {
                    depth -= 1;
                    if depth == 0 {
                        end = idx + 1;
                        break;
                    }
                }
            }
            if end > 0 {
                return Some(after_colon[..end].to_string());
            }
        } else if let Some(stripped) = after_colon.strip_prefix('"') {
            let mut escaped = false;
            let mut end = 0;
            for (idx, ch) in stripped.chars().enumerate() {
                if escaped {
                    escaped = false;
                } else if ch == '\\' {
                    escaped = true;
                } else if ch == '"' {
                    end = idx + 2;
                    break;
                }
            }
            if end > 0 {
                return Some(after_colon[..end].to_string());
            }
        } else {
            let end = after_colon
                .find([',', '}', ']', '\n', '\r'])
                .unwrap_or(after_colon.len());
            return Some(after_colon[..end].trim().to_string());
        }

        None
    }

    fn extract_json_string(val: &str) -> Option<String> {
        let trimmed = val.trim();
        if trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2 {
            Some(trimmed[1..trimmed.len() - 1].to_string())
        } else {
            Some(trimmed.to_string())
        }
    }

    fn format_remote_object(val: JsValue, vm: &mut Vm) -> (&'static str, String) {
        if val.is_number() {
            ("number", format!("{}", val.to_number().unwrap_or(0.0)))
        } else if val.is_boolean() {
            ("boolean", format!("{}", val.is_truthy()))
        } else if val.is_null() {
            ("object", "null".to_string())
        } else if val.is_undefined() {
            ("undefined", "\"undefined\"".to_string())
        } else {
            let s = vm.to_string_value(val);
            ("string", format!("\"{}\"", s.replace('"', "\\\"")))
        }
    }
}

impl Default for CdpDispatcher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
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

        let rm_req = r#"{"id":4,"method":"Debugger.removeBreakpoint","params":{"breakpointId":"bp_1"}}"#;
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
}
