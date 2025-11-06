//! Test utilities for working with Git repositories.

#[cfg(test)]
pub mod repo {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::ExitStatus;
    use tempfile::TempDir;

    /// Runs a git command as a subprocess.
    ///
    /// Suitable for testing environments since it ignores any global git configuration.
    ///
    /// Assumes the git command will run, ie is for testing purposes only with the following conditions:
    ///     - the 'git' command can be found in $PATH
    ///     - the working directory exists
    ///     - permissions are valid
    macro_rules! _gitcmd {
    (args: $($arg:expr),*; dir: $dir:expr) => {
        std::process::Command::new("git")
            $(.arg($arg))*
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .current_dir($dir)
            .output()
            .unwrap()
        };
    }

    /// Represents a temporary mock git repo.
    ///
    /// Creates a temp directory and provides methods for common git operations. The temp directory
    /// is automatically cleaned up when the struct is dropped.
    ///
    /// Note: this is designed to be used in testing environments only where the conditions for the
    /// `_gitcmd` macro are met.
    pub struct TestRepo {
        pub dir: PathBuf,
        _dir: TempDir, // ensure temp dir stays on disk
        last_status: Option<ExitStatus>,
    }

    impl TestRepo {
        /// Initializes a TestRepo object with a temp directory.
        fn new() -> Self {
            let _dir = TempDir::new().expect("Expected to be able to create temp dir");
            let dir = _dir.path().to_path_buf();
            Self {
                dir,
                _dir,
                last_status: None,
            }
        }

        /// Configures a git repo for a testing environment.
        ///
        /// Sets local options to use a dummy user.
        fn set_local_opts(&mut self) -> &mut Self {
            _gitcmd!(args: "config", "--local", "user.email", "test@example.com"; dir: &self.dir);
            let stat =
                _gitcmd!(args: "config", "--local", "user.name", "Test User"; dir: &self.dir);
            self.last_status = Some(stat.status);
            self
        }

        /// Creates a TestRepo object with a git repo that has an initial commit.
        pub fn init() -> Self {
            let mut repo = TestRepo::new();
            _gitcmd!(args: "init"; dir: &repo.dir);
            repo.set_local_opts()
                .commit("README.md", "todo", "Initial commit");
            repo
        }

        /// Creates a TestRepo object with a bare git repo.
        ///
        /// Intended to be used for a mock upstream/remote repo.
        pub fn init_bare() -> Self {
            let mut repo = TestRepo::new();
            _gitcmd!(args: "init", "--bare"; dir: &repo.dir);
            repo.set_local_opts()
                .commit("README.md", "todo", "Initial commit");
            repo
        }

        /// Creates a TestRepo object that is a clone of another repo.
        pub fn clone(from: &Path) -> Self {
            let mut repo = TestRepo::new();

            _gitcmd!(
                args:
                "clone",
                &from,
                &repo.dir;
                dir: &std::env::current_dir().unwrap()
            );

            repo.set_local_opts();
            repo
        }

        pub fn create_branch(&mut self, name: &str) -> &mut Self {
            let stat = _gitcmd!(args: "checkout", "-b", name; dir: &self.dir);
            self.last_status = Some(stat.status);
            self
        }

        pub fn commit(&mut self, filename: &str, content: &str, message: &str) -> &mut Self {
            self.stage(filename, content);
            let stat = _gitcmd!(args: "commit", "-m", message; dir: &self.dir);
            self.last_status = Some(stat.status);
            self
        }

        pub fn make_dirty(&mut self) -> &mut Self {
            let _ = fs::write(self.dir.join("dirty.txt"), "uncommitted changes");
            self
        }

        pub fn stage(&mut self, filename: &str, content: &str) -> &mut Self {
            let _ = fs::write(self.dir.join(filename), content);
            let stat = _gitcmd!(args: "add", filename; dir: &self.dir);
            self.last_status = Some(stat.status);
            self
        }

        pub fn add_to_stash(&mut self) -> &mut Self {
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();

            let filename = format!("stash_file_{timestamp}.txt");
            self.commit(&filename, "original", "Add file for stashing");
            let _ = fs::write(self.dir.join(&filename), "modified");
            let stat = _gitcmd!(args: "stash", "push", "-m", "Test stash"; dir: &self.dir);
            self.last_status = Some(stat.status);
            self
        }

        pub fn add_remote(&mut self, name: &str, url: &str) -> &mut Self {
            let stat = _gitcmd!(args: "remote", "add", name, url; dir: &self.dir);
            self.last_status = Some(stat.status);
            self
        }

        pub fn checkout(&mut self, branch: &str) -> &mut Self {
            let stat = _gitcmd!(args: "checkout", branch; dir: &self.dir);
            self.last_status = Some(stat.status);
            self
        }

        pub fn create_merge_conflict(&mut self) -> &mut Self {
            self.create_branch("conflict-branch")
                .commit("conflict.txt", "branch content", "Branch commit")
                .checkout("master")
                .commit("conflict.txt", "main content", "Main commit");
            let stat = _gitcmd!(args: "merge", "conflict-branch"; dir: &self.dir);
            self.last_status = Some(stat.status);
            self
        }

        pub fn set_upstream(&mut self, remote_path: &Path) -> &mut Self {
            self.add_remote("origin", remote_path.to_str().unwrap());
            let stat = _gitcmd!(args: "push", "-u", "origin", "master"; dir: &self.dir);
            self.last_status = Some(stat.status);
            self
        }

        pub fn create_commits_ahead(&mut self, count: usize) -> &mut Self {
            for i in 0..count {
                self.commit(
                    &format!("ahead{i}.txt"),
                    "content",
                    &format!("Ahead commit {i}"),
                );
            }
            self
        }

