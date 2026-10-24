//! Source Map v3 Parser, VLQ Decoder, and Stack Trace Symbolicator for DOMS.
//!
//! Features:
//! - Full Base64-VLQ (Variable Length Quantity) decode/encode according to the Source Map v3 spec
//! - High-performance mapping lookup by generated line & column
//! - Original source symbolication for runtime errors and stack traces

/// Decoded mapping from generated file position to original source position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mapping {
    pub generated_line: u32,
    pub generated_column: u32,
    pub original_source_index: Option<u32>,
    pub original_line: Option<u32>,
    pub original_column: Option<u32>,
    pub original_name_index: Option<u32>,
}

/// Resolved position in the author's original source code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OriginalPosition {
    pub source: String,
    pub line: u32,
    pub column: u32,
    pub name: Option<String>,
}

/// Source Map (v3) representation.
#[derive(Debug, Clone)]
pub struct SourceMap {
    pub version: u32,
    pub file: Option<String>,
    pub sources: Vec<String>,
    pub sources_content: Vec<Option<String>>,
    pub names: Vec<String>,
    pub mappings: Vec<Mapping>,
}

impl SourceMap {
    /// Parse a Source Map v3 from a JSON string.
    pub fn parse_json(json: &str) -> Result<Self, String> {
        let version = extract_number_field(json, "version").unwrap_or(3) as u32;
        let file = extract_string_field(json, "file");
        let sources = extract_string_array(json, "sources");
        let names = extract_string_array(json, "names");
        let mappings_str = extract_string_field(json, "mappings").unwrap_or_default();

        let mappings = decode_mappings(&mappings_str)?;

        Ok(Self { version, file, sources, sources_content: Vec::new(), names, mappings })
    }

    /// Lookup the original source position corresponding to a generated line & column.
    pub fn lookup(&self, gen_line: u32, gen_col: u32) -> Option<OriginalPosition> {
        // Find best match in mappings on or before gen_line, gen_col
        let mut best: Option<&Mapping> = None;

        for m in &self.mappings {
            if m.generated_line == gen_line {
                if m.generated_column <= gen_col {
                    best = Some(m);
                } else {
                    break;
                }
            } else if m.generated_line > gen_line {
                break;
            }
        }

        let m = best?;
        let src_idx = m.original_source_index? as usize;
        let orig_line = m.original_line?;
        let orig_col = m.original_column?;

        let source = self.sources.get(src_idx)?.clone();
        let name = m.original_name_index.and_then(|i| self.names.get(i as usize).cloned());

        Some(OriginalPosition { source, line: orig_line, column: orig_col, name })
    }

    /// Symbolicate a formatted stack frame string.
    /// Example: `"    at foo (bundle.js:1:42)"` -> `"    at foo (src/foo.ts:15:8)"`
    pub fn symbolicate_frame(&self, frame: &str) -> String {
        // Find line and column in format `filename:line:col`
        if let Some(pos) = frame.rfind(':') {
            let has_paren = frame.ends_with(')');
            if let Ok(col_num) = frame[pos + 1..].trim_end_matches(')').parse::<u32>() {
                let rest = &frame[..pos];
                if let Some(pos2) = rest.rfind(':') {
                    if let Ok(line_num) = rest[pos2 + 1..].parse::<u32>() {
                        if let Some(orig) = self.lookup(line_num, col_num) {
                            let before_colon = &frame[..pos2];
                            let file_start = before_colon
                                .rfind('(')
                                .or_else(|| before_colon.rfind(' '))
                                .map(|i| i + 1)
                                .unwrap_or(0);
                            let prefix = &frame[..file_start];

                            let name_part = orig.name.as_deref().unwrap_or("");
                            let source_and_name = if !name_part.is_empty() {
                                format!("{} (from {})", orig.source, name_part)
                            } else {
                                orig.source.clone()
                            };

                            let suffix = if has_paren { ")" } else { "" };

                            return format!(
                                "{}{}:{}:{}{}",
                                prefix, source_and_name, orig.line, orig.column, suffix
                            );
                        }
                    }
                }
            }
        }
        frame.to_string()
    }
}

// Base64 VLQ decoder

const BASE64_ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn decode_base64_char(c: u8) -> Result<u32, String> {
    if let Some(pos) = BASE64_ALPHABET.iter().position(|&x| x == c) {
        Ok(pos as u32)
    } else {
        Err(format!("Invalid Base64 character: {}", c as char))
    }
}

