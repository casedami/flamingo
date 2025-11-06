use flamingo_config::git::GitConfig;
use flamingo_err::GitError;
use std::path::Path;

mod test_utils;

/// Runs a git command as a subprocess in a directory and returns an `Output` object.
///
/// Assumes that git can be found in $PATH and the directory is valid, therefore the
/// `Option<Output>` object that is returned from `std::process::Command` is automatically
/// unwrapped. Be sure to use this macro *only* in circumstances where the **execution** of the git
/// command will not fail. Note, the execution of the command failing is separate from whether or
/// not the command fails.
///
/// # Examples
///
/// ```ignore
/// // Run `git commit -am "My commit message" in the current directory
/// gitcmd!(args: "commit", "-a", "-m", "My commit message"; dir: &std::env::current_dir().unwrap());
/// ```
macro_rules! gitcmd {
    (args: $($arg:expr),*; dir: $dir:expr) => {
        std::process::Command::new("git")
            $(.arg($arg))*
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .current_dir($dir)
            .output()
            .unwrap()
    };
}

/// Reads a slice of vector bytes into a String.
#[macro_export]
macro_rules! read_stdout {
    ($cmd:expr) => {{ String::from_utf8_lossy(&$cmd).trim().to_string() }};
}

#[derive(Debug, Default)]
struct GitInfo {
    branch: String,
    ahead: usize,
    behind: usize,
    is_dirty: bool,
    stash_count: usize,
    remote: Option<String>,
    state: GitState,
}

#[derive(Debug, Default)]
enum GitState {
    #[default]
    Clean,
    Merging,
    Rebasing,
    CherryPicking,
    Reverting,
    Bisecting,
}

impl GitState {
    fn as_str(&self) -> &str {
        match self {
            GitState::Clean => "",
            GitState::Merging => "MERGING",
            GitState::Rebasing => "REBASING",
            GitState::CherryPicking => "CHERRY-PICKING",
            GitState::Reverting => "REVERTING",
            GitState::Bisecting => "BISECTING",
        }
    }
}

pub fn git<P: AsRef<Path>>(path: P, config: &GitConfig) -> Result<String, GitError> {
    try_exec_git_in_dir(&path)
        .map_err(GitError::ExecutionError)?
        .then_some(())
        .ok_or(GitError::NotARepository)?;

    let branch = get_current_branch(&path).ok_or(GitError::NotARepository)?;
    let (ahead, behind) = get_ahead_behind(&path);

    Ok(format_git_info(
        &GitInfo {
            branch,
            is_dirty: is_dirty(&path),
            stash_count: get_stash_count(&path),
            remote: get_remote_name(&path),
            state: get_repo_state(&path),
            ahead,
            behind,
        },
        config,
    ))
}

fn try_exec_git_in_dir<P: AsRef<Path>>(path: P) -> Result<bool, std::io::Error> {
    Ok(std::process::Command::new("git")
        .arg("rev-parse")
        .arg("--is-inside-work-tree")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .current_dir(&path)
        .output()?
        .status
        .success())
}

fn format_git_info(git_info: &GitInfo, config: &GitConfig) -> String {
    config
        .format
        .replace("$branch", &git_info.branch)
        .replace("$state", git_info.state.as_str())
        .replace("$remote", git_info.remote.as_deref().unwrap_or(""))
        .replace(
            "$dirty",
            if git_info.is_dirty {
                &config.symbols.dirty
            } else {
                ""
            },
        )
        .replace(
            "$ahead",
            &match git_info.ahead {
                0 => String::new(),
                n => format!("{}{}", config.symbols.ahead, n),
            },
        )
        .replace(
            "$behind",
            &match git_info.behind {
                0 => String::new(),
                n => format!("{}{}", config.symbols.behind, n),
            },
        )
        .replace(
            "$stash",
            &match git_info.stash_count {
                0 => String::new(),
                n => format!("${n}"),
            },
        )
        .trim()
        .to_string()
}

fn get_current_branch<P: AsRef<Path>>(path: P) -> Option<String> {
    let proc = gitcmd!(args: "branch", "--show-current"; dir: &path);
    if proc.status.success() {
        let branch = read_stdout!(&proc.stdout);
        if !branch.is_empty() {
            return Some(branch);
        }
    }

    // Detached HEAD state - get short commit hash
    let proc = gitcmd!(args: "rev-parse", "--short", "HEAD"; dir: &path);
    proc.status
        .success()
        .then(|| format!("HEAD@{}", read_stdout!(&proc.stdout)))
}

fn get_ahead_behind<P: AsRef<Path>>(path: P) -> (usize, usize) {
    let proc =
        gitcmd!(args: "rev-list", "--left-right", "--count", "HEAD...@{upstream}"; dir: path);
    if !proc.status.success() {
        // No upstream configured or invalid ref
        return (0, 0);
    }
    let counts = read_stdout!(&proc.stdout);
    let ahead_behind: Vec<&str> = counts.split_whitespace().collect();

    match ahead_behind.as_slice() {
        [ahead, behind] => {
            let ahead = ahead.parse::<usize>().unwrap_or(0);
            let behind = behind.parse::<usize>().unwrap_or(0);
            (ahead, behind)
        }
        _ => (0, 0),
    }
}

