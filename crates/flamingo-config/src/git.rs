use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct GitConfig {
    /// Whether to truncate branch name
    pub max_branch_chars: Option<usize>,

    /// Format template for git info display
    /// Available placeholders: {branch}, {ahead}, {behind}, {dirty}, {stash}, {state}
    #[serde(default = "default_git_format")]
    pub format: String,

    /// Symbol definitions
    #[serde(default)]
    pub symbols: GitSymbols,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GitSymbols {
    /// Symbol for commits ahead of remote
    #[serde(default = "default_remote_symbol")]
    pub remote: String,

    /// Symbol for commits ahead of remote
    #[serde(default = "default_ahead_symbol")]
    pub ahead: String,

    /// Symbol for commits behind remote
    #[serde(default = "default_behind_symbol")]
    pub behind: String,

    /// Symbol for clean working directory
    #[serde(default = "default_clean_symbol")]
    pub clean: String,

    /// Symbol for dirty working directory
    #[serde(default = "default_dirty_symbol")]
    pub dirty: String,

    /// Symbol for stash count
    #[serde(default = "default_stash_symbol")]
    pub stash: String,

    /// Symbol for when in a merging state
    #[serde(default = "default_merging")]
    pub merging: String,

    /// Symbol for when in a rebasing state
    #[serde(default = "default_rebasing")]
    pub rebasing: String,

    /// Symbol for when in a cherry-picking state
    #[serde(default = "default_cherry_picking")]
    pub cherry_picking: String,

    /// Symbol for when in a reverting merging state
    #[serde(default = "default_reverting")]
    pub reverting: String,

    /// Symbol for when in a bisecting state
    #[serde(default = "default_bisecting")]
    pub bisecting: String,

    /// Symbol for when in a detached-head state
    #[serde(default = "default_detached")]
    pub detached_head: String,
}

impl Default for GitConfig {
    fn default() -> Self {
        Self {
            max_branch_chars: None,
            format: default_git_format(),
            symbols: GitSymbols::default(),
        }
    }
}

impl Default for GitSymbols {
    fn default() -> Self {
        Self {
            remote: default_remote_symbol(),
            ahead: default_ahead_symbol(),
            behind: default_behind_symbol(),
            clean: default_clean_symbol(),
            dirty: default_dirty_symbol(),
            stash: default_stash_symbol(),
            merging: default_merging(),
            rebasing: default_rebasing(),
            cherry_picking: default_cherry_picking(),
            reverting: default_reverting(),
            bisecting: default_bisecting(),
            detached_head: default_detached(),
        }
    }
}

// Default functions
fn default_git_format() -> String {
    "$state $branch ($stash:$dirty:$ahead:$behind)".to_string()
}
fn default_remote_symbol() -> String {
    // TODO:
    "".to_string()
}
fn default_ahead_symbol() -> String {
    "↑".to_string()
}
fn default_behind_symbol() -> String {
    "↓".to_string()
}
fn default_clean_symbol() -> String {
    "".to_string()
}
fn default_dirty_symbol() -> String {
    "*".to_string()
}
fn default_stash_symbol() -> String {
    "$".to_string()
}
fn default_merging() -> String {
    "MERGE".to_string()
}
fn default_rebasing() -> String {
    "REBASE".to_string()
}
fn default_cherry_picking() -> String {
    "CHERRY-PICK".to_string()
}
fn default_reverting() -> String {
    "REVERT".to_string()
}
fn default_bisecting() -> String {
    "BISECT".to_string()
}
fn default_detached() -> String {
    "DETACHED".to_string()
}
