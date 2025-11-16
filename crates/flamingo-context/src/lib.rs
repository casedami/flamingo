use flamingo_config::FlamingoConfig;
use flamingo_err::GitError;
use flamingo_git::git;
use flamingo_path::{cwd, flamingo_path};

use bitflags::bitflags;

bitflags! {
    pub struct FlamingoContext: u32 {
        const PATH = 0b01;
        const GIT = 0b10;
    }
}

pub fn format(config: &FlamingoConfig, context: FlamingoContext) -> String {
    let mut parts = Vec::new();

    if context.contains(FlamingoContext::PATH) {
        parts.push(flamingo_path(&config.path, None));
    }

    if context.contains(FlamingoContext::GIT) {
        let path = cwd();
        match git(&path, &config.git) {
            Ok(git_info) => parts.push(git_info),
            Err(GitError::ExecutionError(e)) => log::error!("{e}"),
            Err(GitError::CommandFailed(e)) => log::error!("{e}"),
            Err(GitError::ParseError(e)) => log::error!("{e}"),
            Err(GitError::NotARepository) => {}
        }
    }

    parts.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use flamingo_config::git::GitConfig;
    use flamingo_config::path::{PathConfig, TruncateStrategy};

    #[test]
    fn test_format() {
        let pathcfg = PathConfig {
            truncate: TruncateStrategy::Tail { size: 2 },
            shorten_home: false,
        };

        let gitcfg = GitConfig {
            format: "$branch$dirty".to_string(),
            ..Default::default()
        };

        let config = FlamingoConfig {
            path: pathcfg,
            git: gitcfg,
            ..Default::default()
        };

        let res = format(&config, FlamingoContext::PATH | FlamingoContext::GIT);
        print!("{res}");
    }
}
