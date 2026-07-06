pub trait Logger {
    fn log(&self, message: &str);
}

pub struct FileLogger {
    pub file_path: String,
}

impl Logger for FileLogger {
    fn log(&self, message: &str) {
        println!("Logging to {}: {}", self.file_path, message);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_logger() {
        let logger = FileLogger { file_path: "test.log".to_string() };
        logger.log("test message");
    }
}