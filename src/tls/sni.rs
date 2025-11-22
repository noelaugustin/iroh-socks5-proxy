/// Extract SNI (Server Name Indication) from TLS ClientHello
///
/// Parses the TLS ClientHello message to extract the SNI extension.
/// Returns the hostname if found, None otherwise.
pub fn extract_sni(data: &[u8]) -> Option<String> {
    // TLS record must be at least 43 bytes
    if data.len() < 43 {
        return None;
    }

    // Check if this is a TLS handshake (0x16) with ClientHello (0x01)
    if data[0] != 0x16 {
        return None;
    }

    // TLS version should be 3.x
    if data[1] != 0x03 {
        return None;
    }

    // Skip to handshake type
    if data[5] != 0x01 {
        return None; // Not a ClientHello
    }

    // Parse through the ClientHello to find extensions
    let mut pos = 43; // Skip fixed header, random, and session ID length

    // Skip session ID
    if pos >= data.len() {
        return None;
    }
    let session_id_len = data[pos] as usize;
    pos += 1 + session_id_len;

    // Skip cipher suites
    if pos + 2 > data.len() {
        return None;
    }
    let cipher_suites_len = u16::from_be_bytes([data[pos], data[pos + 1]]) as usize;
    pos += 2 + cipher_suites_len;

    // Skip compression methods
    if pos >= data.len() {
        return None;
    }
    let compression_len = data[pos] as usize;
    pos += 1 + compression_len;

    // Extensions length
    if pos + 2 > data.len() {
        return None;
    }
    let extensions_len = u16::from_be_bytes([data[pos], data[pos + 1]]) as usize;
    pos += 2;

    let extensions_end = pos + extensions_len;

    // Parse extensions
    while pos + 4 <= extensions_end && pos + 4 <= data.len() {
        let ext_type = u16::from_be_bytes([data[pos], data[pos + 1]]);
        let ext_len = u16::from_be_bytes([data[pos + 2], data[pos + 3]]) as usize;
        pos += 4;

        // SNI extension type is 0x0000
        if ext_type == 0x0000 && pos + ext_len <= data.len() {
            // SNI extension format:
            // - 2 bytes: list length
            // - 1 byte: name type (0 = hostname)
            // - 2 bytes: hostname length
            // - hostname bytes
            if ext_len >= 5 {
                let name_type = data[pos + 2];
                if name_type == 0 {
                    let hostname_len = u16::from_be_bytes([data[pos + 3], data[pos + 4]]) as usize;
                    if pos + 5 + hostname_len <= data.len() {
                        if let Ok(hostname) =
                            std::str::from_utf8(&data[pos + 5..pos + 5 + hostname_len])
                        {
                            return Some(hostname.to_string());
                        }
                    }
                }
            }
            return None;
        }

        pos += ext_len;
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_sni_too_short() {
        let short_data = vec![0u8; 10];
        assert_eq!(extract_sni(&short_data), None);
    }

    #[test]
    fn test_extract_sni_not_tls_handshake() {
        let mut data = vec![0u8; 50];
        data[0] = 0x17; // Not a handshake
        assert_eq!(extract_sni(&data), None);
    }

    #[test]
    fn test_extract_sni_not_clienthello() {
        let mut data = vec![0u8; 50];
        data[0] = 0x16; // TLS handshake
        data[1] = 0x03; // TLS 1.x
        data[5] = 0x02; // Not ClientHello
        assert_eq!(extract_sni(&data), None);
    }

    #[test]
    fn test_extract_sni_valid_hostname() {
        // Minimal TLS ClientHello with SNI extension for "example.com"
        let mut data = vec![0u8; 100];
        data[0] = 0x16; // Handshake
        data[1] = 0x03; // TLS 1.x
        data[5] = 0x01; // ClientHello

        // Session ID length at position 43
        data[43] = 0;

        // Cipher suites length at position 44-45 (2 bytes)
        data[44] = 0;
        data[45] = 2;
        // Cipher suite
        data[46] = 0;
        data[47] = 0;

        // Compression methods length at position 48
        data[48] = 1;
        data[49] = 0; // No compression

        // Extensions length at position 50-51 (2 bytes)
        let ext_len = 19u16; // SNI extension total length
        data[50] = (ext_len >> 8) as u8;
        data[51] = (ext_len & 0xFF) as u8;

        // SNI extension type at position 52-53 (0x0000)
        data[52] = 0;
        data[53] = 0;

        // SNI extension length at position 54-55
        let sni_ext_len = 15u16;
        data[54] = (sni_ext_len >> 8) as u8;
        data[55] = (sni_ext_len & 0xFF) as u8;

        // Server name list length at position 56-57
        let list_len = 13u16;
        data[56] = (list_len >> 8) as u8;
        data[57] = (list_len & 0xFF) as u8;

        // Name type at position 58 (0 = hostname)
        data[58] = 0;

        // Hostname length at position 59-60
        let hostname = b"example.com";
        let hostname_len = hostname.len() as u16;
        data[59] = (hostname_len >> 8) as u8;
        data[60] = (hostname_len & 0xFF) as u8;

        // Hostname at position 61+
        for (i, &byte) in hostname.iter().enumerate() {
            data[61 + i] = byte;
        }

        assert_eq!(extract_sni(&data), Some("example.com".to_string()));
    }
}
