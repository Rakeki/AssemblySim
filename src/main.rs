mod logger;

use logger::{Logger, LogLevel};

fn main() {
    let logger = Logger::new(LogLevel::Debug);
    
    logger.debug("Application started");
    logger.info("System initialized");
    logger.warning("This is a warning message");
    logger.error("An error occurred");
}