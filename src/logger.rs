use std::fs::OpenOptions;
use std::io::Write;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    Debug = 0,
    Info = 1,
    Warning = 2,
    Error = 3,
}

impl LogLevel {
    fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Debug => "DEBUG",
            LogLevel::Info => "INFO",
            LogLevel::Warning => "WARNING",
            LogLevel::Error => "ERROR",
        }
    }
}

pub struct Logger {
    min_level: LogLevel,
    log_file: Option<Arc<Mutex<std::fs::File>>>,
    console_output: bool,
}

impl Logger {
    /// Creates a new logger with console output only
    pub fn new(min_level: LogLevel) -> Self {
        Logger {
            min_level,
            log_file: None,
            console_output: true,
        }
    }

    /// Creates a new logger with both console and file output
    pub fn with_file(min_level: LogLevel, file_path: &str) -> std::io::Result<Self> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(file_path)?;

        Ok(Logger {
            min_level,
            log_file: Some(Arc::new(Mutex::new(file))),
            console_output: true,
        })
    }

    /// Sets whether console output is enabled
    pub fn set_console_output(&mut self, enabled: bool) {
        self.console_output = enabled;
    }

    /// Sets the minimum log level
    pub fn set_min_level(&mut self, level: LogLevel) {
        self.min_level = level;
    }

    /// Internal method to log a message
    fn log(&self, level: LogLevel, message: &str) {
        if level < self.min_level {
            return;
        }

        let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
        let formatted = format!("[{}] [{}] {}", timestamp, level.as_str(), message);

        if self.console_output {
            println!("{}", formatted);
        }

        if let Some(file) = &self.log_file {
            if let Ok(mut f) = file.lock() {
                let _ = writeln!(f, "{}", formatted);
            }
        }
    }

    /// Log a debug message
    pub fn debug(&self, message: &str) {
        self.log(LogLevel::Debug, message);
    }

    /// Log an info message
    pub fn info(&self, message: &str) {
        self.log(LogLevel::Info, message);
    }

    /// Log a warning message
    pub fn warning(&self, message: &str) {
        self.log(LogLevel::Warning, message);
    }

    /// Log an error message
    pub fn error(&self, message: &str) {
        self.log(LogLevel::Error, message);
    }
}

impl Default for Logger {
    fn default() -> Self {
        Logger::new(LogLevel::Info)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_level_ordering() {
        assert!(LogLevel::Debug < LogLevel::Info);
        assert!(LogLevel::Info < LogLevel::Warning);
        assert!(LogLevel::Warning < LogLevel::Error);
    }

    #[test]
    fn test_logger_creation() {
        let logger = Logger::new(LogLevel::Debug);
        assert_eq!(logger.min_level, LogLevel::Debug);
    }

    #[test]
    fn test_logger_default() {
        let logger = Logger::default();
        assert_eq!(logger.min_level, LogLevel::Info);
    }
}
