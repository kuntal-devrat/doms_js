use super::*;

impl Test262Runner {
    /// Parse YAML frontmatter delimited by `/*---` and `---*/`.
    pub fn parse_metadata(content: &str) -> TestMetadata {
        let mut meta = TestMetadata::default();
        let start_tag = "/*---";
        let end_tag = "---*/";

        let start_idx = match content.find(start_tag) {
            Some(i) => i + start_tag.len(),
            None => return meta,
        };

        let end_idx = match content[start_idx..].find(end_tag) {
            Some(i) => start_idx + i,
            None => return meta,
        };

        let yaml = &content[start_idx..end_idx];
        let mut in_negative = false;

        for raw_line in yaml.lines() {
            let line = raw_line.trim();

            if line.starts_with("description:") {
                let desc = line.trim_start_matches("description:").trim();
                meta.description = Some(desc.to_string());
                in_negative = false;
            } else if line.starts_with("flags:") {
                in_negative = false;
                let flags_str = line.trim_start_matches("flags:").trim();
                if flags_str.starts_with('[') && flags_str.ends_with(']') {
                    let inside = &flags_str[1..flags_str.len() - 1];
                    for f in inside.split(',') {
                        let flag = f.trim();
                        if !flag.is_empty() {
                            meta.flags.push(flag.to_string());
                            if flag == "raw" {
                                meta.is_raw = true;
                            } else if flag == "onlyStrict" {
                                meta.is_only_strict = true;
                            } else if flag == "module" {
                                meta.is_module = true;
                            }
                        }
                    }
                }
            } else if line.starts_with("includes:") {
                in_negative = false;
                let inc_str = line.trim_start_matches("includes:").trim();
                if inc_str.starts_with('[') && inc_str.ends_with(']') {
                    let inside = &inc_str[1..inc_str.len() - 1];
                    for i in inside.split(',') {
                        let inc = i.trim();
                        if !inc.is_empty() {
                            meta.includes.push(inc.to_string());
                        }
                    }
                }
            } else if line.starts_with("features:") {
                in_negative = false;
                let feat_str = line.trim_start_matches("features:").trim();
                if feat_str.starts_with('[') && feat_str.ends_with(']') {
                    let inside = &feat_str[1..feat_str.len() - 1];
                    for f in inside.split(',') {
                        let feat = f.trim();
                        if !feat.is_empty() {
                            meta.features.push(feat.to_string());
                        }
                    }
                }
            } else if line.starts_with("negative:") {
                in_negative = true;
            } else if in_negative {
                if line.starts_with("phase:") {
                    let p = line.trim_start_matches("phase:").trim();
                    meta.negative_phase = Some(p.to_string());
                } else if line.starts_with("type:") {
                    let t = line.trim_start_matches("type:").trim();
                    meta.negative_type = Some(t.to_string());
                }
            }
        }

        meta
    }
}