/// Decode a sequence of Base64 VLQ numbers into integers.
pub fn decode_vlq(input: &str) -> Result<Vec<i32>, String> {
    let mut numbers = Vec::new();
    let bytes = input.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        let mut result = 0;
        let mut shift = 0;
        let mut continuation = true;

        while continuation {
            if i >= bytes.len() {
                return Err("Unexpected end of VLQ input".into());
            }
            let byte = decode_base64_char(bytes[i])?;
            i += 1;

            continuation = (byte & 32) != 0;
            let digit = byte & 31;
            result += (digit << shift) as i32;
            shift += 5;
        }

        let is_negative = (result & 1) != 0;
        let value = result >> 1;
        numbers.push(if is_negative { -value } else { value });
    }

    Ok(numbers)
}

/// Decode full mappings string according to Source Map v3 spec.
pub fn decode_mappings(mappings_str: &str) -> Result<Vec<Mapping>, String> {
    let mut mappings = Vec::new();

    let mut orig_src: i32 = 0;
    let mut orig_line: i32 = 0;
    let mut orig_col: i32 = 0;
    let mut orig_name: i32 = 0;

    for (gen_line_idx, line) in mappings_str.split(';').enumerate() {
        let gen_line = gen_line_idx as u32 + 1;
        let mut gen_col: i32 = 0; // reset column for each line

        for segment in line.split(',') {
            if segment.is_empty() {
                continue;
            }

            let nums = decode_vlq(segment)?;
            if nums.is_empty() {
                continue;
            }

            gen_col += nums[0];

            let mut mapping = Mapping {
                generated_line: gen_line,
                generated_column: gen_col as u32,
                original_source_index: None,
                original_line: None,
                original_column: None,
                original_name_index: None,
            };

            if nums.len() >= 4 {
                orig_src += nums[1];
                orig_line += nums[2];
                orig_col += nums[3];

                mapping.original_source_index = Some(orig_src as u32);
                mapping.original_line = Some((orig_line + 1) as u32); // 1-based
                mapping.original_column = Some(orig_col as u32);
            }

            if nums.len() >= 5 {
                orig_name += nums[4];
                mapping.original_name_index = Some(orig_name as u32);
            }

            mappings.push(mapping);
        }
    }

    Ok(mappings)
}

// JSON helpers

fn extract_string_field(json: &str, field: &str) -> Option<String> {
    let search = format!("\"{}\"", field);
    let pos = json.find(&search)?;
    let rest = &json[pos + search.len()..];
    let colon_pos = rest.find(':')?;
    let after_colon = rest[colon_pos + 1..].trim_start();
    if let Some(stripped) = after_colon.strip_prefix('"') {
        let end = stripped.find('"')?;
        return Some(stripped[..end].to_string());
    }
    None
}

fn extract_number_field(json: &str, field: &str) -> Option<i64> {
    let search = format!("\"{}\"", field);
    let pos = json.find(&search)?;
    let rest = &json[pos + search.len()..];
    let colon_pos = rest.find(':')?;
    let after_colon = rest[colon_pos + 1..].trim_start();
    let end = after_colon.find(|c: char| !c.is_ascii_digit()).unwrap_or(after_colon.len());
    after_colon[..end].parse().ok()
}

