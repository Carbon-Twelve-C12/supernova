//! Hex encoding and decoding utilities

/// Convert a hexadecimal string to bytes
pub fn hex_to_bytes(hex: &str) -> Result<Vec<u8>, hex::FromHexError> {
    // Remove "0x" prefix if present
    let hex_str = if hex.starts_with("0x") {
        &hex[2..]
    } else {
        hex
    };

    hex::decode(hex_str)
}

/// Convert bytes to a hexadecimal string
pub fn bytes_to_hex(bytes: &[u8]) -> String {
    hex::encode(bytes)
}

/// Convert bytes to a hexadecimal string with "0x" prefix
pub fn bytes_to_hex_prefixed(bytes: &[u8]) -> String {
    format!("0x{}", hex::encode(bytes))
}

/// Check if a string is a valid hexadecimal representation
pub fn is_valid_hex(hex: &str) -> bool {
    let hex_str = if hex.starts_with("0x") {
        &hex[2..]
    } else {
        hex
    };

    // Check if the string is a valid hex representation
    hex_str.chars().all(|c| c.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hex_to_bytes() {
        assert_eq!(hex_to_bytes("68656c6c6f").unwrap(), b"hello");
        assert_eq!(hex_to_bytes("0x68656c6c6f").unwrap(), b"hello");
        assert!(hex_to_bytes("invalid").is_err());
    }

    #[test]
    fn test_bytes_to_hex() {
        assert_eq!(bytes_to_hex(b"hello"), "68656c6c6f");
    }

    #[test]
    fn test_bytes_to_hex_prefixed() {
        assert_eq!(bytes_to_hex_prefixed(b"hello"), "0x68656c6c6f");
    }

    #[test]
    fn test_is_valid_hex() {
        assert!(is_valid_hex("68656c6c6f"));
        assert!(is_valid_hex("0x68656c6c6f"));
        assert!(!is_valid_hex("invalid"));
    }
}
