use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Deserialize, Serialize)]
pub struct ProcessConfig {
    /// Number of threads for parallel operations
    #[serde(default = "default_thread_count")]
    pub num_threads: usize,

    /// Session key override
    pub session_key: Option<String>,

    /// Log level: trace, debug, info, warn, error
    #[serde(default = "default_log_level")]
    pub min_log_level: String,

    /// Overwrite default log directory (optional)
    pub log_dir: Option<PathBuf>,
}

impl Default for ProcessConfig {
    fn default() -> Self {
        Self {
            num_threads: default_thread_count(),
            session_key: None,
            min_log_level: default_log_level(),
            log_dir: None,
        }
    }
}

fn default_log_level() -> String {
    "warn".to_string()
}

fn default_thread_count() -> usize {
    8
}
