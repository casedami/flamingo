use clap::{Parser, Subcommand};
use flamingo_config::FlamingoConfig;
use flamingo_context::{FlamingoContext, format};
use flamingo_err::ConfigError;
use flamingo_logger as logger;
use flamingo_path::flamingo_path;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "flm", author, version, about, long_about = None)]
struct Args {
    /// Path to config file
    #[arg(short, long, global = true)]
    config: Option<String>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Path {
        /// Override the path to display (defaults to current directory)
        #[arg(short, long)]
        format: Option<String>,
    },
    /// Show git information
    Git,
}

fn main() {
    let args = Args::parse();

    let cfg = match args.config {
        Some(path) => FlamingoConfig::load_from(path),
        None => FlamingoConfig::load(),
    }
    .unwrap_or_else(|err| {
        match err {
            ConfigError::IoError(_) | ConfigError::ParseError(_) => {
                log::error!("Flamingo {err}\n\n... using defaults as fallback")
            }
            ConfigError::NotFound => {} // silently use defaults
        }
        FlamingoConfig::default()
    });

    logger::init(&cfg.process);
    init_global_threadpool(cfg.process.num_threads);

    rayon::scope(|s| {
        s.spawn(|_| {
            logger::cleanup(logger::default_log_dir());
            if let Some(user_log_dir) = &cfg.process.log_dir {
                logger::cleanup(user_log_dir);
            }
        })
    });

    let context = match &args.command {
        Some(Commands::Path { .. }) => FlamingoContext::PATH,
        Some(Commands::Git) => FlamingoContext::GIT,
        None => FlamingoContext::PATH | FlamingoContext::GIT, // default to both
    };

    if let Some(Commands::Path {
        format: Some(user_path),
    }) = &args.command
    {
        let p = flamingo_path(&cfg.path, Some(PathBuf::from(user_path)));
        print!("{p}");
        return;
    }

    let ctx = format(&cfg, context);
    print!("{ctx}");
}

fn init_global_threadpool(n: usize) {
    rayon::ThreadPoolBuilder::new()
        .num_threads(n)
        .build_global()
        .expect("Failed to initialize worker thread pool");
}