        pub fn fetch_remote(&mut self) -> &mut Self {
            let stat = _gitcmd!(args: "fetch", "origin"; dir: &self.dir);
            self.last_status = Some(stat.status);
            self
        }

        pub fn push(&mut self) -> &mut Self {
            let stat = _gitcmd!(args: "push", "origin", "master"; dir: &self.dir);
            self.last_status = Some(stat.status);
            self
        }

        pub fn detach_head(&mut self) -> &mut Self {
            let stat = _gitcmd!(args: "checkout", "HEAD~1"; dir: &self.dir);
            self.last_status = Some(stat.status);
            self
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::read_stdout;

        #[test]
        fn test_init() {
            let repo = TestRepo::init();
            assert!(repo.dir.join(".git").exists());
        }

        #[test]
        fn test_create_branch() {
            let mut repo = TestRepo::init();
            let output = _gitcmd!(args: "branch"; dir: &repo.dir);
            let stdout = read_stdout!(&output.stdout);
            let original_branch_count = stdout.lines().count();

            repo.create_branch("new_branch");
            let output = _gitcmd!(args: "branch"; dir: &repo.dir);
            let stdout = read_stdout!(&output.stdout);
            let branch_count = stdout.lines().count();

            assert!(branch_count == original_branch_count + 1);
        }

        #[test]
        fn test_commit_file() {
            let mut repo = TestRepo::init();
            repo.commit("test.txt", "Hello, world!", "Add test file");
            let output = _gitcmd!(args: "show", "--name-status"; dir: &repo.dir);
            let stdout = read_stdout!(&output.stdout);
            assert!(stdout.contains("Add test file"));
            assert!(stdout.contains("A\ttest.txt"));
        }

        #[test]
        fn test_make_dirty() {
            let mut repo = TestRepo::init();
            let status = _gitcmd!(args: "status", "--porcelain"; dir: &repo.dir);
            assert!(status.stdout.is_empty(), "Repo should start clean");
            repo.make_dirty();
            let output = _gitcmd!(args: "status", "--porcelain"; dir: &repo.dir);
            let stdout = read_stdout!(&output.stdout);
            assert!(stdout.contains("?? dirty.txt"));
        }

        #[test]
        fn test_stage_file() {
            let mut repo = TestRepo::init();
            repo.stage("test.txt", "Hello, world!");
            let output = _gitcmd!(args: "status", "--porcelain"; dir: &repo.dir);
            let stdout = read_stdout!(&output.stdout);
            assert!(stdout.contains("A  test.txt"));
        }

        #[test]
        fn test_create_stash() {
            let mut repo = TestRepo::init();
            let output = _gitcmd!(args: "stash", "list"; dir: &repo.dir);
            let stdout = read_stdout!(&output.stdout);
            let original_stash_count = stdout.lines().count();

            repo.add_to_stash().add_to_stash().add_to_stash();

            let output = _gitcmd!(args: "stash", "list"; dir: &repo.dir);
            let stdout = read_stdout!(&output.stdout);
            let stash_count = stdout.lines().count();

            assert!(stash_count == original_stash_count + 3);
        }

        #[test]
        fn test_add_remote() {
            let mut repo = TestRepo::init();
            repo.add_remote("origin", "https://github.com/user/repo.git");
            let output = _gitcmd!(args: "remote", "-v"; dir: &repo.dir);
            let stdout = read_stdout!(&output.stdout);
            assert!(stdout.contains("origin\thttps://github.com/user/repo.git (fetch)"));
            assert!(stdout.contains("origin\thttps://github.com/user/repo.git (push)"));
        }

        #[test]
        fn test_create_merge_conflict() {
            let mut repo = TestRepo::init();
            assert!(
                !repo
                    .create_merge_conflict()
                    .last_status
                    .expect("Expected to be in a git repo")
                    .success()
            );
        }

        #[test]
        fn test_set_upstream() {
            let upstream = TestRepo::init_bare();
            let mut repo = TestRepo::init();

            assert!(
                repo.commit("test.txt", "content", "Initial commit")
                    .set_upstream(&upstream.dir)
                    .last_status
                    .expect("Expected to be in a git repo")
                    .success()
            );
            let output = _gitcmd!(args: "config", "branch.master.remote"; dir: &repo.dir);
            let stdout = read_stdout!(&output.stdout);
            assert_eq!(stdout.trim(), "origin");
        }

        #[test]
        fn test_add_commits() {
            let mut repo = TestRepo::init();
            let output = _gitcmd!(args: "log", "--oneline"; dir: &repo.dir);
            let stdout = read_stdout!(&output.stdout);
            let before_count = stdout.lines().count();

            repo.create_commits_ahead(3);

            let output = _gitcmd!(args: "log", "--oneline"; dir: &repo.dir);
            let stdout = read_stdout!(&output.stdout);
            let after_count = stdout.lines().count();

            assert_eq!(after_count, before_count + 3);
        }

        #[test]
        fn test_detach_head() {
            let mut repo = TestRepo::init();

            repo.commit("file1.txt", "content1", "First commit").commit(
                "file2.txt",
                "content2",
                "Second commit",
            );

            let output = _gitcmd!(args: "branch", "--show-current"; dir: &repo.dir);
            let stdout = read_stdout!(&output.stdout);
            assert_eq!(stdout.trim(), "master");

            repo.detach_head();

            let output = _gitcmd!(args: "branch", "--show-current"; dir: &repo.dir);
            let stdout = read_stdout!(&output.stdout);
            assert!(stdout.is_empty(), "Should not be on any branch");
        }
    }
}
