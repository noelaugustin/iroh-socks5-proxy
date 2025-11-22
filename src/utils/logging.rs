use chrono::Local;

/// Format log messages with timestamp
pub fn format_log(direction: &str, host: &str, port: u16) -> String {
    let timestamp = Local::now().format("%H:%M:%S");
    format!("[{}] {} {}:{}", timestamp, direction, host, port)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_log() {
        let log = format_log("TEST", "example.com", 443);
        assert!(log.contains("TEST"));
        assert!(log.contains("example.com:443"));
        assert!(log.contains("[")); // Has timestamp
    }
}
