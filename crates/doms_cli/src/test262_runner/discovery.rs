use super::*;

impl Test262Runner {
    /// Discover test files according to configuration.
    pub fn discover_tests(&self) -> Vec<PathBuf> {
        let base_test_dir = self.config.test262_dir.join("test");
        let start_dir = if let Some(ref suite) = self.config.suite_filter {
            base_test_dir.join(suite)
        } else {
            base_test_dir
        };

        let mut tests = Vec::new();
        self.collect_tests_recursive(&start_dir, &mut tests);

        if let Some(ref pat) = self.config.path_filter {
            tests.retain(|p| p.to_string_lossy().contains(pat));
        }

        tests.sort();

        if let Some(max) = self.config.max_tests {
            if tests.len() > max {
                tests.truncate(max);
            }
        }

        tests
    }

    fn collect_tests_recursive(&self, dir: &Path, list: &mut Vec<PathBuf>) {
        if dir.is_file() {
            list.push(dir.to_path_buf());
            return;
        }
        if !dir.is_dir() {
            return;
        }
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    self.collect_tests_recursive(&p, list);
                } else if p.extension().and_then(|s| s.to_str()) == Some("js") {
                    list.push(p);
                }
            }
        }
    }

    /// Extract category from relative path.
    pub(super) fn extract_category(base: &Path, test_path: &Path) -> String {
        if let Ok(rel) = test_path.strip_prefix(base) {
            let comps: Vec<&str> = rel.iter().filter_map(|s| s.to_str()).collect();
            if comps.len() >= 3 {
                format!("{}/{}", comps[0], comps[1])
            } else if !comps.is_empty() {
                comps[0].to_string()
            } else {
                "misc".to_string()
            }
        } else {
            "misc".to_string()
        }
    }
}
