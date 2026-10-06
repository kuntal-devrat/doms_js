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
}

impl Default for RunnerConfig {
    fn default() -> Self {
        let threads = std::thread::available_parallelism()
            .map(|n| (n.get() / 2).clamp(1, 4))
            .unwrap_or(2);
        Self {
            test262_dir: PathBuf::from("tests/test262/test262"),
            suite_filter: None,
            path_filter: None,
            max_tests: None,
            threads,
            output_file: Some(PathBuf::from("tests/test262/results/summary.json")),
            verbose: false,
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
        let parsed = doms_parser::parse(&full_source);
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

        // 2. Compilation Phase
        let compiled = doms_compiler::compile(&program);
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
        vm.max_instructions = Some(100_000);
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
                    TestStatus::Fail(format!("Runtime error: {}", run_err))
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
                    let status = runner_copy.execute_file(&path);

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
                        eprint!("\r[Live Progress] {:6}/{} ({:5.1}%) | Passed: {:6} ({:5.1}%) | Failed: {:6}    ",
                            comp, total, (comp as f64 * 100.0) / total as f64, p, pct, f);
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

        // Export summary JSON
        if let Some(ref out_path) = self.config.output_file {
            if let Some(parent) = out_path.parent() {
                let _ = fs::create_dir_all(parent);
            }

            let mut categories_json = Vec::new();
            for (cat, st) in &sorted_cats {
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
  "categories": {{
{}
  }}
}}"#,
                total,
                final_passed,
                final_failed,
                final_skipped,
                final_pct,
                elapsed.as_secs_f64(),
                chrono_now_utc(),
                categories_json.join(",\n")
            );

            if fs::write(out_path, json_body).is_ok() {
                println!("Metrics saved to: {}", out_path.display());
            }
        }
    }
}

fn chrono_now_utc() -> String {
    let now = std::time::SystemTime::now();
    let secs = now.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
    format!("{}-epoch", secs)
}
