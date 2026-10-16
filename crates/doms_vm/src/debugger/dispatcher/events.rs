use super::*;

impl CdpDispatcher {
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
}
