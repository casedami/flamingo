use flamingo_config::process::ProcessConfig;
use log::{Level, LevelFilter, Metadata, Record};
use nu_ansi_term::Color;
use std::os::unix::process;
use std::path::Path;
use std::sync::OnceLock;
use std::{
    collections::HashSet,
    env,
    fs::{self, File, OpenOptions},
    io::Write,
    path::PathBuf,
    sync::{Mutex, RwLock},
};

pub struct FlamingoLogger {
    file_handle: OnceLock<Result<Mutex<File>, std::io::Error>>,
    path: PathBuf,
    content: RwLock<HashSet<String>>,
    level: Level,
}

/// Returns a session identifier for grouping log files. Uses the
/// following priority hierarchy:
///
/// * FLAMINGO_SESSION_KEY environment variable, if set
/// * PPID
fn get_session_id() -> String {
    // First try the environment variable
    if let Ok(session_key) = env::var("FLAMINGO_SESSION_KEY") {
        if !session_key.is_empty() {
            return session_key;
        }
    }

    process::parent_id().to_string()
}

/// Returns the path to the log directory, which is located in the user's
/// cache directory or, in the case where the cache directory can't be
/// located, in a temporary directory.
pub fn default_log_dir() -> PathBuf {
    dirs::home_dir()
        .map(|home| home.join(".cache"))
        .or_else(dirs::cache_dir)
        .unwrap_or_else(std::env::temp_dir)
        .join("flamingo")
}

/// Deletes all log files in the log directory that were modified more
/// than 24 hours ago.
pub fn cleanup<P: AsRef<Path>>(path: P) {
    let log_dir = path.as_ref();
    let Ok(log_files) = fs::read_dir(log_dir) else {
        // Avoid noisily handling errors in this cleanup function
        return;
    };

    for file in log_files {
        // Skip files that can't be read
        let Ok(file) = file else {
            continue;
        };

        // Avoid deleting files that don't look like log files
        if file.path().extension() != Some("log".as_ref()) {
            continue;
        }

        // Read metadata to check file age
        let Ok(metadata) = file.metadata() else {
            continue;
        };

        // Avoid handling anything that isn't a file
        if !metadata.is_file() {
            continue;
        }

        // Get the file's modification time
        let Ok(modified) = metadata.modified() else {
            continue;
        };

        // Delete the file if it hasn't changed in 24 hours
        if modified.elapsed().unwrap_or_default().as_secs() > 60 * 60 * 24 {
            let _ = fs::remove_file(file.path());
        }
    }
}

impl Default for FlamingoLogger {
    /// Used primarily for testing. Otherwise use
    /// `config::FlamingoConfig::default().setup_logger()`.
    fn default() -> Self {
        let log_dir = default_log_dir();

        if let Err(err) = fs::create_dir_all(&log_dir) {
            eprintln!("Unable to create log dir {log_dir:?}: {err:?}!")
        };

        let session_id = get_session_id();
        let session_log_file = log_dir.join(format!("session_{session_id}.log"));

        Self {
            content: RwLock::new(
                fs::read_to_string(&session_log_file)
                    .unwrap_or_default()
                    .lines()
                    .map(std::string::ToString::to_string)
                    .collect(),
            ),
            file_handle: OnceLock::new(),
            path: session_log_file,
            level: Level::Warn,
        }
    }
}

impl FlamingoLogger {
    /// Create a FlamingoLogger from logging configuration
    pub fn from_config(config: &ProcessConfig) -> Self {
        let log_dir = config.log_dir.clone().unwrap_or_else(default_log_dir);

        if let Err(err) = fs::create_dir_all(&log_dir) {
            eprintln!("Unable to create log dir {log_dir:?}: {err:?}!")
        };

        let session_id = config.session_key.clone().unwrap_or_else(get_session_id);
        let session_log_file = log_dir.join(format!("session_{session_id}.log"));

        let log_level = match config.min_log_level.to_ascii_lowercase().as_str() {
            "trace" => Level::Trace,
            "debug" => Level::Debug,
            "info" => Level::Info,
            "warn" => Level::Warn,
            "error" => Level::Error,
            _ => Level::Warn,
        };

        Self {
            content: RwLock::new(
                fs::read_to_string(&session_log_file)
                    .unwrap_or_default()
                    .lines()
                    .map(std::string::ToString::to_string)
                    .collect(),
            ),
            file_handle: OnceLock::new(),
            path: session_log_file,
            level: log_level,
        }
    }

