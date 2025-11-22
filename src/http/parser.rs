/// Extract HTTP request details (method, path, Host header) from HTTP request
///
/// Parses the HTTP request to extract method, path, and Host header.
/// Returns a struct with the extracted information if found, None otherwise.

#[derive(Debug, Clone, PartialEq)]
pub struct HttpRequestInfo {
    pub method: String,
    pub path: String,
    pub host: Option<String>,
}

pub fn extract_http_info(data: &[u8]) -> Option<HttpRequestInfo> {
    // HTTP request must have at least "GET / HTTP/1.x\r\n" which is about 16 bytes minimum
    if data.len() < 16 {
        return None;
    }

    // Try to parse as UTF-8 string
    let text = std::str::from_utf8(data).ok()?;

    // Split into lines
    let mut lines = text.lines();
    let first_line = lines.next()?;

    // Parse request line: "METHOD /path HTTP/1.x"
    let parts: Vec<&str> = first_line.split_whitespace().collect();
    if parts.len() < 3 {
        return None;
    }

    let method = parts[0];
    let path = parts[1];
    let version = parts[2];

    // Check if this looks like an HTTP request
    if !version.starts_with("HTTP/1.") {
        return None;
    }

    // Valid HTTP methods
    let valid_methods = [
        "GET", "POST", "PUT", "DELETE", "HEAD", "OPTIONS", "PATCH", "CONNECT", "TRACE",
    ];
    if !valid_methods.contains(&method) {
        return None;
    }

    // Extract Host header
    let mut host = None;
    for line in lines {
        if line.is_empty() {
            break; // End of headers
        }

        if let Some((header_name, header_value)) = line.split_once(':') {
            if header_name.trim().eq_ignore_ascii_case("host") {
                host = Some(header_value.trim().to_string());
                break;
            }
        }
    }

    Some(HttpRequestInfo {
        method: method.to_string(),
        path: path.to_string(),
        host,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_http_info_simple_get() {
        let request = b"GET / HTTP/1.1\r\nHost: example.com\r\n\r\n";
        let info = extract_http_info(request).unwrap();
        assert_eq!(info.method, "GET");
        assert_eq!(info.path, "/");
        assert_eq!(info.host, Some("example.com".to_string()));
    }

    #[test]
    fn test_extract_http_info_with_path() {
        let request = b"POST /api/users HTTP/1.1\r\nHost: api.example.com\r\nContent-Type: application/json\r\n\r\n";
        let info = extract_http_info(request).unwrap();
        assert_eq!(info.method, "POST");
        assert_eq!(info.path, "/api/users");
        assert_eq!(info.host, Some("api.example.com".to_string()));
    }

    #[test]
    fn test_extract_http_info_no_host() {
        let request = b"GET /test HTTP/1.1\r\nUser-Agent: test\r\n\r\n";
        let info = extract_http_info(request).unwrap();
        assert_eq!(info.method, "GET");
        assert_eq!(info.path, "/test");
        assert_eq!(info.host, None);
    }

    #[test]
    fn test_extract_http_info_too_short() {
        let request = b"GET";
        assert_eq!(extract_http_info(request), None);
    }

    #[test]
    fn test_extract_http_info_not_http() {
        let request = b"INVALID REQUEST\r\n\r\n";
        assert_eq!(extract_http_info(request), None);
    }

    #[test]
    fn test_extract_http_info_binary_data() {
        let request = vec![0x16, 0x03, 0x01, 0x00, 0x05]; // TLS handshake
        assert_eq!(extract_http_info(&request), None);
    }

    #[test]
    fn test_extract_http_info_different_methods() {
        let methods = ["GET", "POST", "PUT", "DELETE", "HEAD"];
        for method in &methods {
            let request = format!("{} /path HTTP/1.1\r\nHost: test.com\r\n\r\n", method);
            let info = extract_http_info(request.as_bytes()).unwrap();
            assert_eq!(info.method, *method);
        }
    }
}
