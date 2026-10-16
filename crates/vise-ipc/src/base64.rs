//! Base64 encoding for inline preview images.
//!
//! Preview frames are returned inside a JSON response, so they need a text
//! encoding. This is a standard RFC 4648 encoder/decoder with padding; it
//! exists so the worker does not need a dependency for a single call site.

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Encode bytes as padded base64.
pub fn encode(input: &[u8]) -> String {
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let triple = (b0 << 16) | (b1 << 8) | b2;

        out.push(ALPHABET[((triple >> 18) & 0x3F) as usize] as char);
        out.push(ALPHABET[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            out.push(ALPHABET[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(ALPHABET[(triple & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

/// Decode padded base64, rejecting any character outside the alphabet.
pub fn decode(input: &str) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(input.len() / 4 * 3);
    let mut accumulator: u32 = 0;
    let mut bits = 0u32;
    let bytes = input.as_bytes();

    for (position, byte) in bytes.iter().enumerate() {
        if *byte == b'=' {
            break;
        }
        if *byte == b'\n' || *byte == b'\r' {
            continue;
        }
        let value = ALPHABET
            .iter()
            .position(|c| c == byte)
            .ok_or_else(|| format!("invalid base64 character at position {position}"))?
            as u32;
        accumulator = (accumulator << 6) | value;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((accumulator >> bits) & 0xFF) as u8);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_known_vectors() {
        assert_eq!(encode(b""), "");
        assert_eq!(encode(b"f"), "Zg==");
        assert_eq!(encode(b"fo"), "Zm8=");
        assert_eq!(encode(b"foo"), "Zm9v");
        assert_eq!(encode(b"foob"), "Zm9vYg==");
        assert_eq!(encode(b"fooba"), "Zm9vYmE=");
        assert_eq!(encode(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn round_trips_arbitrary_bytes() {
        let data: Vec<u8> = (0u16..=511).map(|v| (v % 256) as u8).collect();
        assert_eq!(decode(&encode(&data)).expect("decode"), data);
    }

    #[test]
    fn rejects_invalid_characters() {
        assert!(decode("Zm9v!").is_err());
    }
}
