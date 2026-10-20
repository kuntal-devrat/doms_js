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
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

mod discovery;
mod execute;
mod metadata;
mod module_resolution;
mod report;
mod runner;
mod source;
mod summary;
mod types;

pub use runner::Test262Runner;
pub use types::{CategoryStats, RunnerConfig, TestMetadata, TestStatus};

pub(super) use module_resolution::validate_module_resolution;
pub(super) use summary::{chrono_now_utc, load_existing_categories};
