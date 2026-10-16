use super::*;

pub(crate) fn load_existing_categories(path: &Path) -> HashMap<String, CategoryStats> {
    let mut map = HashMap::new();
    let Ok(content) = fs::read_to_string(path) else { return map; };
    for line in content.lines() {
        let trimmed = line.trim();
        // Look for: "category_name": { "total": ..., "passed": ..., "failed": ... }
        if let Some(colon_pos) = trimmed.find("\": {") {
            if let Some(start_quote) = trimmed.find('"') {
                if start_quote < colon_pos {
                    let cat_name = &trimmed[start_quote + 1..colon_pos];
                    if cat_name != "categories" && cat_name != "session" {
                        let total = extract_json_num(trimmed, "\"total\":");
                        let passed = extract_json_num(trimmed, "\"passed\":");
                        let failed = extract_json_num(trimmed, "\"failed\":");
                        if let (Some(total), Some(passed), Some(failed)) = (total, passed, failed) {
                            map.insert(cat_name.to_string(), CategoryStats { total, passed, failed });
                        }
                    }
                }
            }
        }
    }
    map
}

fn extract_json_num(line: &str, key: &str) -> Option<usize> {
    if let Some(pos) = line.find(key) {
        let after = &line[pos + key.len()..];
        let val_str: String = after
            .chars()
            .skip_while(|c| c.is_whitespace())
            .take_while(|c| c.is_ascii_digit())
            .collect();
        val_str.parse::<usize>().ok()
    } else {
        None
    }
}

pub(crate) fn chrono_now_utc() -> String {
    let now = std::time::SystemTime::now();
    let secs = now.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
    format!("{}-epoch", secs)
}
