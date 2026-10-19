use super::*;

pub(crate) fn load_existing_categories(path: &Path) -> HashMap<String, CategoryStats> {
    let mut map = HashMap::new();
    let Ok(content) = fs::read_to_string(path) else { return map; };

    let Some(cat_idx) = content.find("\"categories\"") else { return map; };
    let after_cat = &content[cat_idx..];
    let Some(open_brace) = after_cat.find('{') else { return map; };
    let mut depth = 0;
    let mut start_cat_block = 0;
    let mut end_cat_block = 0;
    for (i, c) in after_cat[open_brace..].char_indices() {
        if c == '{' {
            if depth == 0 {
                start_cat_block = open_brace;
            }
            depth += 1;
        } else if c == '}' {
            depth -= 1;
            if depth == 0 {
                end_cat_block = open_brace + i;
                break;
            }
        }
    }
    if end_cat_block > start_cat_block {
        let block = &after_cat[start_cat_block + 1..end_cat_block];
        let mut rem = block;
        while let Some(quote_start) = rem.find('"') {
            let after_q1 = &rem[quote_start + 1..];
            let Some(quote_end) = after_q1.find('"') else { break; };
            let cat_name = &after_q1[..quote_end];
            let after_name = &after_q1[quote_end + 1..];
            let Some(sub_open) = after_name.find('{') else { break; };
            let Some(sub_close) = after_name[sub_open..].find('}') else { break; };
            let sub_content = &after_name[sub_open..sub_open + sub_close + 1];

            let total = extract_json_num(sub_content, "\"total\":");
            let passed = extract_json_num(sub_content, "\"passed\":");
            let failed = extract_json_num(sub_content, "\"failed\":");
            if let (Some(total), Some(passed), Some(failed)) = (total, passed, failed) {
                map.insert(cat_name.to_string(), CategoryStats { total, passed, failed });
            }
            rem = &after_name[sub_open + sub_close + 1..];
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