    /// Sets the log level and syncs it with the global max level filter.
    pub fn set_log_level(&mut self, level: log::Level) {
        self.level = level;
        log::set_max_level(match level {
            Level::Error => LevelFilter::Error,
            Level::Warn => LevelFilter::Warn,
            Level::Info => LevelFilter::Info,
            Level::Debug => LevelFilter::Debug,
            Level::Trace => LevelFilter::Trace,
        });
    }

    /// Override the log file path and reload content from the new file
    /// for duplicate detection.
    ///
    /// **Note**: This won't change the active file handle if a log file was
    /// already opened. The new path will only take effect for future
    /// logging operations.
    pub fn set_log_file_path(&mut self, path: PathBuf) {
        let contents = fs::read_to_string(&path)
            .unwrap_or_default()
            .lines()
            .map(std::string::ToString::to_string)
            .collect();
        self.content = RwLock::new(contents);
        self.path = path;
    }
}

impl log::Log for FlamingoLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.level
    }

    fn log(&self, record: &Record) {
        // Early return if the log level is not enabled
        if !self.enabled(record.metadata()) {
            return;
        }

        let to_print = format!("[{}]: {}", record.level(), record.args());

        // A log message is only written to the log file if it's not
        // already in the log file. To help with debugging, duplicate
        // detection only runs if the log level is less than Error.
        let is_duplicate = {
            record.level() > Level::Error
                && self
                    .content
                    .read()
                    .map(|c| c.contains(to_print.as_str()))
                    .unwrap_or(false)
        };

        if is_duplicate {
            return;
        }

        if record.level() <= self.level {
            let log_file = match self.file_handle.get_or_init(|| {
                OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&self.path)
                    .map(Mutex::new)
            }) {
                Ok(log_file) => log_file,
                Err(err) => {
                    eprintln!("Unable to open session log file {:?}: {err:?}!", self.path);
                    return;
                }
            };

            let mut file_handle = match log_file.lock() {
                Ok(file_handle) => file_handle,
                Err(err) => {
                    eprintln!("Log file writer mutex was poisoned! {err:?}",);
                    return;
                }
            };
            if let Err(err) = writeln!(file_handle, "{to_print}") {
                eprintln!("Unable to write to session log file {err:?}!",);
            };
        }

        // Print messages to stderr
        eprintln!(
            "[{}] - ({}): {}",
            match record.level() {
                Level::Trace => Color::Blue.paint(format!("{}", record.level())),
                Level::Debug => Color::Cyan.paint(format!("{}", record.level())),
                Level::Info => Color::White.paint(format!("{}", record.level())),
                Level::Warn => Color::Yellow.paint(format!("{}", record.level())),
                Level::Error => Color::Red.paint(format!("{}", record.level())),
            },
            record.module_path().unwrap_or_default(),
            record.args()
        );

        // Add to duplicate detection set
        if let Ok(mut c) = self.content.write() {
            c.insert(to_print);
        }
    }

    fn flush(&self) {
        if let Some(Ok(m)) = self.file_handle.get() {
            let result = match m.lock() {
                Ok(mut file) => file.flush(),
                Err(err) => return eprintln!("Log file writer mutex was poisoned: {err:?}"),
            };
            if let Err(err) = result {
                eprintln!("Unable to flush the log file: {err:?}");
            }
        }
    }
}

/// Initializes the global logger with a FlamingoLogger instance and syncs
/// the maximum log level. This should be called once at the start of the
/// process.
pub fn init(config: &ProcessConfig) {
    let logger = FlamingoLogger::from_config(config);

    log::set_max_level(match logger.level {
        Level::Error => LevelFilter::Error,
        Level::Warn => LevelFilter::Warn,
        Level::Info => LevelFilter::Info,
        Level::Debug => LevelFilter::Debug,
        Level::Trace => LevelFilter::Trace,
    });
    log::set_boxed_logger(Box::new(logger)).unwrap();
}

#[cfg(test)]
mod test {
    use super::*;
    use log::Log;
    use std::fs::{File, FileTimes};
    use std::io;
    use std::time::SystemTime;

