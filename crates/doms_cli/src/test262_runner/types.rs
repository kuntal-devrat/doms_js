use super::*;

#[derive(Debug, Clone, Default)]
pub struct TestMetadata {
    pub description: Option<String>,
    pub flags: Vec<String>,
    pub includes: Vec<String>,
    pub features: Vec<String>,
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
