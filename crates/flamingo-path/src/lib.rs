use flamingo_config::path::{PathConfig, Side, TruncateStrategy};
use std::env;
use std::path::{Path, PathBuf};

/// Returns the current path formatted according to a specified configuration
pub fn path(config: &PathConfig) -> String {
    let mut cwd = cwd();

    if config.shorten_home {
        cwd = substitute_home_dir(cwd);
    }

    apply_truncation_strategy(&cwd, &config.truncate)
}

/// Returns the current working directory
///
/// Uses the $PWD environment variable because it preserves symlinks, with a fallback to
/// `std::env::current_dir`. In the that both methods fail, the root directory is returned.
fn cwd() -> PathBuf {
    env::var("PWD")
        .map(PathBuf::from)
        .ok()
        .filter(|p| p.exists())
        .or_else(|| env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("/"))
}

/// Substitutes $HOME with "~"
///
/// Takes ownership of the path argument. In the case that the home directory is unable to be
/// located, ownership of the original path argument is returned. Otherwise, a new PathBuf is
/// returned with $HOME properly substituted.
fn substitute_home_dir(path: PathBuf) -> PathBuf {
    if let Some(home_dir) = dirs::home_dir() {
        if let Ok(relative_path) = path.strip_prefix(&home_dir) {
            return PathBuf::from("~").join(relative_path);
        }
    }
    path
}

/// Applies a truncation strategy to a Path and returns a String
///
/// See flamingo-config::path for exmaples.
fn apply_truncation_strategy(path: &Path, strategy: &TruncateStrategy) -> String {
    let path_str = path.to_string_lossy();

    match strategy {
        TruncateStrategy::None => path_str.to_string(),

        TruncateStrategy::Smart {
            tail_size,
            dir_chars,
        } => truncate_smart(path, *tail_size, *dir_chars),

        TruncateStrategy::Tail { size } => truncate_tail(path, *size),

        TruncateStrategy::Length {
            max,
            start_side,
            symbol,
        } => truncate_length(&path_str, *max, start_side, symbol),

        TruncateStrategy::Adaptive {
            threshold,
            short,
            long,
        } => {
            if path_str.len() <= *threshold {
                truncate_adaptive(path, short)
            } else {
                truncate_adaptive(path, long)
            }
        }
    }
}

fn truncate_adaptive(path: &Path, strategy: &TruncateStrategy) -> String {
    let path_str = path.to_string_lossy();

    match strategy {
        TruncateStrategy::None => path_str.to_string(),
        TruncateStrategy::Smart {
            tail_size,
            dir_chars,
        } => truncate_smart(path, *tail_size, *dir_chars),
        TruncateStrategy::Tail { size } => truncate_tail(path, *size),
        TruncateStrategy::Length {
            max,
            start_side,
            symbol,
        } => truncate_length(&path_str, *max, start_side, symbol),
        TruncateStrategy::Adaptive { .. } => {
            // Prevent infinite recursion by defaulting to None for nested Adaptive
            log::info!(
                "Nested adaptive strategy is not supported. 
                    Please use a different strategy. Using NONE as fallback."
            );
            path_str.to_string()
        }
    }
}

fn truncate_smart(path: &Path, tail_size: usize, dir_chars: usize) -> String {
    let components: Vec<_> = path.components().collect();

    if components.len() <= tail_size {
        return path.to_string_lossy().to_string();
    }

    let mut result = Vec::new();

    // truncate all components except for tail
    for comp in &components[..components.len() - tail_size] {
        let comp = comp.as_os_str().to_string_lossy();
        if comp == "/" {
            result.push(String::from(""));
        } else if comp.len() <= dir_chars {
            result.push(comp.to_string());
        } else {
            result.push(comp.chars().take(dir_chars).collect());
        }
    }

    // add tail components
    for comp in &components[components.len() - tail_size..] {
        result.push(comp.as_os_str().to_string_lossy().to_string());
    }

    result.join("/")
}

