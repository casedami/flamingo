use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct PathConfig {
    /// Defines how the path should be formatted
    pub truncate: TruncateStrategy,
    /// If true, replaces $HOME with "~"
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

    /// Shorten intermediary directories
    ///
    /// # Examples
    ///
    /// ```
    /// let strategy = TruncateStrategy.Smart(tail_size: 1, dir_chars: 1);
    /// // ~/path/to/my/files -> ~/p/t/m/files
    /// ```
    Smart {
        /// Number of full directories to show at the end of the path
        #[serde(default = "default_min")]
        tail_size: usize,
        /// Number of chars to show for truncated directories
        #[serde(default = "default_min")]
        dir_chars: usize,
    },

    /// Only show the last N directories
    ///
    /// # Examples
    ///
    /// ```
    /// let strategy = TruncateStrategy.Tail(size: 2);
    /// // ~/path/to/my/files -> my/files
    /// ```
    Tail { size: usize },

    /// Truncate to character limit
    ///
    /// # Examples
    ///
    /// ```
    /// let strategy = TruncateStrategy.Length(side: Side.Left, max: 10, symbol: "…");
    /// // ~/path/to/my/files -> …/my/files
    ///
    /// let strategy = TruncateStrategy.Length(side: Side.Right, max: 15, symbol: "…");
    /// // ~/path/to/my/files -> ~/path/…/files
    /// ```
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
    ///
    /// # Examples
    ///
    /// ```
    /// let strategy = TruncateStrategy.Adaptive(
    ///     threshold: 10
    ///     short: TruncateStrategy.Tail(size: 2),
    ///     long: TruncateStrategy.Smart(tail_size: 1, dir_chars: 1)
    /// );
    ///
    /// // ~/path/to -> path/to
    /// // ~/path/to/my/files -> ~/p/t/m/files
    /// ```
    Adaptive {
        /// Maximum number of chars before using long strategy
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