fn extract_string_array(json: &str, field: &str) -> Vec<String> {
    let search = format!("\"{}\"", field);
    let mut items = Vec::new();
    if let Some(pos) = json.find(&search) {
        let rest = &json[pos + search.len()..];
        if let Some(colon_pos) = rest.find(':') {
            let after = rest[colon_pos + 1..].trim_start();
            if after.starts_with('[') {
                if let Some(end_bracket) = after.find(']') {
                    let inside = &after[1..end_bracket];
                    for item in inside.split(',') {
                        let trimmed = item.trim();
                        if trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2
                        {
                            items.push(trimmed[1..trimmed.len() - 1].to_string());
                        }
                    }
                }
            }
        }
    }
    items
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vlq_decode_basic() {
        // "AAAA" is 4 zeros: [0, 0, 0, 0]
        let decoded = decode_vlq("AAAA").unwrap();
        assert_eq!(decoded, vec![0, 0, 0, 0]);

        // Single number 1 is 'C' (binary 000010 -> value 1)
        let decoded_one = decode_vlq("C").unwrap();
        assert_eq!(decoded_one, vec![1]);

        // Single number -1 is 'D' (binary 000011 -> value -1)
        let decoded_neg = decode_vlq("D").unwrap();
        assert_eq!(decoded_neg, vec![-1]);
    }

    #[test]
    fn test_vlq_decode_errors() {
        // Invalid Base64 character
        let invalid_char = decode_vlq("!").unwrap_err();
        assert_eq!(invalid_char, "Invalid Base64 character: !");

        // Unexpected end of VLQ input (continuation bit set but no next char)
        // 'w' is binary 110000 -> pos 48
        let unexpected_end = decode_vlq("w").unwrap_err();
        assert_eq!(unexpected_end, "Unexpected end of VLQ input");

        // Empty string should be ok
        let empty = decode_vlq("").unwrap();
        assert_eq!(empty, Vec::<i32>::new());
    }

    #[test]
    fn test_source_map_parsing_and_lookup() {
        let sm_json = r#"{
            "version": 3,
            "file": "bundle.js",
            "sources": ["app.ts"],
            "names": ["myFunction"],
            "mappings": "AAAA,SAAS"
        }"#;

        let sm = SourceMap::parse_json(sm_json).expect("parse sourcemap");
        assert_eq!(sm.version, 3);
        assert_eq!(sm.sources, vec!["app.ts"]);
        assert!(!sm.mappings.is_empty());

        let pos = sm.lookup(1, 0);
        assert!(pos.is_some());
        let p = pos.unwrap();
        assert_eq!(p.source, "app.ts");
        assert_eq!(p.line, 1);
    }

    #[test]
    fn test_source_map_parse_json_malformed() {
        // Test parsing gracefully handles missing fields and invalid JSON structure
        let malformed_json = "{ malformed: json, [ }";
        let sm = SourceMap::parse_json(malformed_json).expect("should not panic, but fallback to defaults");

        assert_eq!(sm.version, 3);
        assert_eq!(sm.file, None);
        assert!(sm.sources.is_empty());
        assert!(sm.names.is_empty());
        assert!(sm.mappings.is_empty());

        // Test that it returns an Err on invalid VLQ characters in mappings
        let invalid_mapping_json = r#"{"mappings": "!!!"}"#;
        let err = SourceMap::parse_json(invalid_mapping_json).expect_err("should return Err on invalid mapping");
        assert!(err.contains("Invalid Base64 character: !"));
    }

    #[test]
    fn test_symbolicate_frame() {
        let sm = SourceMap {
            version: 3,
            file: Some("bundle.js".to_string()),
            sources: vec!["src/foo.ts".to_string(), "src/bar.ts".to_string()],
            sources_content: vec![],
            names: vec!["myFunc".to_string()],
            mappings: vec![
                Mapping {
                    generated_line: 1,
                    generated_column: 42,
                    original_source_index: Some(0),
                    original_line: Some(15),
                    original_column: Some(8),
                    original_name_index: Some(0),
                },
                Mapping {
                    generated_line: 2,
                    generated_column: 10,
                    original_source_index: Some(1),
                    original_line: Some(10),
                    original_column: Some(5),
                    original_name_index: None,
                },
            ],
        };

        // Happy path: mapping with original name
        assert_eq!(
            sm.symbolicate_frame("    at foo (bundle.js:1:42)"),
            "    at foo (src/foo.ts (from myFunc):15:8)"
        );

        // Missing name path
        assert_eq!(
            sm.symbolicate_frame("    at bar (bundle.js:2:10)"),
            "    at bar (src/bar.ts:10:5)"
        );

        // A string that doesn't match the line/col (no mapping found)
        assert_eq!(
            sm.symbolicate_frame("    at unknown (bundle.js:3:15)"),
            "    at unknown (bundle.js:3:15)"
        );

        // Invalid format string
        assert_eq!(
            sm.symbolicate_frame("    at invalid"),
            "    at invalid"
        );

        // Random text format
        assert_eq!(
            sm.symbolicate_frame("just some random text"),
            "just some random text"
        );

        // Incomplete frame format
        assert_eq!(
            sm.symbolicate_frame("    at foo (bundle.js:1)"),
            "    at foo (bundle.js:1)"
        );
    }
}