fn is_dirty<P: AsRef<Path>>(path: P) -> bool {
    let proc = gitcmd!(args: "status", "--porcelain"; dir: path);
    proc.status.success() && !proc.stdout.is_empty()
}

fn get_stash_count<P: AsRef<Path>>(path: P) -> usize {
    let proc = gitcmd!(args: "stash", "list"; dir: path);
    if proc.status.success() {
        read_stdout!(&proc.stdout).lines().count()
    } else {
        0
    }
}

fn get_remote_name<P: AsRef<Path>>(path: P) -> Option<String> {
    let proc = gitcmd!(args: "remote"; dir: path);
    proc.status
        .success()
        .then(|| {
            read_stdout!(&proc.stdout)
                .lines()
                .next()
                .map(|s| s.to_string())
        })
        .flatten()
}

fn get_repo_state<P: AsRef<Path>>(path: P) -> GitState {
    let git_dir = path.as_ref().join(".git");
    let exists = |file: &str| git_dir.join(file).exists();

    match () {
        _ if exists("MERGE_HEAD") => GitState::Merging,
        _ if exists("REBASE_HEAD") || exists("rebase-apply") || exists("rebase-merge") => {
            GitState::Rebasing
        }
        _ if exists("CHERRY_PICK_HEAD") => GitState::CherryPicking,
        _ if exists("REVERT_HEAD") => GitState::Reverting,
        _ if exists("BISECT_LOG") => GitState::Bisecting,
        _ => GitState::Clean,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::repo::TestRepo;
    use flamingo_config::git::{GitConfig, GitSymbols};

    #[test]
    fn test_dirty_repository() {
        let mut repo = TestRepo::init();
        repo.make_dirty();

        let config = GitConfig {
            format: "$branch$dirty".to_string(),
            symbols: GitSymbols {
                dirty: "*".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };

        let result = git(&repo.dir, &config).unwrap();
        assert_eq!(result, String::from("master*"));
    }

    #[test]
    fn test_branch_valid_repo() {
        let repo = TestRepo::init();
        let config = GitConfig {
            format: "$branch".to_string(),
            ..Default::default()
        };

        let result = git(&repo.dir, &config);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), String::from("master"));
    }

    #[test]
    fn test_branch_invalid_repo() {
        let config = GitConfig {
            format: "$branch".to_string(),
            ..Default::default()
        };

        let result = git("not/a/git/repo", &config);
        assert!(result.is_err());
    }

    #[test]
    fn test_branch_detached_head() {
        let mut repo = TestRepo::init();
        let config = GitConfig {
            format: "$branch".to_string(),
            ..Default::default()
        };

        repo.commit("newfile", "content", "Adding new file")
            .detach_head();

        let result = git(&repo.dir, &config);
        assert!(result.is_ok());
        assert!(result.unwrap().contains("HEAD@"));
    }

    #[test]
    fn test_ahead() {
        let upstream = TestRepo::init_bare();
        let mut local = TestRepo::init();
        let _ = local.set_upstream(&upstream.dir);
        local.create_commits_ahead(3);

        let config = GitConfig {
            format: "$ahead$behind".to_string(),
            symbols: GitSymbols {
                ahead: "↑".to_string(),
                behind: "↓".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };

        let result = git(&local.dir, &config).unwrap();
        assert_eq!(result, String::from("↑3"));
    }

    #[test]
    fn test_behind() {
        let upstream = TestRepo::init_bare();

        let mut temp = TestRepo::init();
        let _ = temp.set_upstream(&upstream.dir);
        temp.commit("initial.txt", "initial", "Initial commit")
            .push();

        let mut local = TestRepo::clone(&upstream.dir);

        temp.create_commits_ahead(3).push();
        local.fetch_remote();

        let config = GitConfig {
            format: "$ahead$behind".to_string(),
            symbols: GitSymbols {
                ahead: "↑".to_string(),
                behind: "↓".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };
        let result = git(&local.dir, &config).unwrap();
        assert_eq!(result, String::from("↓3"));
    }

    #[test]
    fn test_stash_count() {
        let mut repo = TestRepo::init();
        repo.add_to_stash().add_to_stash();

        let config = GitConfig {
            format: "$stash".to_string(),
            ..Default::default()
        };

        let result = git(&repo.dir, &config).unwrap();
        assert_eq!(result, String::from("$2"));
    }

    #[test]
    fn test_detached_head() {
        let mut repo = TestRepo::init();
        repo.create_commits_ahead(2).detach_head();

        let config = GitConfig {
            format: "$branch".to_string(),
            ..Default::default()
        };
        let result = git(&repo.dir, &config).unwrap();
        assert!(result.contains("HEAD@"));
    }

    #[test]
    fn test_merge_state() {
        let mut repo = TestRepo::init();
        repo.create_merge_conflict();

        let config = GitConfig {
            format: "$branch $state".to_string(),
            ..Default::default()
        };

        let result = git(&repo.dir, &config).unwrap();
        assert!(result.contains("MERGING"));
    }
}