    #[test]
    fn test_log_to_file() -> io::Result<()> {
        let log_dir = tempfile::tempdir()?;
        let log_file = log_dir.path().join("test.log");

        let mut logger = FlamingoLogger::default();
        logger.set_log_file_path(log_file.clone());
        logger.set_log_level(Level::Warn);

        // Load at all log levels
        logger.log(
            &Record::builder()
                .level(Level::Error)
                .args(format_args!("error"))
                .build(),
        );
        logger.log(
            &Record::builder()
                .level(Level::Warn)
                .args(format_args!("warn"))
                .build(),
        );
        logger.log(
            &Record::builder()
                .level(Level::Info)
                .args(format_args!("info"))
                .build(),
        );
        logger.log(
            &Record::builder()
                .level(Level::Debug)
                .args(format_args!("debug"))
                .build(),
        );
        logger.log(
            &Record::builder()
                .level(Level::Trace)
                .args(format_args!("trace"))
                .build(),
        );

        // Print duplicate messages
        logger.log(
            &Record::builder()
                .level(Level::Warn)
                .args(format_args!("warn"))
                .build(),
        );
        logger.log(
            &Record::builder()
                .level(Level::Error)
                .args(format_args!("error"))
                .build(),
        );

        logger.flush();
        drop(logger);

        let content = fs::read_to_string(log_file)?;

        assert_eq!(content, "[ERROR]: error\n[WARN]: warn\n[ERROR]: error\n");
        log_dir.close()
    }

    #[test]
    fn test_dedup_from_file() -> io::Result<()> {
        let log_dir = tempfile::tempdir()?;
        let log_file = log_dir.path().join("test.log");
        {
            let mut file = File::create(&log_file)?;
            file.write_all(b"[WARN]: warn\n")?;
            file.sync_all()?;
        }

        let mut logger = FlamingoLogger::default();
        logger.set_log_file_path(log_file.clone());
        logger.set_log_level(Level::Warn);

        // This message should not be written to the log file
        logger.log(
            &Record::builder()
                .level(Level::Warn)
                .args(format_args!("warn"))
                .build(),
        );
        // This message should be written to the log file
        logger.log(
            &Record::builder()
                .level(Level::Warn)
                .args(format_args!("warn2"))
                .build(),
        );

        // This message should be written to the log file
        logger.log(
            &Record::builder()
                .level(Level::Error)
                .args(format_args!("error"))
                .build(),
        );
        // This message should be written to the log file
        logger.log(
            &Record::builder()
                .level(Level::Error)
                .args(format_args!("error"))
                .build(),
        );

        logger.flush();
        drop(logger);

        let content = fs::read_to_string(log_file)?;

        assert_eq!(
            content,
            "[WARN]: warn\n[WARN]: warn2\n[ERROR]: error\n[ERROR]: error\n"
        );

        log_dir.close()
    }

    #[test]
    fn test_cleanup() -> io::Result<()> {
        let log_dir = tempfile::tempdir()?;

        // Should not be deleted
        let non_log_file = log_dir.path().join("not-a-log.txt"); // Not a .log file
        let new_log_file = log_dir.path().join("recent.log"); // Recent .log file
        let directory = log_dir.path().join("somedir.log"); // Directory (not a file)

        // Should be deleted
        let old_log_file = log_dir.path().join("old.log"); // Old .log file

        for file in &[&non_log_file, &new_log_file, &old_log_file] {
            File::create(file)?;
        }
        fs::create_dir(&directory)?;

        let old_times = FileTimes::new()
            .set_accessed(SystemTime::UNIX_EPOCH)
            .set_modified(SystemTime::UNIX_EPOCH);

        // Set old_log_file to be older than 24 hours
        let old_file_handle = File::open(&old_log_file)?;
        match old_file_handle.set_times(old_times) {
            Err(err) if err.kind() == io::ErrorKind::PermissionDenied => {
                // Skip test on systems where we can't set file times
                eprintln!("Unable to set file times: {err:?}");
                return Ok(());
            }
            other => other,
        }?;
        old_file_handle.sync_all()?;

        cleanup(log_dir.path());

        assert!(non_log_file.exists(), "Non-log file should not be deleted");
        assert!(
            new_log_file.exists(),
            "Recent log file should not be deleted"
        );
        assert!(directory.exists(), "Directory should not be deleted");
        assert!(!old_log_file.exists(), "Old log file should be deleted");

        log_dir.close()
    }
}
