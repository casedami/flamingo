use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct PathConfig {
    pub truncate: TruncateStrategy,
    pub shorten_home: bool,
}

impl Default for PathConfig {
    fn default() -> Self {
        Self {
            truncate: TruncateStrategy::None,
            shorten_home: true,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub enum TruncateStrategy {
    /// Show entire path
    None,

    /// Shorten middle directories
    /// * TailSize=1, DirChars=1:
    ///   ~/path/to/my/files -> ~/p/t/m/files
    Smart {
        /// Number of full directories to show at the end of the path
        #[serde(default = "default_min")]
        tail_size: usize,
        /// Number of chars to show for truncated directories
        #[serde(default = "default_min")]
        dir_chars: usize,
    },

    /// Only show the last N directories
    /// * Size=2:
    ///   ~/path/to/my/files -> my/files
    Tail { size: usize },

    /// Truncate to character limit
    /// * Side=LEFT, Max=10:
    ///   ~/path/to/my/files -> …/my/files
    ///
    /// * Side=RIGHT, Max=15:
    ///   ~/path/to/my/files -> ~/path/…/files
    Length {
        max: usize,
        /// Which side to start the substitution loop:
        /// * Left => start at the head of the path
        /// * Right => start at the parent directory of the current file
        #[serde(default = "default_side")]
        start_side: Side,
        #[serde(default = "default_symbol")]
        symbol: String,
    },

    /// Use different strategies depending on the path length
    Adaptive {
        /// Maximum number of chars for short strategy
        threshold: usize,
        short: Box<TruncateStrategy>,
        long: Box<TruncateStrategy>,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Side {
    Left,
    Right,
}

fn default_side() -> Side {
    Side::Right
}

fn default_min() -> usize {
    1
}
fn default_symbol() -> String {
    "…".to_string()
}
