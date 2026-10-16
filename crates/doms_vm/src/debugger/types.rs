//! Debugger session state: stepping, breakpoints and call frames.

use super::*;

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
