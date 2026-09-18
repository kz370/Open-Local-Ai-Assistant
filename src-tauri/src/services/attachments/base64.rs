//! Minimal standard base64, used to move file bytes across the IPC boundary.

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity((bytes.len() + 2) / 3 * 4);
    for chunk in bytes.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        out.push(ALPHABET[(n >> 18) as usize & 63] as char);
        out.push(ALPHABET[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { ALPHABET[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { ALPHABET[n as usize & 63] as char } else { '=' });
    }
    out
}

/// Decodes standard base64, ignoring whitespace and an optional `data:` prefix.
pub fn decode(input: &str) -> Result<Vec<u8>, String> {
    let body = match input.find(";base64,") {
        Some(i) if input.starts_with("data:") => &input[i + 8..],
        _ => input,
    };
    let mut out = Vec::with_capacity(body.len() / 4 * 3);
    let mut acc: u32 = 0;
    let mut bits = 0u32;
    for c in body.bytes() {
        if c.is_ascii_whitespace() || c == b'=' {
            continue;
        }
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => return Err(format!("invalid base64 character {:?}", c as char)),
        } as u32;
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_every_length() {
        for len in 0..40usize {
            let data: Vec<u8> = (0..len).map(|i| (i * 7 + 3) as u8).collect();
            assert_eq!(decode(&encode(&data)).unwrap(), data, "length {len}");
        }
    }

    #[test]
    fn known_vectors() {
        assert_eq!(encode(b"any carnal pleasure."), "YW55IGNhcm5hbCBwbGVhc3VyZS4=");
        assert_eq!(encode(b"sure"), "c3VyZQ==");
        assert_eq!(decode("c3VyZQ==").unwrap(), b"sure");
    }

    #[test]
    fn accepts_data_urls_and_rejects_junk() {
        assert_eq!(decode("data:image/png;base64,c3VyZQ==").unwrap(), b"sure");
        assert!(decode("ab*d").is_err());
    }
}
