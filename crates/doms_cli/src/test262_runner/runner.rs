use super::*;

pub struct Test262Runner {
    pub(super) config: RunnerConfig,
    pub(super) harness_cache: Arc<HashMap<String, String>>,
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
}
