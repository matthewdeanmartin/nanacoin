//! Allocation-free JSON decoding, including UTF-16 surrogate pairs emitted by
//! clients such as Python. serde-json-core 0.6 handles individual escapes only.
use serde::de::DeserializeOwned;

pub(crate) fn decode<T: DeserializeOwned>(input: &[u8]) -> Result<T, ()> {
    if input.len() > 1024 {
        return Err(());
    }
    let mut normalized = [0u8; 1024];
    let mut read = 0;
    let mut written = 0;
    while read < input.len() {
        if input[read] == b'\\' && input.get(read + 1) == Some(&b'u') {
            let high = hex(input.get(read + 2..read + 6).ok_or(())?)?;
            if (0xd800..=0xdbff).contains(&high) {
                if input.get(read + 6..read + 8) != Some(b"\\u") {
                    return Err(());
                }
                let low = hex(input.get(read + 8..read + 12).ok_or(())?)?;
                if !(0xdc00..=0xdfff).contains(&low) {
                    return Err(());
                }
                let c =
                    char::from_u32(0x10000 + ((high - 0xd800) << 10) + low - 0xdc00).ok_or(())?;
                let mut utf8 = [0; 4];
                let bytes = c.encode_utf8(&mut utf8).as_bytes();
                normalized[written..written + bytes.len()].copy_from_slice(bytes);
                written += bytes.len();
                read += 12;
                continue;
            }
        }
        normalized[written] = input[read];
        written += 1;
        // Copy any escape's second byte verbatim. In particular \\uXXXX is
        // literal text, not a Unicode escape that should be normalized.
        if input[read] == b'\\' && read + 1 < input.len() {
            read += 1;
            normalized[written] = input[read];
            written += 1;
        }
        read += 1;
    }
    let mut unescape = [0; 1024];
    let (value, used) = serde_json_core::from_slice_escaped(&normalized[..written], &mut unescape)
        .map_err(|_| ())?;
    if !normalized[used..written]
        .iter()
        .all(u8::is_ascii_whitespace)
    {
        return Err(());
    }
    Ok(value)
}

fn hex(bytes: &[u8]) -> Result<u32, ()> {
    bytes.iter().try_fold(0u32, |n, byte| {
        let digit = match byte {
            b'0'..=b'9' => byte - b'0',
            b'a'..=b'f' => byte - b'a' + 10,
            b'A'..=b'F' => byte - b'A' + 10,
            _ => return Err(()),
        };
        Ok(n * 16 + digit as u32)
    })
}