fn truncate_tail(path: &Path, size: usize) -> String {
    let components: Vec<_> = path.components().collect();

    if components.len() <= size {
        return path.to_string_lossy().to_string();
    }

    let tail_components = &components[components.len() - size..];
    tail_components
        .iter()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn truncate_length(path: &str, max: usize, start_side: &Side, symbol: &str) -> String {
    if path.len() <= max {
        return path.to_string();
    }

    match start_side {
        Side::Left => {
            let res = path
                .split('/')
                .rfold((String::new(), 0), |(mut acc, len), component| {
                    let new_len = if len == 0 {
                        component.len()
                    } else {
                        len + component.len() + 1 // +1 for separator
                    };

                    if new_len < max {
                        if !acc.is_empty() {
                            acc = format!("{component}/{acc}");
                        } else {
                            acc = component.to_string();
                        }
                        (acc, new_len)
                    } else {
                        (acc, len) // Stop accumulating, keep current result
                    }
                })
                .0;
            format!("{symbol}{res}")
        }
        Side::Right => {
            let tail_len = path.split('/').next_back().unwrap_or_default().len();
            let adjusted_max = max.saturating_sub(tail_len);
            if adjusted_max == 0 {
                return String::from("");
            }

            let res = path
                .split('/')
                .scan(0usize, |acc_len, component| {
                    let new_len = if *acc_len == 0 {
                        component.len()
                    } else {
                        *acc_len + component.len() + 1 // +1 to account for separator
                    };

                    if new_len < adjusted_max {
                        *acc_len = new_len;
                        Some(component)
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
                .join("/");
            format!("{res}/{symbol}/{}", &path[path.len() - tail_len..])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flamingo_config::path::{PathConfig, Side, TruncateStrategy};
    use std::path::PathBuf;

    fn create_test_config(truncate: TruncateStrategy, shorten_home: bool) -> PathConfig {
        PathConfig {
            truncate,
            shorten_home,
        }
    }

    #[test]
    fn test_smart_truncation() {
        // Test long path gets truncated properly
        let path = PathBuf::from("/very/long/path/to/some/deep/directory/structure");
        let result = truncate_smart(&path, 2, 1);
        assert_eq!(result, "/v/l/p/t/s/d/directory/structure");

        // Test short path remains unchanged
        let short_path = PathBuf::from("/home/user");
        let result = truncate_smart(&short_path, 2, 1);
        assert_eq!(result, "/home/user");

        // Test edge case: path length equals tail_size
        let equal_path = PathBuf::from("/home/user");
        let result = truncate_smart(&equal_path, 2, 1);
        assert_eq!(result, "/home/user");
    }

    #[test]
    fn test_tail_truncation() {
        // Test long path gets truncated to last 3 components
        let path = PathBuf::from("/very/long/path/to/some/directory");
        let result = truncate_tail(&path, 3);
        assert_eq!(result, "to/some/directory");

        // Test short path remains unchanged
        let short_path = PathBuf::from("/home/user");
        let result = truncate_tail(&short_path, 3);
        assert_eq!(result, "/home/user");

        // Test single component
        let single_path = PathBuf::from("/");
        let result = truncate_tail(&single_path, 3);
        assert_eq!(result, "/");
    }

    #[test]
    fn test_length_truncation_right() {
        // Test truncation from right side
        let long_path = "/very/long/path/to/directory";
        let result = truncate_length(long_path, 20, &Side::Right, "…");
        assert_eq!(result, "/very/long/…/directory");

        // Test path shorter than max length
        let short_path = "/short";
        let result = truncate_length(short_path, 10, &Side::Right, "…");
        assert_eq!(result, "/short");

        // Test edge case: exactly max length
        let exact_path = "/exactly/20/chars";
        assert_eq!(exact_path.len(), 17);
        let result = truncate_length(exact_path, 17, &Side::Right, "…");
        assert_eq!(result, "/exactly/20/chars");
    }

    #[test]
    fn test_length_truncation_left() {
        // Test truncation from left side
        let long_path = "/very/long/path/to/directory";
        let result = truncate_length(long_path, 20, &Side::Left, "…");
        assert_eq!(result, "…/path/to/directory");
    }

    #[test]
    fn test_home_shortening() {
        // This test assumes we can get the home directory
        if let Some(home_dir) = dirs::home_dir() {
            let home_subpath = home_dir.join("Documents").join("projects");
            let result = substitute_home_dir(home_subpath);
            assert_eq!(result, PathBuf::from("~/Documents/projects"));

            // Test path not under home directory
            let other_path = PathBuf::from("/usr/local/bin");
            let result = substitute_home_dir(other_path.clone());
            assert_eq!(result, other_path);

            // Test exact home directory
            let result = substitute_home_dir(home_dir);
            assert_eq!(result, PathBuf::from("~"));
        }
    }

    #[test]
    fn test_adaptive_truncation() {
        let short_strategy = TruncateStrategy::None;
        let long_strategy = TruncateStrategy::Tail { size: 2 };

        let config = create_test_config(
            TruncateStrategy::Adaptive {
                threshold: 15,
                short: Box::new(short_strategy),
                long: Box::new(long_strategy),
            },
            false,
        );

        // Test short path uses short strategy (None)
        let short_path = PathBuf::from("/short/path");
        let result = apply_truncation_strategy(&short_path, &config.truncate);
        assert_eq!(result, "/short/path");

        // Test long path uses long strategy (Tail)
        let long_path = PathBuf::from("/very/long/path/to/some/directory");
        let result = apply_truncation_strategy(&long_path, &config.truncate);
        assert_eq!(result, "some/directory");
    }

    #[test]
    fn test_no_truncation() {
        let config = create_test_config(TruncateStrategy::None, false);
        let path = PathBuf::from("/very/long/path/to/directory");
        let result = apply_truncation_strategy(&path, &config.truncate);
        assert_eq!(result, "/very/long/path/to/directory");
    }

    #[test]
    fn test_edge_cases() {
        let config = create_test_config(
            TruncateStrategy::Smart {
                tail_size: 1,
                dir_chars: 1,
            },
            false,
        );

        // Test root path
        let root_path = PathBuf::from("/");
        let result = apply_truncation_strategy(&root_path, &config.truncate);
        assert_eq!(result, "/");

        // Test empty path
        let empty_path = PathBuf::from("");
        let result = apply_truncation_strategy(&empty_path, &config.truncate);
        assert_eq!(result, "");

        // Test single directory
        let single_path = PathBuf::from("directory");
        let result = apply_truncation_strategy(&single_path, &config.truncate);
        assert_eq!(result, "directory");
    }

    #[test]
    fn test_length_truncation_edge_cases() {
        // Test when symbol is longer than max length
        let short_path = "/test";
        let result = truncate_length(short_path, 2, &Side::Right, "...");
        assert_eq!(result, "");

        // Test empty string
        let empty_path = "";
        let result = truncate_length(empty_path, 10, &Side::Right, "…");
        assert_eq!(result, "");
    }

    #[test]
    fn test_smart_truncation_edge_cases() {
        let path = PathBuf::from("/very/long/path/to/directory");
        let result = truncate_smart(&path, 2, 0);
        // With 0 dir_chars, truncated directories should be empty
        // This might not be desirable behavior, but tests current implementation
        assert_eq!(result, "////to/directory");
    }
}
