#[derive(Debug)]
pub enum GitError {
    NotARepository,
    CommandFailed(String),
    ParseError(String),
}

impl std::fmt::Display for GitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GitError::NotARepository => write!(f, "Not a git repository"),
            GitError::CommandFailed(msg) => write!(f, "Git command failed: {msg}"),
            GitError::ParseError(msg) => write!(f, "Failed to parse git output: {msg}"),
        }
    }
}

impl std::error::Error for GitError {}

#[derive(Debug)]
pub enum ConfigError {
    NotFound,
    IoError(std::io::Error),
    ParseError(toml::de::Error),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::NotFound => write!(f, "Config file not found"),
            ConfigError::IoError(err) => write!(f, "IO error: {err}"),
            ConfigError::ParseError(err) => write!(f, "Parse error: {err}"),
        }
    }
}

impl std::error::Error for ConfigError {}
