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
    if !is_git_repository(&path)? {
        return Err(GitError::NotARepository);
    }

    let branch = get_current_branch(&path)?;

    // Get ahead/behind counts
    if let Some(b) = branch {
        // WARN: might cause issues if in detached HEAD state
        let (ahead, behind) = get_ahead_behind(&path)?;
        let git_info = GitInfo {
            branch: b,
            is_dirty: is_working_directory_dirty(&path)?,
            stash_count: get_stash_count(&path)?,
            remote: get_remote_name(&path)?,
            state: get_repository_state(&path)?,
            ahead,
            behind,
        };
        Ok(format_git_info(&git_info, config))
    } else {
        Err(GitError::NotARepository)
    }
}

/// Format git information according to the configured format string
fn format_git_info(git_info: &GitInfo, config: &GitConfig) -> String {
    let mut result = config.format.clone();

    // Replace branch
    result = result.replace("$branch", git_info.branch.as_str());

    // Replace dirty indicator
    let dirty_indicator = if git_info.is_dirty {
        config.symbols.dirty.as_str()
    } else {
        ""
    };
    result = result.replace("$dirty", dirty_indicator);

    // Replace ahead/behind indicators
    let ahead_indicator = if git_info.ahead > 0 {
        format!("{}{}", config.symbols.ahead, git_info.ahead)
    } else {
        String::new()
    };
    result = result.replace("$ahead", &ahead_indicator);

    let behind_indicator = if git_info.behind > 0 {
        format!("{}{}", config.symbols.behind, git_info.behind)
    } else {
        String::new()
    };
    result = result.replace("$behind", &behind_indicator);

    // Replace state
    result = result.replace("$state", git_info.state.as_str());

    // Replace remote
    if let Some(ref remote) = git_info.remote {
        result = result.replace("$remote", remote);
    } else {
        result = result.replace("$remote", "");
    }

    // Replace stash count
    let stash_indicator = if git_info.stash_count > 0 {
        format!("${}", git_info.stash_count)
    } else {
        String::new()
    };
    result = result.replace("$stash", &stash_indicator);

    result
}

/// Check if the given path is inside a git repository
fn is_git_repository<P: AsRef<Path>>(path: P) -> Result<bool, GitError> {
    let output = Command::new("git")
        .arg("rev-parse")
        .arg("--is-inside-work-tree")
        .current_dir(path)
        .output()
        .map_err(|e| GitError::CommandFailed(e.to_string()))?;

    Ok(output.status.success())
}

/// Get the current branch name
fn get_current_branch<P: AsRef<Path>>(path: P) -> Result<Option<String>, GitError> {
    let output = Command::new("git")
        .arg("branch")
        .arg("--show-current")
        .current_dir(&path)
        .output()
        .map_err(|e| GitError::CommandFailed(e.to_string()))?;

    if !output.status.success() {
        return Ok(None);
    }

    let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if branch.is_empty() {
        // Might be in detached HEAD state, try to get commit hash
        let output = Command::new("git")
            .arg("rev-parse")
            .arg("--short")
            .arg("HEAD")
            .current_dir(&path)
            .output()
            .map_err(|e| GitError::CommandFailed(e.to_string()))?;

        if output.status.success() {
            let commit = String::from_utf8_lossy(&output.stdout).trim().to_string();
            Ok(Some(format!("HEAD@{commit}")))
        } else {
            Ok(None)
        }
    } else {
        Ok(Some(branch))
    }
}

/// Get ahead/behind counts relative to upstream
fn get_ahead_behind<P: AsRef<Path>>(path: P) -> Result<(usize, usize), GitError> {
    let output = Command::new("git")
        .arg("rev-list")
        .arg("--left-right")
        .arg("--count")
        .arg("HEAD...@{upstream}")
        .current_dir(path)
        .output()
        .map_err(|e| GitError::CommandFailed(e.to_string()))?;
    if !output.status.success() {
        // No upstream configured
        return Ok((0, 0));
    }

    let counts = String::from_utf8_lossy(&output.stdout);
    let counts = counts.trim(); // Separate line for clarity
    let parts: Vec<&str> = counts.split_whitespace().collect();

    if parts.len() != 2 {
        return Ok((0, 0));
    }
    let ahead = parts[0].parse::<usize>().unwrap_or(0);
    let behind = parts[1].parse::<usize>().unwrap_or(0);
    Ok((ahead, behind))
}

/// Check if the working directory has uncommitted changes
fn is_working_directory_dirty<P: AsRef<Path>>(path: P) -> Result<bool, GitError> {
    let output = Command::new("git")
        .arg("status")
        .arg("--porcelain")
        .current_dir(path)
        .output()
        .map_err(|e| GitError::CommandFailed(e.to_string()))?;

    if !output.status.success() {
        return Ok(false);
    }

    Ok(!output.stdout.is_empty())
}

/// Get the number of stashed changes
fn get_stash_count<P: AsRef<Path>>(path: P) -> Result<usize, GitError> {
    let output = Command::new("git")
        .arg("stash")
        .arg("list")
        .current_dir(path)
        .output()
        .map_err(|e| GitError::CommandFailed(e.to_string()))?;

    if !output.status.success() {
        return Ok(0);
    }

    let stash_list = String::from_utf8_lossy(&output.stdout);
    Ok(stash_list.lines().count())
}

/// Get the name of the remote (usually "origin")
fn get_remote_name<P: AsRef<Path>>(path: P) -> Result<Option<String>, GitError> {
    let output = Command::new("git")
        .arg("remote")
        .current_dir(path)
        .output()
        .map_err(|e| GitError::CommandFailed(e.to_string()))?;

    if !output.status.success() {
        return Ok(None);
    }

    let remotes = String::from_utf8_lossy(&output.stdout);
    let first_remote = remotes.lines().next();
    Ok(first_remote.map(|s| s.to_string()))
}

/// Get the current repository state (merging, rebasing, etc.)
fn get_repository_state<P: AsRef<Path>>(path: P) -> Result<GitState, GitError> {
    let git_dir = path.as_ref().join(".git");

    if git_dir.join("MERGE_HEAD").exists() {
        return Ok(GitState::Merging);
    }

    if git_dir.join("REBASE_HEAD").exists()
        || git_dir.join("rebase-apply").exists()
        || git_dir.join("rebase-merge").exists()
    {
        return Ok(GitState::Rebasing);
    }

    if git_dir.join("CHERRY_PICK_HEAD").exists() {
        return Ok(GitState::CherryPicking);
    }

    if git_dir.join("REVERT_HEAD").exists() {
        return Ok(GitState::Reverting);
    }

    if git_dir.join("BISECT_LOG").exists() {
        return Ok(GitState::Bisecting);
    }

    Ok(GitState::Clean)
}
