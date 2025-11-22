/// SOCKS5 protocol constants and types

pub const SOCKS_VERSION: u8 = 5;
pub const SOCKS_ADDR_TYPE_IPV4: u8 = 1;
pub const SOCKS_ADDR_TYPE_DOMAIN: u8 = 3;
pub const SOCKS_ADDR_TYPE_IPV6: u8 = 4;
pub const SOCKS_CMD_CONNECT: u8 = 1;

/// Check if the target is a loopback address on common SOCKS ports
/// This prevents infinite loops when tunneling to localhost
pub fn is_loopback_address(host: &str, port: u16) -> bool {
    let is_loopback_host =
        host == "localhost" || host == "127.0.0.1" || host == "::1" || host.starts_with("127.");

    let is_socks_port = port == 1080 || port == 1081 || port == 9050;

    is_loopback_host && is_socks_port
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_loopback_address_localhost() {
        assert!(is_loopback_address("localhost", 1080));
        assert!(is_loopback_address("127.0.0.1", 1080));
        assert!(is_loopback_address("127.0.0.5", 1080));
    }

    #[test]
    fn test_is_loopback_address_non_socks_port() {
        assert!(!is_loopback_address("localhost", 8080));
        assert!(!is_loopback_address("127.0.0.1", 443));
    }

    #[test]
    fn test_is_loopback_address_non_loopback() {
        assert!(!is_loopback_address("example.com", 1080));
        assert!(!is_loopback_address("192.168.1.1", 1080));
    }
}
