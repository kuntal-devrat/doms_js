//! Comprehensive Test262 ECMAScript Conformance Test Runner.
//!
//! Features:
//! - Multi-threaded test execution using hardware threads
//! - In-memory harness caching (`sta.js`, `assert.js`, includes)
//! - Full YAML frontmatter metadata extraction (flags, includes, negative phases)
//! - Live real-time terminal progress indicators
//! - Category-level metric breakdown and summary export to `results/summary.json`

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::io::IsTerminal;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Debug, Clone, Default)]
pub struct TestMetadata {
    pub description: Option<String>,
    pub flags: Vec<String>,
    pub includes: Vec<String>,
    pub is_raw: bool,
    pub is_only_strict: bool,
    pub is_module: bool,
    pub negative_phase: Option<String>,
    pub negative_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TestStatus {
    Pass,
    Fail(String),
    Skip(String),
}

#[derive(Debug, Clone, Default)]
pub struct CategoryStats {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
}

#[derive(Debug, Clone)]
pub struct RunnerConfig {
    pub test262_dir: PathBuf,
    pub suite_filter: Option<String>,
    pub path_filter: Option<String>,
    pub max_tests: Option<usize>,
    pub threads: usize,
    pub output_file: Option<PathBuf>,
    pub verbose: bool,
    pub clean_summary: bool,
}

impl Default for RunnerConfig {
    fn default() -> Self {
        Self {
            test262_dir: PathBuf::from("tests/test262/test262"),
            suite_filter: None,
            path_filter: None,
            max_tests: None,
            threads: 1, // Safe single-threaded by default to prevent laptop freezes
            output_file: Some(PathBuf::from("tests/test262/results/summary.json")),
            verbose: false,
            clean_summary: false,
        }
    }
}

pub struct Test262Runner {
    config: RunnerConfig,
    harness_cache: Arc<HashMap<String, String>>,
}

impl Test262Runner {
    pub fn new(config: RunnerConfig) -> Self {
        let harness_dir = config.test262_dir.join("harness");
        let mut harness_cache = HashMap::new();

        if harness_dir.is_dir() {
            if let Ok(entries) = fs::read_dir(&harness_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|s| s.to_str()) == Some("js") {
                        if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                            if let Ok(content) = fs::read_to_string(&path) {
                                harness_cache.insert(name.to_string(), content);
                            }
                        }
                    }
                }
            }
        }

        Self {
            config,
            harness_cache: Arc::new(harness_cache),
        }
    }

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

    /// Prepare full JS source for execution with injected harness.
    pub fn build_source(&self, content: &str, meta: &TestMetadata) -> String {
        if meta.is_raw {
            return content.to_string();
        }

        let mut out = String::with_capacity(content.len() + 8192);

        if meta.is_only_strict {
            out.push_str("\"use strict\";\n");
        }

        // Prepend sta.js and assert.js
        if let Some(sta) = self.harness_cache.get("sta.js") {
            out.push_str(sta);
            out.push('\n');
        }
        if let Some(assert_js) = self.harness_cache.get("assert.js") {
            out.push_str(assert_js);
            out.push('\n');
        }

        // Host object shim for $262
        out.push_str("var $262 = typeof $262 !== 'undefined' ? $262 : {};\n");
        out.push_str("if (!$262.global) $262.global = typeof globalThis !== 'undefined' ? globalThis : this;\n");
        out.push_str("if (!$262.createRealm || !$262.createRealm().global) {\n");
        out.push_str("    $262.createRealm = function() {\n");
        out.push_str("        var g = typeof globalThis !== 'undefined' ? globalThis : this;\n");
        out.push_str("        return {\n");
        out.push_str("            global: g,\n");
        out.push_str("            evalScript: function(src) { return eval(src); }\n");
        out.push_str("        };\n");
        out.push_str("    };\n");
        out.push_str("}\n");
        out.push_str("if (!$262.evalScript) $262.evalScript = function(src) { return eval(src); };\n");
        out.push_str("if (!$262.gc) $262.gc = function() {};\n");
        out.push_str("if (!$262.agent) $262.agent = {};\n");

        // Additional includes
        for inc in &meta.includes {
            if let Some(inc_content) = self.harness_cache.get(inc) {
                out.push_str(inc_content);
                out.push('\n');
            }
        }

        out.push_str(content);
        out
    }

    /// Execute a single test file and return TestStatus.
    pub fn execute_file(&self, path: &Path) -> TestStatus {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.execute_file_inner(path)
        }));
        match result {
            Ok(status) => status,
            Err(_) => TestStatus::Fail("Panic during test execution".into()),
        }
    }

    fn execute_file_inner(&self, path: &Path) -> TestStatus {
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => return TestStatus::Fail(format!("Failed to read file: {}", e)),
        };

        // Skip fixtures
        if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
            if name.contains("_FIXTURE") {
                return TestStatus::Skip("Fixture file".into());
            }
        }

        let meta = Self::parse_metadata(&content);
        let full_source = self.build_source(&content, &meta);

        // 1. Parsing Phase
        let parsed = if meta.is_module {
            doms_parser::parse_module(&full_source)
        } else if meta.is_only_strict {
            doms_parser::parse_strict(&full_source)
        } else {
            doms_parser::parse(&full_source)
        };
        if let Err(parse_err) = parsed {
            if meta.negative_phase.as_deref() == Some("parse") {
                return TestStatus::Pass;
            }
            return TestStatus::Fail(format!("Parse error: {}", parse_err));
        }

        let program = parsed.unwrap();
        if meta.negative_phase.as_deref() == Some("parse") {
            return TestStatus::Fail("Expected parse error, but parsed successfully".into());
        }

        let is_top_level_strict = meta.is_only_strict
            || meta.is_module
            || program.body.iter()
                .take_while(|s| matches!(s, doms_parser::ast::Statement::Expression(doms_parser::ast::Expression::Literal(doms_parser::ast::Literal::String { .. }))))
                .any(|s| matches!(s, doms_parser::ast::Statement::Expression(doms_parser::ast::Expression::Literal(doms_parser::ast::Literal::String { value, has_escape: false })) if value == "use strict"));

        // 2. Compilation Phase
        let compiled = if is_top_level_strict {
            doms_compiler::compile_strict(&program)
        } else {
            doms_compiler::compile(&program)
        };
        if let Err(comp_err) = compiled {
            if meta.negative_phase.as_deref() == Some("parse")
                || meta.negative_phase.as_deref() == Some("runtime")
            {
                return TestStatus::Pass;
            }
            return TestStatus::Fail(format!("Compile error: {}", comp_err));
        }

        let chunk = compiled.unwrap();

        // 3. Execution Phase
        let mut vm = doms_vm::Vm::new();
        vm.is_strict = is_top_level_strict;
        vm.max_instructions = Some(10_000_000);
        vm.max_objects = Some(20_000);
        vm.max_evals = Some(100_000);
        vm.deadline = Some(Instant::now() + std::time::Duration::from_millis(2500));
        let result = vm.run(&chunk);

        match result {
            Ok(_) => {
                if meta.negative_phase.as_deref() == Some("runtime") {
                    TestStatus::Fail("Expected runtime error, but executed successfully".into())
                } else {
                    TestStatus::Pass
                }
            }
            Err(run_err) => {
                if meta.negative_phase.as_deref() == Some("runtime") {
                    TestStatus::Pass
                } else {
                    let msg = match run_err {
                        doms_vm::VmError::Thrown(val) => {
                            let str_val = vm.to_string_value(val);
                            let name_prop = vm.get_property(val, "name");
                            let message_prop = vm.get_property(val, "message");
                            if !message_prop.is_undefined() {
                                if !name_prop.is_undefined() {
                                    format!("{}: {}", vm.to_string_value(name_prop), vm.to_string_value(message_prop))
                                } else {
                                    format!("{}: {}", str_val, vm.to_string_value(message_prop))
                                }
                            } else {
                                str_val
                            }
                        }
                        other => other.to_string(),
                    };
                    TestStatus::Fail(format!("Runtime error: {}", msg))
                }
            }
        }
    }

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
    fn extract_category(base: &Path, test_path: &Path) -> String {
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

    /// Run the test suite with parallel worker threads and real-time live metrics.
    pub fn run(&self) {
        let start_time = Instant::now();
        let tests = self.discover_tests();
        let total = tests.len();

        println!("============================================================");
        println!("DOMS JavaScript Engine — Official Test262 Conformance Runner");
        println!("============================================================");
        println!("Test directory : {}", self.config.test262_dir.display());
        println!("Parallel workers: {}", self.config.threads);
        println!("Tests discovered: {}", total);
        if let Some(ref suite) = self.config.suite_filter {
            println!("Suite filter    : {}", suite);
        }
        if let Some(ref filter) = self.config.path_filter {
            println!("Path filter     : {}", filter);
        }
        println!("============================================================");

        if total == 0 {
            println!("No test files found matching criteria.");
            return;
        }

        let passed_count = Arc::new(AtomicUsize::new(0));
        let failed_count = Arc::new(AtomicUsize::new(0));
        let skipped_count = Arc::new(AtomicUsize::new(0));
        let completed_count = Arc::new(AtomicUsize::new(0));

        let category_stats = Arc::new(Mutex::new(HashMap::<String, CategoryStats>::new()));
        let base_test_dir = self.config.test262_dir.join("test");

        let chunk_size = total.div_ceil(self.config.threads);
        let test_chunks: Vec<Vec<PathBuf>> = tests.chunks(chunk_size).map(|c| c.to_vec()).collect();

        let mut handles = Vec::new();

        for chunk in test_chunks {
            let runner_copy = Self {
                config: self.config.clone(),
                harness_cache: Arc::clone(&self.harness_cache),
            };
            let passed_c = Arc::clone(&passed_count);
            let failed_c = Arc::clone(&failed_count);
            let skipped_c = Arc::clone(&skipped_count);
            let completed_c = Arc::clone(&completed_count);
            let cat_stats_c = Arc::clone(&category_stats);
            let base_dir = base_test_dir.clone();

            let handle = std::thread::Builder::new()
                .name(format!("test262-worker-{}", handles.len()))
                .stack_size(16 * 1024 * 1024)
                .spawn(move || {
                for path in chunk {
                    let cat = Self::extract_category(&base_dir, &path);
                    let status = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| runner_copy.execute_file(&path))) {
                        Ok(s) => s,
                        Err(e) => {
                            let msg = if let Some(s) = e.downcast_ref::<&str>() {
                                s.to_string()
                            } else if let Some(s) = e.downcast_ref::<String>() {
                                s.clone()
                            } else {
                                "Test execution panicked".to_string()
                            };
                            TestStatus::Fail(format!("Worker panic: {}", msg))
                        }
                    };

                    let mut stats = cat_stats_c.lock().unwrap();
                    let entry = stats.entry(cat).or_default();
                    entry.total += 1;

                    if runner_copy.config.verbose {
                        match &status {
                            TestStatus::Pass => eprintln!("[PASS] {}", path.display()),
                            TestStatus::Fail(msg) => eprintln!("[FAIL] {}: {}", path.display(), msg),
                            TestStatus::Skip(msg) => eprintln!("[SKIP] {}: {}", path.display(), msg),
                        }
                    }

                    match status {
                        TestStatus::Pass => {
                            passed_c.fetch_add(1, Ordering::Relaxed);
                            entry.passed += 1;
                        }
                        TestStatus::Fail(_) => {
                            failed_c.fetch_add(1, Ordering::Relaxed);
                            entry.failed += 1;
                        }
                        TestStatus::Skip(_) => {
                            skipped_c.fetch_add(1, Ordering::Relaxed);
                        }
                    }

                    let comp = completed_c.fetch_add(1, Ordering::Relaxed) + 1;
                    if comp % 100 == 0 || comp == total {
                        let p = passed_c.load(Ordering::Relaxed);
                        let f = failed_c.load(Ordering::Relaxed);
                        let pct = if comp > 0 { (p as f64 * 100.0) / comp as f64 } else { 0.0 };
                        if std::io::stderr().is_terminal() {
                            eprint!("\r[Live Progress] {:6}/{} ({:5.1}%) | Passed: {:6} ({:5.1}%) | Failed: {:6}    ",
                                comp, total, (comp as f64 * 100.0) / total as f64, p, pct, f);
                        } else if comp % 500 == 0 || comp == total {
                            eprintln!("[Progress] {:6}/{} ({:5.1}%) | Passed: {:6} ({:5.1}%) | Failed: {:6}",
                                comp, total, (comp as f64 * 100.0) / total as f64, p, pct, f);
                        }
                    }
                }
            }).expect("Failed to spawn thread");

            handles.push(handle);
        }

        for h in handles {
            let _ = h.join();
        }

        eprintln!(); // newline after live progress
        let elapsed = start_time.elapsed();

        let final_passed = passed_count.load(Ordering::SeqCst);
        let final_failed = failed_count.load(Ordering::SeqCst);
        let final_skipped = skipped_count.load(Ordering::SeqCst);
        let final_pct = if total > 0 {
            (final_passed as f64 * 100.0) / total as f64
        } else {
            0.0
        };

        println!("\n============================================================");
        println!("Test262 Category Breakdown & Live Metrics");
        println!("============================================================");
        println!("{:<36} | {:>7} | {:>7} | {:>7} | {:>8}", "Category", "Total", "Pass", "Fail", "Pass %");
        println!("{:-<36}-+-{:-<7}-+-{:-<7}-+-{:-<7}-+-{:-<8}", "", "", "", "", "");

        let stats_map = category_stats.lock().unwrap();
        let mut sorted_cats: Vec<_> = stats_map.iter().collect();
        sorted_cats.sort_by_key(|(k, _)| (*k).clone());

        for (cat, st) in &sorted_cats {
            let pct = if st.total > 0 {
                (st.passed as f64 * 100.0) / st.total as f64
            } else {
                0.0
            };
            println!("{:<36} | {:>7} | {:>7} | {:>7} | {:>7.1}%", cat, st.total, st.passed, st.failed, pct);
        }

        println!("============================================================");
        println!(
            "Total Evaluated: {} | Passed: {} | Failed: {} | Skipped: {}",
            total, final_passed, final_failed, final_skipped
        );
        println!("Overall Conformance Rate: {:.2}%", final_pct);
        println!("Time Elapsed            : {:.2}s ({:.1} tests/sec)", elapsed.as_secs_f64(), total as f64 / elapsed.as_secs_f64().max(0.001));
        println!("============================================================");

        // Export summary JSON with persistent category merging
        if let Some(ref out_path) = self.config.output_file {
            if let Some(parent) = out_path.parent() {
                let _ = fs::create_dir_all(parent);
            }

            // Load existing categories if summary merging is enabled
            let mut merged_categories: HashMap<String, CategoryStats> = if !self.config.clean_summary && out_path.is_file() {
                load_existing_categories(out_path)
            } else {
                HashMap::new()
            };

            // Merge current run's categories (overwriting updated ones)
            for (cat, st) in &sorted_cats {
                merged_categories.insert((*cat).clone(), (*st).clone());
            }

            // Calculate overall cumulative stats across ALL recorded categories
            let mut cumulative_total = 0;
            let mut cumulative_passed = 0;
            let mut cumulative_failed = 0;
            for st in merged_categories.values() {
                cumulative_total += st.total;
                cumulative_passed += st.passed;
                cumulative_failed += st.failed;
            }
            let cumulative_pct = if cumulative_total > 0 {
                (cumulative_passed as f64 * 100.0) / cumulative_total as f64
            } else {
                0.0
            };

            let mut sorted_merged: Vec<_> = merged_categories.iter().collect();
            sorted_merged.sort_by_key(|(k, _)| (*k).clone());

            let mut categories_json = Vec::new();
            for (cat, st) in &sorted_merged {
                let pct = if st.total > 0 { (st.passed as f64 * 100.0) / st.total as f64 } else { 0.0 };
                categories_json.push(format!(
                    "    \"{}\": {{ \"total\": {}, \"passed\": {}, \"failed\": {}, \"percentage\": {:.2} }}",
                    cat, st.total, st.passed, st.failed, pct
                ));
            }

            let json_body = format!(
                r#"{{
  "suite": "Official Test262",
  "total": {},
  "passed": {},
  "failed": {},
  "skipped": {},
  "percentage": {:.2},
  "elapsed_seconds": {:.2},
  "timestamp": "{}",
  "session": {{
    "total": {},
    "passed": {},
    "failed": {},
    "percentage": {:.2}
  }},
  "categories": {{
{}
  }}
}}"#,
                cumulative_total,
                cumulative_passed,
                cumulative_failed,
                final_skipped,
                cumulative_pct,
                elapsed.as_secs_f64(),
                chrono_now_utc(),
                total,
                final_passed,
                final_failed,
                final_pct,
                categories_json.join(",\n")
            );

            if fs::write(out_path, json_body).is_ok() {
                println!("Metrics saved to: {} (cumulative categories: {})", out_path.display(), merged_categories.len());
                if merged_categories.len() > sorted_cats.len() {
                    println!("Cumulative Across All Categories: {} tests | Passed: {} | Conformance Rate: {:.2}%",
                        cumulative_total, cumulative_passed, cumulative_pct);
                }
            }
        }
    }
}

fn load_existing_categories(path: &Path) -> HashMap<String, CategoryStats> {
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

fn chrono_now_utc() -> String {
    let now = std::time::SystemTime::now();
    let secs = now.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
    format!("{}-epoch", secs)
}

