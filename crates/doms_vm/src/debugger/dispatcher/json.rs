use super::*;

impl CdpDispatcher {
    // ── Simple JSON Parsing & Formatting Helpers ──

    pub(super) fn parse_json_rpc(json: &str) -> (Option<String>, Option<String>, HashMap<String, String>) {
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

    pub(super) fn extract_json_field(json: &str, field: &str) -> Option<String> {
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

    pub(super) fn extract_json_string(val: &str) -> Option<String> {
        let trimmed = val.trim();
        if trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2 {
            Some(trimmed[1..trimmed.len() - 1].to_string())
        } else {
            Some(trimmed.to_string())
        }
    }

    pub(super) fn format_remote_object(val: JsValue, vm: &mut Vm) -> (&'static str, String) {
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
