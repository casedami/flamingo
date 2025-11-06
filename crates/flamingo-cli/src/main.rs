use flamingo_config::FlamingoConfig;
use flamingo_err::ConfigError;
use flamingo_logger as logger;

fn main() {
    let cfg = FlamingoConfig::load().unwrap_or_else(|err| {
        match err {
            ConfigError::IoError(_) | ConfigError::ParseError(_) => {
                eprintln!("Flamingo {err}\n\n... using defaults as fallback")
            }
            ConfigError::NotFound => {} // silently use defaults
        }
        FlamingoConfig::default()
    });
    logger::init(&cfg.process);
    init_global_threadpool(cfg.process.num_threads);

    rayon::scope(|s| {
        s.spawn(|_| {
            // cleanup default and custom log dirs
            logger::cleanup(logger::default_log_dir());
            if let Some(user_log_dir) = &cfg.process.log_dir {
                logger::cleanup(user_log_dir);
            }
        })
    });

    log::trace!("Successfully loaded config");
    // let ctx = Context::new(&cfg);
}

/// Initialize global `rayon` thread pool
fn init_global_threadpool(n: usize) {
    rayon::ThreadPoolBuilder::new()
        .num_threads(n)
        .build_global()
        .expect("Failed to initialize worker thread pool");
}
