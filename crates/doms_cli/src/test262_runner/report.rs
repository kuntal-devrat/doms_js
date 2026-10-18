use super::*;

impl Test262Runner {
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
                .stack_size(4 * 1024 * 1024)
                .spawn(move || {
                let mut local_stats = HashMap::<String, CategoryStats>::new();
                for path in chunk {
                    if runner_copy.config.verbose {
                        eprintln!("[START] {}", path.display());
                    }
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

                    let entry = local_stats.entry(cat).or_default();

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
                            entry.total += 1;
                        }
                        TestStatus::Fail(_) => {
                            failed_c.fetch_add(1, Ordering::Relaxed);
                            entry.failed += 1;
                            entry.total += 1;
                        }
                        TestStatus::Skip(_) => {
                            skipped_c.fetch_add(1, Ordering::Relaxed);
                        }
                    }

                    let comp = completed_c.fetch_add(1, Ordering::Relaxed) + 1;
                    if comp % 25 == 0 || comp == total {
                        #[cfg(target_os = "linux")]
                        unsafe {
                            unsafe extern "C" {
                                fn malloc_trim(pad: usize) -> i32;
                            }
                            malloc_trim(0);
                        }
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

                // Merge thread-local stats once per worker, completely eliminating lock contention
                {
                    let mut global_stats = cat_stats_c.lock().unwrap();
                    for (cat, st) in local_stats {
                        let entry = global_stats.entry(cat).or_default();
                        entry.passed += st.passed;
                        entry.failed += st.failed;
                        entry.total += st.total;
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

            // Merge current run's categories (overwriting updated ones only on full suite runs)
            if self.config.path_filter.is_none() && self.config.max_tests.is_none() {
                for (cat, st) in &sorted_cats {
                    merged_categories.insert((*cat).clone(), (*st).clone());
                }
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
